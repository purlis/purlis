// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A shipped skill changes a workspace's charter through purlis's tools, never by hand (#5720).
//!
//! A sandboxed chat is refused a write of its workspace's `workspace.md` (ADR 0067 §2), and the
//! refusal points to `workspace_section`. A skill that tells the chat to open the file and edit
//! it walks the chat into that block. Smart close folds what lasts, and compacting moves a
//! lesson into the charter: both name `workspace_section` and `workspace_vision` for the parts
//! of `workspace.md` they touch.

use super::every_command_charter_names_is_one_it_has::{repo, shipped_skills};

use std::sync::LazyLock;

/// Every place in `text` that tells a chat to open or edit `workspace.md` itself, as written.
fn by_hand(text: &str) -> Vec<String> {
    static EDIT: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r"(?i)\b(open|edit|change|write)\s+(?:(?:its|the|this|your|their|a)\s+)?(?:workspace's\s+)?`?workspace\.md`?",
        )
        .expect("the pattern compiles")
    });
    EDIT.find_iter(text)
        .map(|m| m.as_str().split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

fn skill(name: &str) -> String {
    let path = repo().join(format!("app/src-tauri/plugin/skills/{name}/SKILL.md"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn no_shipped_skill_tells_a_chat_to_edit_its_workspace_md_by_hand() {
    let root = repo();
    let skills = shipped_skills(&root);
    assert!(skills.len() > 10, "the walk found the skills: {skills:?}");
    let found: Vec<String> = skills
        .iter()
        .flat_map(|file| {
            let text = std::fs::read_to_string(file).expect("a skill reads");
            let rel = file
                .strip_prefix(&root)
                .unwrap_or(file)
                .display()
                .to_string();
            by_hand(&text)
                .into_iter()
                .map(move |said| format!("{rel}: `{said}`"))
        })
        .collect();
    assert!(
        found.is_empty(),
        "these skills tell a chat to edit workspace.md, which a sandboxed chat is refused — \
         name the workspace_section and workspace_vision tools instead:\n{}",
        found.join("\n")
    );
}

#[test]
fn smart_close_compact_and_remember_fold_into_the_charter_through_its_tools() {
    for name in [
        "purlis-smart-close",
        "purlis-tidy-memory",
        "purlis-remember",
    ] {
        let text = skill(name);
        for tool in ["workspace_section", "workspace_vision"] {
            assert!(
                text.contains(&format!("`{tool}`")),
                "{name} does not name {tool}"
            );
        }
        for section in ["decisions", "glossary"] {
            assert!(
                text.contains(&format!("`{section}`")),
                "{name} does not name the `{section}` section of workspace_section"
            );
        }
    }
}

#[test]
fn an_edit_by_hand_is_found_and_a_tool_is_not() {
    assert_eq!(
        by_hand("In a workspace, open its `workspace.md`. Move"),
        ["open its `workspace.md`"]
    );
    assert_eq!(
        by_hand("then edit\nworkspace.md to add it"),
        ["edit workspace.md"]
    );
    assert!(by_hand("add it with the `workspace_section` tool to `workspace.md`").is_empty());
    assert!(by_hand("and its `workspace.md`.").is_empty());
}
