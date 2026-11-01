// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::fs;

use super::*;

/// A plane with `charter.toml` holding `shared`, and workspace `alpha` holding `settings`.
fn plane(shared: &str, settings: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("charter.toml"),
        format!("schema = 1\n{shared}"),
    )
    .unwrap();
    let ws = dir.path().join("workspaces/alpha");
    fs::create_dir_all(&ws).unwrap();
    fs::write(
        ws.join("workspace.json"),
        format!("{{\"name\":\"alpha\",\"settings\":{settings}}}"),
    )
    .unwrap();
    dir
}

const PROJECT: &str = "[skills.budget]\nskills = 80\nchars = 20000\n";

#[test]
fn nothing_written_leaves_each_harness_its_adapters_numbers() {
    let dir = plane("", "{}");
    for harness in Harness::ALL {
        assert_eq!(
            budget_in(dir.path(), Some("alpha"), harness),
            harness.skill_budget()
        );
    }
}

#[test]
fn a_projects_budget_overrides_every_harness_and_keeps_what_each_lists() {
    let dir = plane(PROJECT, "{}");
    let codex = budget_in(dir.path(), None, Harness::Codex);
    assert_eq!((codex.skills, codex.chars), (80, 20_000));
    assert!(codex.lists_paths, "what a harness lists stays its own");
}

#[test]
fn a_workspace_wins_key_by_key_and_only_in_that_workspace() {
    let dir = plane(PROJECT, r#"{"skills":{"budget":{"skills":12}}}"#);
    let here = budget_in(dir.path(), Some("alpha"), Harness::ClaudeCode);
    assert_eq!((here.skills, here.chars), (12, 20_000));
    let root = budget_in(dir.path(), None, Harness::ClaudeCode);
    assert_eq!((root.skills, root.chars), (80, 20_000));
}

#[test]
fn a_value_no_reader_takes_leaves_the_number_below_it() {
    let dir = plane("[skills.budget]\nchars = 0\nskills = \"many\"\n", "{}");
    assert_eq!(set_in(dir.path(), None), Set::default());
}

#[test]
fn a_raised_claude_code_budget_raises_what_a_chat_is_handed() {
    let dir = plane(PROJECT, "{}");
    let budget = budget_in(dir.path(), None, Harness::ClaudeCode);
    assert_eq!(Harness::ClaudeCode.skill_limit_handed(budget), Some(29_532));
}

#[test]
fn a_value_past_a_harness_limit_warns_and_is_not_refused() {
    let set = Set {
        skills: None,
        chars: Some(15_000),
    };
    let said = warnings(set);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].starts_with("Codex"), "{said:?}");
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        keyed_at(
            dir.path(),
            "[skills.budget]\nchars = 15000000\n",
            Which::Shared
        ),
        Vec::new()
    );
}

#[test]
fn nothing_written_warns_of_nothing() {
    assert_eq!(warnings(Set::default()), Vec::<String>::new());
}

#[test]
fn a_shared_file_holds_a_whole_number_budget_and_nothing_else_under_skills() {
    let dir = tempfile::tempdir().unwrap();
    let keys = |text: &str, which| {
        keyed_at(dir.path(), text, which)
            .into_iter()
            .map(|one| one.key.unwrap_or_default().join("."))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        keys("[skills.budget]\nchars = 0\n", Which::Shared),
        ["skills.budget.chars"]
    );
    assert_eq!(
        keys("[skills.budget]\nfast = 1\n", Which::Shared),
        ["skills.budget.fast"]
    );
    assert_eq!(keys("[skills]\noff = []\n", Which::Shared), ["skills.off"]);
    assert_eq!(
        keys("[skills.budget]\nchars = 9\n", Which::Local),
        ["skills"]
    );
}

#[test]
fn a_saved_project_budget_reaches_the_reader() {
    let dir = plane("", "{}");
    let base = fs::read_to_string(dir.path().join("charter.toml")).unwrap();
    crate::settings::save(
        dir.path(),
        Which::Shared,
        Some(&base),
        &format!("{base}{PROJECT}"),
    )
    .expect("a save the file's readers take");
    assert_eq!(set_in(dir.path(), None).skills, Some(80));
}
