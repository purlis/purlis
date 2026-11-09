// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The catalogue queries an artifact's page calls** (ADR 0120 §7, #5969), declared in the
//! same data block as its data files ([`super::data`]), so the person's approval covers them:
//!
//! ```html
//! <script type="application/json" id="purlis">
//!   { "api": 1, "queries": { "purlis.todo_list@1": ["done"], "purlis.change_status@1": [] } }
//! </script>
//! ```
//!
//! - **Each is `<id>@<N>`**, a catalogue id ([`crate::catalogue::find`]) and an API version this
//!   build has shipped, with the parameters the page passes it. A query a newer id replaces, or
//!   one deprecated, still answers, and the bar says so ([`Listed::warning`]).
//! - **The approval lists them** ([`listed`]): each id, its scope, and every declared parameter
//!   that widens what it reads, with what it then reads.
//! - **A call is admitted only as declared** ([`admit`]): the id as written, with only the
//!   parameters declared for it; anything else is `not-approved`. A query of plane scope needs
//!   the plane grant of its own besides ([`super::approval::grant_plane`]).

use serde_json::Value;

use crate::catalogue::{self, API, ErrorKind, Query, Scope};

/// One query a page declared it calls: the id, the API version, and the parameters it passes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub id: String,
    pub api: u32,
    pub params: Vec<String>,
}

impl Call {
    /// `<id>@<N>`, as the page declares and asks for it.
    pub fn declared_as(&self) -> String {
        format!("{}@{}", self.id, self.api)
    }

    fn query(&self) -> Option<(&'static Query, Option<String>)> {
        catalogue::find(&self.id)
    }
}

/// **The calls a declaration's `queries` names**, or the sentence saying why it names none: an
/// object of `<id>@<N>` to the list of parameter names passed.
pub fn parse(queries: &Value) -> Result<Vec<Call>, String> {
    let Value::Object(declared) = queries else {
        return Err("its \"queries\" is not an object of id@N to parameter names".into());
    };
    let mut calls = Vec::new();
    for (key, params) in declared {
        let shown = crate::personas::one_line(key);
        let Some((id, api)) = key.rsplit_once('@') else {
            return Err(format!(
                "it declares {shown}, which names no API version (id@N)"
            ));
        };
        let api = api
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=API).contains(n) && api == n.to_string())
            .ok_or_else(|| format!("it declares {shown}, and this purlis answers @1 to @{API}"))?;
        let Some((query, _)) = catalogue::find(id) else {
            return Err(format!(
                "it declares {shown}, which is no query (`purlis queries` lists them)"
            ));
        };
        let takes = query.parameters();
        let Value::Array(params) = params else {
            return Err(format!("its parameters of {shown} are not a list of names"));
        };
        let mut names = Vec::new();
        for param in params {
            let Some(name) = param
                .as_str()
                .filter(|n| takes["properties"].get(*n).is_some())
            else {
                return Err(format!(
                    "it declares {param} for {shown}, which takes no such parameter"
                ));
            };
            if !names.iter().any(|n| n == name) {
                names.push(name.to_owned());
            }
        }
        calls.push(Call {
            id: id.to_owned(),
            api,
            params: names,
        });
    }
    Ok(calls)
}

/// One declared call as the approval lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    /// `<id>@<N>`.
    pub id: String,
    pub scope: Scope,
    /// Each declared parameter that widens what it reads, as `<name>: <what it then reads>`.
    pub widens: Vec<String>,
    /// That the id is deprecated, or replaced by a newer one, which the bar says.
    pub warning: Option<String>,
}

/// **What the approval lists of `calls`**.
pub fn listed(calls: &[Call]) -> Vec<Listed> {
    calls
        .iter()
        .filter_map(|call| {
            let (query, warning) = call.query()?;
            let widens = query
                .widens
                .iter()
                .filter(|(name, _)| call.params.iter().any(|p| p == name))
                .map(|(name, what)| format!("{name}: {what}"))
                .collect();
            Some(Listed {
                id: call.declared_as(),
                scope: query.scope,
                widens,
                warning,
            })
        })
        .collect()
}

/// **Whether a page that declared `calls` may ask `asked` with `args`**, and the query that
/// answers it: declared as written, with only declared parameters, and a plane query only with
/// `plane` granted; otherwise `not-approved`.
pub fn admit(
    calls: &[Call],
    plane: bool,
    asked: &str,
    args: &Value,
) -> Result<&'static Query, ErrorKind> {
    let call = calls
        .iter()
        .find(|call| call.declared_as() == asked)
        .ok_or(ErrorKind::NotApproved)?;
    let (query, _) = call.query().ok_or(ErrorKind::NotApproved)?;
    admitted(query, call, plane, args)?;
    Ok(query)
}

fn admitted(query: &Query, call: &Call, plane: bool, args: &Value) -> Result<(), ErrorKind> {
    if query.scope == Scope::Plane && !plane {
        return Err(ErrorKind::NotApproved);
    }
    let passes_only_declared = match args {
        Value::Null => true,
        Value::Object(args) => args.keys().all(|k| call.params.iter().any(|p| p == k)),
        // Its type is the query's to refuse.
        _ => true,
    };
    if passes_only_declared {
        Ok(())
    } else {
        Err(ErrorKind::NotApproved)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn calls(declared: Value) -> Vec<Call> {
        parse(&declared).unwrap()
    }

    #[test]
    fn a_page_declares_each_query_as_id_at_n_with_the_parameters_it_passes() {
        assert_eq!(
            calls(json!({ "purlis.todo_list@1": ["done", "done"], "purlis.change_status@1": [] })),
            [
                Call {
                    id: "purlis.todo_list".into(),
                    api: 1,
                    params: vec!["done".into()],
                },
                Call {
                    id: "purlis.change_status".into(),
                    api: 1,
                    params: vec![],
                },
            ],
            "in the order the page wrote them"
        );
    }

    #[test]
    fn a_declaration_of_no_query_an_api_never_shipped_or_a_parameter_not_taken_is_refused() {
        for declared in [
            json!(["purlis.todo_list@1"]),
            json!({ "purlis.todo_list": [] }),
            json!({ "purlis.todo_list@0": [] }),
            json!({ "purlis.todo_list@2": [] }),
            json!({ "purlis.todo_list@01": [] }),
            json!({ "purlis.todo_list@x": [] }),
            json!({ "purlis.todo_lists@1": [] }),
            json!({ "purlis.todo_add@1": [] }),
            json!({ "purlis.persona_where@1": [] }),
            json!({ "todo_list@1": [] }),
            json!({ "purlis.todo_list@1": "done" }),
            json!({ "purlis.todo_list@1": ["workspace"] }),
            json!({ "purlis.todo_list@1": [true] }),
        ] {
            assert!(parse(&declared).is_err(), "{declared}");
        }
    }

    #[test]
    fn every_api_version_shipped_keeps_answering() {
        for n in 1..=API {
            let declared = calls(json!({ format!("purlis.todo_list@{n}"): [] }));
            let asked = format!("purlis.todo_list@{n}");
            assert!(admit(&declared, false, &asked, &json!({})).is_ok(), "@{n}");
        }
    }

    #[test]
    fn an_undeclared_query_or_parameter_is_not_approved() {
        let declared = calls(json!({ "purlis.todo_list@1": [] }));
        let not = Err(ErrorKind::NotApproved);
        assert_eq!(
            admit(&declared, true, "purlis.memory_search@1", &json!({})).map(|q| q.name),
            not
        );
        assert_eq!(
            admit(&declared, true, "purlis.todo_list", &json!({})).map(|q| q.name),
            not
        );
        assert_eq!(
            admit(&declared, true, "PURLIS.todo_list@1", &json!({})).map(|q| q.name),
            not
        );
        // A parameter the page did not declare widens nothing it was approved for.
        assert_eq!(
            admit(
                &declared,
                true,
                "purlis.todo_list@1",
                &json!({ "done": true })
            )
            .map(|q| q.name),
            not
        );
        assert_eq!(
            admit(&declared, false, "purlis.todo_list@1", &Value::Null).map(|q| q.name),
            Ok("todo_list")
        );
        assert_eq!(
            admit(&[], true, "purlis.todo_list@1", &json!({})).map(|q| q.name),
            not
        );
    }

    #[test]
    fn the_approval_lists_each_query_its_scope_and_the_widening_parameters_declared() {
        let declared =
            calls(json!({ "purlis.memory_search@1": ["query"], "purlis.todo_list@1": ["done"] }));
        let listed = listed(&declared);
        assert_eq!(listed[0].id, "purlis.memory_search@1");
        assert!(listed[0].widens.is_empty());
        assert_eq!(listed[1].id, "purlis.todo_list@1");
        assert_eq!(listed[1].scope, Scope::Workspace);
        assert_eq!(
            listed[1].widens,
            ["done: the done todos, not only the open ones"]
        );
        assert_eq!(listed[1].warning, None);
        let narrow = super::listed(&calls(json!({ "purlis.todo_list@1": [] })));
        assert!(
            narrow[0].widens.is_empty(),
            "an undeclared parameter is not named"
        );
    }

    fn todos() -> Value {
        json!({})
    }

    #[test]
    fn a_plane_query_answers_only_with_the_plane_grant_of_its_own() {
        // No core query reads the whole plane yet; one that did is held so.
        let plane = Query {
            name: "todo_list",
            since: "0.5.1",
            replaces: None,
            deprecated: None,
            scope: Scope::Plane,
            widens: &[],
            result: todos,
        };
        let call = Call {
            id: "purlis.todo_list".into(),
            api: 1,
            params: vec![],
        };
        assert_eq!(
            admitted(&plane, &call, false, &json!({})),
            Err(ErrorKind::NotApproved)
        );
        assert_eq!(admitted(&plane, &call, true, &json!({})), Ok(()));
    }
}
