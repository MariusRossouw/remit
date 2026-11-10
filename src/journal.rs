//! A gate whose decisions are written down before they are released.
//!
//! The journal is the gate's memory. Every request and its outcome is one line.
//! On start the lines are replayed through a fresh gate, which rebuilds every
//! count exactly, because the gate decides from its inputs alone. If the journal
//! cannot be read, does not match the contracts, or replays differently, the
//! gate refuses to start. If a line cannot be written, the call is refused.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

use crate::diag::json_string;
use crate::gate::{Decision, Delivered, Deny, Gate};
use crate::json::{parse, Value};

pub struct Session<'w> {
    gate: Gate<'w>,
    file: Option<File>,
}

/// A short fingerprint of the contracts, so a journal is never replayed against
/// different ones. Not a security hash: the journal's folder must be trusted.
pub fn fingerprint(texts: &[String]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for t in texts {
        for b in t.bytes().chain([0u8]) {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{:016x}", h)
}

fn outcome(d: &Decision) -> String {
    match d {
        Decision::Allow { call, .. } => format!("allow {}", call),
        Decision::Ask { call, .. } => format!("ask {}", call),
        Decision::Deny(d) => format!("deny {}", d.code),
    }
}

fn done<T>(r: &Result<T, Deny>) -> String {
    match r {
        Ok(_) => "ok".into(),
        Err(d) => format!("deny {}", d.code),
    }
}

impl<'w> Session<'w> {
    /// A gate with no journal. Its counts are lost when the process ends.
    pub fn in_memory(gate: Gate<'w>) -> Session<'w> {
        Session { gate, file: None }
    }

    /// Opens or creates the journal at `path` and replays it.
    pub fn open(mut gate: Gate<'w>, path: &str, grant: &str, fingerprint: &str) -> Result<Session<'w>, String> {
        let head = format!("{{\"remit\":1,\"grant\":{},\"contracts\":{}}}", json_string(grant), json_string(fingerprint));
        if let Ok(existing) = File::open(path) {
            let mut lines = BufReader::new(existing).lines();
            match lines.next() {
                None => {}
                Some(Ok(first)) if first == head => {
                    for (n, line) in lines.enumerate() {
                        let line = line.map_err(|e| format!("The journal cannot be read: {}.", e))?;
                        replay(&mut gate, &line).map_err(|why| format!("Line {} of the journal {}.", n + 2, why))?;
                    }
                    let file = OpenOptions::new().append(true).open(path).map_err(|e| format!("The journal cannot be written: {}.", e))?;
                    return Ok(Session { gate, file: Some(file) });
                }
                Some(_) => return Err("The journal belongs to a different grant or to different contracts.".into()),
            }
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path).map_err(|e| format!("The journal cannot be written: {}.", e))?;
        writeln!(file, "{}", head).and_then(|_| file.sync_data()).map_err(|e| format!("The journal cannot be written: {}.", e))?;
        Ok(Session { gate, file: Some(file) })
    }

    pub fn gate(&self) -> &Gate<'w> {
        &self.gate
    }

    /// Writes one line and makes sure it is on disk. False if that failed.
    fn record(&mut self, now: i64, body: String, out: &str) -> bool {
        let Some(file) = &mut self.file else {
            return true;
        };
        let line = format!("{{\"t\":{},{},\"out\":{}}}", now, body, json_string(out));
        writeln!(file, "{}", line).and_then(|_| file.sync_data()).is_ok()
    }

    fn unrecorded() -> Deny {
        Deny { code: "G10", reason: "The gate could not record this decision, so it is refused.".into() }
    }

    pub fn call(&mut self, service: &str, action: &str, args: &Value, now: i64) -> Decision {
        let d = self.gate.call(service, action, args, now);
        let body = format!("\"op\":\"call\",\"service\":{},\"action\":{},\"args\":{}", json_string(service), json_string(action), args.text());
        if self.record(now, body, &outcome(&d)) {
            return d;
        }
        // The gate has counted the call. Close it, so nothing can be charged to it.
        if let Decision::Allow { call, .. } = d {
            let _ = self.gate.fail(call);
        }
        Decision::Deny(Self::unrecorded())
    }

    pub fn confirm(&mut self, call: u64, approved: bool, now: i64) -> Decision {
        let d = self.gate.confirm(call, approved, now);
        if self.record(now, format!("\"op\":\"confirm\",\"call\":{},\"approved\":{}", call, approved), &outcome(&d)) {
            return d;
        }
        if let Decision::Allow { call, .. } = d {
            let _ = self.gate.fail(call);
        }
        Decision::Deny(Self::unrecorded())
    }

    pub fn charge(&mut self, call: u64, amount: &str, now: i64) -> Result<(), Deny> {
        let r = self.gate.charge(call, amount, now);
        if self.record(now, format!("\"op\":\"charge\",\"call\":{},\"amount\":{}", call, json_string(amount)), &done(&r)) {
            r
        } else {
            Err(Self::unrecorded())
        }
    }

    pub fn result(&mut self, call: u64, value: &Value, now: i64) -> Result<Delivered, Deny> {
        let r = self.gate.result(call, value, now);
        if self.record(now, format!("\"op\":\"result\",\"call\":{},\"value\":{}", call, value.text()), &done(&r)) {
            r
        } else {
            Err(Self::unrecorded())
        }
    }

    pub fn fail(&mut self, call: u64, now: i64) -> Result<(), Deny> {
        let r = self.gate.fail(call);
        self.record(now, format!("\"op\":\"fail\",\"call\":{}", call), &done(&r));
        r
    }

    pub fn revoke(&mut self, now: i64) {
        self.gate.revoke();
        self.record(now, "\"op\":\"revoke\"".into(), "ok");
    }

    pub fn state(&self, now: i64) -> String {
        self.gate.state(now)
    }
}

/// Applies one journal line to the gate and checks it comes out as recorded.
fn replay(gate: &mut Gate, line: &str) -> Result<(), String> {
    let v = parse(line).map_err(|_| "is not valid".to_string())?;
    let text = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("");
    let now: i64 = match v.get("t") {
        Some(Value::Num(n)) => n.parse().map_err(|_| "has no time".to_string())?,
        _ => return Err("has no time".into()),
    };
    let call = v.get("call").and_then(|x| x.as_u64()).unwrap_or(0);
    let null = Value::Null;
    let got = match text("op") {
        "call" => outcome(&gate.call(text("service"), text("action"), v.get("args").unwrap_or(&null), now)),
        "confirm" => outcome(&gate.confirm(call, v.get("approved") == Some(&Value::Bool(true)), now)),
        "charge" => done(&gate.charge(call, text("amount"), now)),
        "result" => done(&gate.result(call, v.get("value").unwrap_or(&null), now)),
        "fail" => done(&gate.fail(call)),
        "revoke" => {
            gate.revoke();
            "ok".into()
        }
        _ => return Err("holds an unknown request".into()),
    };
    if got == text("out") {
        Ok(())
    } else {
        Err(format!("replays as `{}` but was recorded as `{}`", got, text("out")))
    }
}
