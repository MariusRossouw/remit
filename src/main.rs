use std::process::ExitCode;

use remit::diag::{json_string, to_json, to_text};
use remit::gate::{Binding, Decision, Deny, Gate};
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

fn gate(rest: &[String]) -> ExitCode {
    let usage = |why: &str| {
        eprintln!("remit gate: {}\n\n{}", why, HELP);
        ExitCode::from(2)
    };
    let (mut file, mut grant, mut binds, mut fixed_now) = (None, None, Vec::new(), None);
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--grant" => grant = it.next().cloned(),
            "--bind" => binds.extend(it.next().cloned()),
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
    let mut parsed = Vec::new();
    for b in &binds {
        let parts = b.split_once('=').and_then(|(name, at)| at.rsplit_once('#').map(|(path, service)| (name, path, service)));
        match parts {
            Some((name, path, service)) => parsed.push((name.to_string(), plain(path), service.to_string())),
            None => return usage(&format!("`{}` is not of the form name=file#Service.", b)),
        }
    }
    let mut entries = vec![file.clone()];
    entries.extend(parsed.iter().map(|(_, path, _)| path.clone()));
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
            let grants: Vec<&str> = world.files[f]
                .decls
                .iter()
                .filter(|d| d.kind() == "grant")
                .map(|d| d.name().name.as_str())
                .collect();
            match grants[..] {
                [one] => one.to_string(),
                _ => return usage("the file does not hold exactly one grant, so name one with --grant."),
            }
        }
    };
    let bindings = parsed
        .iter()
        .map(|(name, path, service)| Binding { name: name.clone(), address: path.clone(), file: index(path), service: service.clone() })
        .collect();
    let mut gate = match Gate::new(&world, f, &grant, bindings) {
        Ok(g) => g,
        Err(why) => return usage(&why),
    };
    let now = || {
        fixed_now.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(i64::MAX)
        })
    };
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
                    "fail" => done(gate.fail(call)),
                    "result" => match gate.result(call, req.get("value").unwrap_or(&Value::Null), now()) {
                        Ok(d) => {
                            let paths: Vec<String> = d.untrusted.iter().map(|p| json_string(p)).collect();
                            format!("{{\"ok\":true,\"value\":{},\"untrusted\":[{}]}}", d.value.text(), paths.join(","))
                        }
                        Err(d) => refused(&d),
                    },
                    "revoke" => {
                        gate.revoke();
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
