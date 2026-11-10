//! Prints what a contract file declares, as JSON, for tools that need to read
//! a contract without parsing the language themselves.

use crate::ast::*;
use crate::check::World;
use crate::diag::json_string;

fn opt(s: &Option<String>) -> String {
    match s {
        Some(s) => json_string(s),
        None => "null".into(),
    }
}

fn action_json(a: &Action) -> String {
    let params: Vec<String> = a
        .params
        .iter()
        .map(|p| {
            format!(
                "{{\"name\":{},\"type\":{},\"optional\":{}}}",
                json_string(&p.name.name),
                json_string(&p.ty.show()),
                p.ty.optional
            )
        })
        .collect();
    let effects: Vec<String> = a
        .effects
        .iter()
        .map(|e| {
            let max = e.max.as_ref().map(|m| json_string(&m.text)).unwrap_or_else(|| "null".into());
            format!("{{\"kind\":{},\"max\":{}}}", json_string(e.kind.name()), max)
        })
        .collect();
    let undo = match &a.undo {
        Some(u) => format!(
            "{{\"action\":{},\"within\":{}}}",
            json_string(&u.target.name),
            u.within.as_ref().map(|d| json_string(&d.text)).unwrap_or_else(|| "null".into())
        ),
        None => "null".into(),
    };
    format!(
        "{{\"name\":{},\"description\":{},\"params\":[{}],\"result\":{},\"effects\":[{}],\"undo\":{}}}",
        json_string(&a.name.name),
        opt(&a.doc),
        params.join(","),
        a.result.as_ref().map(|r| json_string(&r.show())).unwrap_or_else(|| "null".into()),
        effects.join(","),
        undo
    )
}

/// Everything file `f` declares. A service lists the actions it inherits as well as its own.
pub fn list(w: &World, f: usize) -> String {
    let (mut types, mut enums, mut interfaces, mut services, mut grants) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for d in &w.files[f].decls {
        match d {
            Decl::Type(t) => {
                let fields: Vec<String> = t
                    .fields
                    .iter()
                    .map(|x| {
                        format!(
                            "{{\"name\":{},\"type\":{}}}",
                            json_string(&x.name.name),
                            json_string(&x.ty.show())
                        )
                    })
                    .collect();
                types.push(format!("{{\"name\":{},\"fields\":[{}]}}", json_string(&t.name.name), fields.join(",")));
            }
            Decl::Enum(e) => {
                let members: Vec<String> = e.members.iter().map(|m| json_string(&m.name)).collect();
                enums.push(format!("{{\"name\":{},\"values\":[{}]}}", json_string(&e.name.name), members.join(",")));
            }
            Decl::Interface(i) => {
                let actions: Vec<String> = i.actions.iter().map(action_json).collect();
                interfaces.push(format!(
                    "{{\"name\":{},\"description\":{},\"actions\":[{}]}}",
                    json_string(&i.name.name),
                    opt(&i.doc),
                    actions.join(",")
                ));
            }
            Decl::Service(s) => {
                let actions: Vec<String> = w.service_actions(f, s).values().map(|a| action_json(a.action)).collect();
                let implements: Vec<String> = s.implements.iter().map(|q| json_string(&q.show())).collect();
                services.push(format!(
                    "{{\"name\":{},\"description\":{},\"implements\":[{}],\"actions\":[{}]}}",
                    json_string(&s.name.name),
                    opt(&s.doc),
                    implements.join(","),
                    actions.join(",")
                ));
            }
            Decl::Grant(g) => grants.push(json_string(&g.name.name)),
        }
    }
    format!(
        "{{\"file\":{},\"types\":[{}],\"enums\":[{}],\"interfaces\":[{}],\"services\":[{}],\"grants\":[{}]}}",
        json_string(&w.files[f].path),
        types.join(","),
        enums.join(","),
        interfaces.join(","),
        services.join(","),
        grants.join(",")
    )
}
