// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for a change to several skills at once (#5331): each on a temporary plane that is a
//! real git repository, read back through its commits, the Deleted list and the lockfile.

use std::path::Path;

use super::*;
use crate::repocmd::Say;
use crate::settings::Which;
use crate::skills::source::lock::{AtLevel, Kept, Locked, Lockfile, Wrote};
use crate::skills::source::{Commit, Remote};
use crate::skills::verbs::{By, restore};
use crate::testgit::run;

const TO_SPEC: &str =
    "---\nname: to-spec\ndescription: Writes a spec.\n---\n\nCall skills:grilling first.\n";

fn skill_text(name: &str) -> String {
    format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nbody\n")
}

fn remote() -> Remote {
    Remote {
        url: "https://example.invalid/acme/skills".to_owned(),
        branch: None,
    }
}

fn write(root: &Path, level: &str, name: &str, text: &str) {
    let dir = root.join(level).join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), text).unwrap();
}

/// The lockfile entry of `name`, installed from [`remote`] as `text`.
fn locked(level: Level, name: &str, text: &str) -> Locked {
    let sha = crate::skills::source::review::sha256(text.as_bytes());
    Locked {
        name: name.to_owned(),
        source_name: None,
        level: AtLevel(level),
        source: remote(),
        folder: format!("skills/{name}"),
        commit: Commit::parse(&"a".repeat(40)).unwrap(),
        digest: "sha256:0".to_owned(),
        files: vec![Wrote {
            path: "SKILL.md".to_owned(),
            executable: false,
            sha256: sha,
        }],
        brought_by: None,
    }
}

/// A plane that saves by committing: `grilling`, `tdd` and `review` installed from [`remote`] at
/// the project's level, `to-spec` the project's own and naming `grilling`, a workspace `ops`, and
/// `grilling` installed at the project's local level too.
fn plane() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[plane]\nmode = \"commit\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join(".gitignore"),
        ".charter/\n.purlis/\nskills.local/\nskills.local.lock\ncharter.local.toml\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    std::fs::write(root.join("workspaces/ops/workspace.md"), "ops\n").unwrap();
    let mut shared = Lockfile::default();
    for name in ["grilling", "tdd", "review"] {
        write(root, "skills", name, &skill_text(name));
        shared
            .skills
            .push(locked(Level::project(), name, &skill_text(name)));
    }
    write(root, "skills", "to-spec", TO_SPEC);
    lock::write(root, Kept::Shared, None, &shared).unwrap();
    write(root, "skills.local", "grilling", &skill_text("grilling"));
    let local = Lockfile {
        skills: vec![locked(
            Level::project().local(true),
            "grilling",
            &skill_text("grilling"),
        )],
        ..Lockfile::default()
    };
    lock::write(root, Kept::Local, None, &local).unwrap();
    run(root, &["init", "-q", "-b", "main", "."]);
    run(root, &["config", "user.name", "Fixture"]);
    run(root, &["config", "user.email", "fixture@example.invalid"]);
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "one"]);
    tmp
}

fn output(root: &Path, args: &[&str]) -> String {
    git(root, args).unwrap()
}

fn did<T>(root: &Path, change: impl FnOnce(&mut Ctx<'_>) -> T) -> T {
    let mut say = |_: Say| {};
    change(&mut Ctx {
        plane: root,
        built_in: None,
        by: By::Window,
        say: &mut say,
    })
}

fn project(name: &str) -> Member {
    Member {
        level: Level::project(),
        name: name.to_owned(),
    }
}

fn commits(root: &Path) -> usize {
    output(root, &["rev-list", "--count", "HEAD"])
        .trim()
        .parse()
        .unwrap()
}

fn subject(root: &Path) -> String {
    output(root, &["log", "-1", "--format=%s"])
        .trim()
        .to_owned()
}

fn there(root: &Path, level: &str, name: &str) -> bool {
    root.join(level).join(name).join("SKILL.md").exists()
}

fn deleted_named(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = deleted(root).into_iter().map(|one| one.name).collect();
    names.sort();
    names
}

#[test]
fn deleting_a_set_is_one_commit_and_every_skill_of_it_is_listed_as_one_group() {
    let tmp = plane();
    let root = tmp.path();
    let before = commits(root);
    let gone = did(root, |cx| {
        delete_set(
            cx,
            &[project("grilling"), project("tdd")],
            Some("acme/skills"),
            None,
        )
    })
    .unwrap();
    assert_eq!(commits(root), before + 1, "one commit, one History entry");
    assert!(
        subject(root).contains("skill rm 2 from acme/skills"),
        "{}",
        subject(root)
    );
    assert!(!there(root, "skills", "grilling") && !there(root, "skills", "tdd"));
    assert!(there(root, "skills", "review"), "what was not named stays");
    assert_eq!(gone.len(), 2);
    let listed = deleted(root);
    assert_eq!(listed.len(), 2);
    let set = listed[0].set.clone().expect("it is a group");
    assert_eq!(set.from.as_deref(), Some("acme/skills"));
    assert_eq!(listed[1].set.as_ref(), Some(&set), "the same group");
}

#[test]
fn a_set_past_the_most_deleted_is_listed_whole_so_it_can_come_back_whole() {
    let tmp = plane();
    let root = tmp.path();
    let mut members = Vec::new();
    for at in 0..40 {
        let name = format!("many-{at}");
        write(root, "skills", &name, &skill_text(&name));
        members.push(project(&name));
    }
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "many"]);
    did(root, |cx| delete_set(cx, &members, None, None)).unwrap();
    assert_eq!(deleted(root).len(), 40);
    let before = commits(root);
    did(root, |cx| restore_set(cx, &deleted(root), None)).unwrap();
    assert_eq!(commits(root), before + 1);
    assert!(there(root, "skills", "many-0") && there(root, "skills", "many-39"));
    assert!(deleted(root).is_empty());
}

#[test]
fn a_set_that_cannot_all_go_changes_nothing() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::write(
        root.join("skills/tdd/SKILL.md"),
        "---\nname: tdd\ndescription: Edited, not committed.\n---\n",
    )
    .unwrap();
    let before = commits(root);
    let refused = did(root, |cx| {
        delete_set(cx, &[project("grilling"), project("tdd")], None, None)
    })
    .unwrap_err();
    assert!(
        refused.contains("tdd") && refused.contains("not committed"),
        "{refused}"
    );
    assert_eq!(commits(root), before);
    assert!(there(root, "skills", "grilling"), "nothing was written");
    let status = output(root, &["status", "--porcelain", "--", "skills/grilling"]);
    assert_eq!(status.trim(), "");
    let missing = did(root, |cx| {
        delete_set(cx, &[project("grilling"), project("no-such")], None, None)
    });
    assert!(missing.is_err());
    assert!(there(root, "skills", "grilling"));
}

#[test]
fn a_commit_that_cannot_land_puts_every_skill_back_with_those_kept_on_this_machine() {
    let tmp = plane();
    let root = tmp.path();
    // The save signs and the signer refuses: the removal is made, and its commit cannot be.
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[plane]\nmode = \"commit\"\nsign = true\n",
    )
    .unwrap();
    run(root, &["add", "charter.toml"]);
    run(root, &["commit", "-q", "-m", "signs"]);
    run(root, &["config", "gpg.program", "false"]);
    let before = commits(root);
    let local = Member {
        level: Level::project().local(true),
        name: "grilling".to_owned(),
    };
    let refused = did(root, |cx| {
        delete_set(
            cx,
            &[project("grilling"), project("tdd"), local],
            None,
            None,
        )
    });
    assert!(refused.is_err());
    assert_eq!(commits(root), before);
    assert!(there(root, "skills", "grilling") && there(root, "skills", "tdd"));
    assert!(
        there(root, "skills.local", "grilling"),
        "the local one came back too"
    );
    assert!(deleted(root).is_empty(), "{:?}", deleted(root));
    assert_eq!(output(root, &["status", "--porcelain"]).trim(), "");
}

#[test]
fn a_set_across_shared_and_local_levels_is_one_group_and_comes_back_whole() {
    let tmp = plane();
    let root = tmp.path();
    let local = Member {
        level: Level::project().local(true),
        name: "grilling".to_owned(),
    };
    let before = commits(root);
    did(root, |cx| {
        delete_set(cx, &[project("grilling"), local], Some("acme/skills"), None)
    })
    .unwrap();
    assert_eq!(commits(root), before + 1);
    let listed = deleted(root);
    assert_eq!(listed.len(), 2);
    assert!(listed[0].set.is_some());
    assert_eq!(listed[0].set, listed[1].set);
    did(root, |cx| restore_set(cx, &listed, None)).unwrap();
    assert!(there(root, "skills", "grilling") && there(root, "skills.local", "grilling"));
}

#[test]
fn one_skill_of_a_set_comes_back_alone_and_the_rest_stay_in_their_group() {
    let tmp = plane();
    let root = tmp.path();
    did(root, |cx| {
        delete_set(
            cx,
            &[project("grilling"), project("tdd"), project("review")],
            None,
            None,
        )
    })
    .unwrap();
    let tdd = deleted(root)
        .into_iter()
        .find(|one| one.name == "tdd")
        .unwrap();
    did(root, |cx| restore(cx, &tdd, None)).unwrap();
    assert!(there(root, "skills", "tdd"));
    assert!(!there(root, "skills", "grilling"));
    let left = deleted(root);
    assert_eq!(deleted_named(root), ["grilling", "review"]);
    assert_eq!(left[0].set, left[1].set);
    assert!(left[0].set.is_some());
}

#[test]
fn a_set_that_cannot_all_come_back_brings_none_back() {
    let tmp = plane();
    let root = tmp.path();
    did(root, |cx| {
        delete_set(cx, &[project("grilling"), project("tdd")], None, None)
    })
    .unwrap();
    let listed = deleted(root);
    // One of them is no longer listed: a skill of its name is there again.
    write(root, "skills", "tdd", &skill_text("tdd"));
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "tdd again"]);
    let before = commits(root);
    assert!(did(root, |cx| restore_set(cx, &listed, None)).is_err());
    assert_eq!(commits(root), before);
    assert!(!there(root, "skills", "grilling"));
}

#[test]
fn removing_the_source_with_its_skills_is_the_same_commit_and_undo_brings_both_back() {
    let tmp = plane();
    let root = tmp.path();
    let source = crate::skills::source::Source {
        remote: remote(),
        folder: None,
        sign_in: None,
    };
    did(root, |cx| {
        crate::skills::source::add(cx, &source, Which::Shared)
    })
    .unwrap();
    let before = commits(root);
    did(root, |cx| {
        delete_set(
            cx,
            &[project("grilling")],
            Some("acme/skills"),
            Some(&remote()),
        )
    })
    .unwrap();
    assert_eq!(commits(root), before + 1);
    assert!(crate::skills::source::sources(root).unwrap().is_empty());
    assert_eq!(output(root, &["status", "--porcelain"]).trim(), "");
    did(root, |cx| {
        restore_set(cx, &deleted(root), Some((&source, Which::Shared)))
    })
    .unwrap();
    assert_eq!(commits(root), before + 2);
    assert!(there(root, "skills", "grilling"));
    assert_eq!(crate::skills::source::sources(root).unwrap().len(), 1);
    assert_eq!(output(root, &["status", "--porcelain"]).trim(), "");
}

#[test]
fn a_skill_deleted_is_no_longer_installed_and_comes_back_pinned_as_it_was() {
    let tmp = plane();
    let root = tmp.path();
    assert_eq!(installed_from(root, &remote(), None).len(), 4);
    did(root, |cx| {
        delete_set(cx, &[project("grilling")], None, None)
    })
    .unwrap();
    let names: Vec<String> = installed_from(root, &remote(), None)
        .into_iter()
        .filter(|one| !one.member.level.local)
        .map(|one| one.member.name)
        .collect();
    assert_eq!(names, ["review", "tdd"]);
    // Every reader of the lockfiles agrees: what the Sources page counts, too.
    let (every, _) = lock::every(root);
    assert!(
        !every
            .iter()
            .any(|(_, one)| one.is(&Level::project(), "grilling"))
    );
    // The pin is left, so one that comes back is pinned as it was.
    let held = lock::read(root, Kept::Shared).unwrap();
    assert!(held.lock.skills.iter().any(|one| one.name == "grilling"));
    did(root, |cx| restore_set(cx, &deleted(root), None)).unwrap();
    assert_eq!(installed_from(root, &remote(), None).len(), 4);
}

#[test]
fn what_is_installed_from_a_source_says_each_level_and_what_changed_since_install() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::write(
        root.join("skills/tdd/SKILL.md"),
        "---\nname: tdd\ndescription: Mine.\n---\n",
    )
    .unwrap();
    let copies = installed_from(root, &remote(), None);
    let of = |level: &Level, name: &str| {
        copies
            .iter()
            .find(|one| one.member.level == *level && one.member.name == name)
            .unwrap()
    };
    assert!(of(&Level::project(), "tdd").changed);
    assert!(!of(&Level::project(), "review").changed);
    assert!(!of(&Level::project().local(true), "grilling").changed);
    assert!(
        !copies.iter().any(|one| one.member.name == "to-spec"),
        "its own is not the source's"
    );
    assert_eq!(
        of(&Level::project(), "review").on_here,
        None,
        "no workspace is asked of"
    );
}

#[test]
fn the_review_warns_of_a_skill_left_naming_one_that_goes_and_says_what_could_come_too() {
    let tmp = plane();
    let root = tmp.path();
    // The local copy of the name would still answer, so it is taken out of the way.
    std::fs::remove_dir_all(root.join("skills.local/grilling")).unwrap();
    let review = review_set(root, Act::Delete, &[project("grilling")], None);
    assert_eq!(review.names.len(), 1);
    let one = &review.names[0];
    assert_eq!(one.namer, "to-spec");
    assert_eq!(one.named, "grilling");
    assert_eq!(one.add, project("to-spec"), "the skill the set can take in");
    // Added, nothing is left naming it.
    let both = review_set(
        root,
        Act::Delete,
        &[project("grilling"), project("to-spec")],
        None,
    );
    assert!(both.names.is_empty());
}

#[test]
fn a_skill_named_by_its_bare_name_in_quotes_is_warned_of_as_well() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::remove_dir_all(root.join("skills.local/grilling")).unwrap();
    write(
        root,
        "skills",
        "grill-with-docs",
        "---\nname: grill-with-docs\ndescription: Grills.\n---\n\nCall the Skill tool for \"grilling\".\n",
    );
    let review = review_set(root, Act::Delete, &[project("grilling")], None);
    let namers: Vec<&str> = review.names.iter().map(|one| one.namer.as_str()).collect();
    assert_eq!(namers, ["grill-with-docs", "to-spec"]);
    // A word in a sentence names nothing.
    write(
        root,
        "skills",
        "grill-with-docs",
        "---\nname: grill-with-docs\ndescription: Grills.\n---\n\nStop grilling people.\n",
    );
    let review = review_set(root, Act::Delete, &[project("grilling")], None);
    let namers: Vec<&str> = review.names.iter().map(|one| one.namer.as_str()).collect();
    assert_eq!(namers, ["to-spec"]);
}

#[test]
fn another_copy_of_the_name_that_stays_answers_so_no_warning_is_given() {
    let tmp = plane();
    let root = tmp.path();
    let review = review_set(root, Act::Delete, &[project("grilling")], None);
    assert!(review.names.is_empty(), "{:?}", review.names);
}

#[test]
fn the_review_marks_a_skill_that_cannot_apply_and_says_what_changed_since_install() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::write(
        root.join("skills/tdd/SKILL.md"),
        "---\nname: tdd\ndescription: Edited, not committed.\n---\n",
    )
    .unwrap();
    let review = review_set(
        root,
        Act::Delete,
        &[project("tdd"), project("review"), project("gone")],
        None,
    );
    let of = |name: &str| {
        review
            .skills
            .iter()
            .find(|one| one.member.name == name)
            .unwrap()
    };
    assert!(
        of("tdd")
            .cannot
            .as_deref()
            .is_some_and(|why| why.contains("not committed"))
    );
    assert!(of("tdd").changed);
    assert_eq!(of("review").cannot, None);
    assert!(of("gone").cannot.is_some());
}

#[test]
fn turning_a_set_off_is_one_commit_and_turning_it_on_takes_back_what_it_turned_off() {
    let tmp = plane();
    let root = tmp.path();
    for name in ["grilling", "tdd", "review"] {
        crate::skills::grant(root, name, &crate::skills::Grant::All).unwrap();
    }
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "granted"]);
    let before = commits(root);
    let changed = did(root, |cx| {
        turn_set(cx, "ops", &["grilling".into(), "tdd".into()], false)
    })
    .unwrap();
    assert_eq!(changed, ["grilling", "tdd"]);
    assert_eq!(commits(root), before + 1);
    assert_eq!(crate::skills::turned_off(root, "ops"), ["grilling", "tdd"]);
    let copies = installed_from(root, &remote(), Some("ops"));
    let on = |name: &str| {
        copies
            .iter()
            .find(|one| one.member.name == name && !one.member.level.local)
            .unwrap()
            .on_here
    };
    assert_eq!(on("tdd"), Some(false));
    assert_eq!(on("review"), Some(true));
    // One already so is not counted, so an Undo leaves it as it was.
    let changed = did(root, |cx| {
        turn_set(cx, "ops", &["tdd".into(), "review".into()], false)
    })
    .unwrap();
    assert_eq!(changed, ["review"]);
    did(root, |cx| turn_set(cx, "ops", &changed, true)).unwrap();
    assert_eq!(crate::skills::turned_off(root, "ops"), ["grilling", "tdd"]);
}

#[test]
fn a_set_one_of_which_cannot_turn_changes_nothing() {
    let tmp = plane();
    let root = tmp.path();
    crate::skills::grant(root, "tdd", &crate::skills::Grant::All).unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "granted"]);
    let before = commits(root);
    assert!(
        did(root, |cx| {
            turn_set(cx, "ops", &["tdd".into(), "no-such".into()], false)
        })
        .is_err()
    );
    assert_eq!(commits(root), before);
    assert!(crate::skills::turned_off(root, "ops").is_empty());
}
