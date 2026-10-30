// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for a skill's history and grades across a rename and a move (#3918): the versions
//! from before, with their grades, its Grades and a grade's scorecard under its new name, and
//! the chats this machine kept of its grades.

use std::path::Path;

use super::*;
use crate::repocmd::Say;
use crate::skills::history::{self, Grade};
use crate::skills::verbs::{By, Ctx};

fn did<T>(root: &Path, change: impl FnOnce(&mut Ctx<'_>) -> T) -> T {
    let mut say = |_: Say| {};
    change(&mut Ctx {
        plane: root,
        built_in: None,
        by: By::Command,
        say: &mut say,
    })
}

/// [`plane`]'s `deploy`, graded once as it was made (`g1`), and edited once since.
fn graded_then_edited() -> tempfile::TempDir {
    let tmp = plane();
    let root = tmp.path();
    record(
        root,
        &Level::project(),
        &graded(root, "g1", "2026-10-25T10:00:00+00:00", Spent::default()),
        "laptop",
    );
    let level = Level::project();
    let (_, was) = crate::skills::verbs::text_of(root, &level, "deploy").unwrap();
    did(root, |cx| {
        crate::skills::verbs::edit(
            cx,
            &level,
            "deploy",
            &was,
            &was.replace("Step one.", "Step two."),
        )
    })
    .unwrap();
    tmp
}

fn subjects(history: &history::History) -> Vec<String> {
    history
        .versions
        .iter()
        .map(|version| {
            version
                .subject
                .rsplit_once(" skill ")
                .map_or(version.subject.clone(), |(_, said)| format!("skill {said}"))
        })
        .collect()
}

const GRADED: Option<Grade> = Some(Grade {
    passed: 1,
    fixtures: 1,
});

#[test]
fn after_a_rename_history_lists_the_versions_from_before_with_their_grades() {
    let tmp = graded_then_edited();
    let root = tmp.path();

    did(root, |cx| {
        crate::skills::verbs::rename(cx, &Level::project(), "deploy", "ship")
    })
    .unwrap();

    let read = history::history(root, &Level::project(), "ship").unwrap();
    assert_eq!(
        subjects(&read),
        [
            "skill rename deploy ship",
            "skill edit deploy",
            "make deploy"
        ]
    );
    let first = &read.versions[2];
    assert_eq!(first.grade, GRADED, "the grade from before the rename");
    assert_eq!(
        (first.name.as_str(), first.folder.as_str()),
        ("deploy", "skills/deploy")
    );
    let places = history::places(root, &Level::project(), "ship").unwrap();
    assert_eq!(
        places
            .iter()
            .map(|one| (one.name.as_str(), one.folder.as_str()))
            .collect::<Vec<_>>(),
        [("ship", "skills/ship"), ("deploy", "skills/deploy")]
    );

    // Its Grades read the grade under its new name, and so does the grade's scorecard.
    let grades = read_followed(root, &Level::project(), "ship").grades;
    assert_eq!(
        grades
            .iter()
            .map(|one| one.grade.as_str())
            .collect::<Vec<_>>(),
        ["g1"]
    );
    assert_eq!(
        grade_of(root, "ship", "g1").map(|one| one.grade),
        Some("g1".into())
    );
    assert!(
        read(root, "ship", Some(&Level::project()))
            .grades
            .is_empty(),
        "its own name's rows alone hold none"
    );
}

#[test]
fn after_a_move_history_lists_the_versions_from_before_with_their_grades() {
    let tmp = graded_then_edited();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    std::fs::write(root.join("workspaces/ops/workspace.md"), "# ops\n").unwrap();
    run(root, &["add", "-A"]);
    run(
        root,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "add ops",
        ],
    );
    let ops = Level::workspace("ops");

    did(root, |cx| {
        crate::skills::verbs::move_to(cx, &Level::project(), "deploy", &ops, None)
    })
    .unwrap();

    let read = history::history(root, &ops, "deploy").unwrap();
    let said = subjects(&read);
    assert!(said[0].starts_with("skill mv deploy "), "{said:?}");
    assert_eq!(said[1..], ["skill edit deploy", "make deploy"]);
    assert_eq!(read.versions[2].grade, GRADED);
    assert_eq!(read.versions[2].folder, "skills/deploy");
    let grades = read_followed(root, &ops, "deploy").grades;
    assert_eq!(
        grades
            .iter()
            .map(|one| one.grade.as_str())
            .collect::<Vec<_>>(),
        ["g1"]
    );

    // A grade of it moved compares with its version before, read from the folder it had then.
    let planned = planned(root, &ops, "deploy", None).unwrap();
    assert_eq!(planned.previous_refused, None);
    assert_eq!(
        planned.previous.map(|one| one.commit),
        Some(read.versions[2].commit.clone())
    );
}

#[test]
fn a_versions_diff_shows_the_rename_and_a_revert_from_before_it_is_refused() {
    let tmp = graded_then_edited();
    let root = tmp.path();
    did(root, |cx| {
        crate::skills::verbs::rename(cx, &Level::project(), "deploy", "ship")
    })
    .unwrap();
    let read = history::history(root, &Level::project(), "ship").unwrap();

    let renamed = history::diff(root, &Level::project(), "ship", &read.versions[0].commit).unwrap();
    assert!(
        renamed.text.contains("rename from skills/deploy/SKILL.md"),
        "{}",
        renamed.text
    );
    assert!(renamed.text.contains("+name: ship"), "{}", renamed.text);
    let edited = history::diff(root, &Level::project(), "ship", &read.versions[1].commit).unwrap();
    assert!(edited.text.contains("+Step two."), "{}", edited.text);

    let refused = did(root, |cx| {
        history::revert(cx, &Level::project(), "ship", &read.versions[1].commit)
    })
    .unwrap_err();
    assert!(
        refused.contains("from before it was renamed or moved"),
        "{refused}"
    );
}

#[test]
fn a_rename_takes_the_chats_this_machine_kept_of_its_grades_along() {
    let tmp = graded_then_edited();
    let root = tmp.path();
    kept::keep(root, "deploy", "01GRADE", "01CHAT", "screen").unwrap();

    did(root, |cx| {
        crate::skills::verbs::rename(cx, &Level::project(), "deploy", "ship")
    })
    .unwrap();

    assert_eq!(
        kept::kept(root, "ship", "01GRADE", "01CHAT").as_deref(),
        Some("screen")
    );
    assert_eq!(kept::kept(root, "deploy", "01GRADE", "01CHAT"), None);
}

#[test]
fn grading_after_each_edit_follows_a_rename_and_a_move_and_goes_with_a_delete() {
    use super::after_edit;
    let tmp = graded_then_edited();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    std::fs::write(root.join("workspaces/ops/workspace.md"), "# ops\n").unwrap();
    run(root, &["add", "-A"]);
    run(
        root,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "add ops",
        ],
    );
    let (project, ops) = (Level::project(), Level::workspace("ops"));
    let accepted = after_edit::Accepted {
        profile: "work".into(),
        model: None,
        chats: 6,
        token_cap: cap::LEAST,
        limit_usd: 1.0,
        tests: "0".repeat(64),
        previous: None,
        runs_on: None,
        at: "2026-10-30T10:00:00+00:00".into(),
        graded: None,
        paused: None,
    };
    after_edit::turn_on(root, &project, "deploy", accepted.clone()).unwrap();

    did(root, |cx| {
        crate::skills::verbs::rename(cx, &project, "deploy", "ship")
    })
    .unwrap();
    assert_eq!(after_edit::of(root, &project, "deploy"), None);
    assert_eq!(
        after_edit::of(root, &project, "ship"),
        Some(accepted.clone())
    );

    did(root, |cx| {
        crate::skills::verbs::move_to(cx, &project, "ship", &ops, None)
    })
    .unwrap();
    assert_eq!(after_edit::of(root, &project, "ship"), None);
    assert_eq!(after_edit::of(root, &ops, "ship"), Some(accepted));

    did(root, |cx| crate::skills::verbs::delete(cx, &ops, "ship")).unwrap();
    assert_eq!(after_edit::of(root, &ops, "ship"), None);
}

#[test]
fn a_skill_that_takes_the_old_name_since_lends_none_of_its_grades() {
    let tmp = graded_then_edited();
    let root = tmp.path();
    did(root, |cx| {
        crate::skills::verbs::rename(cx, &Level::project(), "deploy", "ship")
    })
    .unwrap();
    // Another skill takes the name `deploy` in the project's skills, and is graded.
    std::fs::create_dir_all(root.join("skills/deploy/evals")).unwrap();
    std::fs::write(
        root.join("skills/deploy/SKILL.md"),
        "---\nname: deploy\ndescription: Another skill.\n---\n\nSomething else.\n",
    )
    .unwrap();
    std::fs::write(root.join("skills/deploy/evals/triggers.json"), TRIGGERS).unwrap();
    run(root, &["add", "-A"]);
    run(
        root,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "another deploy",
        ],
    );
    record(
        root,
        &Level::project(),
        &graded(root, "g2", "2026-10-26T10:00:00+00:00", Spent::default()),
        "laptop",
    );

    let grades = read_followed(root, &Level::project(), "ship").grades;
    assert_eq!(
        grades
            .iter()
            .map(|one| one.grade.as_str())
            .collect::<Vec<_>>(),
        ["g1"]
    );
    assert_eq!(grade_of(root, "ship", "g2"), None);
    assert_eq!(
        grade_of(root, "deploy", "g2").map(|one| one.grade),
        Some("g2".into())
    );
}
