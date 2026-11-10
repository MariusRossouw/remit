//! Diagnostics: one structured record per broken rule.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone)]
pub struct Diag {
    pub code: &'static str,
    pub file: String,
    pub pos: Pos,
    pub message: String,
    pub fix: String,
}

/// The rule behind a code, in one sentence. Mirrors section 13 of SPEC.md.
pub fn rule(code: &str) -> &'static str {
    match code {
        "R00" => "The file does not follow the grammar.",
        "R01" => "A name does not resolve, or is declared twice.",
        "R02" => "A name has the wrong case for its kind.",
        "R03" => "A required description is missing or empty.",
        "R04" => "A type refers to itself.",
        "R05" => "Untrusted<T> is used where T is expected.",
        "R06" => "`read` is combined with another effect, or a `read` action has an undo.",
        "R07" => "An undo target is missing, does not accept the action's result, is the action itself, or spends.",
        "R08" => "A service does not conform to an interface it claims.",
        "R09" => "A grant rule names an action or parameter that does not exist.",
        "R10" => "A grant condition has the wrong type, is repeated, or `spend <=` is on an action that does not spend.",
        "R11" => "An `allow` rule covers an irreversible action without `irreversible`.",
        "R12" => "A fetched file does not match remit.lock.",
        "R13" => "Files use each other in a cycle.",
        "R14" => "A name collides with a built-in.",
        "R15" => "An `allow` rule covers an action that spends without `spend <=`.",
        "R16" => "`Returned` is used somewhere other than a parameter, on something that is not a record, or on a type nothing returns.",
        "R17" => "Two `allow` rules, or two `ask` rules, can cover the same call.",
        "T01" => "This checker does not support this yet.",
        _ => "",
    }
}

/// Characters that change how text is displayed without being seen: control
/// characters, line and paragraph separators, and marks that reverse or hide text.
pub fn hidden(c: char) -> bool {
    let n = c as u32;
    n < 0x20
        || (0x7f..=0x9f).contains(&n)
        || matches!(n, 0x200b..=0x200f | 0x2028..=0x202e | 0x2060..=0x2069 | 0xfeff)
}

pub fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if hidden(c) => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn to_json(diags: &[Diag]) -> String {
    let mut out = String::new();
    out.push_str(&format!("{{\"ok\":{},\"errors\":[", diags.is_empty()));
    for (i, d) in diags.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"code\":{},\"file\":{},\"line\":{},\"col\":{},\"rule\":{},\"message\":{},\"fix\":{}}}",
            json_string(d.code),
            json_string(&d.file),
            d.pos.line,
            d.pos.col,
            json_string(rule(d.code)),
            json_string(&d.message),
            json_string(&d.fix),
        ));
    }
    out.push_str("]}");
    out
}

pub fn to_text(diags: &[Diag]) -> String {
    let mut out = String::new();
    for d in diags {
        out.push_str(&format!(
            "{}:{}:{}: {} {}\n  rule: {}\n  fix:  {}\n",
            d.file,
            d.pos.line,
            d.pos.col,
            d.code,
            d.message,
            rule(d.code),
            d.fix
        ));
    }
    out
}
