//! Recursive-descent parser for the grammar in SPEC.md section 4.
//!
//! A syntax error stops the file (R00). A name with the wrong case is reported
//! (R02) and parsing carries on, so one run can report more.

use crate::ast::*;
use crate::diag::{Diag, Pos};
use crate::lexer::{lex, Tok, Token};

type R<T> = Result<T, ()>;

struct Parser<'a> {
    path: &'a str,
    toks: Vec<Token>,
    i: usize,
    diags: Vec<Diag>,
}

pub fn parse(path: &str, src: &str) -> (Option<File>, Vec<Diag>) {
    let toks = match lex(src) {
        Ok(toks) => toks,
        Err((pos, message, fix)) => {
            let diag = Diag { code: "R00", file: path.to_string(), pos, message, fix };
            return (None, vec![diag]);
        }
    };
    let mut p = Parser { path, toks, i: 0, diags: Vec::new() };
    let file = p.file().ok();
    (file, p.diags)
}

fn capitalise(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn lower_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_ascii_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

impl<'a> Parser<'a> {
    fn peek(&self) -> &Tok {
        &self.toks[self.i].tok
    }

    fn peek_at(&self, n: usize) -> &Tok {
        let idx = (self.i + n).min(self.toks.len() - 1);
        &self.toks[idx].tok
    }

    fn pos(&self) -> Pos {
        self.toks[self.i].pos
    }

    fn next(&mut self) -> Token {
        let t = self.toks[self.i].clone();
        if self.i < self.toks.len() - 1 {
            self.i += 1;
        }
        t
    }

    fn push(&mut self, code: &'static str, pos: Pos, message: String, fix: String) {
        self.diags.push(Diag { code, file: self.path.to_string(), pos, message, fix });
    }

    fn fail<T>(&mut self, pos: Pos, message: String, fix: String) -> R<T> {
        self.push("R00", pos, message, fix);
        Err(())
    }

    /// Fails with "expected X, found Y".
    fn expected<T>(&mut self, what: &str, fix: &str) -> R<T> {
        let found = self.peek().describe();
        let pos = self.pos();
        self.fail(pos, format!("Expected {}, found {}.", what, found), fix.to_string())
    }

    fn expect(&mut self, tok: Tok, fix: &str) -> R<Pos> {
        if *self.peek() == tok {
            Ok(self.next().pos)
        } else {
            let what = tok.describe();
            self.expected(&what, fix)
        }
    }

    fn is_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), Tok::Lower(s) if s == kw)
    }

    fn expect_kw(&mut self, kw: &str, fix: &str) -> R<Pos> {
        if self.is_kw(kw) {
            Ok(self.next().pos)
        } else {
            self.expected(&format!("`{}`", kw), fix)
        }
    }

    /// A name that must start with an upper-case letter.
    fn upper(&mut self, what: &str) -> R<Ident> {
        match self.peek().clone() {
            Tok::Upper(name) => Ok(Ident { name, pos: self.next().pos }),
            Tok::Lower(name) => {
                let pos = self.next().pos;
                self.push(
                    "R02",
                    pos,
                    format!("`{}` is {}, so it must start with an upper-case letter.", name, what),
                    format!("Rename it to `{}`.", capitalise(&name)),
                );
                Ok(Ident { name, pos })
            }
            _ => self.expected(what, "Add the missing name."),
        }
    }

    /// A name that must start with a lower-case letter.
    fn lower(&mut self, what: &str) -> R<Ident> {
        match self.peek().clone() {
            Tok::Lower(name) => Ok(Ident { name, pos: self.next().pos }),
            Tok::Upper(name) => {
                let pos = self.next().pos;
                self.push(
                    "R02",
                    pos,
                    format!("`{}` is {}, so it must start with a lower-case letter.", name, what),
                    format!("Rename it to `{}`.", lower_first(&name)),
                );
                Ok(Ident { name, pos })
            }
            _ => self.expected(what, "Add the missing name."),
        }
    }

    fn opt_doc(&mut self) -> Option<String> {
        if let Tok::Str(s) = self.peek().clone() {
            self.next();
            Some(s)
        } else {
            None
        }
    }

    fn file(&mut self) -> R<File> {
        let mut uses = Vec::new();
        while self.is_kw("use") {
            self.next();
            let alias = self.lower("a name for the file being used")?;
            self.expect_kw("from", "Write `use name from \"./file.remit\"`.")?;
            let address_pos = self.pos();
            let address = match self.peek().clone() {
                Tok::Str(s) => {
                    self.next();
                    s
                }
                _ => return self.expected("an address in double quotes", "Write `use name from \"./file.remit\"`."),
            };
            uses.push(Use { alias, address, address_pos });
        }
        let mut decls = Vec::new();
        while *self.peek() != Tok::Eof {
            let doc = self.opt_doc();
            let decl = match self.peek().clone() {
                Tok::Lower(kw) => match kw.as_str() {
                    "type" => Decl::Type(self.type_decl(doc)?),
                    "enum" => Decl::Enum(self.enum_decl(doc)?),
                    "interface" => Decl::Interface(self.interface(doc)?),
                    "service" => Decl::Service(self.service(doc)?),
                    "grant" => Decl::Grant(self.grant(doc)?),
                    "use" => {
                        return self.expected(
                            "a declaration",
                            "Move every `use` line to the top of the file.",
                        )
                    }
                    _ => return self.expected_decl(),
                },
                _ => return self.expected_decl(),
            };
            decls.push(decl);
        }
        Ok(File { path: self.path.to_string(), uses, decls })
    }

    fn expected_decl<T>(&mut self) -> R<T> {
        self.expected(
            "a declaration",
            "Start a declaration with `type`, `enum`, `interface`, `service`, or `grant`.",
        )
    }

    fn type_decl(&mut self, doc: Option<String>) -> R<TypeDecl> {
        self.next();
        let name = self.upper("a type name")?;
        self.expect(Tok::LBrace, "Put the fields in braces: `type Name { field: Text }`.")?;
        let mut fields = Vec::new();
        while *self.peek() != Tok::RBrace {
            let doc = self.opt_doc();
            let name = self.lower("a field name")?;
            self.expect(Tok::Colon, "Write each field as `name: Type`.")?;
            let ty = self.typeref()?;
            if *self.peek() == Tok::Comma {
                return self.expected(
                    "the next field or `}`",
                    "Remove the comma. Fields are separated by spaces or new lines.",
                );
            }
            fields.push(Field { doc, name, ty });
        }
        self.next();
        Ok(TypeDecl { doc, name, fields })
    }

    fn enum_decl(&mut self, doc: Option<String>) -> R<EnumDecl> {
        self.next();
        let name = self.upper("an enum name")?;
        self.expect(Tok::LBrace, "Put the values in braces: `enum Name { a, b }`.")?;
        let mut members = vec![self.lower("an enum value")?];
        while *self.peek() == Tok::Comma {
            self.next();
            members.push(self.lower("an enum value")?);
        }
        self.expect(Tok::RBrace, "Separate enum values with commas and end with `}`.")?;
        Ok(EnumDecl { doc, name, members })
    }

    fn typeref(&mut self) -> R<TypeRef> {
        let pos = self.pos();
        let base = match self.peek().clone() {
            Tok::Upper(name) => match name.as_str() {
                "List" | "Untrusted" => {
                    self.next();
                    let fix = format!("Write `{}<Type>`.", name);
                    self.expect(Tok::LAngle, &fix)?;
                    let inner = Box::new(self.typeref()?);
                    self.expect(Tok::RAngle, &fix)?;
                    if name == "List" {
                        Base::List(inner)
                    } else {
                        Base::Untrusted(inner)
                    }
                }
                _ => {
                    let pos = self.next().pos;
                    match Scalar::from_name(&name) {
                        Some(s) => Base::Scalar(s),
                        None => Base::Named(QName { alias: None, name: Ident { name, pos } }),
                    }
                }
            },
            Tok::Lower(name) => {
                if *self.peek_at(1) == Tok::Dot {
                    let alias = Ident { name, pos: self.next().pos };
                    self.next();
                    let name = self.upper("a type name")?;
                    Base::Named(QName { alias: Some(alias), name })
                } else {
                    let pos = self.next().pos;
                    let fixed = capitalise(&name);
                    self.push(
                        "R02",
                        pos,
                        format!("`{}` is a type name, so it must start with an upper-case letter.", name),
                        format!("Write `{}`.", fixed),
                    );
                    match Scalar::from_name(&fixed) {
                        Some(s) => Base::Scalar(s),
                        None => Base::Named(QName { alias: None, name: Ident { name: fixed, pos } }),
                    }
                }
            }
            _ => return self.expected("a type", "Write a type such as `Text`, `List<Text>`, or a declared type name."),
        };
        let optional = if *self.peek() == Tok::Question {
            self.next();
            true
        } else {
            false
        };
        Ok(TypeRef { base, optional, pos })
    }

    /// `[alias.]Name`
    fn qname(&mut self, what: &str) -> R<QName> {
        if let Tok::Lower(name) = self.peek().clone() {
            if *self.peek_at(1) == Tok::Dot {
                let alias = Ident { name, pos: self.next().pos };
                self.next();
                let name = self.upper(what)?;
                return Ok(QName { alias: Some(alias), name });
            }
        }
        let name = self.upper(what)?;
        Ok(QName { alias: None, name })
    }

    fn interface(&mut self, doc: Option<String>) -> R<Interface> {
        self.next();
        let name = self.upper("an interface name")?;
        self.expect(Tok::LBrace, "Put the actions in braces.")?;
        let mut actions = Vec::new();
        while *self.peek() != Tok::RBrace {
            let doc = self.opt_doc();
            actions.push(self.action(doc)?);
        }
        self.next();
        Ok(Interface { doc, name, actions })
    }

    fn service(&mut self, doc: Option<String>) -> R<Service> {
        self.next();
        let name = self.upper("a service name")?;
        let mut implements = Vec::new();
        if self.is_kw("implements") {
            self.next();
            implements.push(self.qname("an interface name")?);
            while *self.peek() == Tok::Comma {
                self.next();
                implements.push(self.qname("an interface name")?);
            }
        }
        self.expect(Tok::LBrace, "Put the service body in braces.")?;
        let mut uses = Vec::new();
        let mut actions = Vec::new();
        while *self.peek() != Tok::RBrace {
            if self.is_kw("uses") {
                self.next();
                let name = self.lower("a name for the resource")?;
                self.expect(Tok::Colon, "Write `uses name: Store`.")?;
                let resource = self.resource()?;
                uses.push(Uses { name, resource });
            } else {
                let doc = self.opt_doc();
                actions.push(self.action(doc)?);
            }
        }
        self.next();
        Ok(Service { doc, name, implements, uses, actions })
    }

    fn resource(&mut self) -> R<Resource> {
        match self.peek().clone() {
            Tok::Upper(name) if name == "Store" => {
                self.next();
                Ok(Resource::Store)
            }
            Tok::Upper(name) if name == "Http" => {
                self.next();
                let fix = "Write `Http(\"https://host\")`.";
                self.expect(Tok::LParen, fix)?;
                let origin = match self.peek().clone() {
                    Tok::Str(s) => {
                        self.next();
                        s
                    }
                    _ => return self.expected("an origin in double quotes", fix),
                };
                self.expect(Tok::RParen, fix)?;
                Ok(Resource::Http(origin))
            }
            _ => Ok(Resource::Interface(self.qname("`Store`, `Http(...)`, or an interface name")?)),
        }
    }

    fn action(&mut self, doc: Option<String>) -> R<Action> {
        self.expect_kw("action", "Declare an action as `action name(param: Type) -> Type`.")?;
        let name = self.lower("an action name")?;
        self.expect(Tok::LParen, "Put the parameters in parentheses, even when there are none.")?;
        let mut params = Vec::new();
        while *self.peek() != Tok::RParen {
            let name = self.lower("a parameter name")?;
            self.expect(Tok::Colon, "Write each parameter as `name: Type`.")?;
            let ty = self.typeref()?;
            params.push(Param { name, ty });
            if *self.peek() == Tok::Comma {
                self.next();
            } else if *self.peek() != Tok::RParen {
                return self.expected("`,` or `)`", "Separate parameters with commas.");
            }
        }
        self.next();
        let result = if *self.peek() == Tok::Arrow {
            self.next();
            Some(self.typeref()?)
        } else {
            None
        };
        let effects_pos = self.pos();
        self.expect_kw("effects", "Every action declares its effects, for example `effects: read`.")?;
        self.expect(Tok::Colon, "Write `effects: read`.")?;
        let mut effects = Vec::new();
        loop {
            let pos = self.pos();
            let kind = match self.peek().clone() {
                Tok::Lower(word) => match word.as_str() {
                    "read" => EffectKind::Read,
                    "write" => EffectKind::Write,
                    "send" => EffectKind::Send,
                    "spend" => EffectKind::Spend,
                    _ => return self.expected("an effect", "The effects are `read`, `write`, `send`, and `spend`."),
                },
                _ => return self.expected("an effect", "The effects are `read`, `write`, `send`, and `spend`."),
            };
            self.next();
            let mut max = None;
            if *self.peek() == Tok::LParen {
                let fix = "A ceiling is written `spend(max: 500 ZAR)`. Other effects take nothing.";
                if kind != EffectKind::Spend {
                    return self.expected("`,` or the end of the effects", fix);
                }
                self.next();
                self.expect_kw("max", fix)?;
                self.expect(Tok::Colon, fix)?;
                max = Some(self.money()?);
                self.expect(Tok::RParen, fix)?;
            }
            effects.push(Effect { kind, max, pos });
            if *self.peek() == Tok::Comma {
                self.next();
            } else {
                break;
            }
        }
        let mut undo = None;
        if self.is_kw("undo") && *self.peek_at(1) == Tok::Colon {
            self.next();
            self.next();
            let target = self.lower("the name of the action that reverses this one")?;
            let mut within = None;
            if self.is_kw("within") {
                self.next();
                within = match self.peek().clone() {
                    Tok::Duration(minutes, text) => {
                        self.next();
                        Some(DurationLit { minutes, text })
                    }
                    _ => return self.expected("a duration", "Write a period such as `within 24h`."),
                };
            }
            undo = Some(Undo { target, within });
        }
        Ok(Action { doc, name, params, result, effects, effects_pos, undo })
    }

    fn is_currency(tok: &Tok) -> bool {
        matches!(tok, Tok::Upper(c) if c.len() == 3 && c.chars().all(|ch| ch.is_ascii_uppercase()))
    }

    fn money(&mut self) -> R<Money> {
        let pos = self.pos();
        let number = match self.peek().clone() {
            Tok::Int(n) => n.to_string(),
            Tok::Decimal(s) => s,
            _ => return self.expected("an amount of money", "Write an amount and a currency code, for example `500 ZAR`."),
        };
        self.next();
        let currency = match self.peek().clone() {
            tok if Self::is_currency(&tok) => match self.next().tok {
                Tok::Upper(c) => c,
                _ => unreachable!(),
            },
            _ => return self.expected("a three-letter currency code", "Write an amount and a currency code, for example `500 ZAR`."),
        };
        self.make_money(pos, number, currency)
    }

    fn make_money(&mut self, pos: Pos, number: String, currency: String) -> R<Money> {
        let (whole, frac) = match number.split_once('.') {
            Some((w, f)) => (w, f),
            None => (number.as_str(), ""),
        };
        if frac.len() > 6 {
            return self.fail(
                pos,
                "An amount of money can have at most six decimal places.".into(),
                "Use fewer decimal places.".into(),
            );
        }
        let padded = format!("{}{:0<6}", whole, frac);
        match padded.parse::<i128>() {
            Ok(amount) => Ok(Money { amount, currency: currency.clone(), text: format!("{} {}", number, currency) }),
            Err(_) => self.fail(pos, "This amount is too large.".into(), "Use a smaller amount.".into()),
        }
    }

    fn literal(&mut self) -> R<Literal> {
        let pos = self.pos();
        let tok = self.peek().clone();
        match tok {
            Tok::Str(s) => {
                self.next();
                Ok(Literal::Str(s))
            }
            Tok::Int(_) | Tok::Decimal(_) => {
                if Self::is_currency(self.peek_at(1)) {
                    return Ok(Literal::Money(self.money()?));
                }
                self.next();
                match tok {
                    Tok::Int(n) => Ok(Literal::Int(n)),
                    Tok::Decimal(s) => Ok(Literal::Decimal(s)),
                    _ => unreachable!(),
                }
            }
            Tok::Duration(minutes, text) => {
                self.next();
                Ok(Literal::Duration(DurationLit { minutes, text }))
            }
            Tok::Date(s) => {
                self.next();
                Ok(Literal::Date(s))
            }
            Tok::DateTime(s) => {
                self.next();
                Ok(Literal::DateTime(s))
            }
            Tok::LBracket => {
                self.next();
                let mut items = Vec::new();
                while *self.peek() != Tok::RBracket {
                    items.push(self.literal()?);
                    if *self.peek() == Tok::Comma {
                        self.next();
                    } else if *self.peek() != Tok::RBracket {
                        return self.expected("`,` or `]`", "Separate list values with commas.");
                    }
                }
                self.next();
                Ok(Literal::List(items))
            }
            _ => {
                let _ = pos;
                self.expected("a value", "Write a value such as \"text\", 4, 500 ZAR, 24h, or [\"a\", \"b\"].")
            }
        }
    }

    fn grant(&mut self, doc: Option<String>) -> R<Grant> {
        self.next();
        let name = self.upper("a grant name")?;
        self.expect(Tok::LBrace, "Put the rules in braces.")?;
        let mut rules = Vec::new();
        loop {
            if self.is_kw("allow") || self.is_kw("ask") {
                let pos = self.pos();
                let ask = self.is_kw("ask");
                self.next();
                let target = self.qname("an interface or service name")?;
                self.expect(Tok::Dot, "Name the action as `Interface.action`.")?;
                let action = self.lower("an action name")?;
                let mut conds = Vec::new();
                if self.is_kw("where") {
                    self.next();
                    conds.push(self.cond()?);
                    while *self.peek() == Tok::Comma {
                        self.next();
                        conds.push(self.cond()?);
                    }
                }
                rules.push(Rule { ask, target, action, conds, pos });
            } else if self.is_kw("expires") {
                self.next();
                let expires = match self.peek().clone() {
                    Tok::DateTime(s) => {
                        self.next();
                        s
                    }
                    _ => {
                        return self.expected(
                            "a date and time",
                            "Write when the grant ends, for example `expires 2026-11-13T23:59+02:00`.",
                        )
                    }
                };
                self.expect(Tok::RBrace, "`expires` is the last line of a grant. Move the rules above it.")?;
                return Ok(Grant { doc, name, rules, expires });
            } else {
                return self.expected(
                    "`allow`, `ask`, or `expires`",
                    "Each rule starts with `allow` or `ask`. Every grant ends with an `expires` line.",
                );
            }
        }
    }

    fn cond(&mut self) -> R<Cond> {
        let pos = self.pos();
        let name = match self.peek().clone() {
            Tok::Lower(name) => name,
            _ => return self.expected("a condition", "Write a condition such as `spend <= 500 ZAR` or `times <= 1`."),
        };
        match name.as_str() {
            "irreversible" => {
                self.next();
                Ok(Cond::Irreversible(pos))
            }
            "spend" => {
                self.next();
                self.expect(Tok::Le, "Write a spending limit as `spend <= 500 ZAR`.")?;
                Ok(Cond::Spend(self.money()?, pos))
            }
            "times" => {
                self.next();
                self.expect(Tok::Le, "Write a call limit as `times <= 1`.")?;
                match self.peek().clone() {
                    Tok::Int(n) => {
                        self.next();
                        Ok(Cond::Times(n, pos))
                    }
                    _ => self.expected("a whole number", "Write a call limit as `times <= 1`."),
                }
            }
            "service" => {
                self.next();
                self.expect_kw("in", "Write `service in [\"host/path\"]`.")?;
                Ok(Cond::Service(self.literal()?, pos))
            }
            _ => {
                let name = Ident { name, pos: self.next().pos };
                let op = match self.peek().clone() {
                    Tok::EqEq => Op::Eq,
                    Tok::Le => Op::Le,
                    Tok::Ge => Op::Ge,
                    Tok::Lower(word) if word == "in" => Op::In,
                    _ => return self.expected("`==`, `<=`, `>=`, or `in`", "Compare a parameter with a value, for example `party <= 4`."),
                };
                self.next();
                let value = self.literal()?;
                Ok(Cond::Param { name, op, value })
            }
        }
    }
}
