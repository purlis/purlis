// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use super::{at as place, plain as args};
use serde_json::{Value, json};

/// A directory for a plane, canonical so the workspace writer's containment checks agree
/// with it, and the tempdir that keeps it alive.
fn plane() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = std::fs::canonicalize(dir.path())
        .expect("canonical")
        .join("plane");
    std::fs::create_dir_all(&root).expect("the plane's directory");
    (dir, root)
}

/// A settings file as JSON.
fn read(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("the file")).expect("json")
}

#[test]
fn init_writes_the_report_ask_rule_in_every_harness_that_can_hold_one() {
    let (_d, root) = plane();
    let outcome = init(&place(&root, false), &args());
    assert_eq!(outcome.code, 0, "{:?}", outcome.said);
    assert_eq!(
        read(&root.join(settings::SETTINGS))["permissions"]["ask"],
        json!([
            "Bash(charter report *--yes*)",
            "Bash(charter *todo*promote*)",
            "Bash(purlis report *--yes*)",
            "Bash(purlis *todo*promote*)"
        ])
    );
    assert_eq!(
        read(&root.join(settings::OPENCODE))["permission"]["bash"],
        json!({"charter report *--yes*": "ask", "charter *todo*promote*": "ask",
               "purlis report *--yes*": "ask", "purlis *todo*promote*": "ask"})
    );
}

/// V42: `charter ws todo promote` sends a todo's text to a forge, so both harnesses ask
/// first, whatever the call's spelling or the order of its flags.
#[test]
fn every_spelling_of_a_todo_promote_asks_in_both_harnesses_and_no_other_todo_does() {
    let (_d, root) = plane();
    let outcome = init(&place(&root, false), &args());
    assert_eq!(outcome.code, 0, "{:?}", outcome.said);
    let claude = read(&root.join(settings::SETTINGS))["permissions"]["ask"].clone();
    let opencode = read(&root.join(settings::OPENCODE))["permission"]["bash"].clone();
    let claude_asks = |command: &str| {
        claude.as_array().unwrap().iter().any(|rule| {
            rule.as_str()
                .and_then(|r| r.strip_prefix("Bash(")?.strip_suffix(')'))
                .and_then(|p| glob::Pattern::new(p).ok())
                .is_some_and(|p| p.matches(command))
        })
    };
    // opencode: the last rule that matches decides.
    let opencode_asks = |command: &str| {
        opencode
            .as_object()
            .unwrap()
            .iter()
            .rfind(|(p, _)| glob::Pattern::new(p).is_ok_and(|p| p.matches(command)))
            .is_some_and(|(_, decision)| decision == "ask")
    };
    for promote in [
        "charter ws todo promote port-the-picker",
        "charter ws todo promote port-the-picker --repo api",
        "charter ws todo -w alpha promote port-the-picker",
        "charter ws -w alpha todo promote port-the-picker --repo api",
        "charter workspace todo promote port-the-picker",
        "charter workspace todo --workspace alpha promote port-the-picker --repo api",
    ] {
        assert!(
            claude_asks(promote),
            "Claude Code does not ask for {promote:?}: {claude}"
        );
        assert!(
            opencode_asks(promote),
            "opencode does not ask for {promote:?}: {opencode}"
        );
    }
    for other in [
        "charter ws todo done port-the-picker",
        "charter ws todo -w alpha",
    ] {
        assert!(!claude_asks(other), "{other:?}");
        assert!(!opencode_asks(other), "{other:?}");
    }
}

#[test]
fn reinit_adds_the_report_rule_to_an_existing_plane_and_touches_no_other_rule() {
    let (_d, root) = plane();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(root.join(".claude")).unwrap();
    std::fs::write(
        root.join(settings::SETTINGS),
        r#"{"permissions": {"ask": ["Bash(charter handoff *)", "Bash(terraform *)"], "allow": ["Bash(ls *)"], "deny": ["Bash(rm -rf *)"]}}"#,
    )
    .unwrap();
    std::fs::write(
        root.join(settings::OPENCODE),
        r#"{"permission": {"bash": {"charter handoff *": "ask", "ls *": "allow"}}}"#,
    )
    .unwrap();

    let outcome = reinit(&place(&root, true));

    assert_eq!(outcome.code, 0, "{:?}", outcome.said);
    let said = format!("{:?}", outcome.said);
    assert!(
        said.contains(
            ".claude/settings.json (ask: charter report --yes), opencode.json (ask: charter \
             report --yes)"
        ),
        "{said}"
    );
    let claude = read(&root.join(settings::SETTINGS));
    assert_eq!(
        claude["permissions"]["ask"],
        // The handoff rule an older `init` wrote is kept, and given no `purlis` twin
        // (#1444): the `handoff-rule` fix is what removes it.
        json!([
            "Bash(charter handoff *)",
            "Bash(terraform *)",
            "Bash(charter report *--yes*)",
            "Bash(charter *todo*promote*)",
            "Bash(purlis report *--yes*)",
            "Bash(purlis *todo*promote*)"
        ])
    );
    assert_eq!(claude["permissions"]["allow"], json!(["Bash(ls *)"]));
    assert_eq!(claude["permissions"]["deny"], json!(["Bash(rm -rf *)"]));
    assert_eq!(
        read(&root.join(settings::OPENCODE))["permission"]["bash"],
        json!({"charter handoff *": "ask", "ls *": "allow", "charter report *--yes*": "ask",
               "charter *todo*promote*": "ask",
               "purlis report *--yes*": "ask", "purlis *todo*promote*": "ask"})
    );

    // Once it is there, `reinit` has nothing to add and does not mention it.
    let again = reinit(&place(&root, true));
    assert!(
        !format!("{:?}", again.said).contains("charter report"),
        "{:?}",
        again.said
    );
}

#[test]
fn reinit_carries_the_report_rule_into_a_workspaces_generated_settings() {
    let (_d, root) = plane();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(root.join(".claude")).unwrap();
    std::fs::write(root.join(settings::SETTINGS), "{}\n").unwrap();
    crate::wscmd::ensure::ensure(
        &root,
        "alpha",
        "2026-05-04T11:32:17Z".parse().unwrap(),
        "fixture",
    )
    .unwrap();
    let layer = root.join("workspaces/alpha/.claude/settings.json");

    let outcome = reinit(&place(&root, true));

    assert_eq!(outcome.code, 0, "{:?}", outcome.said);
    let ask = read(&layer)["permissions"]["ask"].clone();
    assert!(
        ask.as_array()
            .is_some_and(|a| a.contains(&json!("Bash(charter report *--yes*)"))),
        "{ask}"
    );
    assert!(
        outcome.said.contains(&Say::Info(
            "  Carried into the generated settings of workspace alpha."
                .to_owned()
                .into()
        )),
        "{:?}",
        outcome.said
    );
}

#[test]
fn a_broken_opencode_json_stops_the_report_rule_everywhere() {
    let (_d, root) = plane();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::write(root.join(settings::OPENCODE), "{broken").unwrap();

    let outcome = reinit(&place(&root, true));

    let settings = std::fs::read_to_string(root.join(settings::SETTINGS)).unwrap_or_default();
    assert!(!settings.contains("charter report"), "{settings}");
    assert!(
        outcome.said.iter().any(|s| matches!(
            s,
            Say::Warn(w) if w.starts_with("the ask rule for `charter report --yes` was not written anywhere")
                && w.ends_with("run purlis reinit again")
        )),
        "{:?}",
        outcome.said
    );
}
