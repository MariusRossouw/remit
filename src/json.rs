//! A small JSON reader and writer, so the gate needs no outside code.
//!
//! Stricter than most: an object may not repeat a key, and nesting is limited.
//! Two readers that disagree about a repeated key are a classic way past a guard.

use crate::diag::json_string;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    /// The number exactly as written, so no precision is lost.
    Num(String),
    Str(String),
    List(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

const MAX_DEPTH: u32 = 64;

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Num(n) => n.parse().ok(),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn write(&self, out: &mut String) {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Num(n) => out.push_str(n),
            Value::Str(s) => out.push_str(&json_string(s)),
            Value::List(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Value::Obj(fields) => {
                out.push('{');
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&json_string(k));
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }

    pub fn text(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }
}

pub fn parse(src: &str) -> Result<Value, String> {
    let mut p = Reader { c: src.as_bytes(), i: 0 };
    p.space();
    let v = p.value(0)?;
    p.space();
    if p.i != p.c.len() {
        return Err(format!("unexpected text at byte {}", p.i));
    }
    Ok(v)
}

struct Reader<'a> {
    c: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn space(&mut self) {
        while self.i < self.c.len() && matches!(self.c[self.i], b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }

    fn eat(&mut self, word: &str) -> bool {
        if self.c[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self, depth: u32) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err("nested too deeply".into());
        }
        match self.c.get(self.i) {
            None => Err("unexpected end".into()),
            Some(b'n') if self.eat("null") => Ok(Value::Null),
            Some(b't') if self.eat("true") => Ok(Value::Bool(true)),
            Some(b'f') if self.eat("false") => Ok(Value::Bool(false)),
            Some(b'"') => Ok(Value::Str(self.string()?)),
            Some(b'[') => {
                self.i += 1;
                let mut items = Vec::new();
                self.space();
                if self.c.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(Value::List(items));
                }
                loop {
                    self.space();
                    items.push(self.value(depth + 1)?);
                    self.space();
                    match self.c.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Value::List(items));
                        }
                        _ => return Err(format!("expected `,` or `]` at byte {}", self.i)),
                    }
                }
            }
            Some(b'{') => {
                self.i += 1;
                let mut fields: Vec<(String, Value)> = Vec::new();
                self.space();
                if self.c.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(Value::Obj(fields));
                }
                loop {
                    self.space();
                    if self.c.get(self.i) != Some(&b'"') {
                        return Err(format!("expected a key at byte {}", self.i));
                    }
                    let key = self.string()?;
                    if fields.iter().any(|(k, _)| *k == key) {
                        return Err(format!("the key \"{}\" appears twice", key));
                    }
                    self.space();
                    if self.c.get(self.i) != Some(&b':') {
                        return Err(format!("expected `:` at byte {}", self.i));
                    }
                    self.i += 1;
                    self.space();
                    let v = self.value(depth + 1)?;
                    fields.push((key, v));
                    self.space();
                    match self.c.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Value::Obj(fields));
                        }
                        _ => return Err(format!("expected `,` or `}}` at byte {}", self.i)),
                    }
                }
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(format!("unexpected character at byte {}", self.i)),
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.i;
        if self.c[self.i] == b'-' {
            self.i += 1;
        }
        let digits = |r: &mut Self| {
            let from = r.i;
            while r.i < r.c.len() && r.c[r.i].is_ascii_digit() {
                r.i += 1;
            }
            r.i > from
        };
        if !digits(self) {
            return Err(format!("bad number at byte {}", start));
        }
        if self.c.get(self.i) == Some(&b'.') {
            self.i += 1;
            if !digits(self) {
                return Err(format!("bad number at byte {}", start));
            }
        }
        if matches!(self.c.get(self.i), Some(b'e' | b'E')) {
            return Err("numbers with an exponent are not accepted".into());
        }
        Ok(Value::Num(String::from_utf8_lossy(&self.c[start..self.i]).to_string()))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let s = self.c.get(self.i..self.i + 4).ok_or("bad \\u escape")?;
        let s = std::str::from_utf8(s).map_err(|_| "bad \\u escape")?;
        let n = u32::from_str_radix(s, 16).map_err(|_| "bad \\u escape")?;
        self.i += 4;
        Ok(n)
    }

    fn string(&mut self) -> Result<String, String> {
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let Some(&b) = self.c.get(self.i) else {
                return Err("a string does not end".into());
            };
            self.i += 1;
            match b {
                b'"' => break,
                b'\\' => {
                    let Some(&e) = self.c.get(self.i) else {
                        return Err("a string does not end".into());
                    };
                    self.i += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let mut n = self.hex4()?;
                            if (0xD800..0xDC00).contains(&n) {
                                if !self.eat("\\u") {
                                    return Err("bad \\u escape".into());
                                }
                                let low = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&low) {
                                    return Err("bad \\u escape".into());
                                }
                                n = 0x10000 + ((n - 0xD800) << 10) + (low - 0xDC00);
                            }
                            char::from_u32(n).ok_or("bad \\u escape")?
                        }
                        _ => return Err("bad escape in a string".into()),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                b if b < 0x20 => return Err("a control character inside a string".into()),
                b => out.push(b),
            }
        }
        String::from_utf8(out).map_err(|_| "a string is not valid UTF-8".into())
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text())
    }
}
