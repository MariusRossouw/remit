//! The gate: decides at call time whether a call is inside a grant.
//!
//! The agent is not trusted. Whatever it asks for, only calls a rule of the
//! grant permits get through, and every limit is counted here, not by the agent.
//! The gate keeps no clock and opens no connection: the host passes the time in.
//! See SPEC.md section 14.

use crate::ast::*;
use crate::check::{RTy, Ty, World};
use crate::json::Value;

/// A service the host has connected, under the name the agent calls it by.
pub struct Binding {
    pub name: String,
    pub address: String,
    pub file: usize,
    pub service: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Deny {
    pub code: &'static str,
    pub reason: String,
}

fn deny<T>(code: &'static str, reason: String) -> Result<T, Deny> {
    Err(Deny { code, reason })
}

/// What each refusal code means, in one sentence.
pub fn refusal(code: &str) -> &'static str {
    match code {
        "G01" => "The grant has ended or was revoked.",
        "G02" => "No rule in the grant covers this call.",
        "G03" => "The arguments do not fit the action.",
        "G04" => "A condition of the rule does not hold.",
        "G05" => "The rule's call limit is used up.",
        "G06" => "A spending limit would be exceeded.",
        "G07" => "The person declined.",
        "G08" => "The call is unknown, already finished, or its result does not fit the action.",
        _ => "",
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// The call may go to the service. `reserve` is the most it may charge.
    Allow { call: u64, reserve: Option<String>, undo: bool },
    /// The person must confirm first. Pass their answer to `confirm`.
    Ask { call: u64, prompt: String },
    Deny(Deny),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Delivered {
    /// The result, holding only the fields the action declares.
    pub value: Value,
    /// JSON pointers to every part marked `Untrusted`.
    pub untrusted: Vec<String>,
}

// ---------------------------------------------------------------------------
// Values on the wire
// ---------------------------------------------------------------------------

const SCALE: i128 = 1_000_000;

/// "12" or "12.5" as millionths. At most six decimal places.
pub fn parse_scaled(s: &str) -> Option<i128> {
    let (neg, body) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let (whole, frac) = body.split_once('.').unwrap_or((body, ""));
    let digits = |t: &str| t.bytes().all(|b| b.is_ascii_digit());
    if whole.is_empty() || whole.len() > 24 || !digits(whole) || !digits(frac) || frac.len() > 6 {
        return None;
    }
    if body.contains('.') && frac.is_empty() {
        return None;
    }
    let n: i128 = format!("{}{:0<6}", whole, frac).parse().ok()?;
    Some(if neg { -n } else { n })
}

pub fn show_scaled(n: i128) -> String {
    let (sign, n) = if n < 0 { ("-", -n) } else { ("", n) };
    let frac = format!("{:06}", n % SCALE);
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        format!("{}{}", sign, n / SCALE)
    } else {
        format!("{}{}.{}", sign, n / SCALE, frac)
    }
}

/// "500 ZAR". Never negative: a negative charge would raise a limit.
pub fn parse_money(s: &str) -> Option<(i128, String)> {
    let (number, currency) = s.split_once(' ')?;
    if currency.len() != 3 || !currency.bytes().all(|b| b.is_ascii_uppercase()) {
        return None;
    }
    let amount = parse_scaled(number)?;
    if amount < 0 {
        return None;
    }
    Some((amount, currency.to_string()))
}

pub fn parse_duration(s: &str) -> Option<i128> {
    let per = match s.as_bytes().last()? {
        b'm' => 1,
        b'h' => 60,
        b'd' => 60 * 24,
        _ => return None,
    };
    let digits = &s[..s.len() - 1];
    if digits.is_empty() || digits.len() > 12 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(digits.parse::<i128>().ok()? * per)
}

fn num(b: &[u8], from: usize, len: usize) -> Option<i64> {
    let part = b.get(from..from + len)?;
    if !part.iter().all(|c| c.is_ascii_digit()) {
        return None;
    }
    std::str::from_utf8(part).ok()?.parse().ok()
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn date_at(b: &[u8]) -> Option<i64> {
    let (y, m, d) = (num(b, 0, 4)?, num(b, 5, 2)?, num(b, 8, 2)?);
    if b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') {
        return None;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let last = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => 28 + leap as i64,
        _ => return None,
    };
    if d < 1 || d > last {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

/// Days since 1970-01-01.
pub fn parse_date(s: &str) -> Option<i64> {
    if s.len() != 10 {
        return None;
    }
    date_at(s.as_bytes())
}

/// Seconds since 1970-01-01T00:00Z. The offset is required.
pub fn parse_datetime(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let days = date_at(b)?;
    if b.get(10) != Some(&b'T') || b.get(13) != Some(&b':') {
        return None;
    }
    let (h, mi) = (num(b, 11, 2)?, num(b, 14, 2)?);
    let mut i = 16;
    let mut sec = 0;
    if b.get(i) == Some(&b':') {
        sec = num(b, i + 1, 2)?;
        i += 3;
    }
    if h > 23 || mi > 59 || sec > 59 {
        return None;
    }
    let offset = match b.get(i)? {
        b'Z' => {
            i += 1;
            0
        }
        sign @ (b'+' | b'-') => {
            let (oh, om) = (num(b, i + 1, 2)?, num(b, i + 4, 2)?);
            if b.get(i + 3) != Some(&b':') || oh > 23 || om > 59 {
                return None;
            }
            i += 6;
            (oh * 3600 + om * 60) * if *sign == b'+' { 1 } else { -1 }
        }
        _ => return None,
    };
    if i != b.len() {
        return None;
    }
    Some(days * 86400 + h * 3600 + mi * 60 + sec - offset)
}

/// A value reduced to something two sides of a condition can compare.
#[derive(PartialEq)]
enum Key {
    Text(String),
    Num(i128),
    Money(i128, String),
}

fn key_of_value(s: Scalar, v: &Value) -> Option<Key> {
    Some(match (s, v) {
        (Scalar::Text, Value::Str(t)) => Key::Text(t.clone()),
        (Scalar::Int | Scalar::Decimal, Value::Num(n)) => Key::Num(parse_scaled(n)?),
        (Scalar::Decimal, Value::Str(n)) => Key::Num(parse_scaled(n)?),
        (Scalar::Money, Value::Str(m)) => {
            let (a, c) = parse_money(m)?;
            Key::Money(a, c)
        }
        (Scalar::Duration, Value::Str(d)) => Key::Num(parse_duration(d)?),
        (Scalar::Date, Value::Str(d)) => Key::Num(parse_date(d)? as i128),
        (Scalar::DateTime, Value::Str(d)) => Key::Num(parse_datetime(d)? as i128),
        _ => return None,
    })
}

fn key_of_literal(l: &Literal) -> Option<Key> {
    Some(match l {
        Literal::Str(s) => Key::Text(s.clone()),
        Literal::Int(n) => Key::Num(*n as i128 * SCALE),
        Literal::Decimal(s) => Key::Num(parse_scaled(s)?),
        Literal::Money(m) => Key::Money(m.amount, m.currency.clone()),
        Literal::Duration(d) => Key::Num(d.minutes as i128),
        Literal::Date(s) => Key::Num(parse_date(s)? as i128),
        Literal::DateTime(s) => Key::Num(parse_datetime(s)? as i128),
        Literal::List(_) => return None,
    })
}

/// Does `value op literal` hold? Anything that cannot be compared does not hold.
fn cond_holds(scalar: Scalar, op: Op, literal: &Literal, value: &Value) -> bool {
    let Some(have) = key_of_value(scalar, value) else {
        return false;
    };
    let ordered = |want: &Literal, le: bool| match (&have, key_of_literal(want)) {
        (Key::Num(a), Some(Key::Num(b))) => (le && *a <= b) || (!le && *a >= b),
        (Key::Money(a, ac), Some(Key::Money(b, bc))) => *ac == bc && ((le && *a <= b) || (!le && *a >= b)),
        _ => false,
    };
    match op {
        Op::Eq => key_of_literal(literal).is_some_and(|k| k == have),
        Op::Le => ordered(literal, true),
        Op::Ge => ordered(literal, false),
        Op::In => match literal {
            Literal::List(items) => items.iter().any(|i| key_of_literal(i).is_some_and(|k| k == have)),
            _ => false,
        },
    }
}

// ---------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------

#[derive(Default)]
struct RuleState {
    calls: i64,
    spent: i128,
    /// Reserved by calls that have not finished.
    held: i128,
}

#[derive(PartialEq)]
enum Status {
    Pending,
    Open,
    Closed,
}

struct Call {
    binding: usize,
    action: String,
    rule: Option<usize>,
    status: Status,
    spends: bool,
    reserve: Option<(i128, String)>,
    charged: i128,
    args: Value,
}

struct UndoRight {
    binding: usize,
    action: String,
    /// The result that may be undone, as canonical text.
    value: String,
    until: Option<i64>,
    used: bool,
}

pub struct Gate<'w> {
    w: &'w World,
    f: usize,
    grant: &'w Grant,
    expires: i64,
    bindings: Vec<Binding>,
    rules: Vec<RuleState>,
    calls: Vec<Call>,
    undos: Vec<UndoRight>,
    revoked: bool,
}

impl<'w> Gate<'w> {
    /// `f` is the file that holds the grant. Refuses contracts that did not pass the checker.
    pub fn new(w: &'w World, f: usize, grant: &str, bindings: Vec<Binding>) -> Result<Gate<'w>, String> {
        if !w.diags.is_empty() {
            return Err("The contracts did not pass the checker.".into());
        }
        let grant = match w.find_decl(f, grant) {
            Some(Decl::Grant(g)) => g,
            _ => return Err(format!("There is no grant called `{}`.", grant)),
        };
        let expires = parse_datetime(&grant.expires).ok_or("The grant's expiry is not a valid date and time.")?;
        for (i, b) in bindings.iter().enumerate() {
            if !matches!(w.find_decl(b.file, &b.service), Some(Decl::Service(_))) {
                return Err(format!("There is no service called `{}`.", b.service));
            }
            if bindings[..i].iter().any(|o| o.name == b.name) {
                return Err(format!("Two services are connected as `{}`.", b.name));
            }
        }
        let rules = grant.rules.iter().map(|_| RuleState::default()).collect();
        Ok(Gate { w, f, grant, expires, bindings, rules, calls: Vec::new(), undos: Vec::new(), revoked: false })
    }

    /// Ends the grant now. Nothing is permitted afterwards.
    pub fn revoke(&mut self) {
        self.revoked = true;
    }

    fn live(&self, now: i64) -> Result<(), Deny> {
        if self.revoked {
            return deny("G01", "The grant was revoked.".into());
        }
        if now >= self.expires {
            return deny("G01", format!("The grant ended at {}.", self.grant.expires));
        }
        Ok(())
    }

    fn service(&self, b: usize) -> &'w Service {
        match self.w.find_decl(self.bindings[b].file, &self.bindings[b].service) {
            Some(Decl::Service(s)) => s,
            _ => unreachable!("checked in new"),
        }
    }

    fn rule_covers(&self, rule: &Rule, b: usize, action: &str) -> bool {
        if rule.action.name != action {
            return false;
        }
        let binding = &self.bindings[b];
        let Some(tf) = self.w.file_of(self.f, &rule.target.alias) else {
            return false;
        };
        let name = &rule.target.name.name;
        match self.w.find_decl(tf, name) {
            Some(Decl::Service(_)) => tf == binding.file && *name == binding.service,
            Some(Decl::Interface(_)) => self
                .service(b)
                .implements
                .iter()
                .any(|q| q.name.name == *name && self.w.file_of(binding.file, &q.alias) == Some(tf)),
            _ => false,
        }
    }

    /// Checks `v` against a type and returns it holding only declared fields.
    /// `strict` also refuses fields the type does not declare.
    fn fit(&self, ty: &RTy, v: &Value, strict: bool, path: &str, untrusted: &mut Vec<String>) -> Result<Value, String> {
        if v.is_null() {
            return if ty.optional { Ok(Value::Null) } else { Err(format!("`{}` is required", path)) };
        }
        if ty.untrusted {
            untrusted.push(path.to_string());
        }
        let wrong = |want: &str| Err(format!("`{}` must be {}", path, want));
        match &ty.ty {
            Ty::Unknown => Err(format!("`{}` has a type that did not resolve", path)),
            Ty::Scalar(s) => {
                let ok = match (s, v) {
                    (Scalar::Bool, Value::Bool(_)) => true,
                    (Scalar::Text, Value::Str(_)) => true,
                    (Scalar::Int, Value::Num(n)) => n.parse::<i64>().is_ok(),
                    (Scalar::Decimal, Value::Num(n) | Value::Str(n)) => parse_scaled(n).is_some(),
                    (Scalar::Money, Value::Str(m)) => parse_money(m).is_some(),
                    (Scalar::Duration, Value::Str(d)) => parse_duration(d).is_some(),
                    (Scalar::Date, Value::Str(d)) => parse_date(d).is_some(),
                    (Scalar::DateTime, Value::Str(d)) => parse_datetime(d).is_some(),
                    _ => false,
                };
                if ok {
                    return Ok(v.clone());
                }
                match s {
                    Scalar::Bool => wrong("true or false"),
                    Scalar::Text => wrong("text"),
                    Scalar::Int => wrong("a whole number"),
                    Scalar::Decimal => wrong("a number"),
                    Scalar::Money => wrong("money written like \"500 ZAR\""),
                    Scalar::Duration => wrong("a duration written like \"24h\""),
                    Scalar::Date => wrong("a date written like \"2026-11-13\""),
                    Scalar::DateTime => wrong("a date and time written like \"2026-11-13T19:00+02:00\""),
                }
            }
            Ty::List(inner) => {
                let Value::List(items) = v else {
                    return wrong("a list");
                };
                let mut out = Vec::with_capacity(items.len());
                for (i, item) in items.iter().enumerate() {
                    out.push(self.fit(inner, item, strict, &format!("{}/{}", path, i), untrusted)?);
                }
                Ok(Value::List(out))
            }
            Ty::Enum(f, name) => {
                let Some(Decl::Enum(e)) = self.w.find_decl(*f, name) else {
                    return Err(format!("`{}` has a type that did not resolve", path));
                };
                match v {
                    Value::Str(s) if e.members.iter().any(|m| m.name == *s) => Ok(v.clone()),
                    _ => {
                        let names: Vec<&str> = e.members.iter().map(|m| m.name.as_str()).collect();
                        wrong(&format!("one of: {}", names.join(", ")))
                    }
                }
            }
            Ty::Record(f, name) => {
                let Some(Decl::Type(t)) = self.w.find_decl(*f, name) else {
                    return Err(format!("`{}` has a type that did not resolve", path));
                };
                let Value::Obj(fields) = v else {
                    return wrong(&format!("an object of type {}", name));
                };
                if strict {
                    if let Some((k, _)) = fields.iter().find(|(k, _)| !t.fields.iter().any(|x| x.name.name == *k)) {
                        return Err(format!("`{}` has no field called `{}`", path, k));
                    }
                }
                let mut out = Vec::with_capacity(t.fields.len());
                for field in &t.fields {
                    let fty = self.w.resolve_type(*f, &field.ty);
                    let got = v.get(&field.name.name).unwrap_or(&Value::Null);
                    let fitted = self.fit(&fty, got, strict, &format!("{}/{}", path, field.name.name), untrusted)?;
                    if !fitted.is_null() {
                        out.push((field.name.name.clone(), fitted));
                    }
                }
                Ok(Value::Obj(out))
            }
        }
    }

    /// Checks arguments against an action's parameters.
    fn fit_args(&self, file: usize, action: &Action, args: &Value) -> Result<Value, Deny> {
        let given: &[(String, Value)] = match args {
            Value::Obj(fields) => fields,
            Value::Null => &[],
            _ => return deny("G03", "The arguments must be an object.".into()),
        };
        if let Some((k, _)) = given.iter().find(|(k, _)| !action.params.iter().any(|p| p.name.name == *k)) {
            let names: Vec<&str> = action.params.iter().map(|p| p.name.name.as_str()).collect();
            return deny(
                "G03",
                format!("`{}` has no parameter called `{}`. It takes: {}.", action.name.name, k, names.join(", ")),
            );
        }
        let mut out = Vec::new();
        for p in &action.params {
            let ty = self.w.resolve_type(file, &p.ty);
            let got = args.get(&p.name.name).unwrap_or(&Value::Null);
            match self.fit(&ty, got, true, &p.name.name, &mut Vec::new()) {
                Ok(Value::Null) => {}
                Ok(v) => out.push((p.name.name.clone(), v)),
                Err(why) => return deny("G03", format!("{}.", why)),
            }
        }
        Ok(Value::Obj(out))
    }

    /// Does rule `r` permit this call now? Returns the most the call may charge.
    fn permits(&self, r: usize, b: usize, file: usize, action: &Action, args: &Value) -> Result<Option<(i128, String)>, Deny> {
        let rule = &self.grant.rules[r];
        let state = &self.rules[r];
        let mut limit: Option<&Money> = None;
        for cond in &rule.conds {
            match cond {
                Cond::Irreversible(_) => {}
                Cond::Spend(m, _) => limit = Some(m),
                Cond::Times(n, _) => {
                    if state.calls >= *n {
                        return deny("G05", format!("This grant permits `{}` {} time(s), and they are used.", action.name.name, n));
                    }
                }
                Cond::Service(list, _) => {
                    let address = &self.bindings[b].address;
                    let listed = matches!(list, Literal::List(items)
                        if items.iter().any(|i| matches!(i, Literal::Str(s) if s == address)));
                    if !listed {
                        return deny("G04", format!("This rule covers only the services {}.", list.show()));
                    }
                }
                Cond::Param { name, path, op, value } => {
                    let mut arg = args.get(&name.name);
                    for seg in path {
                        arg = arg.and_then(|v| v.get(&seg.name));
                    }
                    let holds = match (self.w.cond_type(file, action, name, path).map(|t| t.ty), arg) {
                        (Ok(Ty::Scalar(s)), Some(arg)) => cond_holds(s, *op, value, arg),
                        _ => false,
                    };
                    if !holds {
                        let words = match op {
                            Op::Eq => "be",
                            Op::Le => "be at most",
                            Op::Ge => "be at least",
                            Op::In => "be one of",
                        };
                        let got = arg.map(|v| v.text()).unwrap_or_else(|| "nothing".into());
                        return deny(
                            "G04",
                            format!("This grant requires `{}` to {} {}. The call has {}.", Cond::shown(name, path), words, value.show(), got),
                        );
                    }
                }
            }
        }
        let max = action.effect(EffectKind::Spend).and_then(|e| e.max.as_ref());
        match (limit, max) {
            (None, None) => Ok(None),
            (None, Some(m)) => Ok(Some((m.amount, m.currency.clone()))),
            (Some(l), max) => {
                let remaining = l.amount - state.spent - state.held;
                if remaining <= 0 && l.amount > 0 {
                    return deny("G06", format!("This grant permits spending {} in total, and it is used.", l.text));
                }
                match max {
                    Some(m) if m.currency != l.currency => {
                        deny("G06", format!("The grant's limit is in {} but the action charges in {}.", l.currency, m.currency))
                    }
                    Some(m) => Ok(Some((remaining.min(m.amount), l.currency.clone()))),
                    None => Ok(Some((remaining, l.currency.clone()))),
                }
            }
        }
    }

    fn open(&mut self, call: usize, r: usize, reserve: Option<(i128, String)>) {
        self.rules[r].calls += 1;
        if let Some((amount, _)) = &reserve {
            self.rules[r].held += *amount;
        }
        self.calls[call].reserve = reserve;
        self.calls[call].status = Status::Open;
    }

    fn show_reserve(reserve: &Option<(i128, String)>) -> Option<String> {
        reserve.as_ref().map(|(a, c)| format!("{} {}", show_scaled(*a), c))
    }

    /// The agent asks to call `action` on the service connected as `service`.
    pub fn call(&mut self, service: &str, action: &str, args: &Value, now: i64) -> Decision {
        match self.decide(service, action, args, now) {
            Ok(d) => d,
            Err(d) => Decision::Deny(d),
        }
    }

    fn decide(&mut self, service: &str, action: &str, args: &Value, now: i64) -> Result<Decision, Deny> {
        self.live(now)?;
        let Some(b) = self.bindings.iter().position(|x| x.name == service) else {
            return deny("G02", format!("No service is connected as `{}`.", service));
        };
        let actions = self.w.service_actions(self.bindings[b].file, self.service(b));
        let Some(found) = actions.get(action) else {
            return deny("G02", format!("`{}` has no action called `{}`.", service, action));
        };
        let (file, act) = (found.file, found.action);
        let args = self.fit_args(file, act, args)?;
        let spends = act.effect(EffectKind::Spend).is_some();
        let new_call = |rule, status, args| Call {
            binding: b,
            action: action.to_string(),
            rule,
            status,
            spends,
            reserve: None,
            charged: 0,
            args,
        };

        // The right to undo a result this grant produced. Used once, charges nothing.
        let required: Vec<&Param> = act.params.iter().filter(|p| !p.ty.optional).collect();
        if let [param] = required[..] {
            if let Some(arg) = args.get(&param.name.name) {
                let text = arg.text();
                let right = self.undos.iter().position(|u| {
                    !u.used && u.binding == b && u.action == action && u.value == text && u.until.is_none_or(|t| now <= t)
                });
                if let Some(u) = right {
                    self.undos[u].used = true;
                    let mut call = new_call(None, Status::Open, args);
                    call.spends = false;
                    self.calls.push(call);
                    return Ok(Decision::Allow { call: self.calls.len() as u64, reserve: None, undo: true });
                }
            }
        }

        let mut first_refusal = None;
        for r in 0..self.grant.rules.len() {
            if !self.rule_covers(&self.grant.rules[r], b, action) {
                continue;
            }
            match self.permits(r, b, file, act, &args) {
                Err(d) => {
                    first_refusal.get_or_insert(d);
                }
                Ok(reserve) => {
                    let id = self.calls.len();
                    if self.grant.rules[r].ask {
                        let prompt = self.prompt(service, act, &actions, &args, &reserve);
                        self.calls.push(new_call(Some(r), Status::Pending, args));
                        return Ok(Decision::Ask { call: id as u64 + 1, prompt });
                    }
                    self.calls.push(new_call(Some(r), Status::Pending, args));
                    let shown = Self::show_reserve(&reserve);
                    self.open(id, r, reserve);
                    return Ok(Decision::Allow { call: id as u64 + 1, reserve: shown, undo: false });
                }
            }
        }
        Err(first_refusal.unwrap_or_else(|| Deny {
            code: "G02",
            reason: format!("The grant has no rule for `{}.{}`.", service, action),
        }))
    }

    fn prompt(
        &self,
        service: &str,
        act: &Action,
        siblings: &std::collections::BTreeMap<String, crate::check::ActionIn>,
        args: &Value,
        reserve: &Option<(i128, String)>,
    ) -> String {
        let mut out = format!("{}\n", act.doc.as_deref().unwrap_or("(no description)"));
        out.push_str(&format!("  action: {}.{}\n", service, act.name.name));
        out.push_str(&format!("  with:   {}\n", args.text()));
        out.push_str(&format!("  may:    {}\n", crate::explain::effect_words(act)));
        match Self::show_reserve(reserve) {
            Some(r) => out.push_str(&format!("  charge: at most {}\n", r)),
            None if act.effect(EffectKind::Spend).is_some() => out.push_str("  charge: NO LIMIT\n"),
            None => {}
        }
        if act.is_acting() {
            match &act.undo {
                Some(u) => {
                    let doc = siblings.get(&u.target.name).and_then(|t| t.action.doc.as_deref()).unwrap_or("(no description)");
                    out.push_str(&format!("  undo:   {}\n", doc));
                }
                None => out.push_str("  undo:   CANNOT BE UNDONE\n"),
            }
        }
        out
    }

    fn pending(&self, call: u64, status: Status) -> Result<usize, Deny> {
        let id = (call as usize).wrapping_sub(1);
        match self.calls.get(id) {
            Some(c) if c.status == status => Ok(id),
            _ => deny("G08", format!("There is no call {} waiting for this.", call)),
        }
    }

    /// The person's answer to an `Ask`. Limits are checked again, because time has passed.
    pub fn confirm(&mut self, call: u64, approved: bool, now: i64) -> Decision {
        let id = match self.pending(call, Status::Pending) {
            Ok(id) => id,
            Err(d) => return Decision::Deny(d),
        };
        let refuse = |g: &mut Self, d: Deny| {
            g.calls[id].status = Status::Closed;
            Decision::Deny(d)
        };
        if !approved {
            return refuse(self, Deny { code: "G07", reason: "The person declined this call.".into() });
        }
        if let Err(d) = self.live(now) {
            return refuse(self, d);
        }
        let (b, r) = (self.calls[id].binding, self.calls[id].rule.expect("asked calls have a rule"));
        let actions = self.w.service_actions(self.bindings[b].file, self.service(b));
        let found = &actions[&self.calls[id].action];
        match self.permits(r, b, found.file, found.action, &self.calls[id].args) {
            Err(d) => refuse(self, d),
            Ok(reserve) => {
                let shown = Self::show_reserve(&reserve);
                self.open(id, r, reserve);
                Decision::Allow { call, reserve: shown, undo: false }
            }
        }
    }

    /// The service asks to take money during an open call. All payment goes through here.
    pub fn charge(&mut self, call: u64, amount: &str) -> Result<(), Deny> {
        let id = self.pending(call, Status::Open)?;
        let Some((amount, currency)) = parse_money(amount) else {
            return deny("G06", "A charge must be money written like \"200 ZAR\", and not negative.".into());
        };
        let c = &mut self.calls[id];
        if !c.spends {
            return deny("G06", format!("`{}` does not declare `spend`, so it cannot charge.", c.action));
        }
        if let Some((reserve, reserve_currency)) = &c.reserve {
            if currency != *reserve_currency {
                return deny("G06", format!("This call may charge only in {}.", reserve_currency));
            }
            if c.charged + amount > *reserve {
                return deny(
                    "G06",
                    format!("This call may charge at most {} {} in total.", show_scaled(*reserve), reserve_currency),
                );
            }
        }
        c.charged += amount;
        Ok(())
    }

    fn close(&mut self, id: usize) {
        let c = &mut self.calls[id];
        c.status = Status::Closed;
        if let Some(r) = c.rule {
            if let Some((reserve, _)) = &c.reserve {
                self.rules[r].held -= *reserve;
            }
            self.rules[r].spent += c.charged;
        }
    }

    /// The service's result. It reaches the agent only if it fits the action's result type.
    pub fn result(&mut self, call: u64, value: &Value, now: i64) -> Result<Delivered, Deny> {
        let id = self.pending(call, Status::Open)?;
        self.close(id);
        let b = self.calls[id].binding;
        let actions = self.w.service_actions(self.bindings[b].file, self.service(b));
        let found = &actions[&self.calls[id].action];
        let mut untrusted = Vec::new();
        let Some(result) = &found.action.result else {
            return if value.is_null() {
                Ok(Delivered { value: Value::Null, untrusted })
            } else {
                deny("G08", format!("`{}` declares no result, but the service returned one.", found.action.name.name))
            };
        };
        let ty = self.w.resolve_type(found.file, result);
        let fitted = match self.fit(&ty, value, false, "", &mut untrusted) {
            Ok(v) => v,
            Err(why) => return deny("G08", format!("The service's result does not fit `{}`: {}.", result.show(), why)),
        };
        if let (Some(_), Some(undo)) = (self.calls[id].rule, &found.action.undo) {
            // Keep the result in the shape the undo action takes, so it can be matched later.
            let shaped = actions.get(&undo.target.name).and_then(|t| {
                let param = t.action.params.iter().find(|p| !p.ty.optional)?;
                let pty = self.w.resolve_type(t.file, &param.ty);
                self.fit(&pty, &fitted, false, "", &mut Vec::new()).ok()
            });
            if let Some(shaped) = shaped {
                self.undos.push(UndoRight {
                    binding: b,
                    action: undo.target.name.clone(),
                    value: shaped.text(),
                    until: undo.within.as_ref().map(|d| now + d.minutes as i64 * 60),
                    used: false,
                });
            }
        }
        Ok(Delivered { value: fitted, untrusted })
    }

    /// The service failed. What it already charged still counts.
    pub fn fail(&mut self, call: u64) -> Result<(), Deny> {
        let id = self.pending(call, Status::Open)?;
        self.close(id);
        Ok(())
    }

    /// What has been used so far, as JSON.
    pub fn state(&self, now: i64) -> String {
        let rules: Vec<String> = self
            .grant
            .rules
            .iter()
            .zip(&self.rules)
            .map(|(rule, s)| {
                format!(
                    "{{\"rule\":{},\"calls\":{},\"spent\":{}}}",
                    crate::diag::json_string(&format!("{}.{}", rule.target.show(), rule.action.name)),
                    s.calls,
                    crate::diag::json_string(&show_scaled(s.spent))
                )
            })
            .collect();
        format!(
            "{{\"grant\":{},\"live\":{},\"rules\":[{}]}}",
            crate::diag::json_string(&self.grant.name.name),
            self.live(now).is_ok(),
            rules.join(",")
        )
    }
}
