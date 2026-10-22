// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `project-skills` (SK-21, #2784): the doctor fix that moves a project's `.claude/skills` into
//! the Skill studio's `skills/`, granted to every workspace, in ONE commit, and retires the copy
//! of `.claude/skills` into clones. Driven through the fix's own entry point on a real git
//! repository with a real clone, as `purlis doctor --fix project-skills` drives it.
//!
//! The proof is the skill set of every chat: at the project's root and in a clone, on Claude
//! Code, Codex and opencode, before the move (the plane's skills read where each program reads
//! them, the clone's through purlis's copy) and after it (the project's skills, by each
//! program's route). Claude Code and opencode keep exactly the skills they had; Codex, which
//! never read `.claude/skills`, gains them.

mod support;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use purlis_core::compat::{self, Compat};
use purlis_core::doctor::fix::{Fixed, project_skills};
use purlis_core::guest;
use purlis_core::harness::Harness;
use purlis_core::skills::{self, Grant};

/// purlis's own built-in skills, as the bundle ships them.
fn built_in() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/src-tauri/plugin/skills")
}

const DEPLOY: &str = "---\nname: deploy\ndescription: Ship the service.\n---\nHand off with \
                      `charter:handoff` once it is live.\n";
const REVIEW: &str = "---\nname: review\ndescription: Review a change.\n---\nRead the notes.\n";
const NOTES: &str = "# Notes\n\nAsk charter:secrets for the token. charter: is prose.\n";
const PERSONA: &str = "---\nrole: Ops\nskills: charter:handoff, deploy, charter:nope\n---\nUse \
                       `charter:secrets` for tokens.\n";

/// A project whose skills are in `.claude/skills`, with a persona naming them the old way,
/// committed, and a clone in its workspace `alpha` that purlis's copy reached.
fn project() -> support::Fixture {
    let f = support::plane_with_clone("svc");
    let files: [(&str, &str); 6] = [
        (".gitignore", "/workspaces/*/*\n/.purlis/\n/.charter/\n"),
        ("workspaces/alpha/workspace.json", "{\"name\": \"alpha\", \"repos\": []}\n"),
        (".claude/skills/deploy/SKILL.md", DEPLOY),
        (".claude/skills/review/SKILL.md", REVIEW),
        (".claude/skills/review/notes/token.md", NOTES),
        ("personas/ops/persona.md", PERSONA),
    ];
    for (rel, text) in files {
        let at = f.plane.join(rel);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }
    support::git(&f.plane, &["init", "-q", "-b", "main", "."]);
    support::git(&f.plane, &["add", "-A"]);
    support::git(&f.plane, &["commit", "-q", "-m", "a project with skills"]);
    guest::wire(&f.plane, &f.clone);
    assert!(
        f.clone.join(".claude/skills/deploy/SKILL.md").is_file(),
        "purlis's copy reached the clone before the move"
    );
    f
}

fn apply(plane: &Path) -> Fixed {
    project_skills::apply_in(plane, Some(&built_in()), &|_| None, true)
}

fn said(fixed: &Fixed) -> String {
    match fixed {
        Fixed::Ran { said, complete } => {
            assert!(complete, "{said:?}");
            said.join("\n")
        }
        Fixed::Refused(why) => panic!("refused: {why}"),
    }
}

fn refused(fixed: Fixed) -> String {
    match fixed {
        Fixed::Refused(why) => why,
        ran => panic!("expected a refusal, got {ran:?}"),
    }
}

fn head(plane: &Path) -> String {
    String::from_utf8_lossy(&support::git(plane, &["rev-parse", "HEAD"]).stdout)
        .trim()
        .to_owned()
}

fn commits(plane: &Path) -> usize {
    String::from_utf8_lossy(&support::git(plane, &["rev-list", "--count", "HEAD"]).stdout)
        .trim()
        .parse()
        .unwrap()
}

/// The names of every skill a chat on `harness` gets: at the project's root (`clone` `None`),
/// where the program reads the plane's own folders as the repository it starts in, or in the
/// clone, where it reads that clone's. purlis's own and the project's come by the program's route.
fn skills_of(f: &support::Fixture, harness: Harness, clone: Option<&Path>) -> BTreeSet<String> {
    let workspace = clone.map(|_| f.ws.as_str());
    let set = skills::of_a_chat(
        Some(&built_in()),
        &f.plane,
        workspace,
        None,
        harness.skills(),
    );
    let outside = match clone {
        None => purlis_core::harness_plugin::repo_skills(
            harness.adapter().plugins(),
            &purlis_core::harness_plugin::Choices::read_in(&f.plane, None),
            &[purlis_core::repos::Repo {
                name: "plane".to_owned(),
                path: f.plane.clone(),
            }],
        ),
        Some(clone) => skills::repo_skills_of_chat(harness, &f.plane, &f.ws, clone),
    };
    set.effective
        .iter()
        .map(|one| one.skill.name.clone())
        .chain(outside.into_iter().map(|one| one.skill.name))
        .collect()
}

/// Every chat's skills, on each program, at the root and in the clone.
fn every_chat(f: &support::Fixture) -> Vec<(Harness, &'static str, BTreeSet<String>)> {
    let mut all = Vec::new();
    for harness in Harness::ALL {
        all.push((harness, "root", skills_of(f, harness, None)));
        all.push((harness, "clone", skills_of(f, harness, Some(&f.clone))));
    }
    all
}

#[test]
fn every_chat_keeps_its_skills_on_every_program_and_the_clone_loses_its_copy() {
    purlis_core::unsteered!();
    let f = project();
    let before = every_chat(&f);
    let was = commits(&f.plane);

    let report = said(&apply(&f.plane));

    // One commit, and nothing left over.
    assert_eq!(commits(&f.plane), was + 1, "{report}");
    assert_eq!(f.status(&f.plane), "", "the commit holds every change");
    for skill in ["deploy", "review"] {
        assert!(!f.plane.join(".claude/skills").join(skill).exists());
        let folder = f.plane.join("skills").join(skill);
        assert!(folder.join("SKILL.md").is_file(), "{report}");
        assert_eq!(skills::grant_of(&folder), Ok(Grant::All));
        assert!(
            report.contains(&format!(
                "moved .claude/skills/{skill} to skills/{skill}, granted to every workspace"
            )),
            "{report}"
        );
    }
    // The references, rewritten where they are references and nowhere else.
    let persona = std::fs::read_to_string(f.plane.join("personas/ops/persona.md")).unwrap();
    assert!(
        persona.contains("skills: purlis:handoff, deploy, purlis:nope"),
        "{persona}"
    );
    assert!(persona.contains("`purlis:secrets`"), "{persona}");
    let deploy = std::fs::read_to_string(f.plane.join("skills/deploy/SKILL.md")).unwrap();
    assert!(deploy.contains("`purlis:handoff`"), "{deploy}");
    let notes = std::fs::read_to_string(f.plane.join("skills/review/notes/token.md")).unwrap();
    assert_eq!(
        notes,
        "# Notes\n\nAsk purlis:secrets for the token. charter: is prose.\n"
    );
    // Every reference resolved, or said with why.
    assert!(
        report.contains("the persona ops names purlis:nope: its persona names it, and purlis has no built-in skill of this name"),
        "{report}"
    );
    assert!(!report.contains("names purlis:handoff"), "{report}");
    assert!(!report.contains("names deploy"), "{report}");
    assert!(report.contains("To undo it: `git revert "), "{report}");
    // A build without the feature opens the moved project read-only.
    assert_eq!(compat::read(&f.plane), Compat::Writable);
    assert!(matches!(
        compat::read_knowing(&f.plane, &[purlis_core::names::PURLIS_NAMES_FEATURE]),
        Compat::ReadOnly(_)
    ));

    // The clone's copy is withdrawn at its next wire, and nothing is copied again.
    guest::wire(&f.plane, &f.clone);
    assert!(
        !f.clone.join(".claude/skills").exists(),
        "purlis's copy left the clone"
    );
    assert_eq!(f.status(&f.clone), "", "and left nothing showing");

    let after = every_chat(&f);
    for ((harness, place, had), (_, _, has)) in before.iter().zip(&after) {
        assert!(
            had.is_subset(has),
            "a {harness:?} chat at the {place} lost {:?}",
            had.difference(has).collect::<Vec<_>>()
        );
        for moved in ["deploy", "review"] {
            assert!(has.contains(moved), "a {harness:?} chat at the {place}: {has:?}");
        }
        if *harness == Harness::Codex {
            assert!(!had.contains("deploy"), "Codex never read .claude/skills");
        } else {
            assert_eq!(had, has, "a {harness:?} chat at the {place} gets the same skills");
        }
    }
}

#[test]
fn a_second_run_changes_nothing() {
    purlis_core::unsteered!();
    let f = project();
    said(&apply(&f.plane));
    let at = head(&f.plane);

    let again = said(&apply(&f.plane));

    assert!(again.contains("nothing to do"), "{again}");
    assert_eq!(head(&f.plane), at, "no second commit");
    assert_eq!(f.status(&f.plane), "");
}

#[test]
fn a_plane_with_changes_not_committed_is_refused_and_nothing_is_written() {
    purlis_core::unsteered!();
    let f = project();
    std::fs::write(f.plane.join("personas/ops/persona.md"), "---\nrole: Mine\n---\n").unwrap();
    let at = head(&f.plane);

    let why = refused(apply(&f.plane));

    assert!(why.contains("changes that are not committed"), "{why}");
    assert!(why.contains("the move must be a commit of its own"), "{why}");
    assert_eq!(head(&f.plane), at);
    assert!(f.plane.join(".claude/skills/deploy/SKILL.md").is_file());
    assert!(!f.plane.join("skills").exists());
}

#[test]
fn a_chat_and_a_machine_without_the_preview_are_refused() {
    purlis_core::unsteered!();
    let f = project();
    let in_a_chat = |name: &str| (name == purlis_core::active::SESSION_ID_ENV).then(|| "s1".to_owned());

    let why = refused(project_skills::apply_in(&f.plane, Some(&built_in()), &in_a_chat, true));
    assert_eq!(why, project_skills::FROM_A_CHAT);
    let why = refused(project_skills::apply_in(&f.plane, Some(&built_in()), &|_| None, false));
    assert_eq!(why, project_skills::STUDIO_OFF);
    assert!(!f.plane.join("skills").exists());
}

#[test]
fn a_skill_a_chat_would_lose_is_refused_before_anything_moves() {
    purlis_core::unsteered!();
    let f = project();
    // A built-in skill's name is never taken, so this copy would be left out of every chat.
    let at = f.plane.join(".claude/skills/hand-off/SKILL.md");
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(&at, "---\nname: handoff\ndescription: Mine.\n---\n").unwrap();
    f.commit(&f.plane, "mine");
    let head_was = head(&f.plane);

    let why = refused(apply(&f.plane));

    assert!(why.contains("the skill handoff in .claude/skills/hand-off"), "{why}");
    assert!(why.contains("a built-in skill has this name"), "{why}");
    assert_eq!(head(&f.plane), head_was);
    assert_eq!(f.status(&f.plane), "");
}

#[test]
fn what_a_move_cannot_carry_is_refused() {
    purlis_core::unsteered!();
    // An untracked file beside the committed ones.
    let f = project();
    std::fs::write(f.plane.join(".claude/skills/deploy/draft.md"), "draft\n").unwrap();
    let why = refused(apply(&f.plane));
    assert!(why.contains("holds files git does not track (.claude/skills/deploy/draft.md)"), "{why}");
    assert!(!f.plane.join("skills").exists());

    // A link, committed.
    let f = project();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("../../README.md", f.plane.join(".claude/skills/review/link.md"))
            .unwrap();
        f.commit(&f.plane, "a link");
        let why = refused(apply(&f.plane));
        assert!(why.contains(".claude/skills/review/link.md is a link"), "{why}");
        assert!(!f.plane.join("skills").exists());
    }
}

#[test]
fn a_project_with_nothing_to_move_says_so() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("svc");
    std::fs::write(f.plane.join(".gitignore"), "/workspaces/*/*\n").unwrap();
    support::git(&f.plane, &["init", "-q", "-b", "main", "."]);
    support::git(&f.plane, &["add", "-A"]);
    support::git(&f.plane, &["commit", "-q", "-m", "empty"]);
    let at = head(&f.plane);

    let report = said(&apply(&f.plane));

    assert!(report.contains("nothing to do"), "{report}");
    assert_eq!(head(&f.plane), at);
    assert!(!compat::requires(&f.plane, skills::PROJECT_SKILLS_FEATURE));
}
