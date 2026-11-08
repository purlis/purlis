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
    assert_eq!(
        budget_in(dir.path(), None, Harness::Codex),
        Harness::Codex.skill_budget()
    );
}

#[test]
fn a_value_past_a_harness_limit_warns_and_is_not_refused() {
    let written = plane("[skills.budget]\nchars = 15000\n", "{}");
    let said = warnings(written.path(), None);
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
    let dir = plane("", "{}");
    assert_eq!(warnings(dir.path(), None), Vec::<String>::new());
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
    assert_eq!(budget_in(dir.path(), None, Harness::Codex).skills, 80);
}

// ---- #5353: a number per program, where each comes from, and what a change would do.

#[test]
fn a_programs_own_number_wins_for_that_program_alone() {
    let dir = plane(
        "[skills.budget]\nskills = 80\n[skills.budget.codex]\nskills = 9\n",
        "{}",
    );
    assert_eq!(budget_in(dir.path(), None, Harness::Codex).skills, 9);
    assert_eq!(budget_in(dir.path(), None, Harness::ClaudeCode).skills, 80);
    assert_eq!(budget_in(dir.path(), None, Harness::Opencode).skills, 80);
}

#[test]
fn a_workspaces_number_for_every_program_wins_over_the_projects_for_one() {
    let dir = plane(
        "[skills.budget.codex]\nskills = 9\n",
        r#"{"skills":{"budget":{"skills":12,"opencode":{"skills":3}}}}"#,
    );
    let in_it = |harness| budget_in(dir.path(), Some("alpha"), harness).skills;
    assert_eq!(in_it(Harness::Codex), 12, "the narrower level wins");
    assert_eq!(in_it(Harness::Opencode), 3, "its own beats its every");
    assert_eq!(budget_in(dir.path(), None, Harness::Codex).skills, 9);
}

#[test]
fn a_program_number_no_reader_takes_leaves_the_number_below_it() {
    let dir = plane(
        "[skills.budget]\nskills = 80\n[skills.budget.codex]\nskills = 0\n",
        "{}",
    );
    assert_eq!(budget_in(dir.path(), None, Harness::Codex).skills, 80);
}

#[test]
fn a_save_takes_a_programs_table_and_refuses_what_is_wrong_inside_it_or_an_unknown_program() {
    let dir = tempfile::tempdir().unwrap();
    let keys = |text: &str| {
        keyed_at(dir.path(), text, Which::Shared)
            .into_iter()
            .map(|one| one.key.unwrap_or_default().join("."))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        keys("[skills.budget.claude]\nskills = 100\nchars = 9\n"),
        Vec::<String>::new()
    );
    assert_eq!(
        keys("[skills.budget.codex]\nskills = 0\n"),
        ["skills.budget.codex.skills"]
    );
    assert_eq!(
        keys("[skills.budget.codex]\nfast = 1\n"),
        ["skills.budget.codex.fast"]
    );
    assert_eq!(
        keys("[skills.budget.emacs]\nskills = 1\n"),
        ["skills.budget.emacs"]
    );
    assert_eq!(
        keys("[skills.budget]\ncodex = 3\n"),
        ["skills.budget.codex"]
    );
}

#[test]
fn only_codex_warns_and_only_at_its_own_number() {
    let dir = plane("[skills.budget.claude]\nchars = 90000\n", "{}");
    assert_eq!(warnings(dir.path(), None), Vec::<String>::new());
    let dir = plane("[skills.budget.codex]\nchars = 9000\n", "{}");
    let said = warnings(dir.path(), None);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].starts_with("Codex"), "{said:?}");
}

#[test]
fn in_force_says_the_number_the_workspace_writes_with_what_the_project_and_the_program_say() {
    let dir = plane(
        "[skills.budget.claude]\nskills = 70\n",
        r#"{"skills":{"budget":{"claude":{"skills":100}}}}"#,
    );
    let here = in_force(dir.path(), Some("alpha"), Harness::ClaudeCode);
    assert_eq!(here.skills.in_force, 100);
    assert_eq!(here.skills.from, Written::Workspace);
    assert_eq!(here.skills.here, Some(100));
    assert_eq!(
        here.skills.said(Some("alpha"), Harness::ClaudeCode),
        "100, this workspace · project says 70 · Claude Code's default is 60"
    );
    assert_eq!(here.chars.from, Written::Default);
    assert_eq!(
        here.chars.said(Some("alpha"), Harness::ClaudeCode),
        "15,000, Claude Code's default"
    );
}

#[test]
fn in_force_says_a_number_the_workspace_takes_from_the_project_and_that_it_writes_nothing() {
    let dir = plane("[skills.budget]\nskills = 80\n", "{}");
    let here = in_force(dir.path(), Some("alpha"), Harness::Codex);
    assert_eq!(here.skills.here, None, "nothing to reset in this workspace");
    assert_eq!(
        here.skills.said(Some("alpha"), Harness::Codex),
        "80, the project, on every program · Codex's default is 30"
    );
    let root = in_force(dir.path(), None, Harness::Codex);
    assert_eq!(root.skills.here, Some(80));
    assert_eq!(
        root.skills.said(None, Harness::Codex),
        "80, this project, on every program · Codex's default is 30"
    );
}

#[test]
fn a_write_at_the_workspace_reaches_that_program_alone_and_empty_takes_it_out() {
    let dir = plane(PROJECT, "{}");
    let at = Level::Workspace("alpha");
    let wanted = Set {
        skills: Some(100),
        chars: None,
    };
    assert!(write(dir.path(), at, Harness::ClaudeCode, wanted).unwrap());
    assert_eq!(
        budget_in(dir.path(), Some("alpha"), Harness::ClaudeCode).skills,
        100
    );
    assert_eq!(
        budget_in(dir.path(), Some("alpha"), Harness::Codex).skills,
        80
    );
    assert!(
        !write(dir.path(), at, Harness::ClaudeCode, wanted).unwrap(),
        "unchanged"
    );

    assert!(write(dir.path(), at, Harness::ClaudeCode, Set::default()).unwrap());
    assert_eq!(
        budget_in(dir.path(), Some("alpha"), Harness::ClaudeCode).skills,
        80
    );
    let manifest = fs::read_to_string(dir.path().join("workspaces/alpha/workspace.json")).unwrap();
    assert!(
        !manifest.contains("claude"),
        "nothing left behind: {manifest}"
    );
}

#[test]
fn a_write_at_the_project_keeps_the_file_around_it() {
    let dir = plane("# ours\n[skills.budget.codex]\nskills = 9\n", "{}");
    let wanted = Set {
        skills: None,
        chars: Some(20_000),
    };
    assert!(write(dir.path(), Level::Project, Harness::Opencode, wanted).unwrap());
    let text = fs::read_to_string(dir.path().join("charter.toml")).unwrap();
    assert!(text.contains("# ours"), "{text}");
    assert_eq!(budget_in(dir.path(), None, Harness::Opencode).chars, 20_000);
    assert_eq!(budget_in(dir.path(), None, Harness::Codex).skills, 9);
    write(dir.path(), Level::Project, Harness::Codex, Set::default()).unwrap();
    write(
        dir.path(),
        Level::Project,
        Harness::Opencode,
        Set::default(),
    )
    .unwrap();
    let text = fs::read_to_string(dir.path().join("charter.toml")).unwrap();
    assert!(!text.contains("skills"), "no empty table left: {text}");
}

#[test]
fn resetting_one_program_under_a_number_for_every_program_leaves_the_others_theirs() {
    let dir = plane(PROJECT, "{}");
    assert!(
        write(
            dir.path(),
            Level::Project,
            Harness::ClaudeCode,
            Set::default()
        )
        .unwrap()
    );
    let budget = |harness| budget_in(dir.path(), None, harness);
    assert_eq!(
        budget(Harness::ClaudeCode).skills,
        60,
        "back to its default"
    );
    assert_eq!(budget(Harness::ClaudeCode).chars, 15_000);
    assert_eq!(
        (budget(Harness::Codex).skills, budget(Harness::Codex).chars),
        (80, 20_000)
    );
    assert_eq!(
        (
            budget(Harness::Opencode).skills,
            budget(Harness::Opencode).chars
        ),
        (80, 20_000)
    );
}

#[test]
fn a_program_already_on_its_own_number_keeps_it_when_the_every_program_number_is_unfolded() {
    let dir = plane(
        "[skills.budget]\nskills = 80\n[skills.budget.codex]\nskills = 9\n",
        "{}",
    );
    write(
        dir.path(),
        Level::Project,
        Harness::ClaudeCode,
        Set::default(),
    )
    .unwrap();
    assert_eq!(budget_in(dir.path(), None, Harness::Codex).skills, 9);
    assert_eq!(budget_in(dir.path(), None, Harness::Opencode).skills, 80);
}

#[test]
fn a_write_below_one_is_refused_and_a_huge_one_is_not() {
    let dir = plane("", "{}");
    let zero = Set {
        skills: Some(0),
        chars: None,
    };
    let why = write(dir.path(), Level::Project, Harness::Codex, zero).unwrap_err();
    assert!(why.contains("at least 1"), "{why}");
    let huge = Set {
        skills: Some(2_000_000),
        chars: Some(90_000_000),
    };
    assert!(write(dir.path(), Level::Project, Harness::Codex, huge).unwrap());
}

/// A plane with `count` project skills, each described by 100 characters.
fn with_skills(shared: &str, settings: &str, count: usize) -> tempfile::TempDir {
    skills_in(shared, settings, count, "skills")
}

fn skills_in(shared: &str, settings: &str, count: usize, under: &str) -> tempfile::TempDir {
    let dir = plane(shared, settings);
    for at in 0..count {
        let folder = dir.path().join(format!("{under}/skill-{at:02}"));
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("SKILL.md"),
            format!(
                "---\nname: skill-{at:02}\ndescription: {}\n---\nDo it.\n",
                "d".repeat(100)
            ),
        )
        .unwrap();
    }
    dir
}

#[test]
fn a_preview_names_what_would_be_hidden_and_the_fewer_tokens() {
    let dir = with_skills("", "{}", 8);
    let wanted = Set {
        skills: Some(4),
        chars: None,
    };
    let said = preview(None, dir.path(), None, None, Harness::ClaudeCode, wanted);
    assert_eq!(said.now_listed, Vec::<String>::new());
    assert_eq!(said.now_hidden.len(), 8, "{said:?}");
    assert!(said.more_tokens < 0, "{said:?}");
    assert_eq!(said.past_limit, None);
    assert_eq!(said.over, None);
}

#[test]
fn a_preview_names_what_would_now_be_listed_when_the_number_rises() {
    let dir = with_skills("[skills.budget.codex]\nskills = 4\n", "{}", 8);
    let wanted = Set {
        skills: Some(50),
        chars: None,
    };
    let said = preview(None, dir.path(), None, None, Harness::Codex, wanted);
    assert_eq!(said.now_listed.len(), 8, "{said:?}");
    assert_eq!(said.now_hidden, Vec::<String>::new());
    assert!(said.more_tokens > 0, "{said:?}");
}

#[test]
fn a_preview_with_nothing_typed_is_what_empty_would_save_the_number_below() {
    let dir = with_skills("[skills.budget.codex]\nskills = 4\n", "{}", 8);
    let said = preview(
        None,
        dir.path(),
        Some("alpha"),
        None,
        Harness::Codex,
        Set::default(),
    );
    assert_eq!(
        (
            said.now_listed.len(),
            said.now_hidden.len(),
            said.more_tokens
        ),
        (0, 0, 0)
    );
}

#[test]
fn a_preview_warns_past_codexs_limit_and_never_refuses() {
    let dir = with_skills("", "{}", 2);
    let wanted = Set {
        skills: None,
        chars: Some(9_000),
    };
    let said = preview(None, dir.path(), None, None, Harness::Codex, wanted);
    assert!(
        said.past_limit
            .is_some_and(|line| line.starts_with("Codex"))
    );
    let said = preview(None, dir.path(), None, None, Harness::ClaudeCode, wanted);
    assert_eq!(said.past_limit, None, "Claude Code is handed its limit");
}

#[test]
fn a_preview_below_the_pins_warns_that_they_are_still_listed() {
    let dir = skills_in(
        "",
        r#"{"skills":{"pinned":["skill-00","skill-01","skill-02"]}}"#,
        3,
        "workspaces/alpha/skills",
    );
    let wanted = Set {
        skills: Some(2),
        chars: None,
    };
    let said = preview(
        None,
        dir.path(),
        Some("alpha"),
        None,
        Harness::ClaudeCode,
        wanted,
    );
    assert!(
        said.over
            .as_deref()
            .is_some_and(|line| line.contains("3 pinned skills")),
        "{said:?}"
    );
}

#[test]
fn a_preview_below_the_pins_says_one_pin_as_one() {
    let dir = skills_in(
        "",
        r#"{"skills":{"pinned":["skill-00"]}}"#,
        3,
        "workspaces/alpha/skills",
    );
    let wanted = Set {
        skills: Some(1),
        chars: None,
    };
    let said = preview(
        None,
        dir.path(),
        Some("alpha"),
        None,
        Harness::ClaudeCode,
        wanted,
    );
    assert!(
        said.over
            .as_deref()
            .is_some_and(|line| line.starts_with("1 pinned skill is still")),
        "{said:?}"
    );
}

#[test]
fn a_preview_below_what_no_router_hides_still_names_the_pins() {
    let built_in = tempfile::tempdir().unwrap();
    for name in ["own-a", "own-b", "own-c"] {
        let folder = built_in.path().join(name);
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Ours.\n---\nDo it.\n"),
        )
        .unwrap();
    }
    let dir = skills_in(
        "",
        r#"{"skills":{"pinned":["skill-00","skill-01"]}}"#,
        2,
        "workspaces/alpha/skills",
    );
    let wanted = Set {
        skills: Some(1),
        chars: None,
    };
    let said = preview(
        Some(built_in.path()),
        dir.path(),
        Some("alpha"),
        None,
        Harness::ClaudeCode,
        wanted,
    );
    assert!(
        said.over
            .as_deref()
            .is_some_and(|line| line.contains("2 pinned skills")),
        "{said:?}"
    );
}

#[test]
fn raising_one_program_over_a_number_for_every_program_leaves_that_number_written() {
    let dir = plane(PROJECT, "{}");
    let wanted = Set {
        skills: Some(100),
        chars: Some(20_000),
    };
    write(dir.path(), Level::Project, Harness::ClaudeCode, wanted).unwrap();
    let text = fs::read_to_string(dir.path().join("charter.toml")).unwrap();
    assert!(
        !text.contains("codex"),
        "nothing moved for the others: {text}"
    );
    assert_eq!(budget_in(dir.path(), None, Harness::ClaudeCode).skills, 100);
    assert_eq!(budget_in(dir.path(), None, Harness::Codex).skills, 80);
}

#[test]
fn a_programs_number_changes_what_a_start_on_that_program_is_routed_by() {
    let dir = with_skills("[skills.budget.codex]\nskills = 2\n", "{}", 8);
    let at = crate::skills::PlaneSkills {
        plane: dir.path(),
        workspace: None,
        repo: &[],
        own: &[],
        routed: None,
    };
    let hidden = |harness| {
        super::super::routed(None, Some(at), None, harness)
            .routing
            .map_or(0, |routing| routing.hidden.len())
    };
    assert_eq!(hidden(Harness::Codex), 8, "over its own number");
    assert_eq!(
        hidden(Harness::ClaudeCode),
        0,
        "the same skills fit its default"
    );
}
