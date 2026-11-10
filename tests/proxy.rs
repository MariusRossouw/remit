//! The proxy in front of an MCP server that knows nothing about Remit.
//! Needs python3, which runs the stand-in server in tests/proxy/fake_server.py.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use remit::json::{parse, Value};

const BIN: &str = env!("CARGO_BIN_EXE_remit");
const SLOT: &str = r#"{"id":"s1","start":"2026-11-13T19:00+02:00","seats":4,"deposit":"200 ZAR"}"#;

struct Client {
    child: Child,
    to: ChildStdin,
    from: BufReader<ChildStdout>,
    id: u64,
}

fn scratch(name: &str) -> String {
    let path = format!("{}/{}", env!("CARGO_TARGET_TMPDIR"), name);
    let _ = std::fs::remove_file(&path);
    path
}

fn start(journal: &str, log: &str) -> Client {
    let mut child = Command::new(BIN)
        .args([
            "proxy",
            "examples/dinner/friday-dinner.remit",
            "--server",
            "bistro=examples/dinner/bistro.remit#Bistro=python3 tests/proxy/fake_server.py",
            "--now",
            "2026-11-10T12:00+02:00",
            "--journal",
            journal,
        ])
        .env("FAKE_LOG", log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let to = child.stdin.take().unwrap();
    let from = BufReader::new(child.stdout.take().unwrap());
    let mut c = Client { child, to, from, id: 0 };
    c.ask("initialize", r#"{"protocolVersion":"2025-06-18","capabilities":{}}"#);
    c
}

impl Client {
    fn ask(&mut self, method: &str, params: &str) -> Value {
        self.id += 1;
        writeln!(self.to, r#"{{"jsonrpc":"2.0","id":{},"method":"{}","params":{}}}"#, self.id, method, params).unwrap();
        let mut line = String::new();
        self.from.read_line(&mut line).unwrap();
        let reply = parse(&line).unwrap_or_else(|e| panic!("bad reply {:?}: {}", line, e));
        assert_eq!(reply.get("id").and_then(|i| i.as_u64()), Some(self.id), "{}", line);
        reply.get("result").cloned().unwrap_or_else(|| panic!("no result: {}", line))
    }

    /// Calls a tool. Returns the text it answered with, and whether that was an error.
    fn tool(&mut self, name: &str, args: &str) -> (String, bool) {
        let result = self.ask("tools/call", &format!(r#"{{"name":"{}","arguments":{}}}"#, name, args));
        let text = match result.get("content") {
            Some(Value::List(items)) => items[0].get("text").and_then(|t| t.as_str()).unwrap_or("").to_string(),
            _ => String::new(),
        };
        (text, result.get("isError") == Some(&Value::Bool(true)))
    }

    fn stop(mut self) {
        drop(self.to);
        self.child.wait().unwrap();
    }
}

fn book_args() -> String {
    format!(r#"{{"slot":{},"guest":{{"name":"Thandi"}}}}"#, SLOT)
}

#[test]
fn the_agent_sees_only_granted_actions_in_the_contracts_words() {
    let mut c = start(&scratch("list.journal"), &scratch("list.log"));
    let tools = c.ask("tools/list", "{}").text();
    for shown in ["bistro_findTables", "bistro_reviews", "bistro_book", "bistro_cancel", "Reserve a table. May take a deposit."] {
        assert!(tools.contains(shown), "missing {}: {}", shown, tools);
    }
    // Not granted, not in the contract, or the server's own wording: none of it gets through.
    for hidden in ["todaysSpecials", "wipe", "IMPORTANT"] {
        assert!(!tools.contains(hidden), "{} leaked: {}", hidden, tools);
    }
    assert!(tools.contains(r#""required":["id","start","seats","deposit"]"#), "{}", tools);
    c.stop();
}

#[test]
fn calls_go_through_the_gate_and_only_allowed_ones_reach_the_server() {
    let (journal, log) = (scratch("calls.journal"), scratch("calls.log"));
    let mut c = start(&journal, &log);

    let (text, error) = c.tool("bistro_wipe", "{}");
    assert!(error && text.contains("no tool"), "{}", text);
    let (text, error) = c.tool("bistro_book", &book_args());
    assert!(error && text.contains("G09"), "booking a table nobody offered: {}", text);

    let (tables, error) = c.tool("bistro_findTables", r#"{"date":"2026-11-13","party":4}"#);
    assert!(!error && tables == format!("[{}]", SLOT), "the result is cut to the contract: {}", tables);

    let reviews = c.ask("tools/call", r#"{"name":"bistro_reviews","arguments":{}}"#).text();
    assert!(reviews.contains(r#""remit/untrusted":["/0/body"]"#), "{}", reviews);

    let (booking, error) = c.tool("bistro_book", &book_args());
    assert!(!error && booking.contains(r#""id":"b-1""#) && !booking.contains("staffNote"), "{}", booking);
    let (text, error) = c.tool("bistro_book", &book_args());
    assert!(error && text.contains("G05"), "{}", text);
    c.stop();

    let reached = std::fs::read_to_string(&log).unwrap();
    let tools: Vec<&str> = reached.lines().map(|l| l.split('"').nth(3).unwrap()).collect();
    assert_eq!(tools, vec!["findTables", "reviews", "book"], "{}", reached);
}

#[test]
fn counts_and_undo_rights_survive_a_restart() {
    let (journal, log) = (scratch("restart.journal"), scratch("restart.log"));
    let mut c = start(&journal, &log);
    c.tool("bistro_findTables", r#"{"date":"2026-11-13","party":4}"#);
    let (booking, error) = c.tool("bistro_book", &book_args());
    assert!(!error, "{}", booking);
    c.stop();

    // A new process, with nothing but the journal.
    let mut c = start(&journal, &log);
    let (text, error) = c.tool("bistro_book", &book_args());
    assert!(error && text.contains("G05"), "the one booking was forgotten: {}", text);
    let (text, error) = c.tool("bistro_cancel", &format!(r#"{{"booking":{}}}"#, booking));
    assert!(!error && text == "Done.", "the right to undo was forgotten: {}", text);
    let (text, error) = c.tool("bistro_cancel", &format!(r#"{{"booking":{}}}"#, booking));
    assert!(error && text.contains("G02"), "{}", text);
    c.stop();
}

#[test]
fn a_journal_that_cannot_be_trusted_stops_the_gate() {
    let (journal, log) = (scratch("bad.journal"), scratch("bad.log"));
    let mut c = start(&journal, &log);
    c.tool("bistro_findTables", r#"{"date":"2026-11-13","party":4}"#);
    c.tool("bistro_book", &book_args());
    c.stop();
    let run = |journal: &str| {
        Command::new(BIN)
            .args(["proxy", "examples/dinner/friday-dinner.remit", "--server"])
            .arg("bistro=examples/dinner/bistro.remit#Bistro=python3 tests/proxy/fake_server.py")
            .args(["--now", "2026-11-10T12:00+02:00", "--journal", journal])
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    assert!(run(&journal).status.success());

    // A line removed: the booking would be forgotten. The gate refuses to start.
    let text = std::fs::read_to_string(&journal).unwrap();
    let without_booking: Vec<&str> = text.lines().filter(|l| !l.contains(r#""action":"book""#)).collect();
    std::fs::write(&journal, without_booking.join("\n") + "\n").unwrap();
    let out = run(&journal);
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("nothing is allowed"));

    // A journal written for other contracts is refused too.
    std::fs::write(&journal, "{\"remit\":1,\"grant\":\"FridayDinner\",\"contracts\":\"0000000000000000\"}\n").unwrap();
    assert_eq!(run(&journal).status.code(), Some(1));
}
