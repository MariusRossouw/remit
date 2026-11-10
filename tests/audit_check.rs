//! Audit of the checker (SPEC 11, 13), `remit explain`, and the command line tools.
//! A failing test is a finding; a passing one is coverage.

use std::collections::BTreeMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use remit::gate::{parse_datetime, Binding, Decision, Gate};
use remit::json::parse;

const BIN: &str = env!("CARGO_BIN_EXE_remit");

fn check_file(path: &str) -> Vec<&'static str> {
    let mut read = |p: &str| std::fs::read_to_string(p).map_err(|e| e.to_string());
    remit::load(&[path.to_string()], &mut read).diags.iter().map(|d| d.code).collect()
}

/// Checks in-memory files. The first is the entry; others are reachable as "./name".
fn check_src(files: &[(&str, &str)]) -> Vec<&'static str> {
    let map: BTreeMap<String, String> = files.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let mut read = |p: &str| map.get(p).cloned().ok_or_else(|| "no such file".to_string());
    let w = remit::load(&[files[0].0.to_string()], &mut read);
    w.diags.iter().map(|d| d.code).collect()
}

fn explain(path: &str) -> String {
    let out = Command::new(BIN).args(["explain", path]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

/// Runs a command with a time limit. Returns (exit code or None if killed/signalled, stdout).
fn run_limited(cmd: &mut Command, stdin: &[u8], limit: Duration) -> (Option<i32>, String, bool) {
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    {
        let mut si = child.stdin.take().unwrap();
        let _ = si.write_all(stdin);
    }
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let out = child.wait_with_output().unwrap();
            return (status.code(), String::from_utf8_lossy(&out.stdout).to_string(), false);
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return (None, String::new(), true);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

const WORLD: &str = "tests/audit/world.remit";

// ===========================================================================
// FINDINGS
// ===========================================================================

/// `remit explain` must not show a limit the gate does not enforce. For
/// `spend <= 100 ZAR, spend <= 500 ZAR` it shows both; the gate enforces 500
/// (see audit_gate::finding_two_spend_limits_on_one_rule_the_lower_one_holds).
/// Either the checker should refuse a second `spend <=`, or the gate should keep the lower.
#[test]
fn finding_explain_does_not_show_a_spending_limit_the_gate_ignores() {
    // Fixed by refusing the contract: a rule may carry one spending limit.
    let codes = check_src(&[(
        "a.remit",
        "\"S.\"\nservice S {\n  \"Pay.\"\n  action pay(amount: Money) -> Text\n    effects: spend\n    undo: x\n  \"X.\"\n  action x(t: Text)\n    effects: write\n}\n\"G.\"\ngrant G {\n  allow S.pay where spend <= 100 ZAR, spend <= 500 ZAR\n  expires 2026-11-13T23:59+02:00\n}\n",
    )]);
    assert_eq!(codes, vec!["R10"]);
}

/// SPEC 14: "Rules are tried in the order written. The first that permits the call counts
/// it." With `ask` written before `allow`, the gate always asks, but explain lists the
/// action under "Allowed without asking you each time".
#[test]
fn finding_explain_follows_rule_order() {
    let text = explain(WORLD);
    let section = text.split("\nAskFirst\n").nth(1).unwrap().split("\nKeep\n").next().unwrap();
    let without = section.split("Allowed only after you confirm").next().unwrap();
    // What the gate does:
    let mut read = |p: &str| std::fs::read_to_string(p).map_err(|e| e.to_string());
    let w = remit::load(&[WORLD.to_string()], &mut read);
    let b = Binding { name: "annex".into(), address: "a".into(), file: 0, service: "Annex".into() };
    let mut g = Gate::new(&w, 0, "AskFirst", vec![b]).unwrap();
    let now = parse_datetime("2026-11-10T12:00Z").unwrap();
    let s1 = r#"{"id":"s1","start":"2026-11-13T19:00+02:00","seats":4,"deposit":"200 ZAR"}"#;
    let Decision::Allow { call, .. } = g.call("annex", "findTables", &parse(r#"{"date":"2026-11-13"}"#).unwrap(), now) else { panic!() };
    g.result(call, &parse(&format!("[{}]", s1)).unwrap(), now).unwrap();
    let d = g.call("annex", "book", &parse(&format!(r#"{{"slot":{},"guest":{{"name":"T"}}}}"#, s1)).unwrap(), now);
    // Fixed by trying the `allow` rule first whichever is written first, so the gate and
    // the summary agree: the first booking needs no confirmation.
    assert!(matches!(d, Decision::Allow { .. }), "{:?}", d);
    assert!(without.contains("Restaurant.book"), "{}", section);
}

/// SPEC 6: descriptions "are the words a person sees when asked to approve a grant".
/// A description may hold a carriage return and ANSI escape sequences; `remit check`
/// accepts it and `remit explain` prints them raw, so a terminal shows different words.
#[test]
fn finding_descriptions_cannot_carry_terminal_control_characters() {
    // Fixed by refusing the contract: a string may hold only visible characters.
    assert_eq!(check_file("tests/audit/explain-tricks.remit"), vec!["R00"]);
}

/// SPEC 14: "`times <=` counts every call the gate lets through". An action that is its
/// own undo is accepted by the checker; with `times <= 1` the gate lets it run twice.
#[test]
fn finding_times_counts_an_action_that_is_its_own_undo() {
    // Fixed by refusing the contract: an action cannot be its own undo.
    assert_eq!(check_file("tests/audit/self-undo.remit"), vec!["R07"]);
}

/// SPEC 3: Date and DateTime literals are ISO 8601. `2026-02-30` is not a date. The checker
/// accepts it in a condition and in `expires`; the gate then refuses to start.
#[test]
fn finding_the_checker_refuses_dates_that_do_not_exist() {
    let codes = check_file("tests/audit/bad-dates.remit");
    let out = Command::new(BIN)
        .args(["gate", "tests/audit/bad-dates.remit", "--bind", "d=tests/audit/bad-dates.remit#Diary"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        !codes.is_empty(),
        "check passed; gate says: {}",
        String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("")
    );
}

/// SPEC 3: Money is "number, then a three-letter currency code". The same amount checks
/// with a decimal point and fails (R00 "too large") without one, because the lexer reads
/// whole numbers as i64 while money is held as i128.
#[test]
fn finding_money_amounts_do_not_depend_on_a_decimal_point() {
    // Fixed by refusing both: an amount has at most 18 digits before the point.
    assert_eq!(check_file("tests/audit/big-money-decimal.remit"), vec!["R00"]);
    assert_eq!(check_file("tests/audit/big-money.remit"), vec!["R00"]);
}

/// Robustness: deep nesting in a contract overflows the parser's stack. `remit check`
/// must report R00 (or pass), not die from a signal.
#[test]
fn finding_deeply_nested_types_do_not_crash_the_checker() {
    let dir = env!("CARGO_TARGET_TMPDIR");
    let path = format!("{}/deep.remit", dir);
    let n = 200_000;
    std::fs::write(&path, format!("type A {{ x: {}Text{} }}\n", "List<".repeat(n), ">".repeat(n))).unwrap();
    let (code, _, killed) = run_limited(Command::new(BIN).args(["check", &path]), b"", Duration::from_secs(30));
    assert!(!killed && matches!(code, Some(0) | Some(1)), "remit check ended with {:?} (None = signal)", code);

    let path = format!("{}/deep-list.remit", dir);
    std::fs::write(
        &path,
        format!(
            "\"S.\"\nservice S {{\n  \"R.\"\n  action r(x: Text) effects: read\n}}\n\"G.\"\ngrant G {{\n  allow S.r where x in {}\"a\"{}\n  expires 2026-11-13T23:59+02:00\n}}\n",
            "[".repeat(n),
            "]".repeat(n)
        ),
    )
    .unwrap();
    let (code, _, killed) = run_limited(Command::new(BIN).args(["check", &path]), b"", Duration::from_secs(30));
    assert!(!killed && matches!(code, Some(0) | Some(1)), "remit check (nested list literal) ended with {:?}", code);
}

/// Robustness: a `use` address is read with no check on what it names. A contract that
/// uses a FIFO (or a device such as /dev/zero via `../` climbing) makes `remit check`
/// hang or read without bound.
#[test]
fn finding_a_use_line_cannot_hang_the_checker() {
    let dir = env!("CARGO_TARGET_TMPDIR");
    let fifo = format!("{}/pipe.remit", dir);
    let _ = std::fs::remove_file(&fifo);
    assert!(Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    let main = format!("{}/uses-pipe.remit", dir);
    std::fs::write(&main, "use p from \"./pipe.remit\"\n").unwrap();
    let (code, _, killed) = run_limited(Command::new(BIN).args(["check", &main]), b"", Duration::from_secs(5));
    let _ = std::fs::remove_file(&fifo);
    assert!(!killed, "remit check was still running after 5 s (exit {:?})", code);
}

/// Robustness: one request line that is not UTF-8 ends the gate (read_line errors and the
/// loop breaks). The later, valid request gets no answer, and all counts are lost.
#[test]
fn finding_a_bad_byte_does_not_stop_the_gate() {
    let mut input = b"\xff\n".to_vec();
    input.extend_from_slice(b"{\"op\":\"state\"}\n");
    let (code, out, killed) = run_limited(
        Command::new(BIN).args(["gate", WORLD, "--grant", "Keep", "--bind", &format!("forever={}#Forever", WORLD), "--now", "2026-11-10T12:00Z"]),
        &input,
        Duration::from_secs(10),
    );
    assert!(!killed);
    assert_eq!(out.lines().count(), 2, "gate exited {:?} after the bad line; output: {:?}", code, out);
}

/// `--now` that does not parse is ignored silently and the gate uses the system clock.
#[test]
fn finding_an_invalid_now_is_refused() {
    let (code, out, _) = run_limited(
        Command::new(BIN).args(["gate", WORLD, "--grant", "Keep", "--bind", &format!("forever={}#Forever", WORLD), "--now", "2026-13-45T99:99Z"]),
        b"{\"op\":\"state\"}\n",
        Duration::from_secs(10),
    );
    assert_ne!(code, Some(0), "gate started anyway and answered: {}", out.trim());
}

// ===========================================================================
// COVERAGE: section 11 conformance and section 13 rules
// ===========================================================================

const IFACE: &str = r#"
type Slot { id: Text  start: DateTime }
type Big  { id: Text  start: DateTime  extra: Text }
type Booking { id: Text }
type Note { body: Untrusted<Text> }
"I."
interface I {
  "Find."
  action find(day: Date, note: Untrusted<Text>) -> List<Slot>
    effects: read
  "Book."
  action book(slot: Returned<Slot>, who: Text?) -> Booking
    effects: write, spend(max: 500 ZAR)
    undo: cancel within 24h
  "Cancel."
  action cancel(b: Booking)
    effects: write
  "Notes."
  action notes() -> Note
    effects: read
}
"#;

fn svc(body: &str) -> Vec<&'static str> {
    let src = format!("{}\n\"S.\"\nservice S implements I {{\n{}\n}}\n", IFACE, body);
    let leaked: &'static str = Box::leak(src.into_boxed_str());
    check_src(&[("a.remit", leaked)])
}

#[test]
fn conformance_rules_hold() {
    let cases: &[(&str, &[&str])] = &[
        ("", &[]),
        // 2. parameters
        ("\"F.\" action find(day: Date) -> List<Slot> effects: read", &["R08"]),
        ("\"F.\" action find(day: Date, note: Untrusted<Text>, more: Int) -> List<Slot> effects: read", &["R08"]),
        ("\"F.\" action find(day: Date, note: Untrusted<Text>, more: Int?) -> List<Slot> effects: read", &[]),
        ("\"F.\" action find(day: Date, note: Text) -> List<Slot> effects: read", &["R05"]),
        ("\"F.\" action find(day: DateTime, note: Untrusted<Text>) -> List<Slot> effects: read", &["R08"]),
        ("\"B.\" action book(slot: Slot, who: Text?) -> Booking effects: write, spend(max: 500 ZAR) undo: cancel within 24h", &["R08"]),
        ("\"B.\" action book(slot: Returned<Slot>, who: Text) -> Booking effects: write, spend(max: 500 ZAR) undo: cancel within 24h", &["R08"]),
        // 3. result
        ("\"F.\" action find(day: Date, note: Untrusted<Text>) -> List<Big> effects: read", &[]),
        ("\"F.\" action find(day: Date, note: Untrusted<Text>) -> List<Booking> effects: read", &["R08"]),
        ("\"F.\" action find(day: Date, note: Untrusted<Text>) effects: read", &["R08"]),
        ("\"F.\" action find(day: Date, note: Untrusted<Text>) -> List<Untrusted<Slot>> effects: read", &["R05"]),
        // 4. effects and ceiling
        ("\"B.\" action book(slot: Returned<Slot>, who: Text?) -> Booking effects: write, send, spend(max: 500 ZAR) undo: cancel within 24h", &["R08"]),
        ("\"B.\" action book(slot: Returned<Slot>, who: Text?) -> Booking effects: write, spend(max: 501 ZAR) undo: cancel within 24h", &["R08"]),
        ("\"B.\" action book(slot: Returned<Slot>, who: Text?) -> Booking effects: write, spend(max: 500 USD) undo: cancel within 24h", &["R08"]),
        ("\"B.\" action book(slot: Returned<Slot>, who: Text?) -> Booking effects: write, spend undo: cancel within 24h", &["R08"]),
        ("\"B.\" action book(slot: Returned<Slot>, who: Text?) -> Booking effects: write, spend(max: 499.999999 ZAR) undo: cancel within 24h", &[]),
        ("\"C.\" action cancel(b: Booking) effects: write, spend", &["R08"]),
        // 5. undo
        ("\"B.\" action book(slot: Returned<Slot>, who: Text?) -> Booking effects: write, spend(max: 500 ZAR)", &["R08"]),
        ("\"B.\" action book(slot: Returned<Slot>, who: Text?) -> Booking effects: write, spend(max: 500 ZAR) undo: cancel within 23h", &["R08"]),
        ("\"B.\" action book(slot: Returned<Slot>, who: Text?) -> Booking effects: write, spend(max: 500 ZAR) undo: cancel within 1d", &[]),
        ("\"B.\" action book(slot: Returned<Slot>, who: Text?) -> Booking effects: write, spend(max: 500 ZAR) undo: cancel", &[]),
        ("\"B.\" action book(slot: Returned<Slot>, who: Text?) -> Booking effects: write, spend(max: 500 ZAR) undo: other within 24h\n\"O.\" action other(b: Booking) effects: write", &["R08"]),
        ("\"C.\" action cancel(b: Booking, why: Text) effects: write", &["R08"]),
        // The undo target restated to take something else.
        ("\"C.\" action cancel(b: Slot) effects: write", &["R08"]),
        // R14: parameter names
        ("\"X.\" action x(times: Int) effects: read", &["R14"]),
        // Untrusted result through an interface that does not mark it
        ("\"N.\" action notes() -> Note effects: read", &[]),
    ];
    for (body, want) in cases {
        let mut got = svc(body);
        got.sort();
        got.dedup();
        let mut want = want.to_vec();
        want.sort();
        assert_eq!(got, want, "service body: {}", body);
    }
}

#[test]
fn grant_rules_hold() {
    let g = |rule: &str| {
        let src = format!("{}\n\"S.\"\nservice S implements I {{ }}\n\"G.\"\ngrant G {{\n  {}\n  expires 2026-11-13T23:59+02:00\n}}\n", IFACE, rule);
        let leaked: &'static str = Box::leak(src.into_boxed_str());
        let mut c = check_src(&[("a.remit", leaked)]);
        c.sort();
        c.dedup();
        c
    };
    assert_eq!(g("allow I.book where spend <= 500 ZAR"), Vec::<&str>::new());
    assert_eq!(g("allow I.book"), vec!["R15"]);
    assert_eq!(g("ask I.book"), Vec::<&str>::new());
    assert_eq!(g("allow I.book where spend <= 500 USD"), vec!["R10"]);
    assert_eq!(g("allow I.find where spend <= 5 ZAR"), vec!["R10"]);
    assert_eq!(g("allow I.cancel"), vec!["R11"]);
    assert_eq!(g("allow I.cancel where irreversible"), Vec::<&str>::new());
    assert_eq!(g("allow I.find where day == \"x\""), vec!["R10"]);
    assert_eq!(g("allow I.find where note == \"x\""), Vec::<&str>::new());
    assert_eq!(g("allow I.book where spend <= 5 ZAR, slot.start <= 2026-11-13T19:00Z"), Vec::<&str>::new());
    assert_eq!(g("allow I.book where spend <= 5 ZAR, slot.nope == 1"), vec!["R09"]);
    assert_eq!(g("allow I.book where spend <= 5 ZAR, slot.id.x == 1"), vec!["R10"]);
    assert_eq!(g("allow I.book where spend <= 5 ZAR, who <= \"a\""), vec!["R10"]);
    assert_eq!(g("allow I.book where spend <= 5 ZAR, times <= 0"), vec!["R10"]);
    assert_eq!(g("allow I.book where spend <= 5 ZAR, service in []"), vec!["R10"]);
    assert_eq!(g("allow I.nope"), vec!["R09"]);
    assert_eq!(g("allow S.find"), Vec::<&str>::new());
    assert_eq!(g("allow J.find"), vec!["R09"]);
}

#[test]
fn returned_rules_hold() {
    let one = |src: &str| {
        let leaked: &'static str = Box::leak(src.to_string().into_boxed_str());
        let mut c = check_src(&[("a.remit", leaked)]);
        c.sort();
        c.dedup();
        c
    };
    let base = "type T { id: Text }\ntype U { id: Text }\n\"S.\"\nservice S {\n  \"L.\" action list() -> List<T> effects: read\n";
    assert_eq!(one(&format!("{}  \"A.\" action a(x: Returned<T>) effects: write\n}}\n", base)), Vec::<&str>::new());
    assert_eq!(one(&format!("{}  \"A.\" action a(x: Returned<U>) effects: write\n}}\n", base)), vec!["R16"]);
    assert_eq!(one(&format!("{}  \"A.\" action a(x: List<Returned<T>>) effects: write\n}}\n", base)), vec!["R16"]);
    assert_eq!(one(&format!("{}  \"A.\" action a(x: Returned<Text>) effects: write\n}}\n", base)), vec!["R16"]);
    assert_eq!(one(&format!("{}  \"A.\" action a(x: Returned<Untrusted<T>>) effects: write\n}}\n", base)), vec!["R16"]);
    assert_eq!(one(&format!("{}  \"A.\" action a() -> Returned<T> effects: write undo: b\n  \"B.\" action b(t: T) effects: write\n}}\n", base)), vec!["R16"]);
}

#[test]
fn gate_paths_with_dot_segments() {
    // `./` prefixes are stripped; a binding reached as tests/audit/../audit/world.remit is a
    // different file to the gate, so rules on the interface do not cover it (fails closed).
    let input = b"{\"op\":\"call\",\"service\":\"f\",\"action\":\"reserveHour\"}\n{\"op\":\"call\",\"service\":\"g\",\"action\":\"reserveHour\"}\n";
    let (code, out, _) = run_limited(
        Command::new(BIN).args([
            "gate",
            "./tests/audit/world.remit",
            "--grant",
            "Keep",
            "--bind",
            "f=././tests/audit/world.remit#Forever",
            "--bind",
            "g=tests/audit/../audit/world.remit#Forever",
            "--now",
            "2026-11-10T12:00Z",
        ]),
        input,
        Duration::from_secs(10),
    );
    assert_eq!(code, Some(0));
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines[0].contains("\"allow\""), "{}", out);
    assert!(lines[1].contains("G02"), "{}", out);
}
