//! An MCP server that stands in front of other MCP servers and enforces a grant.
//!
//! The servers behind it need not know Remit exists. The agent sees only the
//! actions the grant could permit, described in the contract's words, never the
//! server's own. Every call goes through the gate before it is passed on.
//!
//! What it cannot do: see or cap what a server really charges. A call that may
//! spend is counted as spending its whole ceiling.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::ast::*;
use crate::check::{RTy, Ty, World};
use crate::gate::{Decision, Offered};
use crate::journal::Session;
use crate::json::{parse, Value};

fn s(text: &str) -> Value {
    Value::Str(text.to_string())
}

fn obj(fields: Vec<(&str, Value)>) -> Value {
    Value::Obj(fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

/// One server behind the proxy, spoken to over its standard input and output.
pub struct Server {
    pub name: String,
    child: Child,
    to: ChildStdin,
    from: BufReader<ChildStdout>,
    next: u64,
}

impl Server {
    pub fn start(name: &str, command: &[String]) -> Result<Server, String> {
        let (program, args) = command.split_first().ok_or("no command given")?;
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("`{}` could not be started: {}", program, e))?;
        let to = child.stdin.take().ok_or("no input pipe")?;
        let from = BufReader::new(child.stdout.take().ok_or("no output pipe")?);
        Ok(Server { name: name.to_string(), child, to, from, next: 0 })
    }

    fn send(&mut self, message: &Value) -> Result<(), String> {
        writeln!(self.to, "{}", message.text()).and_then(|_| self.to.flush()).map_err(|e| e.to_string())
    }

    /// Sends a request and waits for its answer. Requests the server makes of us are declined.
    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next += 1;
        let id = self.next;
        self.send(&obj(vec![("jsonrpc", s("2.0")), ("id", Value::Num(id.to_string())), ("method", s(method)), ("params", params)]))?;
        loop {
            let mut line = String::new();
            if self.from.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                return Err("the server closed the connection".into());
            }
            let Ok(message) = parse(&line) else {
                continue;
            };
            if message.get("method").is_some() {
                if let Some(their_id) = message.get("id") {
                    let refuse = obj(vec![("code", Value::Num("-32601".into())), ("message", s("not supported through this proxy"))]);
                    self.send(&obj(vec![("jsonrpc", s("2.0")), ("id", their_id.clone()), ("error", refuse)]))?;
                }
                continue;
            }
            if message.get("id").and_then(|i| i.as_u64()) != Some(id) {
                continue;
            }
            if let Some(error) = message.get("error") {
                return Err(error.get("message").and_then(|m| m.as_str()).unwrap_or("the server reported an error").to_string());
            }
            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    fn notify(&mut self, method: &str) -> Result<(), String> {
        self.send(&obj(vec![("jsonrpc", s("2.0")), ("method", s(method))]))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The JSON Schema for a type, written from the contract alone.
fn schema(w: &World, ty: &RTy) -> Value {
    let described = |kind: &str, text: &str| obj(vec![("type", s(kind)), ("description", s(text))]);
    match &ty.ty {
        Ty::Unknown => obj(vec![]),
        Ty::Scalar(Scalar::Bool) => obj(vec![("type", s("boolean"))]),
        Ty::Scalar(Scalar::Int) => obj(vec![("type", s("integer"))]),
        Ty::Scalar(Scalar::Decimal) => obj(vec![("type", s("number"))]),
        Ty::Scalar(Scalar::Text) => obj(vec![("type", s("string"))]),
        Ty::Scalar(Scalar::Money) => described("string", "An amount and a currency code, for example \"200 ZAR\""),
        Ty::Scalar(Scalar::Duration) => described("string", "A duration, for example \"24h\""),
        Ty::Scalar(Scalar::Date) => described("string", "A date, for example \"2026-11-13\""),
        Ty::Scalar(Scalar::DateTime) => described("string", "A date and time with an offset, for example \"2026-11-13T19:00+02:00\""),
        Ty::List(inner) => obj(vec![("type", s("array")), ("items", schema(w, inner))]),
        Ty::Enum(f, name) => {
            let members = match w.find_decl(*f, name) {
                Some(Decl::Enum(e)) => e.members.iter().map(|m| s(&m.name)).collect(),
                _ => Vec::new(),
            };
            obj(vec![("type", s("string")), ("enum", Value::List(members))])
        }
        Ty::Record(f, name) => {
            let (mut properties, mut required) = (Vec::new(), Vec::new());
            if let Some(Decl::Type(t)) = w.find_decl(*f, name) {
                for field in &t.fields {
                    let fty = w.resolve_type(*f, &field.ty);
                    if !fty.optional {
                        required.push(s(&field.name.name));
                    }
                    properties.push((field.name.name.clone(), schema(w, &fty)));
                }
            }
            obj(vec![
                ("type", s("object")),
                ("properties", Value::Obj(properties)),
                ("required", Value::List(required)),
                ("additionalProperties", Value::Bool(false)),
            ])
        }
    }
}

/// The tool an agent is shown for an action. Nothing in it comes from the server.
fn tool(w: &World, o: &Offered) -> Value {
    let (mut properties, mut required) = (Vec::new(), Vec::new());
    for p in &o.shown.params {
        let ty = w.resolve_type(o.file, &p.ty);
        if !ty.optional {
            required.push(s(&p.name.name));
        }
        let mut sch = schema(w, &ty);
        if let (true, Value::Obj(fields)) = (ty.returned, &mut sch) {
            fields.push(("description".into(), s("Pass back, unchanged, a value this service returned earlier.")));
        }
        properties.push((p.name.name.clone(), sch));
    }
    let input = obj(vec![
        ("type", s("object")),
        ("properties", Value::Obj(properties)),
        ("required", Value::List(required)),
        ("additionalProperties", Value::Bool(false)),
    ]);
    obj(vec![
        ("name", s(&format!("{}_{}", o.service, o.shown.name.name))),
        ("description", s(o.shown.doc.as_deref().unwrap_or(""))),
        ("inputSchema", input),
    ])
}

/// What a server returned, as a value the gate can check against the action's result type.
fn returned_value(result: &Value, expects_record: bool) -> Value {
    if let Some(structured) = result.get("structuredContent") {
        return match structured {
            // A list or a plain value is commonly wrapped in an object with one field.
            Value::Obj(fields) if !expects_record && fields.len() == 1 => fields[0].1.clone(),
            other => other.clone(),
        };
    }
    let text = match result.get("content") {
        Some(Value::List(items)) => items.iter().find_map(|i| i.get("text").and_then(|t| t.as_str())),
        _ => None,
    };
    match text {
        Some(text) => parse(text).unwrap_or_else(|_| s(text)),
        None => Value::Null,
    }
}

fn text_result(text: &str, error: bool) -> Value {
    let content = Value::List(vec![obj(vec![("type", s("text")), ("text", s(text))])]);
    if error {
        obj(vec![("content", content), ("isError", Value::Bool(true))])
    } else {
        obj(vec![("content", content)])
    }
}

pub struct Proxy<'w> {
    pub session: Session<'w>,
    pub servers: Vec<Server>,
    pub now: Box<dyn Fn() -> i64>,
    client_can_ask: bool,
    next_ask: u64,
}

impl<'w> Proxy<'w> {
    pub fn new(session: Session<'w>, servers: Vec<Server>, now: Box<dyn Fn() -> i64>) -> Proxy<'w> {
        Proxy { session, servers, now, client_can_ask: false, next_ask: 0 }
    }

    /// Greets each server and drops, with a warning, any offered action it does not have.
    pub fn start(&mut self) -> Result<(), String> {
        for server in &mut self.servers {
            let hello = obj(vec![
                ("protocolVersion", s("2025-06-18")),
                ("capabilities", obj(vec![])),
                ("clientInfo", obj(vec![("name", s("remit")), ("version", s(env!("CARGO_PKG_VERSION")))])),
            ]);
            server.request("initialize", hello).map_err(|e| format!("`{}` did not start: {}", server.name, e))?;
            server.notify("notifications/initialized")?;
        }
        Ok(())
    }

    fn offered(&self) -> Vec<Offered<'w>> {
        self.session.gate().offered()
    }

    fn tools(&mut self) -> Value {
        let w = self.session.gate().world();
        let mut tools = Vec::new();
        for o in self.offered() {
            tools.push(tool(w, &o));
        }
        obj(vec![("tools", Value::List(tools))])
    }

    /// Asks the person through the client. None if the client cannot ask.
    fn ask(&mut self, prompt: &str, input: &mut dyn BufRead, output: &mut dyn Write) -> Option<bool> {
        if !self.client_can_ask {
            return None;
        }
        self.next_ask += 1;
        let id = format!("remit-ask-{}", self.next_ask);
        let approve = obj(vec![("type", s("boolean")), ("title", s("Approve this call"))]);
        let wanted = obj(vec![
            ("type", s("object")),
            ("properties", obj(vec![("approve", approve)])),
            ("required", Value::List(vec![s("approve")])),
        ]);
        let request = obj(vec![
            ("jsonrpc", s("2.0")),
            ("id", s(&id)),
            ("method", s("elicitation/create")),
            ("params", obj(vec![("message", s(prompt)), ("requestedSchema", wanted)])),
        ]);
        writeln!(output, "{}", request.text()).and_then(|_| output.flush()).ok()?;
        loop {
            let mut line = String::new();
            if input.read_line(&mut line).ok()? == 0 {
                return Some(false);
            }
            let Ok(message) = parse(&line) else {
                continue;
            };
            if message.get("method").is_some() {
                // Nothing else is served while a person's answer is awaited.
                if let Some(their_id) = message.get("id") {
                    let busy = obj(vec![("code", Value::Num("-32000".into())), ("message", s("waiting for the person to answer"))]);
                    let reply = obj(vec![("jsonrpc", s("2.0")), ("id", their_id.clone()), ("error", busy)]);
                    let _ = writeln!(output, "{}", reply.text()).and_then(|_| output.flush());
                }
                continue;
            }
            if message.get("id").and_then(|i| i.as_str()) != Some(&id) {
                continue;
            }
            let result = message.get("result")?;
            let accepted = result.get("action").and_then(|a| a.as_str()) == Some("accept");
            let approved = result.get("content").and_then(|c| c.get("approve")) == Some(&Value::Bool(true));
            return Some(accepted && approved);
        }
    }

    fn call(&mut self, params: &Value, input: &mut dyn BufRead, output: &mut dyn Write) -> Value {
        let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let args = params.get("arguments").cloned().unwrap_or(Value::Obj(Vec::new()));
        // Only what the grant offers exists, as far as the agent can tell.
        let Some(found) = self.offered().into_iter().find(|o| format!("{}_{}", o.service, o.shown.name.name) == name) else {
            return text_result(&format!("Refused: there is no tool called `{}`.", name), true);
        };
        let (service, action) = (found.service.clone(), found.shown.name.name.clone());
        let w = self.session.gate().world();
        let result_type = found.shown.result.as_ref().map(|r| w.resolve_type(found.file, r));
        let now = (self.now)();
        let mut decision = self.session.call(&service, &action, &args, now);
        if let Decision::Ask { call, prompt } = &decision {
            let call = *call;
            decision = match self.ask(prompt, input, output) {
                Some(approved) => self.session.confirm(call, approved, (self.now)()),
                None => {
                    self.session.confirm(call, false, (self.now)());
                    return text_result("Refused: this call needs the person's confirmation, and this client cannot ask them.", true);
                }
            };
        }
        let (call, reserve) = match decision {
            Decision::Allow { call, reserve, .. } => (call, reserve),
            Decision::Deny(d) => return text_result(&format!("Refused ({}): {}", d.code, d.reason), true),
            Decision::Ask { .. } => return text_result("Refused: the call is still waiting for an answer.", true),
        };
        let Some(server) = self.servers.iter_mut().find(|x| x.name == service) else {
            let _ = self.session.fail(call, (self.now)());
            return text_result("Error: the service is not connected.", true);
        };
        // What the server really charges cannot be seen from here, so the whole ceiling is counted.
        if let Some(reserve) = &reserve {
            let _ = self.session.charge(call, reserve, (self.now)());
        }
        let passed = obj(vec![("name", s(&action)), ("arguments", args)]);
        let answer = match server.request("tools/call", passed) {
            Ok(answer) => answer,
            Err(why) => {
                let _ = self.session.fail(call, (self.now)());
                return text_result(&format!("Error: {}", why), true);
            }
        };
        if answer.get("isError") == Some(&Value::Bool(true)) {
            let _ = self.session.fail(call, (self.now)());
            let said = returned_value(&answer, false);
            let said = said.as_str().map(String::from).unwrap_or_else(|| said.text());
            return text_result(&format!("The service reported an error: {}", said), true);
        }
        let value = match &result_type {
            None => Value::Null,
            Some(ty) => returned_value(&answer, matches!(ty.ty, Ty::Record(..))),
        };
        match self.session.result(call, &value, (self.now)()) {
            Err(d) => text_result(&format!("Withheld ({}): {}", d.code, d.reason), true),
            Ok(delivered) => {
                if delivered.value.is_null() {
                    return text_result("Done.", false);
                }
                let mut out = text_result(&delivered.value.text(), false);
                if let Value::Obj(fields) = &mut out {
                    if matches!(delivered.value, Value::Obj(_)) {
                        fields.push(("structuredContent".into(), delivered.value.clone()));
                    }
                    if !delivered.untrusted.is_empty() {
                        let paths = Value::List(delivered.untrusted.iter().map(|p| s(p)).collect());
                        fields.push(("_meta".into(), obj(vec![("remit/untrusted", paths)])));
                    }
                }
                out
            }
        }
    }

    /// Serves one client until it closes its side.
    pub fn serve(&mut self, input: &mut dyn BufRead, output: &mut dyn Write) {
        loop {
            let mut bytes = Vec::new();
            match input.read_until(b'\n', &mut bytes) {
                Ok(0) | Err(_) => return,
                Ok(_) => {}
            }
            let Ok(message) = std::str::from_utf8(&bytes).map_err(|_| ()).and_then(|l| parse(l).map_err(|_| ())) else {
                continue;
            };
            let (Some(id), Some(method)) = (message.get("id"), message.get("method").and_then(|m| m.as_str())) else {
                continue; // A notification, or an answer nobody is waiting for.
            };
            let null = Value::Null;
            let params = message.get("params").unwrap_or(&null);
            let result = match method {
                "initialize" => {
                    self.client_can_ask = params.get("capabilities").and_then(|c| c.get("elicitation")).is_some();
                    let version = params.get("protocolVersion").cloned().unwrap_or(s("2025-06-18"));
                    Ok(obj(vec![
                        ("protocolVersion", version),
                        ("capabilities", obj(vec![("tools", obj(vec![]))])),
                        ("serverInfo", obj(vec![("name", s("remit")), ("version", s(env!("CARGO_PKG_VERSION")))])),
                    ]))
                }
                "ping" => Ok(obj(vec![])),
                "tools/list" => Ok(self.tools()),
                "tools/call" => Ok(self.call(params, input, output)),
                other => Err(format!("unknown method {}", other)),
            };
            let mut reply = vec![("jsonrpc", s("2.0")), ("id", id.clone())];
            match result {
                Ok(result) => reply.push(("result", result)),
                Err(why) => reply.push(("error", obj(vec![("code", Value::Num("-32601".into())), ("message", s(&why))]))),
            }
            if writeln!(output, "{}", obj(reply).text()).and_then(|_| output.flush()).is_err() {
                return;
            }
        }
    }
}
