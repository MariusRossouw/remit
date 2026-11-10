use std::process::ExitCode;

use remit::diag::{json_string, to_json, to_text};
use remit::gate::{Binding, Decision, Deny, Gate};
use remit::journal::Session;
use remit::proxy::{Proxy, Server};
use remit::json::Value;

const HELP: &str = "\
remit - checker for the Remit contract language

Usage:
  remit check [--json] <file>...   Check contracts. Exit code 1 if any rule is broken.
  remit explain <file>...          Print each grant in plain language.
  remit list <file>...             Print what each file declares, as JSON.
  remit gate <file> [--grant <Name>] --bind <name>=<file>#<Service>...
                                   Enforce a grant. Reads one JSON request per line
                                   on standard input and answers each on one line.
  remit proxy <file> [--grant <Name>] --server <name>=<file>#<Service>=<command>...
                                   Be an MCP server that enforces a grant in front of
                                   the MCP servers the commands start.

gate and proxy take --journal <file>: decisions are written there first, and
counts survive a restart. Without it, a restart forgets what was used.
  remit --version

Files named by `use` lines are loaded automatically.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, rest) = match args.split_first() {
        Some((c, rest)) => (c.as_str(), rest),
        None => {
            print!("{}", HELP);
            return ExitCode::from(2);
        }
    };
    match command {
        "--version" | "-V" => {
            println!("remit {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "--help" | "-h" | "help" => {
            print!("{}", HELP);
            ExitCode::SUCCESS
        }
        "check" | "explain" | "list" => {
            let json = rest.iter().any(|a| a == "--json");
            let files: Vec<String> = rest.iter().filter(|a| !a.starts_with("--")).cloned().collect();
            if files.is_empty() {
                eprintln!("remit {}: name at least one file.", command);
                return ExitCode::from(2);
            }
            let mut read = read_contract;
            let world = remit::load(&files, &mut read);
            if command == "check" {
                if json {
                    println!("{}", to_json(&world.diags));
                } else if world.diags.is_empty() {
                    println!("ok: {} file(s) checked, no rules broken.", world.files.len());
                } else {
                    print!("{}", to_text(&world.diags));
                    println!("{} problem(s) found.", world.diags.len());
                }
            } else {
                if !world.diags.is_empty() {
                    // Never describe a contract that did not pass the checker.
                    eprint!("{}", to_text(&world.diags));
                    eprintln!("Nothing printed: fix the problems above first.");
                    return ExitCode::from(1);
                }
                if command == "explain" {
                    for &f in &world.entries {
                        print!("{}", remit::explain::explain(&world, f));
                    }
                } else {
                    let files: Vec<String> = world.entries.iter().map(|&f| remit::list::list(&world, f)).collect();
                    println!("[{}]", files.join(","));
                }
            }
            if world.diags.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        "gate" => gate(rest),
        "proxy" => proxy(rest),
        other => {
            eprintln!("remit: unknown command `{}`.\n\n{}", other, HELP);
            ExitCode::from(2)
        }
    }
}

fn plain(path: &str) -> String {
    let mut p = path;
    while let Some(rest) = p.strip_prefix("./") {
        p = rest;
    }
    p.to_string()
}

fn refused(d: &Deny) -> String {
    format!("{{\"ok\":false,\"decision\":\"deny\",\"code\":\"{}\",\"reason\":{}}}", d.code, json_string(&d.reason))
}

fn decided(d: &Decision) -> String {
    match d {
        Decision::Allow { call, reserve, undo } => format!(
            "{{\"ok\":true,\"decision\":\"allow\",\"call\":{},\"reserve\":{},\"undo\":{}}}",
            call,
            reserve.as_ref().map(|r| json_string(r)).unwrap_or_else(|| "null".into()),
            undo
        ),
        Decision::Ask { call, prompt } => {
            format!("{{\"ok\":true,\"decision\":\"ask\",\"call\":{},\"prompt\":{}}}", call, json_string(prompt))
        }
        Decision::Deny(d) => refused(d),
    }
}

/// What `gate` and `proxy` are started with.
struct Started {
    /// For each connected service: its name, and the command that runs its server, if any.
    services: Vec<(String, Option<String>)>,
    fixed_now: Option<i64>,
}

fn clock(fixed: Option<i64>) -> i64 {
    fixed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(i64::MAX)
    })
}

/// Reads the options, checks the contracts, opens the gate and its journal, then runs `then`.
fn with_gate(command: &str, rest: &[String], then: &mut dyn FnMut(Session, Started) -> ExitCode) -> ExitCode {
    let usage = |why: &str| {
        eprintln!("remit {}: {}\n\n{}", command, why, HELP);
        ExitCode::from(2)
    };
    let (mut file, mut grant, mut binds, mut fixed_now, mut journal) = (None, None, Vec::new(), None, None);
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--grant" => grant = it.next().cloned(),
            "--bind" | "--server" => binds.extend(it.next().cloned()),
            "--journal" => journal = it.next().cloned(),
            // For tests and repeatable benchmarks only. A host uses its own clock.
            "--now" => match it.next().and_then(|t| remit::gate::parse_datetime(t)) {
                Some(t) => fixed_now = Some(t),
                None => return usage("--now needs a date and time with an offset, such as 2026-11-10T12:00+02:00."),
            },
            a if a.starts_with("--") => return usage(&format!("unknown option `{}`.", a)),
            a => file = Some(plain(a)),
        }
    }
    let Some(file) = file else {
        return usage("name the file that holds the grant.");
    };
    // name=file#Service, or name=file#Service=command for a server the proxy starts.
    let mut parsed = Vec::new();
    for b in &binds {
        let parts = b.split_once('=').and_then(|(name, rest)| {
            let (at, run) = match rest.split_once('=') {
                Some((at, run)) => (at, Some(run.to_string())),
                None => (rest, None),
            };
            at.rsplit_once('#').map(|(path, service)| (name.to_string(), plain(path), service.to_string(), run))
        });
        match parts {
            Some(p) if !p.0.contains('_') && !p.0.is_empty() => parsed.push(p),
            Some(_) => return usage(&format!("in `{}`, the name must not be empty or contain `_`.", b)),
            None => return usage(&format!("`{}` is not of the form name=file#Service.", b)),
        }
    }
    let mut entries = vec![file.clone()];
    entries.extend(parsed.iter().map(|p| p.1.clone()));
    let mut read = read_contract;
    let world = remit::load(&entries, &mut read);
    if !world.diags.is_empty() {
        eprint!("{}", to_text(&world.diags));
        eprintln!("Nothing enforced: fix the problems above first.");
        return ExitCode::from(1);
    }
    let index = |path: &str| world.files.iter().position(|f| f.path == path).expect("loaded above");
    let f = index(&file);
    let grant = match grant {
        Some(g) => g,
        None => {
            let grants: Vec<&str> =
                world.files[f].decls.iter().filter(|d| d.kind() == "grant").map(|d| d.name().name.as_str()).collect();
            match grants[..] {
                [one] => one.to_string(),
                _ => return usage("the file does not hold exactly one grant, so name one with --grant."),
            }
        }
    };
    let bindings = parsed
        .iter()
        .map(|(name, path, service, _)| Binding { name: name.clone(), address: path.clone(), file: index(path), service: service.clone() })
        .collect();
    let gate = match Gate::new(&world, f, &grant, bindings) {
        Ok(g) => g,
        Err(why) => return usage(&why),
    };
    let session = match journal {
        None => Session::in_memory(gate),
        Some(path) => {
            // The journal is tied to these exact contracts and connections.
            let mut texts: Vec<String> = world.files.iter().map(|f| read_contract(&f.path).unwrap_or_default()).collect();
            texts.extend(parsed.iter().map(|p| format!("{}={}#{}", p.0, p.1, p.2)));
            match Session::open(gate, &path, &grant, &remit::journal::fingerprint(&texts)) {
                Ok(s) => s,
                Err(why) => {
                    eprintln!("remit {}: {} Nothing is enforced, and nothing is allowed.", command, why);
                    return ExitCode::from(1);
                }
            }
        }
    };
    let services = parsed.into_iter().map(|p| (p.0, p.3)).collect();
    then(session, Started { services, fixed_now })
}

fn gate(rest: &[String]) -> ExitCode {
    with_gate("gate", rest, &mut |mut gate, started| {
        let now = || clock(started.fixed_now);
        let bad = |why: &str| format!("{{\"ok\":false,\"error\":{}}}", json_string(why));
        let mut bytes = Vec::new();
        let mut input = std::io::stdin().lock();
        loop {
            bytes.clear();
            match std::io::BufRead::read_until(&mut input, b'\n', &mut bytes) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            // A line that is not valid text gets an error, and the gate carries on.
            let line = match std::str::from_utf8(&bytes) {
                Ok(line) => line.to_string(),
                Err(_) => {
                    println!("{}", bad("The request is not valid UTF-8."));
                    continue;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            let answer = match remit::json::parse(&line) {
                Err(why) => bad(&format!("The request is not valid JSON: {}.", why)),
                Ok(req) => {
                    let text = |k: &str| req.get(k).and_then(|v| v.as_str()).unwrap_or("");
                    let call = req.get("call").and_then(|v| v.as_u64()).unwrap_or(0);
                    let done = |r: Result<(), Deny>| match r {
                        Ok(()) => "{\"ok\":true}".to_string(),
                        Err(d) => refused(&d),
                    };
                    match text("op") {
                        "call" => decided(&gate.call(text("service"), text("action"), req.get("args").unwrap_or(&Value::Null), now())),
                        "confirm" => decided(&gate.confirm(call, req.get("approved") == Some(&Value::Bool(true)), now())),
                        "charge" => done(gate.charge(call, text("amount"), now())),
                        "fail" => done(gate.fail(call, now())),
                        "result" => match gate.result(call, req.get("value").unwrap_or(&Value::Null), now()) {
                            Ok(d) => {
                                let paths: Vec<String> = d.untrusted.iter().map(|p| json_string(p)).collect();
                                format!("{{\"ok\":true,\"value\":{},\"untrusted\":[{}]}}", d.value.text(), paths.join(","))
                            }
                            Err(d) => refused(&d),
                        },
                        "revoke" => {
                            gate.revoke(now());
                            "{\"ok\":true}".to_string()
                        }
                        "state" => gate.state(now()),
                        other => bad(&format!("Unknown op `{}`. Use call, confirm, charge, result, fail, revoke, or state.", other)),
                    }
                }
            };
            println!("{}", answer);
        }
        ExitCode::SUCCESS
    })
}

/// Splits a command into words. Double quotes keep spaces together.
fn words(command: &str) -> Vec<String> {
    let (mut out, mut word, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    for c in command.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any || !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                    any = false;
                }
            }
            c => word.push(c),
        }
    }
    if any || !word.is_empty() {
        out.push(word);
    }
    out
}

fn proxy(rest: &[String]) -> ExitCode {
    with_gate("proxy", rest, &mut |session, started| {
        let mut servers = Vec::new();
        for (name, run) in &started.services {
            let Some(run) = run else {
                eprintln!("remit proxy: `{}` has no command. Write --server {}=file#Service=\"command to run\".", name, name);
                return ExitCode::from(2);
            };
            match Server::start(name, &words(run)) {
                Ok(s) => servers.push(s),
                Err(why) => {
                    eprintln!("remit proxy: {}", why);
                    return ExitCode::from(1);
                }
            }
        }
        let fixed = started.fixed_now;
        let mut proxy = Proxy::new(session, servers, Box::new(move || clock(fixed)));
        if let Err(why) = proxy.start() {
            eprintln!("remit proxy: {}", why);
            return ExitCode::from(1);
        }
        proxy.serve(&mut std::io::stdin().lock(), &mut std::io::stdout());
        ExitCode::SUCCESS
    })
}

/// Reads a contract file. Only ordinary files of a sensible size, so a `use`
/// line cannot point the checker at a pipe or a device.
fn read_contract(path: &str) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("it is not an ordinary file".into());
    }
    if meta.len() > 1_000_000 {
        return Err("it is larger than 1 MB".into());
    }
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}
