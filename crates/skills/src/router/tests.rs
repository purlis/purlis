// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The built-in router (SK-15, #2778; ADR 0102 §2, §4, §7), on sets worked out from planes in a
//! temporary directory: everything below the budget, the pins and the index above it, and a
//! reason for every skill it hides.

use std::path::Path;

use super::*;
use crate::{Chat, HarnessSkills, Machine, PROJECT_DIR, Reason, Route, SkillSet, skill_set};

fn skill(dir: &Path, name: &str, description: &str, more: &str) {
    std::fs::create_dir_all(dir.join(name)).unwrap();
    std::fs::write(
        dir.join(name).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n{more}---\n\nbody\n"),
    )
    .unwrap();
}

/// The set of a chat at the root of the plane at `plane` on `route`, with the built-in skills in
/// `built_in`.
fn set_on(plane: &Path, built_in: &Path, route: Route) -> SkillSet {
    skill_set(
        Some(plane),
        &Machine {
            built_in: Some(built_in),
            kept_local: Some(&|_| Ok(())),
            harness: None,
            made_since: None,
        },
        &Chat {
            workspace: None,
            persona: None,
            names: &[],
            turned_off: &[],
            harness: HarnessSkills { route },
        },
    )
}

fn set_at_the_root(plane: &Path, built_in: &Path) -> SkillSet {
    set_on(plane, built_in, Route::Plugin)
}

/// A budget of `skills` skills and plenty of characters.
fn skills(skills: usize) -> Budget {
    Budget {
        skills,
        chars: 100_000,
        per_description: None,
        lists_paths: false,
    }
}

fn names<'a>(set: impl IntoIterator<Item = &'a crate::Effective>) -> Vec<&'a str> {
    set.into_iter().map(|e| e.skill.name.as_str()).collect()
}

/// A plane with one built-in skill and the project skills `deploy`, `lint` and `release`.
fn three(tmp: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let built_in = tmp.join("bundle");
    let plane = tmp.join("plane");
    skill(&built_in, "handoff", "Hands off.", "");
    for name in ["deploy", "lint", "release"] {
        skill(&plane.join(PROJECT_DIR), name, &format!("Does {name}."), "");
    }
    (plane, built_in)
}

#[test]
fn below_the_budget_every_skill_is_listed_and_there_is_no_index() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let set = set_at_the_root(&plane, &built_in).routed(skills(4), &[]);

    let routing = set.routing.as_ref().expect("routed");
    assert_eq!(routing.router, Router::BuiltIn);
    assert!(routing.hidden.is_empty());
    assert_eq!(
        names(set.listed()),
        ["deploy", "handoff", "lint", "release"]
    );
    assert_eq!(set.index(), None);
}

#[test]
fn above_the_budget_the_plane_skills_are_hidden_behind_the_index_and_stay_in_the_set() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let set = set_at_the_root(&plane, &built_in).routed(skills(3), &[]);

    // The built-in skill is never hidden: it comes with purlis's own plugin on every route.
    assert_eq!(names(set.listed()), ["handoff"]);
    assert_eq!(names(set.from_the_plane()), Vec::<&str>::new());
    // Hidden, never left out: the chat still has each one, for `skill_find` and `skill_load`.
    assert_eq!(
        names(&set.effective),
        ["deploy", "handoff", "lint", "release"]
    );
    assert!(set.left_out.is_empty());
    let hidden = set.hidden_by_router("deploy").expect("hidden");
    assert_eq!(hidden.why, HiddenWhy::OverBudget);
    assert_eq!(
        hidden.to_string(),
        "hidden by the built-in router: over this chat's skill budget, so it is in the index skill"
    );
    let index = set.index().expect("an index");
    assert!(index.text.starts_with(&format!("---\nname: {INDEX}\n")));
    for name in ["deploy", "lint", "release"] {
        assert!(
            index.text.contains(&format!("- `{name}`: Does {name}.")),
            "{name} in {}",
            index.text
        );
    }
    assert!(!index.text.contains("`handoff`"));
    assert!(index.text.contains("skill_load"));
}

#[test]
fn over_the_budget_by_characters_alone_hides_too() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    let budget = Budget {
        skills: 30,
        chars: 20,
        per_description: None,
        lists_paths: false,
    };

    let set = set_at_the_root(&plane, &built_in).routed(budget, &[]);

    assert_eq!(names(set.listed()), ["handoff"]);
}

#[test]
fn a_pinned_skill_is_never_hidden_and_counts_first() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let set = set_at_the_root(&plane, &built_in).routed(skills(3), &["lint".to_owned()]);

    assert_eq!(names(set.listed()), ["handoff", "lint"]);
    let routing = set.routing.as_ref().expect("routed");
    assert_eq!(routing.pinned, ["lint"]);
    assert!(!routing.pins_over);
    assert!(set.is_pinned("lint"));
    assert!(!set.index().expect("an index").text.contains("`lint`"));
}

#[test]
fn pins_over_the_budget_are_all_listed_and_say_so() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    let pins = ["deploy".to_owned(), "lint".to_owned(), "release".to_owned()];

    let set = set_at_the_root(&plane, &built_in).routed(skills(2), &pins);

    assert_eq!(
        names(set.listed()),
        ["deploy", "handoff", "lint", "release"]
    );
    assert!(set.routing.as_ref().expect("routed").pins_over);
    assert_eq!(set.index(), None, "nothing is hidden, so no index");
}

#[test]
fn a_pin_naming_no_skill_of_the_set_adds_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let set = set_at_the_root(&plane, &built_in).routed(skills(3), &["ghost".to_owned()]);

    assert!(set.routing.as_ref().expect("routed").pinned.is_empty());
    assert!(set.effective.iter().all(|e| e.skill.name != "ghost"));
}

#[test]
fn a_manual_only_skill_is_never_hidden_since_it_costs_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    skill(
        &plane.join(PROJECT_DIR),
        "by-hand",
        "Only when asked.",
        "disable-model-invocation: true\n",
    );

    let set = set_at_the_root(&plane, &built_in).routed(skills(3), &[]);

    assert_eq!(names(set.listed()), ["by-hand", "handoff"]);
    assert!(!set.index().expect("an index").text.contains("by-hand"));
}

#[test]
fn a_deprecated_skill_is_indexed_after_the_rest_with_its_replacement() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    std::fs::write(
        plane.join(PROJECT_DIR).join("deploy.toml"),
        "lifecycle = \"deprecated\"\nreplacement = \"release\"\n",
    )
    .unwrap();
    let set = set_at_the_root(&plane, &built_in);
    assert!(
        set.effective
            .iter()
            .any(|e| e.skill.name == "deploy" && e.deprecated.is_some()),
        "the plane marks deploy deprecated"
    );

    let index = set.routed(skills(3), &[]).index().expect("an index").text;

    let deploy = index.find("- `deploy`").expect("deploy");
    assert!(index.find("- `release`").expect("release") < deploy);
    assert!(index[deploy..].contains("deprecated: use `release` instead"));
}

#[test]
fn a_description_is_one_line_in_the_index_however_it_is_written() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    let long = "word ".repeat(60);
    skill(
        &plane.join(PROJECT_DIR),
        "wordy",
        &format!("First\t\u{1b}  second {long}"),
        "",
    );

    let index = set_at_the_root(&plane, &built_in)
        .routed(skills(3), &[])
        .index()
        .expect("an index")
        .text;

    let line = index
        .lines()
        .find(|line| line.starts_with("- `wordy`"))
        .expect("wordy");
    assert!(line.starts_with("- `wordy`: First second word"), "{line}");
    assert!(line.ends_with('…'), "{line}");
    assert!(line.chars().count() <= "- `wordy`: ".len() + MOST_LINE_CHARS + 1);
}

#[test]
fn a_chat_reached_only_through_purlis_tools_lists_nothing_so_nothing_is_hidden() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let set = set_on(&plane, &built_in, Route::Mcp).routed(skills(1), &[]);

    assert!(set.routing.as_ref().expect("routed").hidden.is_empty());
    assert_eq!(set.index(), None);
}

#[test]
fn a_plane_skill_named_as_the_index_never_loads() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    skill(&plane.join(PROJECT_DIR), INDEX, "Mine.", "");

    let set = set_at_the_root(&plane, &built_in);

    assert!(set.effective.iter().all(|e| e.skill.name != INDEX));
    let left = set
        .left_out
        .iter()
        .find(|l| l.name == INDEX)
        .expect("left out");
    assert_eq!(left.reason, Reason::IndexName);
    assert_eq!(
        left.reason.to_string(),
        "purlis's index skill has this name, so this skill never loads: rename it"
    );
}

#[test]
fn the_index_costs_one_skill_and_its_description_of_what_is_left_listed() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    // Room for the built-in skill and one pin, and no more: with the index it is over.
    let set = set_at_the_root(&plane, &built_in).routed(skills(2), &["lint".to_owned()]);

    let routing = set.routing.as_ref().expect("routed");
    assert!(routing.pins_over, "handoff, lint and the index are three");
    assert_eq!(names(set.listed()), ["handoff", "lint"]);
    assert!(set.index().is_some());
}
