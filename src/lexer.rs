//! Turns source text into tokens. Follows SPEC.md section 3.

use crate::diag::Pos;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Upper(String),
    Lower(String),
    Str(String),
    Int(i64),
    Decimal(String),
    /// Minutes, and the text as written.
    Duration(u64, String),
    Date(String),
    DateTime(String),
    LBrace,
    RBrace,
    LParen,
    RParen,
    LAngle,
    RAngle,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Dot,
    Question,
    Arrow,
    EqEq,
    Le,
    Ge,
    Eof,
}

impl Tok {
    pub fn describe(&self) -> String {
        match self {
            Tok::Upper(s) | Tok::Lower(s) => format!("`{}`", s),
            Tok::Str(_) => "a string".into(),
            Tok::Int(n) => format!("`{}`", n),
            Tok::Decimal(s) | Tok::Date(s) | Tok::DateTime(s) => format!("`{}`", s),
            Tok::Duration(_, s) => format!("`{}`", s),
            Tok::LBrace => "`{`".into(),
            Tok::RBrace => "`}`".into(),
            Tok::LParen => "`(`".into(),
            Tok::RParen => "`)`".into(),
            Tok::LAngle => "`<`".into(),
            Tok::RAngle => "`>`".into(),
            Tok::LBracket => "`[`".into(),
            Tok::RBracket => "`]`".into(),
            Tok::Comma => "`,`".into(),
            Tok::Colon => "`:`".into(),
            Tok::Dot => "`.`".into(),
            Tok::Question => "`?`".into(),
            Tok::Arrow => "`->`".into(),
            Tok::EqEq => "`==`".into(),
            Tok::Le => "`<=`".into(),
            Tok::Ge => "`>=`".into(),
            Tok::Eof => "the end of the file".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub pos: Pos,
}

/// A lexing failure: where, what went wrong, and how to fix it.
pub type LexError = (Pos, String, String);

fn is_digit(c: &[char], i: usize) -> bool {
    i < c.len() && c[i].is_ascii_digit()
}

fn is_char(c: &[char], i: usize, ch: char) -> bool {
    i < c.len() && c[i] == ch
}

fn two_digits(c: &[char], i: usize) -> Option<u32> {
    if is_digit(c, i) && is_digit(c, i + 1) {
        Some(c[i].to_digit(10).unwrap() * 10 + c[i + 1].to_digit(10).unwrap())
    } else {
        None
    }
}

/// Tries to read `dddd-dd-dd` and an optional time with offset, starting at `i`.
/// Returns the token and the index after it, or None if this is not a date.
fn lex_date(c: &[char], i: usize, pos: Pos) -> Option<Result<(Tok, usize), LexError>> {
    let shape = (0..4).all(|k| is_digit(c, i + k))
        && is_char(c, i + 4, '-')
        && is_char(c, i + 7, '-')
        && two_digits(c, i + 5).is_some()
        && two_digits(c, i + 8).is_some();
    if !shape {
        return None;
    }
    let month = two_digits(c, i + 5).unwrap();
    let day = two_digits(c, i + 8).unwrap();
    let year: u32 = c[i..i + 4].iter().collect::<String>().parse().unwrap_or(0);
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let last = match month {
        4 | 6 | 9 | 11 => 30,
        2 => 28 + leap as u32,
        _ => 31,
    };
    if !(1..=12).contains(&month) || !(1..=last).contains(&day) {
        return Some(Err((
            pos,
            "This date has a month or day out of range.".into(),
            "Write dates as year-month-day, for example 2026-11-13.".into(),
        )));
    }
    let mut j = i + 10;
    if !is_char(c, j, 'T') {
        return Some(Ok((Tok::Date(c[i..j].iter().collect()), j)));
    }
    let bad_time = || {
        Err((
            pos,
            "This date and time is not complete.".into(),
            "Write it with a time and an offset, for example 2026-11-13T23:59+02:00.".into(),
        ))
    };
    j += 1;
    let (hour, minute) = match (two_digits(c, j), two_digits(c, j + 3)) {
        (Some(h), Some(m)) if is_char(c, j + 2, ':') => (h, m),
        _ => return Some(bad_time()),
    };
    if hour > 23 || minute > 59 {
        return Some(bad_time());
    }
    j += 5;
    if is_char(c, j, ':') {
        match two_digits(c, j + 1) {
            Some(s) if s <= 59 => j += 3,
            _ => return Some(bad_time()),
        }
    }
    if is_char(c, j, 'Z') {
        j += 1;
    } else if is_char(c, j, '+') || is_char(c, j, '-') {
        match (two_digits(c, j + 1), two_digits(c, j + 4)) {
            (Some(h), Some(m)) if is_char(c, j + 3, ':') && h <= 23 && m <= 59 => j += 6,
            _ => return Some(bad_time()),
        }
    } else {
        return Some(bad_time());
    }
    Some(Ok((Tok::DateTime(c[i..j].iter().collect()), j)))
}

fn lex_number(c: &[char], i: usize, pos: Pos) -> Result<(Tok, usize), LexError> {
    if let Some(result) = lex_date(c, i, pos) {
        return result;
    }
    let mut j = i;
    while is_digit(c, j) {
        j += 1;
    }
    let digits: String = c[i..j].iter().collect();
    if is_char(c, j, '.') && is_digit(c, j + 1) {
        j += 1;
        while is_digit(c, j) {
            j += 1;
        }
        return Ok((Tok::Decimal(c[i..j].iter().collect()), j));
    }
    let too_big = || {
        (
            pos,
            "This number is too large.".to_string(),
            "Use a smaller number.".to_string(),
        )
    };
    if j < c.len() && matches!(c[j], 'm' | 'h' | 'd') {
        let after_is_name = j + 1 < c.len() && (c[j + 1].is_alphanumeric() || c[j + 1] == '_');
        if !after_is_name {
            let n: u64 = digits.parse().map_err(|_| too_big())?;
            let per = match c[j] {
                'm' => 1,
                'h' => 60,
                _ => 60 * 24,
            };
            let minutes = n.checked_mul(per).ok_or_else(too_big)?;
            return Ok((Tok::Duration(minutes, c[i..j + 1].iter().collect()), j + 1));
        }
    }
    if j < c.len() && (c[j].is_alphabetic() || c[j] == '_') {
        return Err((
            pos,
            "A number runs straight into a name.".into(),
            "Put a space after the number. Durations end in m, h, or d, for example 24h.".into(),
        ));
    }
    let n: i64 = digits.parse().map_err(|_| too_big())?;
    Ok((Tok::Int(n), j))
}

fn lex_string(c: &[char], i: usize, pos: Pos) -> Result<(Tok, usize), LexError> {
    let mut j = i + 1;
    let mut text = String::new();
    loop {
        if j >= c.len() || c[j] == '\n' {
            return Err((
                pos,
                "This string is not closed.".into(),
                "End the string with a double quote on the same line.".into(),
            ));
        }
        match c[j] {
            '"' => return Ok((Tok::Str(text), j + 1)),
            '\\' => {
                if is_char(c, j + 1, '"') || is_char(c, j + 1, '\\') {
                    text.push(c[j + 1]);
                    j += 2;
                } else {
                    return Err((
                        Pos { line: pos.line, col: pos.col + (j - i) as u32 },
                        "This escape is not allowed.".into(),
                        "Only \\\" and \\\\ can follow a backslash in a string.".into(),
                    ));
                }
            }
            ch if crate::diag::hidden(ch) => {
                return Err((
                    Pos { line: pos.line, col: pos.col + (j - i) as u32 },
                    "This string holds a character that cannot be seen or that changes how text is shown.".into(),
                    "Remove it. A string may hold only visible characters and spaces.".into(),
                ));
            }
            ch => {
                text.push(ch);
                j += 1;
            }
        }
    }
}

pub fn lex(src: &str) -> Result<Vec<Token>, LexError> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line, mut col) = (0usize, 1u32, 1u32);
    while i < c.len() {
        let ch = c[i];
        if ch == '\n' {
            i += 1;
            line += 1;
            col = 1;
            continue;
        }
        if ch.is_whitespace() {
            i += 1;
            col += 1;
            continue;
        }
        if ch == '/' && is_char(&c, i + 1, '/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        let pos = Pos { line, col };
        let start = i;
        let tok = if ch.is_ascii_digit() {
            let (tok, end) = lex_number(&c, i, pos)?;
            i = end;
            tok
        } else if ch == '"' {
            let (tok, end) = lex_string(&c, i, pos)?;
            i = end;
            tok
        } else if ch.is_ascii_alphabetic() {
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            let word: String = c[start..i].iter().collect();
            if ch.is_ascii_uppercase() {
                Tok::Upper(word)
            } else {
                Tok::Lower(word)
            }
        } else {
            let two = |second: char| is_char(&c, i + 1, second);
            let (tok, len) = match ch {
                '{' => (Tok::LBrace, 1),
                '}' => (Tok::RBrace, 1),
                '(' => (Tok::LParen, 1),
                ')' => (Tok::RParen, 1),
                '[' => (Tok::LBracket, 1),
                ']' => (Tok::RBracket, 1),
                ',' => (Tok::Comma, 1),
                ':' => (Tok::Colon, 1),
                '.' => (Tok::Dot, 1),
                '?' => (Tok::Question, 1),
                '<' if two('=') => (Tok::Le, 2),
                '>' if two('=') => (Tok::Ge, 2),
                '=' if two('=') => (Tok::EqEq, 2),
                '-' if two('>') => (Tok::Arrow, 2),
                '<' => (Tok::LAngle, 1),
                '>' => (Tok::RAngle, 1),
                other => {
                    return Err((
                        pos,
                        format!("The character `{}` is not part of the language.", other),
                        "Remove it. Comments start with //.".into(),
                    ))
                }
            };
            i += len;
            tok
        };
        col += (i - start) as u32;
        out.push(Token { tok, pos });
    }
    out.push(Token { tok: Tok::Eof, pos: Pos { line, col } });
    Ok(out)
}
