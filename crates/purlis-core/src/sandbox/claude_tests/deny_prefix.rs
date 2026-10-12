// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The deny-prefix refusal as a chat's line meets it (#1344): the Claude Code adapter's
//! `sandboxed_line`, with the chat's hook socket and no proxy ports, the case #1328 added the
//! socket for. The right sandbox is not enough: a `--settings` whose `permissions.deny` is
//! missing, or does not lead with the compiled rules as written, starts nothing, and the
//! socket is never added to it.

use serde_json::json;

use super::super::grant::Grants;
use super::super::{At, Form, Words, claude};
use super::compiled_at;
use crate::harness::Harness;

fn words(armed: Vec<String>) -> Words {
    Words {
        program: "claude".to_owned(),
        command: Vec::new(),
        armed,
        charters: vec!["--charter-word".to_owned()],
    }
}

#[test]
fn the_line_with_a_hook_socket_is_refused_where_the_compiled_deny_rules_do_not_lead() {
    let root = tempfile::tempdir().expect("a project");
    let compiled = compiled_at(root.path(), None, &Grants::default());
    let settings = claude::settings(&compiled).expect("compiles");
    let rules = settings.deny.clone();
    assert!(rules.len() >= 2, "{rules:?}");
    let sockets = tempfile::tempdir().expect("a folder");
    let socket = sockets.path().join("hooks.sock");
    let at = At {
        hook_socket: Some(&socket),
        ..At::default()
    };
    let form = Form::ClaudeCode(settings.clone());
    let line = |deny: Option<serde_json::Value>| {
        let mut doc = json!({ "sandbox": settings.sandbox });
        if let Some(deny) = deny {
            doc["permissions"] = json!({ "deny": deny });
        }
        Harness::ClaudeCode.adapter().sandboxed_line(
            &form,
            words(vec!["--settings".to_owned(), doc.to_string()]),
            &at,
        )
    };

    let mut changed = rules.clone();
    changed[0] = format!("{}x", changed[0]);
    let mut reordered = rules.clone();
    reordered.swap(0, 1);
    for (case, deny) in [
        ("missing", None),
        ("not a list", Some(json!({ "rules": rules }))),
        ("empty", Some(json!([]))),
        ("shorter", Some(json!(rules[1..]))),
        ("a different rule", Some(json!(changed))),
        ("reordered", Some(json!(reordered))),
    ] {
        let why = line(deny).expect_err(case);
        assert!(why.contains("nothing was started"), "{case}: {why}");
    }

    // The control: the compiled rules as written start the chat, with the socket allowed and
    // the line's own words around the settings.
    let started = line(Some(json!(rules))).expect("the compiled rules lead the list");
    assert_eq!(started.program, "claude");
    assert_eq!(started.args.first().map(String::as_str), Some("--settings"));
    assert_eq!(
        started.args.last().map(String::as_str),
        Some("--charter-word")
    );
    let carried: serde_json::Value = serde_json::from_str(&started.args[1]).expect("JSON");
    assert_eq!(
        carried["sandbox"]["network"]["allowUnixSockets"],
        json!([super::super::real(&socket).display().to_string()])
    );
    assert_eq!(
        carried["permissions"]["deny"]
            .as_array()
            .map(|d| &d[..rules.len()]),
        Some(&json!(rules).as_array().expect("a list")[..])
    );
}
