//! Every file in tests/bad breaks exactly the rule its name starts with.
//! Every file in tests/good and examples/ breaks none.

use std::fs;

fn check(path: &str) -> Vec<remit::Diag> {
    let mut read = |p: &str| fs::read_to_string(p).map_err(|e| e.to_string());
    remit::load(&[path.to_string()], &mut read).diags
}

fn remit_files(dir: &str) -> Vec<String> {
    let mut files: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".remit"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .remit files in {}", dir);
    files
}

#[test]
fn bad_files_break_exactly_their_rule() {
    for path in remit_files("tests/bad") {
        let name = path.rsplit('/').next().unwrap();
        let expected = &name[..3];
        let diags = check(&path);
        let codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
        assert!(!codes.is_empty(), "{}: expected {}, but nothing was reported", name, expected);
        assert!(
            codes.iter().all(|c| *c == expected),
            "{}: expected only {}, got {:?}\n{}",
            name,
            expected,
            codes,
            remit::diag::to_text(&diags)
        );
        for d in &diags {
            assert!(!d.message.is_empty() && !d.fix.is_empty(), "{}: empty message or fix", name);
            assert!(d.pos.line >= 1 && d.pos.col >= 1, "{}: missing position", name);
        }
    }
}

#[test]
fn good_files_break_nothing() {
    for dir in ["tests/good", "examples/dinner"] {
        for path in remit_files(dir) {
            let diags = check(&path);
            assert!(diags.is_empty(), "{}:\n{}", path, remit::diag::to_text(&diags));
        }
    }
}

#[test]
fn every_rule_in_the_spec_has_a_failing_example() {
    let spec = fs::read_to_string("SPEC.md").unwrap();
    let names: Vec<String> = remit_files("tests/bad");
    let is_rule = |l: &&str| l.starts_with("| R") && l[3..5].chars().all(|c| c.is_ascii_digit());
    for line in spec.lines().filter(is_rule) {
        let code = &line[2..5];
        if code == "R12" {
            continue; // The lockfile is not implemented yet.
        }
        assert!(
            names.iter().any(|n| n.rsplit('/').next().unwrap().starts_with(code)),
            "SPEC.md lists {} but tests/bad has no example of it",
            code
        );
        assert!(!remit::diag::rule(code).is_empty(), "{} has no rule text in diag.rs", code);
    }
}

#[test]
fn json_output_is_structured() {
    let diags = check("tests/bad/R11-irreversible-allowed.remit");
    let json = remit::diag::to_json(&diags);
    assert!(json.starts_with("{\"ok\":false,\"errors\":[{\"code\":\"R11\""), "{}", json);
    assert!(json.contains("\"line\":10") && json.contains("\"fix\":"), "{}", json);
    assert_eq!(remit::diag::to_json(&[]), "{\"ok\":true,\"errors\":[]}");
}

#[test]
fn a_grant_is_explained_in_the_contracts_own_words() {
    let mut read = |p: &str| fs::read_to_string(p).map_err(|e| e.to_string());
    let world = remit::load(&["examples/dinner/friday-dinner.remit".to_string()], &mut read);
    let text = remit::explain::explain(&world, world.entries[0]);
    for expected in [
        "Book Friday dinner for four and split the deposit.",
        "Reserve a table. May take a deposit.",
        "limits: total spend up to 500 ZAR; at most 1 call",
        "undo:   Cancel a booking and return its deposit.",
        "Nothing else is allowed. Ends 2026-11-13T23:59+02:00.",
    ] {
        assert!(text.contains(expected), "missing {:?} in:\n{}", expected, text);
    }
}

#[test]
fn an_irreversible_action_is_called_out_when_explained() {
    let mut read = |p: &str| fs::read_to_string(p).map_err(|e| e.to_string());
    let world = remit::load(&["tests/good/irreversible-accepted.remit".to_string()], &mut read);
    let text = remit::explain::explain(&world, world.entries[0]);
    assert!(text.contains("CANNOT BE UNDONE"), "{}", text);
    assert!(text.contains("Allowed only after you confirm each time:"), "{}", text);
}
