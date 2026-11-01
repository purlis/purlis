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

    let listed = history::history(root, &Level::project(), "ship").unwrap();
    assert_eq!(
        subjects(&listed),
        [
            "skill rename deploy ship",
            "skill edit deploy",
            "make deploy"
        ]
    );
    let first = &listed.versions[2];
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

    let listed = history::history(root, &ops, "deploy").unwrap();
    let said = subjects(&listed);
    assert!(said[0].starts_with("skill mv deploy "), "{said:?}");
    assert_eq!(said[1..], ["skill edit deploy", "make deploy"]);
    assert_eq!(listed.versions[2].grade, GRADED);
    assert_eq!(listed.versions[2].folder, "skills/deploy");
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
        Some(listed.versions[2].commit.clone())
    );
}

#[test]
fn a_versions_diff_shows_the_rename() {
    let tmp = graded_then_edited();
    let root = tmp.path();
    did(root, |cx| {
        crate::skills::verbs::rename(cx, &Level::project(), "deploy", "ship")
    })
    .unwrap();
    let listed = history::history(root, &Level::project(), "ship").unwrap();

    let renamed =
        history::diff(root, &Level::project(), "ship", &listed.versions[0].commit).unwrap();
    assert!(
        renamed.text.contains("rename from skills/deploy/SKILL.md"),
        "{}",
        renamed.text
    );
    assert!(renamed.text.contains("+name: ship"), "{}", renamed.text);
    let edited =
        history::diff(root, &Level::project(), "ship", &listed.versions[1].commit).unwrap();
    assert!(edited.text.contains("+Step two."), "{}", edited.text);
}

/// The text of `rel` in the plane at `root`.
fn text(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap()
}

/// What `git status` says of the skills' folders in the plane at `root`: empty where all of
/// them is committed.
fn status(root: &Path) -> String {
    run(
        root,
        &["status", "--porcelain", "--", "skills", "workspaces"],
    )
    .out
}

#[test]
fn a_revert_from_before_a_rename_comes_back_under_the_name_it_has_now() {
    let tmp = graded_then_edited();
    let root = tmp.path();
    let project = Level::project();
    did(root, |cx| {
        crate::skills::verbs::rename(cx, &project, "deploy", "ship")
    })
    .unwrap();
    // A file added since, which the version reverted to never had.
    std::fs::write(root.join("skills/ship/notes.md"), "Notes.\n").unwrap();
    run(root, &["add", "-A"]);
    run(
        root,
        &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "notes"],
    );
    let listed = history::history(root, &project, "ship").unwrap();
    let first = listed.versions.last().unwrap();
    assert_eq!(
        first.before,
        Some(history::Before {
            name: "deploy".into(),
            folder: "skills/deploy".into()
        }),
        "the window is told it is from before the rename"
    );
    assert_eq!(listed.versions[0].before, None);
    assert_eq!(
        history::revert_widens(root, &project, "ship", &first.commit),
        Ok(None)
    );

    did(root, |cx| {
        history::revert(cx, &project, "ship", &first.commit)
    })
    .unwrap();

    let now = text(root, "skills/ship/SKILL.md");
    assert!(now.contains("name: ship\n"), "{now}");
    assert!(now.contains("Step one."), "{now}");
    assert!(root.join("skills/ship/evals/triggers.json").is_file());
    assert!(
        !root.join("skills/ship/notes.md").exists(),
        "what it never had is gone"
    );
    assert!(
        !root.join("skills/deploy").exists(),
        "nothing comes back at the old place"
    );
    assert_eq!(status(root), "", "the revert is one commit");
    let after = history::history(root, &project, "ship").unwrap();
    let said = subjects(&after);
    assert!(said[0].starts_with("skill revert ship "), "{said:?}");
}

#[test]
fn a_revert_from_before_a_move_comes_back_in_the_folder_it_has_now() {
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
    let listed = history::history(root, &ops, "deploy").unwrap();
    let first = listed.versions.last().unwrap();
    assert_eq!(
        first.before.as_ref().map(|one| one.folder.as_str()),
        Some("skills/deploy")
    );

    did(root, |cx| {
        history::revert(cx, &ops, "deploy", &first.commit)
    })
    .unwrap();

    let now = text(root, "workspaces/ops/skills/deploy/SKILL.md");
    assert!(
        now.contains("name: deploy\n") && now.contains("Step one."),
        "{now}"
    );
    assert!(!root.join("skills/deploy").exists());
    assert_eq!(status(root), "");
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
        outcomes: None,
        evals: Vec::new(),
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

#[test]
fn a_rename_that_keeps_the_folder_keeps_the_versions_from_before_under_their_name() {
    // `deploy` in a folder named otherwise, `skills/ship`, graded there.
    let tmp = plane();
    let root = tmp.path();
    run(root, &["mv", "skills/deploy", "skills/ship"]);
    run(
        root,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "skill folder ship",
        ],
    );
    let row = graded(root, "g1", "2026-10-25T10:00:00+00:00", Spent::default());
    record(root, &Level::project(), &row, "laptop");

    // A rename to the folder's name changes only its `name:` line.
    did(root, |cx| {
        crate::skills::verbs::rename(cx, &Level::project(), "deploy", "ship")
    })
    .unwrap();

    let listed = history::history(root, &Level::project(), "ship").unwrap();
    let named: Vec<(&str, &str)> = listed
        .versions
        .iter()
        .map(|one| (one.name.as_str(), one.folder.as_str()))
        .collect();
    assert_eq!(
        named,
        [
            ("ship", "skills/ship"),
            ("deploy", "skills/ship"),
            ("deploy", "skills/deploy")
        ]
    );
    assert_eq!(listed.versions[1].grade, GRADED, "the grade from before");
    assert_eq!(listed.versions[0].grade, None);
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
}

#[test]
fn a_local_copys_grades_follow_a_rename_and_a_share() {
    let tmp = plane();
    let root = tmp.path();
    let local = Level::project().local(true);
    std::fs::create_dir_all(root.join("skills.local/draft")).unwrap();
    std::fs::write(
        root.join("skills.local/draft/SKILL.md"),
        SKILL.replace("name: deploy", "name: draft"),
    )
    .unwrap();
    let row = Graded {
        skill: "draft".into(),
        level: local.rel(),
        ..graded(root, "g1", "2026-10-25T10:00:00+00:00", Spent::default())
    };
    record(root, &local, &row, "laptop");
    assert_eq!(read(root, "draft", Some(&local)).grades.len(), 1);

    did(root, |cx| {
        crate::skills::verbs::rename(cx, &local, "draft", "sketch")
    })
    .unwrap();
    let ids =
        |read: Read| -> Vec<String> { read.grades.into_iter().map(|one| one.grade).collect() };
    assert_eq!(ids(read_followed(root, &local, "sketch")), ["g1"]);
    assert_eq!(
        grade_of(root, "sketch", "g1").map(|one| one.grade),
        Some("g1".into())
    );

    did(root, |cx| {
        crate::skills::verbs::share(cx, &crate::skills::verbs::Whose::Project, "sketch")
    })
    .unwrap();
    assert_eq!(
        ids(read_followed(root, &Level::project(), "sketch")),
        ["g1"],
        "and into the shared skills, which git holds from the share on"
    );
}

/// [`plane`] with `draft`, a skill only on this machine, graded once (`g1`), and the level it is
/// at.
fn a_local_draft_graded() -> (tempfile::TempDir, Level) {
    let tmp = plane();
    let root = tmp.path();
    let local = Level::project().local(true);
    std::fs::create_dir_all(root.join("skills.local/draft")).unwrap();
    std::fs::write(
        root.join("skills.local/draft/SKILL.md"),
        SKILL.replace("name: deploy", "name: draft"),
    )
    .unwrap();
    let row = Graded {
        skill: "draft".into(),
        level: local.rel(),
        ..graded(root, "g1", "2026-10-25T10:00:00+00:00", Spent::default())
    };
    record(root, &local, &row, "laptop");
    (tmp, local)
}

#[test]
fn a_local_grade_under_way_across_a_rename_shows_under_the_new_name_4104() {
    let (tmp, local) = a_local_draft_graded();
    let root = tmp.path();
    let row = Graded {
        skill: "draft".into(),
        level: local.rel(),
        ..graded(root, "g2", "2026-10-25T11:00:00+00:00", Spent::default())
    };
    let marker = underway::began(root, &local, &row).unwrap();

    did(root, |cx| {
        crate::skills::verbs::rename(cx, &local, "draft", "sketch")
    })
    .unwrap();
    // It ends after the rename, and is recorded by the name it began under, stamped then.
    let ended = Graded {
        ts: (chrono::Utc::now() + chrono::Duration::seconds(1)).to_rfc3339(),
        ..row
    };
    record(root, &local, &ended, "laptop");
    underway::ended(root, "g2");
    drop(marker);

    let ids =
        |read: Read| -> Vec<String> { read.grades.into_iter().map(|one| one.grade).collect() };
    assert_eq!(ids(read_followed(root, &local, "sketch")), ["g2", "g1"]);
    assert_eq!(
        grade_of(root, "sketch", "g2").map(|one| one.grade),
        Some("g2".into())
    );
}

#[test]
fn a_skill_made_by_hand_at_a_renamed_skills_name_shows_none_of_its_grades_4104() {
    let (tmp, local) = a_local_draft_graded();
    let root = tmp.path();
    did(root, |cx| {
        crate::skills::verbs::rename(cx, &local, "draft", "sketch")
    })
    .unwrap();
    let ids =
        |read: Read| -> Vec<String> { read.grades.into_iter().map(|one| one.grade).collect() };
    assert_eq!(ids(read_followed(root, &local, "sketch")), ["g1"]);

    // `sketch` removed by hand, and another made at its name by hand.
    std::fs::rename(
        root.join("skills.local/sketch"),
        root.join("skills.local/.gone"),
    )
    .unwrap();
    std::fs::create_dir_all(root.join("skills.local/sketch")).unwrap();
    std::fs::write(
        root.join("skills.local/sketch/SKILL.md"),
        SKILL.replace("name: deploy", "name: sketch"),
    )
    .unwrap();
    std::fs::remove_dir_all(root.join("skills.local/.gone")).unwrap();

    if cfg!(unix) {
        assert_eq!(
            ids(read_followed(root, &local, "sketch")),
            Vec::<String>::new()
        );
        assert_eq!(grade_of(root, "sketch", "g1"), None);
    }
}

#[test]
fn a_deleted_local_skill_restored_gets_its_grades_from_before_back_4104() {
    let (tmp, local) = a_local_draft_graded();
    let root = tmp.path();
    did(root, |cx| {
        crate::skills::verbs::rename(cx, &local, "draft", "sketch")
    })
    .unwrap();
    did(root, |cx| {
        crate::skills::verbs::delete(cx, &local, "sketch")
    })
    .unwrap();
    let deleted = crate::skills::verbs::deleted(root)
        .into_iter()
        .find(|one| one.name == "sketch")
        .unwrap();
    did(root, |cx| crate::skills::verbs::restore(cx, &deleted, None)).unwrap();

    let ids =
        |read: Read| -> Vec<String> { read.grades.into_iter().map(|one| one.grade).collect() };
    assert_eq!(ids(read_followed(root, &local, "sketch")), ["g1"]);
}
