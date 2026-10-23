// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for a skill's lifecycle and owners (SK-19, #2782): each change on a temporary plane
//! that is a real git repository, read back through the skill set the studio shows.

use std::path::Path;

use super::*;
use crate::repocmd::Say;
use crate::skills::verbs::By;
use crate::testgit::run;

fn skill(root: &Path, folder: &str, name: &str) {
    std::fs::create_dir_all(root.join(folder)).unwrap();
    std::fs::write(
        root.join(folder).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does {name} for the team.\n---\n\nbody\n"),
    )
    .unwrap();
}

/// A plane that saves by committing, with project skills `deploy` and `ship` and a workspace
/// `ops`, all in one commit.
fn plane() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[plane]\nmode = \"commit\"\n",
    )
    .unwrap();
    std::fs::write(root.join(".gitignore"), ".charter/\n.purlis/\n").unwrap();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    skill(root, "skills/deploy", "deploy");
    skill(root, "skills/ship", "ship");
    run(root, &["init", "-q", "-b", "main", "."]);
    run(root, &["config", "user.name", "Fixture"]);
    run(root, &["config", "user.email", "fixture@example.invalid"]);
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "one"]);
    tmp
}

fn did<T>(root: &Path, change: impl FnOnce(&mut Ctx<'_>) -> T) -> T {
    let mut say = |_: Say| {};
    change(&mut Ctx {
        plane: root,
        built_in: None,
        by: By::Command,
        say: &mut say,
    })
}

fn gets(root: &Path) -> Vec<String> {
    crate::skills::in_the_studio(None, root, None, None)
        .effective
        .into_iter()
        .map(|e| e.skill.name)
        .collect()
}

/// The subject and paths of the last commit.
fn last_commit(root: &Path) -> (String, String) {
    let subject = run(root, &["log", "-1", "--format=%s"]).line().to_owned();
    let paths = run(root, &["show", "--name-only", "--format=", "HEAD"])
        .out
        .trim()
        .to_owned();
    (subject, paths)
}

fn deprecated_for(other: &str) -> Lifecycle {
    Lifecycle::Deprecated(Deprecated {
        replacement: Some(other.to_owned()),
    })
}

#[test]
fn a_deprecated_skill_still_reaches_chats_and_its_change_is_one_commit_of_the_file_beside_it() {
    let tmp = plane();
    let root = tmp.path();

    assert_eq!(
        did(root, |cx| set(
            cx,
            &Level::project(),
            "deploy",
            &deprecated_for("ship")
        )),
        Ok(true)
    );

    assert_eq!(gets(root), ["deploy", "ship"]);
    let (subject, paths) = last_commit(root);
    assert!(subject.ends_with("skill deprecated deploy"), "{subject}");
    assert_eq!(paths, "skills/deploy.toml");
    // Asked again, nothing changes.
    assert_eq!(
        did(root, |cx| set(
            cx,
            &Level::project(),
            "deploy",
            &deprecated_for("ship")
        )),
        Ok(false)
    );
}

#[test]
fn a_replacement_must_be_another_skill_there_is() {
    let tmp = plane();
    let root = tmp.path();
    for (other, says) in [
        ("gone", "no skill is named gone"),
        ("deploy", "cannot replace itself"),
        ("Bad Name", "cannot replace deploy"),
    ] {
        let why = did(root, |cx| {
            set(cx, &Level::project(), "deploy", &deprecated_for(other))
        })
        .unwrap_err();
        assert!(why.contains(says), "{other}: {why}");
    }
    assert!(!root.join("skills/deploy.toml").exists());
}

#[test]
fn an_archived_skill_never_loads_and_making_it_active_brings_it_back() {
    let tmp = plane();
    let root = tmp.path();

    did(root, |cx| {
        set(cx, &Level::project(), "deploy", &Lifecycle::Archived)
    })
    .unwrap();
    assert_eq!(gets(root), ["ship"]);
    let row = crate::skills::studio_rows(
        &crate::skills::in_the_studio(None, root, None, None),
        root,
        &[],
    )
    .into_iter()
    .find(|row| row.name == "deploy")
    .unwrap();
    assert_eq!(row.lifecycle, Lifecycle::Archived);
    assert!(row.left_out.unwrap().starts_with("archived"));

    did(root, |cx| {
        set(cx, &Level::project(), "deploy", &Lifecycle::Active)
    })
    .unwrap();
    assert_eq!(gets(root), ["deploy", "ship"]);
    assert!(!root.join("skills/deploy.toml").exists());
    assert!(last_commit(root).0.ends_with("skill active deploy"));
}

#[test]
fn owners_are_kept_beside_the_skill_and_shown_on_its_row() {
    let tmp = plane();
    let root = tmp.path();
    let owners = vec!["alice".to_owned(), "bob".to_owned()];

    assert_eq!(
        did(root, |cx| set_owners(
            cx,
            &Level::project(),
            "deploy",
            &owners
        )),
        Ok(true)
    );

    let row = crate::skills::studio_rows(
        &crate::skills::in_the_studio(None, root, None, None),
        root,
        &[],
    )
    .into_iter()
    .find(|row| row.name == "deploy")
    .unwrap();
    assert_eq!(row.owners, owners);
    assert!(last_commit(root).0.ends_with("skill owners deploy"));
    assert!(
        did(root, |cx| set_owners(
            cx,
            &Level::project(),
            "deploy",
            &["no way".to_owned()]
        ))
        .is_err()
    );
}

#[test]
fn a_deprecated_skill_is_listed_after_the_rest() {
    let tmp = plane();
    let root = tmp.path();
    did(root, |cx| {
        set(cx, &Level::project(), "deploy", &deprecated_for("ship"))
    })
    .unwrap();
    let names: Vec<String> = crate::skills::studio_rows(
        &crate::skills::in_the_studio(None, root, None, None),
        root,
        &[],
    )
    .into_iter()
    .map(|row| row.name)
    .collect();
    assert_eq!(names, ["ship", "deploy"]);
}

fn logged(root: &Path, skill: &str, at: u64) {
    let dir = root.join("personas/_skills");
    std::fs::create_dir_all(&dir).unwrap();
    let ts = crate::dispatch::stamp(
        chrono::DateTime::from_timestamp(i64::try_from(at).unwrap(), 0).unwrap(),
    );
    let row = serde_json::json!({
        "run": format!("run-{skill}"), "skill": skill, "started_by": "skill-tool", "ts": ts,
        "level": "project", "workspace": null,
    });
    std::fs::write(dir.join("2026-06.box.jsonl"), format!("{row}\n")).unwrap();
}

#[test]
fn a_skill_no_chat_ran_for_sixty_days_is_suggested_and_a_recent_run_or_a_deprecation_keeps_it_off()
{
    let tmp = plane();
    let root = tmp.path();
    let made: u64 = run(root, &["log", "-1", "--format=%ct"])
        .line()
        .parse()
        .unwrap();
    let later = made + 61 * 86_400;

    let names = |now: u64| -> Vec<(String, u64)> {
        unused(root, now)
            .into_iter()
            .map(|one| (one.name, one.days))
            .collect()
    };
    assert!(names(made + 59 * 86_400).is_empty());
    assert_eq!(
        names(later),
        [("deploy".to_owned(), 61), ("ship".to_owned(), 61)]
    );

    logged(root, "skills:ship", made + 30 * 86_400);
    assert_eq!(names(later), [("deploy".to_owned(), 61)]);

    did(root, |cx| {
        set(cx, &Level::project(), "deploy", &deprecated_for("ship"))
    })
    .unwrap();
    assert!(names(later).is_empty());
}
