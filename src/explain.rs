//! Writes a grant out in plain language, using only the words in the contracts.
//!
//! This is the text a person is shown before approving. Nobody asking for
//! access can change it, because it is generated from descriptions that are
//! part of each contract's identity.

use crate::ast::*;
use crate::check::World;

fn doc(d: &Option<String>) -> &str {
    d.as_deref().unwrap_or("(no description)")
}

pub fn effect_words(a: &Action) -> String {
    if !a.is_acting() {
        return "only read".into();
    }
    let mut parts = Vec::new();
    for e in &a.effects {
        match e.kind {
            EffectKind::Read => {}
            EffectKind::Write => parts.push("change data".to_string()),
            EffectKind::Send => parts.push("contact others on your behalf".to_string()),
            EffectKind::Spend => parts.push(match &e.max {
                Some(m) => format!("take payment, up to {} per call", m.text),
                None => "take payment, with no ceiling per call".to_string(),
            }),
        }
    }
    parts.join(", ")
}

fn cond_words(c: &Cond) -> Option<String> {
    Some(match c {
        Cond::Spend(m, _) => format!("total spend up to {}", m.text),
        Cond::Times(1, _) => "at most 1 call".to_string(),
        Cond::Times(n, _) => format!("at most {} calls", n),
        Cond::Service(list, _) => format!("only at {}", list_words(list)),
        Cond::Irreversible(_) => return None,
        Cond::Param { name, path, op, value } => {
            let what = Cond::shown(name, path);
            match op {
                Op::Eq => format!("`{}` is {}", what, value.show()),
                Op::Le => format!("`{}` is at most {}", what, value.show()),
                Op::Ge => format!("`{}` is at least {}", what, value.show()),
                Op::In => format!("`{}` is one of {}", what, list_words(value)),
            }
        }
    })
}

fn list_words(l: &Literal) -> String {
    match l {
        Literal::List(items) => items.iter().map(|i| i.show()).collect::<Vec<_>>().join(", "),
        other => other.show(),
    }
}

/// The plain-language text of every grant in file `f`.
pub fn explain(w: &World, f: usize) -> String {
    let mut out = String::new();
    for d in &w.files[f].decls {
        let Decl::Grant(g) = d else {
            continue;
        };
        out.push_str(&format!("{}\n{}\n", g.name.name, doc(&g.doc)));
        for (ask, heading) in [
            (false, "Allowed without asking you each time:"),
            (true, "Allowed only after you confirm each time:"),
        ] {
            let rules: Vec<&Rule> = g.rules.iter().filter(|r| r.ask == ask).collect();
            if rules.is_empty() {
                continue;
            }
            out.push_str(&format!("\n{}\n", heading));
            for rule in rules {
                let actions = w.target_actions(f, &rule.target);
                let Some(found) = actions.as_ref().and_then(|a| a.get(&rule.action.name)) else {
                    out.push_str(&format!("  - {}.{} (unknown action)\n", rule.target.show(), rule.action.name));
                    continue;
                };
                let a = found.action;
                out.push_str(&format!("  - {}\n", doc(&a.doc)));
                out.push_str(&format!("      action: {}.{}\n", rule.target.show(), a.name.name));
                out.push_str(&format!("      may:    {}\n", effect_words(a)));
                let limits: Vec<String> = rule.conds.iter().filter_map(cond_words).collect();
                if !limits.is_empty() {
                    out.push_str(&format!("      limits: {}\n", limits.join("; ")));
                }
                if a.is_acting() {
                    match &a.undo {
                        Some(u) => {
                            let undo_doc = actions
                                .as_ref()
                                .and_then(|m| m.get(&u.target.name))
                                .map(|t| doc(&t.action.doc))
                                .unwrap_or("(no description)");
                            let period = match &u.within {
                                Some(d) => format!(" (within {})", d.text),
                                None => String::new(),
                            };
                            out.push_str(&format!("      undo:   {}{}\n", undo_doc, period));
                        }
                        None => out.push_str("      undo:   CANNOT BE UNDONE\n"),
                    }
                }
            }
        }
        out.push_str(&format!("\nNothing else is allowed. Ends {}.\n\n", g.expires));
    }
    out
}
