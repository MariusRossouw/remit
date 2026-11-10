//! Loads contract files and applies the rules in SPEC.md section 13.
//!
//! Not implemented yet: remote addresses and the lockfile (R12).

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::*;
use crate::diag::{Diag, Pos};
use crate::parser::parse;

/// Every loaded file, and what the checker found.
pub struct World {
    pub files: Vec<File>,
    /// For each file: alias -> index of the file it names.
    pub aliases: Vec<BTreeMap<String, usize>>,
    /// For each file: aliases whose file failed to load. Used to avoid repeat errors.
    broken: Vec<BTreeSet<String>>,
    paths: BTreeMap<String, usize>,
    /// Indexes of the files named on the command line.
    pub entries: Vec<usize>,
    pub diags: Vec<Diag>,
}

/// Joins a `./` or `../` address onto the folder of the file that uses it.
pub fn resolve(from: &str, address: &str) -> String {
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for seg in address.split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                if matches!(parts.last(), Some(&last) if last != ".." && !last.is_empty()) {
                    parts.pop();
                } else {
                    parts.push("..");
                }
            }
            seg => parts.push(seg),
        }
    }
    parts.join("/")
}

/// Loads the given files and everything they use, then checks all of it.
/// `read` returns the text of a file, so the checker itself never touches a disk.
pub fn load(entries: &[String], read: &mut dyn FnMut(&str) -> Result<String, String>) -> World {
    let mut w = World {
        files: Vec::new(),
        aliases: Vec::new(),
        broken: Vec::new(),
        paths: BTreeMap::new(),
        entries: Vec::new(),
        diags: Vec::new(),
    };
    for entry in entries {
        let mut stack = Vec::new();
        match load_file(&mut w, entry, read, &mut stack) {
            Loaded::Ok(idx) => {
                if !w.entries.contains(&idx) {
                    w.entries.push(idx);
                }
            }
            Loaded::Unreadable(why) => w.diags.push(Diag {
                code: "R01",
                file: entry.clone(),
                pos: Pos { line: 1, col: 1 },
                message: format!("The file cannot be read: {}.", why),
                fix: "Check the path.".into(),
            }),
            Loaded::Broken | Loaded::Cycle => {}
        }
    }
    let mut ctx = Ctx { w: &w, out: Vec::new() };
    for f in 0..w.files.len() {
        ctx.check_file(f);
    }
    let out = ctx.out;
    w.diags.extend(out);
    w.diags.sort_by(|a, b| (&a.file, a.pos, a.code).cmp(&(&b.file, b.pos, b.code)));
    w
}

enum Loaded {
    Ok(usize),
    Unreadable(String),
    /// The file was read but did not parse. Already reported.
    Broken,
    Cycle,
}

fn load_file(
    w: &mut World,
    path: &str,
    read: &mut dyn FnMut(&str) -> Result<String, String>,
    stack: &mut Vec<String>,
) -> Loaded {
    if stack.iter().any(|p| p == path) {
        return Loaded::Cycle;
    }
    if let Some(&idx) = w.paths.get(path) {
        return Loaded::Ok(idx);
    }
    let src = match read(path) {
        Ok(src) => src,
        Err(why) => return Loaded::Unreadable(why),
    };
    let (file, diags) = parse(path, &src);
    w.diags.extend(diags);
    let file = match file {
        Some(file) => file,
        None => return Loaded::Broken,
    };
    let uses = file.uses.clone();
    let idx = w.files.len();
    w.files.push(file);
    w.aliases.push(BTreeMap::new());
    w.broken.push(BTreeSet::new());
    w.paths.insert(path.to_string(), idx);
    stack.push(path.to_string());
    for u in &uses {
        let problem = |w: &mut World, code: &'static str, pos: Pos, message: String, fix: String| {
            w.broken[idx].insert(u.alias.name.clone());
            w.diags.push(Diag { code, file: path.to_string(), pos, message, fix });
        };
        if w.aliases[idx].contains_key(&u.alias.name) || w.broken[idx].contains(&u.alias.name) {
            let message = format!("The name `{}` is used for two files.", u.alias.name);
            w.diags.push(Diag {
                code: "R01",
                file: path.to_string(),
                pos: u.alias.pos,
                message,
                fix: "Give each `use` line a different name.".into(),
            });
            continue;
        }
        if !(u.address.starts_with("./") || u.address.starts_with("../")) {
            problem(
                w,
                "T01",
                u.address_pos,
                format!("The address \"{}\" is not a local file. Fetching from a host is not supported yet.", u.address),
                "Use a relative path that starts with `./`.".into(),
            );
            continue;
        }
        let target = resolve(path, &u.address);
        match load_file(w, &target, read, stack) {
            Loaded::Ok(t) => {
                w.aliases[idx].insert(u.alias.name.clone(), t);
            }
            Loaded::Unreadable(why) => problem(
                w,
                "R01",
                u.address_pos,
                format!("The file \"{}\" cannot be read: {}.", u.address, why),
                "Check the path. It is relative to the file that contains this line.".into(),
            ),
            Loaded::Broken => {
                w.broken[idx].insert(u.alias.name.clone());
            }
            Loaded::Cycle => problem(
                w,
                "R13",
                u.address_pos,
                format!("\"{}\" uses this file, directly or through other files.", u.address),
                "Move the shared declarations into a third file that both can use.".into(),
            ),
        }
    }
    stack.pop();
    Loaded::Ok(idx)
}

// ---------------------------------------------------------------------------
// Resolved types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Scalar(Scalar),
    List(Box<RTy>),
    Record(usize, String),
    Enum(usize, String),
    /// The name did not resolve. Reported once elsewhere, and compatible with anything.
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RTy {
    pub ty: Ty,
    pub optional: bool,
    pub untrusted: bool,
}

enum Why {
    Untrusted,
    Mismatch,
}

/// An action together with the file whose names its types refer to.
#[derive(Clone)]
pub struct ActionIn<'a> {
    pub file: usize,
    pub action: &'a Action,
    /// The interface it was inherited from, if the service did not restate it.
    pub inherited_from: Option<String>,
}

fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut prev = row[0];
        row[0] = i;
        for j in 1..=b.len() {
            let cur = row[j];
            let cost = if a[i - 1].eq_ignore_ascii_case(&b[j - 1]) { 0 } else { 1 };
            row[j] = (prev + cost).min(row[j] + 1).min(row[j - 1] + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// "Did you mean" for a name that did not resolve.
fn suggest<'s>(name: &str, options: impl Iterator<Item = &'s str>, otherwise: &str) -> String {
    let best = options
        .map(|o| (distance(name, o), o))
        .filter(|(d, _)| *d <= 2)
        .min_by_key(|(d, _)| *d);
    match best {
        Some((_, o)) => format!("Did you mean `{}`?", o),
        None => otherwise.to_string(),
    }
}

impl World {
    pub fn find_decl(&self, f: usize, name: &str) -> Option<&Decl> {
        self.files[f].decls.iter().find(|d| d.name().name == name)
    }

    /// The file a qualified name points into, without reporting anything.
    pub fn file_of(&self, f: usize, alias: &Option<Ident>) -> Option<usize> {
        match alias {
            None => Some(f),
            Some(a) => self.aliases[f].get(&a.name).copied(),
        }
    }

    pub fn find_interface(&self, f: usize, q: &QName) -> Option<(usize, &Interface)> {
        let tf = self.file_of(f, &q.alias)?;
        match self.find_decl(tf, &q.name.name) {
            Some(Decl::Interface(i)) => Some((tf, i)),
            _ => None,
        }
    }

    /// Every action a service has: its own, and those it inherits unchanged.
    pub fn service_actions<'a>(&'a self, f: usize, s: &'a Service) -> BTreeMap<String, ActionIn<'a>> {
        let mut map = BTreeMap::new();
        for q in &s.implements {
            if let Some((fi, iface)) = self.find_interface(f, q) {
                for a in &iface.actions {
                    map.entry(a.name.name.clone()).or_insert(ActionIn {
                        file: fi,
                        action: a,
                        inherited_from: Some(q.show()),
                    });
                }
            }
        }
        for a in &s.actions {
            map.insert(a.name.name.clone(), ActionIn { file: f, action: a, inherited_from: None });
        }
        map
    }

    /// The actions a grant rule can name on `target`, which is an interface or a service.
    pub fn target_actions<'a>(&'a self, f: usize, target: &QName) -> Option<BTreeMap<String, ActionIn<'a>>> {
        let tf = self.file_of(f, &target.alias)?;
        match self.find_decl(tf, &target.name.name) {
            Some(Decl::Interface(i)) => Some(
                i.actions
                    .iter()
                    .map(|a| (a.name.name.clone(), ActionIn { file: tf, action: a, inherited_from: None }))
                    .collect(),
            ),
            Some(Decl::Service(s)) => Some(self.service_actions(tf, s)),
            _ => None,
        }
    }

    pub fn resolve_type(&self, f: usize, t: &TypeRef) -> RTy {
        let mut r = match &t.base {
            Base::Scalar(s) => RTy { ty: Ty::Scalar(*s), optional: false, untrusted: false },
            Base::List(inner) => {
                RTy { ty: Ty::List(Box::new(self.resolve_type(f, inner))), optional: false, untrusted: false }
            }
            Base::Untrusted(inner) => {
                let mut r = self.resolve_type(f, inner);
                r.untrusted = true;
                r
            }
            Base::Named(q) => {
                let ty = match self.file_of(f, &q.alias) {
                    None => Ty::Unknown,
                    Some(tf) => match self.find_decl(tf, &q.name.name) {
                        Some(Decl::Type(_)) => Ty::Record(tf, q.name.name.clone()),
                        Some(Decl::Enum(_)) => Ty::Enum(tf, q.name.name.clone()),
                        _ => Ty::Unknown,
                    },
                };
                RTy { ty, optional: false, untrusted: false }
            }
        };
        if t.optional {
            r.optional = true;
        }
        r
    }

    fn record(&self, f: usize, name: &str) -> Option<&TypeDecl> {
        match self.find_decl(f, name) {
            Some(Decl::Type(t)) => Some(t),
            _ => None,
        }
    }

    /// Can a value of type `from` be used where `to` is expected?
    fn assignable(&self, from: &RTy, to: &RTy, depth: u32) -> Result<(), Why> {
        if from.untrusted && !to.untrusted {
            return Err(Why::Untrusted);
        }
        if from.optional && !to.optional {
            return Err(Why::Mismatch);
        }
        let ok = |same: bool| if same { Ok(()) } else { Err(Why::Mismatch) };
        match (&from.ty, &to.ty) {
            (Ty::Unknown, _) | (_, Ty::Unknown) => Ok(()),
            (Ty::Scalar(a), Ty::Scalar(b)) => ok(a == b),
            (Ty::List(a), Ty::List(b)) => self.assignable(a, b, depth + 1),
            (Ty::Enum(af, an), Ty::Enum(bf, bn)) => ok(af == bf && an == bn),
            (Ty::Record(af, an), Ty::Record(bf, bn)) => {
                if af == bf && an == bn {
                    return Ok(());
                }
                // A record with extra fields can stand in for one with fewer.
                let (Some(a), Some(b)) = (self.record(*af, an), self.record(*bf, bn)) else {
                    return Ok(());
                };
                if depth > 32 {
                    return Err(Why::Mismatch);
                }
                for want in &b.fields {
                    let Some(have) = a.fields.iter().find(|x| x.name.name == want.name.name) else {
                        return Err(Why::Mismatch);
                    };
                    let have_ty = self.resolve_type(*af, &have.ty);
                    let want_ty = self.resolve_type(*bf, &want.ty);
                    self.assignable(&have_ty, &want_ty, depth + 1)?;
                }
                Ok(())
            }
            _ => Err(Why::Mismatch),
        }
    }

    /// Record types a type mentions, looking through lists.
    fn mentioned_records(&self, r: &RTy, out: &mut Vec<(usize, String)>) {
        match &r.ty {
            Ty::Record(f, n) => out.push((*f, n.clone())),
            Ty::List(inner) => self.mentioned_records(inner, out),
            _ => {}
        }
    }

    fn reaches(&self, start: &(usize, String), from: &(usize, String), seen: &mut BTreeSet<(usize, String)>) -> bool {
        let Some(decl) = self.record(from.0, &from.1) else {
            return false;
        };
        for field in &decl.fields {
            let mut refs = Vec::new();
            self.mentioned_records(&self.resolve_type(from.0, &field.ty), &mut refs);
            for r in refs {
                if r == *start {
                    return true;
                }
                if seen.insert(r.clone()) && self.reaches(start, &r, seen) {
                    return true;
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

struct Ctx<'a> {
    w: &'a World,
    out: Vec<Diag>,
}

impl<'a> Ctx<'a> {
    fn diag(&mut self, code: &'static str, f: usize, pos: Pos, message: String, fix: String) {
        self.out.push(Diag { code, file: self.w.files[f].path.clone(), pos, message, fix });
    }

    /// Resolves the alias of a qualified name, reporting R01 if no `use` line defines it.
    fn alias_file(&mut self, f: usize, alias: &Option<Ident>) -> Option<usize> {
        let a = match alias {
            None => return Some(f),
            Some(a) => a,
        };
        if let Some(&t) = self.w.aliases[f].get(&a.name) {
            return Some(t);
        }
        if !self.w.broken[f].contains(&a.name) {
            let fix = suggest(
                &a.name,
                self.w.aliases[f].keys().map(|s| s.as_str()),
                &format!("Add `use {} from \"./file.remit\"` at the top of the file.", a.name),
            );
            self.diag("R01", f, a.pos, format!("No `use` line defines `{}`.", a.name), fix);
        }
        None
    }

    fn check_typeref(&mut self, f: usize, t: &TypeRef) {
        match &t.base {
            Base::Scalar(_) => {}
            Base::List(inner) | Base::Untrusted(inner) => self.check_typeref(f, inner),
            Base::Named(q) => {
                let Some(tf) = self.alias_file(f, &q.alias) else {
                    return;
                };
                match self.w.find_decl(tf, &q.name.name) {
                    Some(Decl::Type(_)) | Some(Decl::Enum(_)) => {}
                    Some(other) => {
                        let kind = other.kind();
                        self.diag(
                            "R01",
                            f,
                            q.name.pos,
                            format!("`{}` is {} {}, not a type.", q.show(), article(kind), kind),
                            "Use a `type` or `enum` here.".into(),
                        );
                    }
                    None => {
                        let options = self.w.files[tf].decls.iter().filter_map(|d| match d {
                            Decl::Type(_) | Decl::Enum(_) => Some(d.name().name.as_str()),
                            _ => None,
                        });
                        let fix = suggest(
                            &q.name.name,
                            options.chain(BUILT_IN_TYPES.iter().copied()),
                            &format!("Declare `type {} {{ ... }}`, or use an existing type.", q.name.name),
                        );
                        self.diag("R01", f, q.name.pos, format!("There is no type called `{}`.", q.show()), fix);
                    }
                }
            }
        }
    }

    fn duplicates<'i>(&mut self, f: usize, what: &str, names: impl Iterator<Item = &'i Ident>) {
        let mut seen = BTreeSet::new();
        for id in names {
            if !seen.insert(id.name.clone()) {
                self.diag(
                    "R01",
                    f,
                    id.pos,
                    format!("The {} `{}` is declared twice.", what, id.name),
                    "Rename or remove one of them.".into(),
                );
            }
        }
    }

    fn need_doc(&mut self, f: usize, doc: &Option<String>, kind: &str, name: &Ident) {
        if doc.as_deref().map_or(true, |d| d.trim().is_empty()) {
            self.diag(
                "R03",
                f,
                name.pos,
                format!("The {} `{}` has no description.", kind, name.name),
                format!(
                    "Put a sentence in double quotes on the line before it, saying what the {} does. People read it when asked to approve a grant.",
                    kind
                ),
            );
        }
    }

    fn check_file(&mut self, f: usize) {
        let w = self.w;
        let file = &w.files[f];
        self.duplicates(f, "name", file.decls.iter().map(|d| d.name()));
        for d in &file.decls {
            let name = d.name();
            if BUILT_IN_TYPES.contains(&name.name.as_str()) {
                self.diag(
                    "R14",
                    f,
                    name.pos,
                    format!("`{}` is built into the language and cannot be declared.", name.name),
                    "Choose a different name.".into(),
                );
            }
            match d {
                Decl::Type(t) => {
                    self.duplicates(f, "field", t.fields.iter().map(|x| &x.name));
                    for field in &t.fields {
                        self.check_typeref(f, &field.ty);
                    }
                    let me = (f, t.name.name.clone());
                    if w.reaches(&me, &me, &mut BTreeSet::new()) {
                        self.diag(
                            "R04",
                            f,
                            t.name.pos,
                            format!("The type `{}` refers to itself, directly or through other types.", t.name.name),
                            "Refer to the other value by an id field, such as `parentId: Text`, instead of containing it.".into(),
                        );
                    }
                }
                Decl::Enum(e) => self.duplicates(f, "enum value", e.members.iter()),
                Decl::Interface(i) => {
                    self.need_doc(f, &i.doc, "interface", &i.name);
                    self.duplicates(f, "action", i.actions.iter().map(|a| &a.name));
                    let all: BTreeMap<String, ActionIn> = i
                        .actions
                        .iter()
                        .map(|a| (a.name.name.clone(), ActionIn { file: f, action: a, inherited_from: None }))
                        .collect();
                    for a in &i.actions {
                        self.check_action(f, a, &all);
                    }
                }
                Decl::Service(s) => self.check_service(f, s),
                Decl::Grant(g) => self.check_grant(f, g),
            }
        }
    }

    /// Rules that apply to one action wherever it is declared: R03, R06, R07, R14.
    fn check_action(&mut self, f: usize, a: &Action, siblings: &BTreeMap<String, ActionIn>) {
        let w = self.w;
        self.need_doc(f, &a.doc, "action", &a.name);
        self.duplicates(f, "parameter", a.params.iter().map(|p| &p.name));
        for p in &a.params {
            self.check_typeref(f, &p.ty);
            if CONDITION_WORDS.contains(&p.name.name.as_str()) {
                self.diag(
                    "R14",
                    f,
                    p.name.pos,
                    format!("`{}` has a meaning in grant conditions, so it cannot name a parameter.", p.name.name),
                    "Choose a different parameter name.".into(),
                );
            }
        }
        if let Some(r) = &a.result {
            self.check_typeref(f, r);
        }

        let reads = a.effect(EffectKind::Read).is_some();
        if reads && a.effects.len() > 1 {
            self.diag(
                "R06",
                f,
                a.effects_pos,
                format!("The action `{}` combines `read` with other effects.", a.name.name),
                "Remove `read`. Every action may read; `read` alone means it changes nothing.".into(),
            );
        }
        let mut kinds = Vec::new();
        for e in &a.effects {
            if kinds.contains(&e.kind) {
                self.diag(
                    "R06",
                    f,
                    e.pos,
                    format!("The effect `{}` is listed twice.", e.kind.name()),
                    "List each effect once.".into(),
                );
            }
            kinds.push(e.kind);
        }

        let Some(undo) = &a.undo else {
            return;
        };
        if !a.is_acting() {
            self.diag(
                "R06",
                f,
                undo.target.pos,
                format!("The action `{}` only reads, so there is nothing to undo.", a.name.name),
                "Remove the `undo` line.".into(),
            );
            return;
        }
        let Some(target) = siblings.get(&undo.target.name) else {
            let fix = suggest(
                &undo.target.name,
                siblings.keys().map(|s| s.as_str()),
                &format!("Declare an action called `{}` next to this one.", undo.target.name),
            );
            self.diag(
                "R07",
                f,
                undo.target.pos,
                format!("There is no action called `{}` to undo `{}`.", undo.target.name, a.name.name),
                fix,
            );
            return;
        };
        let Some(result) = &a.result else {
            self.diag(
                "R07",
                f,
                undo.target.pos,
                format!("`{}` returns nothing, so `{}` has nothing to reverse.", a.name.name, undo.target.name),
                format!("Give `{}` a result, such as a record of what it did.", a.name.name),
            );
            return;
        };
        let result_ty = w.resolve_type(f, result);
        let required: Vec<&Param> = target.action.params.iter().filter(|p| !p.ty.optional).collect();
        let fits = required.len() == 1
            && w.assignable(&result_ty, &w.resolve_type(target.file, &required[0].ty), 0).is_ok();
        if !fits {
            self.diag(
                "R07",
                f,
                undo.target.pos,
                format!(
                    "`{}` must take the result of `{}` as its only required parameter.",
                    undo.target.name, a.name.name
                ),
                format!("Give `{}` exactly one required parameter of type `{}`.", undo.target.name, result.show()),
            );
        }
    }

    fn check_service(&mut self, f: usize, s: &Service) {
        let w = self.w;
        self.need_doc(f, &s.doc, "service", &s.name);
        self.duplicates(f, "action", s.actions.iter().map(|a| &a.name));
        self.duplicates(f, "resource", s.uses.iter().map(|u| &u.name));

        let mut seen = BTreeSet::new();
        for q in &s.implements {
            if !seen.insert(q.show()) {
                self.diag(
                    "R01",
                    f,
                    q.pos(),
                    format!("`{}` is listed twice.", q.show()),
                    "List each interface once.".into(),
                );
            }
            self.check_interface_ref(f, q);
        }
        for u in &s.uses {
            if let Resource::Interface(q) = &u.resource {
                self.check_interface_ref(f, q);
            }
        }

        let all = w.service_actions(f, s);
        for a in &s.actions {
            self.check_action(f, a, &all);
        }

        // An inherited action must come from exactly one interface.
        let mut owner: BTreeMap<&str, String> = BTreeMap::new();
        for q in &s.implements {
            let Some((_, iface)) = w.find_interface(f, q) else {
                continue;
            };
            for a in &iface.actions {
                let restated = s.actions.iter().any(|x| x.name.name == a.name.name);
                if let Some(first) = owner.get(a.name.name.as_str()) {
                    if !restated {
                        self.diag(
                            "R08",
                            f,
                            q.pos(),
                            format!("Both `{}` and `{}` have an action called `{}`.", first, q.show(), a.name.name),
                            format!("Restate `{}` in the service so that it satisfies both.", a.name.name),
                        );
                    }
                } else {
                    owner.insert(&a.name.name, q.show());
                }
            }
        }

        for q in &s.implements {
            let Some((fi, iface)) = w.find_interface(f, q) else {
                continue;
            };
            for want in &iface.actions {
                if let Some(have) = s.actions.iter().find(|x| x.name.name == want.name.name) {
                    self.conform(f, s, have, fi, &q.show(), want);
                }
            }
        }
    }

    fn check_interface_ref(&mut self, f: usize, q: &QName) {
        let Some(tf) = self.alias_file(f, &q.alias) else {
            return;
        };
        match self.w.find_decl(tf, &q.name.name) {
            Some(Decl::Interface(_)) => {}
            Some(other) => {
                let kind = other.kind();
                self.diag(
                    "R01",
                    f,
                    q.name.pos,
                    format!("`{}` is {} {}, not an interface.", q.show(), article(kind), kind),
                    "Name an interface here.".into(),
                );
            }
            None => {
                let options = self.w.files[tf].decls.iter().filter_map(|d| match d {
                    Decl::Interface(_) => Some(d.name().name.as_str()),
                    _ => None,
                });
                let fix = suggest(&q.name.name, options, "Check the name, and the file it comes from.");
                self.diag("R01", f, q.name.pos, format!("There is no interface called `{}`.", q.show()), fix);
            }
        }
    }

    /// SPEC.md section 11: `have`, in service `s`, against `want` from interface `iface`.
    fn conform(&mut self, f: usize, s: &Service, have: &Action, fi: usize, iface: &str, want: &Action) {
        let w = self.w;
        let act = &have.name.name;
        let whose = format!("`{}.{}`", s.name.name, act);

        // 2. Parameters: accept everything the interface passes; extras are optional.
        for wp in &want.params {
            let Some(hp) = have.params.iter().find(|p| p.name.name == wp.name.name) else {
                self.diag(
                    "R08",
                    f,
                    have.name.pos,
                    format!("{} does not accept the parameter `{}` that `{}` requires.", whose, wp.name.name, iface),
                    format!("Add the parameter `{}: {}`.", wp.name.name, wp.ty.show()),
                );
                continue;
            };
            match w.assignable(&w.resolve_type(fi, &wp.ty), &w.resolve_type(f, &hp.ty), 0) {
                Ok(()) => {}
                Err(Why::Untrusted) => self.diag(
                    "R05",
                    f,
                    hp.ty.pos,
                    format!(
                        "`{}` passes `{}` as untrusted content, but {} treats it as trusted.",
                        iface, wp.name.name, whose
                    ),
                    format!("Declare the parameter as `{}: {}`.", wp.name.name, wp.ty.show()),
                ),
                Err(Why::Mismatch) => self.diag(
                    "R08",
                    f,
                    hp.ty.pos,
                    format!(
                        "The parameter `{}` of {} has type `{}`, but `{}` passes `{}`.",
                        wp.name.name,
                        whose,
                        hp.ty.show(),
                        iface,
                        wp.ty.show()
                    ),
                    format!("Declare the parameter as `{}: {}`.", wp.name.name, wp.ty.show()),
                ),
            }
        }
        for hp in &have.params {
            let extra = !want.params.iter().any(|p| p.name.name == hp.name.name);
            if extra && !hp.ty.optional {
                self.diag(
                    "R08",
                    f,
                    hp.name.pos,
                    format!(
                        "{} requires the parameter `{}`, which callers of `{}` do not provide.",
                        whose, hp.name.name, iface
                    ),
                    format!("Make it optional: `{}: {}?`.", hp.name.name, hp.ty.show()),
                );
            }
        }

        // 3. Result: at least what the interface promises.
        match (&want.result, &have.result) {
            (Some(wr), Some(hr)) => match w.assignable(&w.resolve_type(f, hr), &w.resolve_type(fi, wr), 0) {
                Ok(()) => {}
                Err(Why::Untrusted) => self.diag(
                    "R05",
                    f,
                    hr.pos,
                    format!(
                        "{} returns untrusted content, but `{}` promises `{}`.",
                        whose,
                        iface,
                        wr.show()
                    ),
                    "Return content the service vouches for, or implement an interface that marks this result `Untrusted`.".into(),
                ),
                Err(Why::Mismatch) => self.diag(
                    "R08",
                    f,
                    hr.pos,
                    format!("{} returns `{}`, but `{}` promises `{}`.", whose, hr.show(), iface, wr.show()),
                    format!("Return `{}`, or a type with the same fields and more.", wr.show()),
                ),
            },
            (Some(wr), None) => self.diag(
                "R08",
                f,
                have.name.pos,
                format!("{} returns nothing, but `{}` promises `{}`.", whose, iface, wr.show()),
                format!("Add `-> {}`.", wr.show()),
            ),
            (None, _) => {}
        }

        // 4. Effects: a subset, with the same or a lower spend ceiling.
        for he in &have.effects {
            if he.kind == EffectKind::Read {
                continue;
            }
            let Some(we) = want.effect(he.kind) else {
                let allowed: Vec<&str> = want.effects.iter().map(|e| e.kind.name()).collect();
                self.diag(
                    "R08",
                    f,
                    he.pos,
                    format!(
                        "{} declares `{}`, which `{}` does not allow. It allows: {}.",
                        whose,
                        he.kind.name(),
                        iface,
                        allowed.join(", ")
                    ),
                    format!("Remove `{}`. A service may do less than its interface, never more.", he.kind.name()),
                );
                continue;
            };
            let Some(ceiling) = &we.max else {
                continue;
            };
            let within = matches!(&he.max, Some(m) if m.currency == ceiling.currency && m.amount <= ceiling.amount);
            if !within {
                let found = match &he.max {
                    Some(m) => format!("a ceiling of {}", m.text),
                    None => "no ceiling".to_string(),
                };
                self.diag(
                    "R08",
                    f,
                    he.pos,
                    format!("`{}` limits spending to {} per call, but {} has {}.", iface, ceiling.text, whose, found),
                    format!("Write `spend(max: {})` or a lower amount in the same currency.", ceiling.text),
                );
            }
        }

        // 5. Undo: kept, for the same period or longer.
        let Some(wu) = &want.undo else {
            return;
        };
        let wanted = match &wu.within {
            Some(d) => format!("undo: {} within {}", wu.target.name, d.text),
            None => format!("undo: {}", wu.target.name),
        };
        match &have.undo {
            None => self.diag(
                "R08",
                f,
                have.name.pos,
                format!("`{}` promises that `{}` can be undone, but {} makes no such promise.", iface, act, whose),
                format!("Add `{}`.", wanted),
            ),
            Some(hu) => {
                let same_target = hu.target.name == wu.target.name;
                let long_enough = match (&wu.within, &hu.within) {
                    (None, None) => true,
                    (None, Some(_)) => false,
                    (Some(_), None) => true,
                    (Some(a), Some(b)) => b.minutes >= a.minutes,
                };
                if !same_target || !long_enough {
                    self.diag(
                        "R08",
                        f,
                        hu.target.pos,
                        format!("{} weakens the undo that `{}` promises.", whose, iface),
                        format!("Write `{}`, or allow a longer period.", wanted),
                    );
                }
            }
        }
    }

    fn check_grant(&mut self, f: usize, g: &Grant) {
        let w = self.w;
        self.need_doc(f, &g.doc, "grant", &g.name);
        for rule in &g.rules {
            let Some(tf) = self.alias_file(f, &rule.target.alias) else {
                continue;
            };
            let Some(actions) = w.target_actions(f, &rule.target) else {
                let options = w.files[tf].decls.iter().filter_map(|d| match d {
                    Decl::Interface(_) | Decl::Service(_) => Some(d.name().name.as_str()),
                    _ => None,
                });
                let fix = suggest(&rule.target.name.name, options, "Name an interface or a service.");
                self.diag(
                    "R09",
                    f,
                    rule.target.name.pos,
                    format!("There is no interface or service called `{}`.", rule.target.show()),
                    fix,
                );
                continue;
            };
            let Some(found) = actions.get(&rule.action.name) else {
                let fix = suggest(
                    &rule.action.name,
                    actions.keys().map(|s| s.as_str()),
                    &format!(
                        "`{}` has these actions: {}.",
                        rule.target.show(),
                        actions.keys().cloned().collect::<Vec<_>>().join(", ")
                    ),
                );
                self.diag(
                    "R09",
                    f,
                    rule.action.pos,
                    format!("`{}` has no action called `{}`.", rule.target.show(), rule.action.name),
                    fix,
                );
                continue;
            };
            let action = found.action;
            let full = format!("{}.{}", rule.target.show(), action.name.name);
            let mut accepted_irreversible = false;
            for cond in &rule.conds {
                match cond {
                    Cond::Irreversible(_) => accepted_irreversible = true,
                    Cond::Spend(limit, pos) => match action.effect(EffectKind::Spend) {
                        None => self.diag(
                            "R10",
                            f,
                            *pos,
                            format!("`{}` does not spend, so a spending limit has no meaning.", full),
                            "Remove the `spend <=` condition.".into(),
                        ),
                        Some(Effect { max: Some(max), .. }) if max.currency != limit.currency => self.diag(
                            "R10",
                            f,
                            *pos,
                            format!("`{}` charges in {}, but this limit is in {}.", full, max.currency, limit.currency),
                            format!("Write the limit in {}.", max.currency),
                        ),
                        Some(_) => {}
                    },
                    Cond::Times(n, pos) => {
                        if *n < 1 {
                            self.diag(
                                "R10",
                                f,
                                *pos,
                                "A call limit must be at least 1.".into(),
                                "Write `times <= 1` or more, or remove the rule.".into(),
                            );
                        }
                    }
                    Cond::Service(value, pos) => {
                        let ok = matches!(value, Literal::List(items)
                            if !items.is_empty() && items.iter().all(|i| matches!(i, Literal::Str(_))));
                        if !ok {
                            self.diag(
                                "R10",
                                f,
                                *pos,
                                "`service in` needs a list of addresses.".into(),
                                "Write `service in [\"host/path\"]`.".into(),
                            );
                        }
                    }
                    Cond::Param { name, op, value } => {
                        let Some(param) = action.params.iter().find(|p| p.name.name == name.name) else {
                            let fix = suggest(
                                &name.name,
                                action.params.iter().map(|p| p.name.name.as_str()),
                                "Use one of the action's parameters, or `spend`, `times`, `service`, `irreversible`.",
                            );
                            self.diag(
                                "R09",
                                f,
                                name.pos,
                                format!("`{}` has no parameter called `{}`.", full, name.name),
                                fix,
                            );
                            continue;
                        };
                        let ty = w.resolve_type(found.file, &param.ty);
                        if let Err(fix) = cond_fits(&ty, *op, value) {
                            self.diag(
                                "R10",
                                f,
                                name.pos,
                                format!(
                                    "The parameter `{}` has type `{}`, which this condition does not fit.",
                                    name.name,
                                    param.ty.show()
                                ),
                                fix,
                            );
                        }
                    }
                }
            }
            if !rule.ask && action.is_acting() && action.undo.is_none() && !accepted_irreversible {
                self.diag(
                    "R11",
                    f,
                    rule.pos,
                    format!("`{}` cannot be undone, and this rule allows it without asking.", full),
                    "Change `allow` to `ask` so the person confirms each call, or add the condition `irreversible` to accept that it cannot be reversed.".into(),
                );
            }
        }
    }
}

fn article(kind: &str) -> &'static str {
    if kind.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    }
}

fn literal_fits(ty: &Ty, value: &Literal) -> bool {
    match (ty, value) {
        (Ty::Unknown, _) => true,
        (Ty::Scalar(Scalar::Text), Literal::Str(_)) => true,
        (Ty::Scalar(Scalar::Int), Literal::Int(_)) => true,
        (Ty::Scalar(Scalar::Decimal), Literal::Decimal(_) | Literal::Int(_)) => true,
        (Ty::Scalar(Scalar::Money), Literal::Money(_)) => true,
        (Ty::Scalar(Scalar::Duration), Literal::Duration(_)) => true,
        (Ty::Scalar(Scalar::Date), Literal::Date(_)) => true,
        (Ty::Scalar(Scalar::DateTime), Literal::DateTime(_)) => true,
        _ => false,
    }
}

/// Does `param op value` make sense for a parameter of this type? Err carries the fix.
fn cond_fits(ty: &RTy, op: Op, value: &Literal) -> Result<(), String> {
    let comparable = matches!(
        ty.ty,
        Ty::Unknown
            | Ty::Scalar(Scalar::Text | Scalar::Int | Scalar::Decimal | Scalar::Money | Scalar::Duration | Scalar::Date | Scalar::DateTime)
    );
    if !comparable {
        return Err("Conditions can compare only Text, numbers, Money, Duration, Date, and DateTime parameters.".into());
    }
    match op {
        Op::In => match value {
            Literal::List(items) if !items.is_empty() && items.iter().all(|i| literal_fits(&ty.ty, i)) => Ok(()),
            _ => Err("After `in`, write a list of values of the parameter's type, such as [\"a\", \"b\"].".into()),
        },
        Op::Eq => {
            if literal_fits(&ty.ty, value) {
                Ok(())
            } else {
                Err(format!("Compare it with a value of its own type, not {}.", value.kind()))
            }
        }
        Op::Le | Op::Ge => {
            if matches!(ty.ty, Ty::Scalar(Scalar::Text)) {
                return Err("Text cannot be compared with `<=` or `>=`. Use `==` or `in`.".into());
            }
            if literal_fits(&ty.ty, value) {
                Ok(())
            } else {
                Err(format!("Compare it with a value of its own type, not {}.", value.kind()))
            }
        }
    }
}
