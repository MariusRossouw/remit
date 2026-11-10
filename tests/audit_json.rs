//! src/json.rs against Python's `json` module (CPython 3, default `json.loads`).
//! The expected Python result for each input is written beside it; it was observed by
//! running `python3 -I -c 'import json; json.loads(...)'` on the same text.
//! A failing test is a finding: an input the two read differently, or that only one accepts.

use remit::json::{parse, Value};

fn ok(s: &str) -> Value {
    parse(s).unwrap_or_else(|e| panic!("{:?} refused: {}", s, e))
}
fn refused(s: &str) -> bool {
    parse(s).is_err()
}

// ---------------------------------------------------------------------------
// Findings
// ---------------------------------------------------------------------------

/// Python: `json.loads('"\\u+041"')` -> JSONDecodeError "Invalid \uXXXX escape".
/// Rust's `u32::from_str_radix` accepts a leading `+`, so the gate reads "A".
#[test]
fn finding_a_plus_sign_inside_a_unicode_escape_is_refused() {
    let got = parse(r#""\u+041""#);
    assert!(got.is_err(), "accepted as {:?}; Python refuses it", got);
}

/// Python: `json.loads('01')`, `'-01'`, `'00'`, `'{"a":01}'` -> JSONDecodeError.
/// The gate accepts them, and an `Int` parameter takes `"qty":01` as 1.
#[test]
fn finding_numbers_with_leading_zeros_are_refused() {
    for s in ["01", "-01", "00", r#"{"a":01}"#, "[007]"] {
        assert!(refused(s), "{} accepted as {:?}; Python refuses it", s, parse(s));
    }
}

// ---------------------------------------------------------------------------
// Coverage: both refuse, or both read the same
// ---------------------------------------------------------------------------

#[test]
fn both_refuse() {
    for s in [
        r#""\u-041""#, // Python: Invalid \uXXXX escape
        "[1,]", "{\"a\":1,}", ".5", "1.", "+1", "1e2", "1E2", "NaN", "Infinity", "nul",
        "\u{feff}1",  // Python: Unexpected UTF-8 BOM
        "\u{a0}1",    // NBSP is not JSON whitespace
        " \u{c}1",    // form feed is not JSON whitespace
        "\"\u{1}\"",  // raw control character
        "\"\t\"",
        r#""\x41""#, r#""\'""#, r#""\u12""#, r#""\u12G4""#,
        "\"abc", "[", "{\"a\"}", "{a:1}", "'a'",
        "1 2",
    ] {
        assert!(refused(s), "{:?} accepted as {:?}", s, parse(s));
    }
}

/// Python reads a repeated key as the last value. The gate refuses it, including when the
/// two spellings differ only by escapes.
#[test]
fn repeated_keys_are_refused_however_they_are_spelled() {
    assert!(refused(r#"{"a":1,"a":2}"#));
    assert!(refused(r#"{"a":1,"\u0061":2}"#));
    assert!(refused(r#"{"to":"x","t\u006f":"y"}"#));
    assert!(refused(r#"{"x":{"a":1,"a":1}}"#));
}

/// Python accepts lone surrogates; the gate refuses them (stricter, no divergent reading).
#[test]
fn surrogates() {
    assert!(refused(r#""\ud800""#));
    assert!(refused(r#""\udc00""#));
    assert!(refused(r#""\ud800\u0041""#));
    assert_eq!(ok(r#""\ud83d\ude00""#), Value::Str("\u{1f600}".into()));
    assert_eq!(ok(r#""\uD83D\uDE00""#), Value::Str("\u{1f600}".into()));
}

#[test]
fn escapes_read_the_same() {
    assert_eq!(ok(r#""\/""#), Value::Str("/".into()));
    assert_eq!(ok(r#""\u00e9""#), Value::Str("\u{e9}".into()));
    assert_eq!(ok(r#""\u0000""#), Value::Str("\u{0}".into()));
    assert_eq!(ok("\"\u{7f}\""), Value::Str("\u{7f}".into()));
    assert_eq!(ok(r#""\b\f\n\r\t\"\\""#), Value::Str("\u{8}\u{c}\n\r\t\"\\".into()));
    // Written back, control characters are escaped and the text parses to the same value.
    let v = ok(r#""a\u0001\u001fb\u2028""#);
    assert_eq!(ok(&v.text()), v);
}

#[test]
fn numbers_keep_their_text() {
    assert_eq!(ok("-0"), Value::Num("-0".into()));
    assert_eq!(ok("1.0"), Value::Num("1.0".into()));
    assert_eq!(ok("0.1234567"), Value::Num("0.1234567".into()));
    let long = "9".repeat(5000);
    assert_eq!(ok(&long), Value::Num(long.clone()));
}

#[test]
fn whitespace_and_depth() {
    assert_eq!(ok(" \t\r\n[ 1 , 2 ] \n"), ok("[1,2]"));
    let deep = |n: usize| format!("{}{}", "[".repeat(n), "]".repeat(n));
    assert!(parse(&deep(65)).is_ok());
    assert!(refused(&deep(66)));
    assert!(refused(&deep(100_000))); // no stack overflow
    let objs = format!("{}1{}", "{\"a\":".repeat(100_000), "}".repeat(100_000));
    assert!(refused(&objs));
}
