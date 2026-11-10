//! Syntax tree for a contract file. Follows the grammar in SPEC.md section 4.

use crate::diag::Pos;

#[derive(Debug, Clone)]
pub struct Ident {
    pub name: String,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct File {
    pub path: String,
    pub uses: Vec<Use>,
    pub decls: Vec<Decl>,
}

#[derive(Debug, Clone)]
pub struct Use {
    pub alias: Ident,
    pub address: String,
    pub address_pos: Pos,
}

#[derive(Debug, Clone)]
pub enum Decl {
    Type(TypeDecl),
    Enum(EnumDecl),
    Interface(Interface),
    Service(Service),
    Grant(Grant),
}

impl Decl {
    pub fn name(&self) -> &Ident {
        match self {
            Decl::Type(d) => &d.name,
            Decl::Enum(d) => &d.name,
            Decl::Interface(d) => &d.name,
            Decl::Service(d) => &d.name,
            Decl::Grant(d) => &d.name,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Decl::Type(_) => "type",
            Decl::Enum(_) => "enum",
            Decl::Interface(_) => "interface",
            Decl::Service(_) => "service",
            Decl::Grant(_) => "grant",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub doc: Option<String>,
    pub name: Ident,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub doc: Option<String>,
    pub name: Ident,
    pub ty: TypeRef,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub doc: Option<String>,
    pub name: Ident,
    pub members: Vec<Ident>,
}

#[derive(Debug, Clone)]
pub struct TypeRef {
    pub base: Base,
    pub optional: bool,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub enum Base {
    Scalar(Scalar),
    List(Box<TypeRef>),
    Untrusted(Box<TypeRef>),
    Named(QName),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scalar {
    Bool,
    Int,
    Decimal,
    Text,
    Date,
    DateTime,
    Duration,
    Money,
}

impl Scalar {
    pub fn from_name(name: &str) -> Option<Scalar> {
        Some(match name {
            "Bool" => Scalar::Bool,
            "Int" => Scalar::Int,
            "Decimal" => Scalar::Decimal,
            "Text" => Scalar::Text,
            "Date" => Scalar::Date,
            "DateTime" => Scalar::DateTime,
            "Duration" => Scalar::Duration,
            "Money" => Scalar::Money,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Scalar::Bool => "Bool",
            Scalar::Int => "Int",
            Scalar::Decimal => "Decimal",
            Scalar::Text => "Text",
            Scalar::Date => "Date",
            Scalar::DateTime => "DateTime",
            Scalar::Duration => "Duration",
            Scalar::Money => "Money",
        }
    }
}

/// Names the language defines. They cannot be redeclared (R14).
pub const BUILT_IN_TYPES: &[&str] = &[
    "Bool", "Int", "Decimal", "Text", "Date", "DateTime", "Duration", "Money", "List",
    "Untrusted", "Store", "Http",
];

/// Words that have a meaning in a grant condition, so cannot name a parameter (R14).
pub const CONDITION_WORDS: &[&str] = &["spend", "times", "service", "irreversible"];

#[derive(Debug, Clone)]
pub struct QName {
    pub alias: Option<Ident>,
    pub name: Ident,
}

impl QName {
    pub fn show(&self) -> String {
        match &self.alias {
            Some(a) => format!("{}.{}", a.name, self.name.name),
            None => self.name.name.clone(),
        }
    }

    pub fn pos(&self) -> Pos {
        match &self.alias {
            Some(a) => a.pos,
            None => self.name.pos,
        }
    }
}

impl TypeRef {
    pub fn show(&self) -> String {
        let base = match &self.base {
            Base::Scalar(s) => s.name().to_string(),
            Base::List(t) => format!("List<{}>", t.show()),
            Base::Untrusted(t) => format!("Untrusted<{}>", t.show()),
            Base::Named(q) => q.show(),
        };
        if self.optional {
            format!("{}?", base)
        } else {
            base
        }
    }
}

#[derive(Debug, Clone)]
pub struct Interface {
    pub doc: Option<String>,
    pub name: Ident,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone)]
pub struct Service {
    pub doc: Option<String>,
    pub name: Ident,
    pub implements: Vec<QName>,
    pub uses: Vec<Uses>,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone)]
pub struct Uses {
    pub name: Ident,
    pub resource: Resource,
}

#[derive(Debug, Clone)]
pub enum Resource {
    Store,
    Http(String),
    Interface(QName),
}

#[derive(Debug, Clone)]
pub struct Action {
    pub doc: Option<String>,
    pub name: Ident,
    pub params: Vec<Param>,
    pub result: Option<TypeRef>,
    pub effects: Vec<Effect>,
    pub effects_pos: Pos,
    pub undo: Option<Undo>,
}

impl Action {
    /// True when the action does anything other than read.
    pub fn is_acting(&self) -> bool {
        self.effects.iter().any(|e| e.kind != EffectKind::Read)
    }

    pub fn effect(&self, kind: EffectKind) -> Option<&Effect> {
        self.effects.iter().find(|e| e.kind == kind)
    }
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: Ident,
    pub ty: TypeRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKind {
    Read,
    Write,
    Send,
    Spend,
}

impl EffectKind {
    pub fn name(self) -> &'static str {
        match self {
            EffectKind::Read => "read",
            EffectKind::Write => "write",
            EffectKind::Send => "send",
            EffectKind::Spend => "spend",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Effect {
    pub kind: EffectKind,
    pub max: Option<Money>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct Money {
    /// Amount in millionths of a unit, so comparisons are exact.
    pub amount: i128,
    pub currency: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct DurationLit {
    pub minutes: u64,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Undo {
    pub target: Ident,
    pub within: Option<DurationLit>,
}

#[derive(Debug, Clone)]
pub struct Grant {
    pub doc: Option<String>,
    pub name: Ident,
    pub rules: Vec<Rule>,
    pub expires: String,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub ask: bool,
    pub target: QName,
    pub action: Ident,
    pub conds: Vec<Cond>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub enum Cond {
    Spend(Money, Pos),
    Times(i64, Pos),
    Service(Literal, Pos),
    Irreversible(Pos),
    Param { name: Ident, op: Op, value: Literal },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Le,
    Ge,
    In,
}

#[derive(Debug, Clone)]
pub enum Literal {
    Str(String),
    Int(i64),
    Decimal(String),
    Money(Money),
    Duration(DurationLit),
    Date(String),
    DateTime(String),
    List(Vec<Literal>),
}

impl Literal {
    pub fn show(&self) -> String {
        match self {
            Literal::Str(s) => format!("\"{}\"", s),
            Literal::Int(n) => n.to_string(),
            Literal::Decimal(s) => s.clone(),
            Literal::Money(m) => m.text.clone(),
            Literal::Duration(d) => d.text.clone(),
            Literal::Date(s) | Literal::DateTime(s) => s.clone(),
            Literal::List(items) => {
                let parts: Vec<String> = items.iter().map(|l| l.show()).collect();
                format!("[{}]", parts.join(", "))
            }
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Literal::Str(_) => "Text",
            Literal::Int(_) => "Int",
            Literal::Decimal(_) => "Decimal",
            Literal::Money(_) => "Money",
            Literal::Duration(_) => "Duration",
            Literal::Date(_) => "Date",
            Literal::DateTime(_) => "DateTime",
            Literal::List(_) => "a list",
        }
    }
}
