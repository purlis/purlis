// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for a draft's grade (#2780): drafts kept the way a chat's write is, on a temporary
//! plane that is a real git repository.

use std::path::Path;

use purlis_skills::grade::{Arm, GradeEnd};

use super::super::tests::plane;
use super::super::{
    Graded, Spent, VERSION, grade_of, grades_of, local_path_for, planned as own_plan,
};
use super::*;
use crate::proposed::Change;
use crate::skills::verbs::drafts::{Draft, draftable, one, take};

const EDITED: &str =
    "---\nname: deploy\ndescription: Deploys a build to staging or production.\n---\n\nStep two.\n";

const NEW_TESTS: &str = r#"[
    {"query": "ship the staging build", "should_trigger": true}
]"#;

/// `text` kept as chat 7's draft of the plane file `rel`, and the draft as it is then.
fn drafted_as(root: &Path, rel: &str, text: &str) -> Draft {
    let file = draftable(root, &root.join(rel)).expect("a skill's file");
    let change = Change::Whole {
        content: text.to_owned(),
    };
    let id = take(root, 7, &file, &change).unwrap();
    one(root, &id).unwrap()
}

/// A grade of `draft`, stopped before any chat, as a row of the record.
fn row_of(id: &str, draft: &Draft, plan: &Planned) -> Graded {
    Graded {
        v: VERSION,
        grade: id.to_owned(),
        skill: draft.name.clone(),
        level: draft.level.rel(),
        tree: "uncommitted".into(),
        commit: None,
        harness: None,
        profile: Some("work".into()),
        ts: crate::dispatch::stamp(chrono::Utc::now()),
        runs: 3,
        report: super::super::underway::interrupted_report(1000, None, None),
        spent: Spent::default(),
        plugins_off: None,
        model: None,
        previous: plan
            .copy
            .as_ref()
            .and_then(|copy| copy.previous.as_ref())
            .and_then(|previous| previous.commit.clone()),
        after_edit: false,
        draft: Some(Mark::of(draft, 12)),
    }
}

#[test]
fn a_draft_is_graded_as_accept_would_write_it_against_the_folders_current_version() {
    let plane = plane();
    let root = plane.path();
    // The folder holds what git does not: the current version is what it holds now.
    let current = "---\nname: deploy\ndescription: Ships it now.\n---\n\nNot committed.\n";
    let draft = drafted_as(root, "skills/deploy/SKILL.md", EDITED);
    std::fs::write(root.join("skills/deploy/SKILL.md"), current).unwrap();
    std::fs::write(root.join("skills/deploy/notes.md"), "kept\n").unwrap();
    let draft = one(root, &draft.id).unwrap();

    let planned = planned(root, &draft, None, &[]).unwrap();

    let request = planned.request.as_ref().unwrap();
    assert_eq!(
        request.arms,
        vec![Arm::With, Arm::Without, Arm::Previous],
        "the current version is compared"
    );
    let copy = planned.copy.as_ref().unwrap();
    assert_eq!(copy.snapshot.files["SKILL.md"], EDITED);
    assert_eq!(
        copy.snapshot.files["notes.md"], "kept\n",
        "the rest is the folder's"
    );
    assert!(
        !copy
            .snapshot
            .files
            .keys()
            .any(|path| path.starts_with("evals/"))
    );
    let previous = copy.previous.as_ref().unwrap();
    assert_eq!(previous.snapshot.files["SKILL.md"], current);
    assert_eq!(
        previous.commit, None,
        "read from the folder, it may be no commit"
    );
    assert_eq!(copy.content.word(), "uncommitted");
}

#[test]
fn a_new_skills_draft_is_graded_against_no_skill_alone() {
    let plane = plane();
    let root = plane.path();
    let skill = EDITED.replace("deploy", "release");
    drafted_as(root, "skills/release/SKILL.md", &skill);
    let draft = drafted_as(root, "skills/release/evals/triggers.json", NEW_TESTS);
    assert!(draft.new);

    let planned = planned(root, &draft, None, &[]).unwrap();

    let request = planned.request.as_ref().unwrap();
    assert_eq!(request.arms, vec![Arm::With, Arm::Without]);
    assert_eq!(request.triggers.len(), 1);
    assert!(planned.copy.unwrap().previous.is_none());
}

#[test]
fn a_new_skills_draft_with_no_instructions_is_refused() {
    let plane = plane();
    let root = plane.path();
    let draft = drafted_as(root, "skills/release/scripts/tag.sh", "git tag\n");

    let refused = planned(root, &draft, None, &[]).unwrap_err();

    assert_eq!(
        refused,
        "The draft of release has no instructions yet, so a grade has nothing to run."
    );
}

#[test]
fn a_test_the_draft_writes_is_the_one_that_runs_and_consent_covers_it() {
    let plane = plane();
    let root = plane.path();
    let draft = drafted_as(root, "skills/deploy/SKILL.md", EDITED);
    let before = planned(root, &draft, None, &[]).unwrap();
    assert_eq!(
        before.request.as_ref().unwrap().triggers.len(),
        2,
        "the folder's"
    );

    let draft = drafted_as(root, "skills/deploy/evals/triggers.json", NEW_TESTS);
    let after = planned(root, &draft, None, &[]).unwrap();

    let triggers = &after.request.as_ref().unwrap().triggers;
    assert_eq!(triggers.len(), 1);
    assert_eq!(triggers[0].query, "ship the staging build");
    assert_ne!(before.consent, after.consent);
}

#[test]
fn a_draft_that_changes_only_evals_gets_no_grade() {
    let plane = plane();
    let root = plane.path();
    let draft = drafted_as(root, "skills/deploy/evals/triggers.json", NEW_TESTS);

    assert_eq!(planned(root, &draft, None, &[]).unwrap_err(), ONLY_EVALS);
    let said = of(root, &draft);
    assert!(said.only_evals);
    assert_eq!(
        said.tests.unwrap().runs,
        1,
        "its own tests are what it holds"
    );
}

#[test]
fn a_tests_file_the_draft_cannot_parse_is_said_as_the_folders_is() {
    let plane = plane();
    let root = plane.path();
    drafted_as(root, "skills/deploy/SKILL.md", EDITED);
    let draft = drafted_as(root, "skills/deploy/evals/triggers.json", "not json");

    let tests = of(root, &draft).tests.unwrap();

    assert_eq!(tests.runs, 0);
    assert!(tests.refused.is_some());
}

#[test]
fn a_drafts_grade_is_kept_here_and_stays_out_of_the_skills_grades() {
    let plane = plane();
    let root = plane.path();
    let level = Level::project();
    let draft = drafted_as(root, "skills/deploy/SKILL.md", EDITED);
    let plan = planned(root, &draft, None, &[]).unwrap();
    let row = row_of("01draftgrade", &draft, &plan);

    let path = super::super::record(root, &level, &row, "device").unwrap();

    assert_eq!(
        path,
        local_path_for(root, chrono::Utc::now()).unwrap(),
        "this machine's, for a committed skill too"
    );
    assert!(
        !root.join("personas/_grades").exists(),
        "nothing of a draft is committed"
    );
    assert!(grades_of(root, "deploy", Some(&level)).is_empty());
    assert!(
        own_plan(root, &level, "deploy", None)
            .unwrap()
            .estimate
            .is_none()
    );
    assert_eq!(
        grade_of(root, "deploy", "01draftgrade").map(|one| one.report.ended),
        Some(GradeEnd::Interrupted),
        "its scorecard reads it by its id"
    );
    let said = of(root, &draft);
    assert_eq!(said.latest.unwrap().grade, "01draftgrade");
    assert!(!said.changed);
}

#[test]
fn a_drafts_grade_held_under_way_goes_to_this_machines_record_too() {
    let plane = plane();
    let root = plane.path();
    let level = Level::project();
    let draft = drafted_as(root, "skills/deploy/SKILL.md", EDITED);
    let plan = planned(root, &draft, None, &[]).unwrap();
    let row = row_of("01draftgrade", &draft, &plan);

    let marker = super::super::underway::began(root, &level, &row).unwrap();
    drop(marker);
    let recorded = super::super::underway::interrupted(root, || "device".to_owned(), |_| false);

    assert_eq!(recorded, vec!["01draftgrade".to_owned()]);
    assert!(!root.join("personas/_grades").exists());
    assert_eq!(of(root, &draft).latest.unwrap().grade, "01draftgrade");
}

#[test]
fn a_draft_changed_since_its_grade_says_so() {
    let plane = plane();
    let root = plane.path();
    let level = Level::project();
    let draft = drafted_as(root, "skills/deploy/SKILL.md", EDITED);
    let plan = planned(root, &draft, None, &[]).unwrap();
    super::super::record(
        root,
        &level,
        &row_of("01draftgrade", &draft, &plan),
        "device",
    )
    .unwrap();

    let draft = drafted_as(
        root,
        "skills/deploy/SKILL.md",
        &EDITED.replace("two", "three"),
    );

    assert!(of(root, &draft).changed);
}

#[test]
fn the_next_draft_of_a_skill_starts_with_no_grade() {
    let plane = plane();
    let root = plane.path();
    let level = Level::project();
    let draft = drafted_as(root, "skills/deploy/SKILL.md", EDITED);
    let plan = planned(root, &draft, None, &[]).unwrap();
    super::super::record(
        root,
        &level,
        &row_of("01draftgrade", &draft, &plan),
        "device",
    )
    .unwrap();
    crate::skills::verbs::drafts::reject(root, &draft.id).unwrap();

    let next = drafted_as(root, "skills/deploy/SKILL.md", EDITED);

    assert_eq!(next.id, draft.id, "one id a skill");
    assert!(of(root, &next).latest.is_none());
}

#[test]
fn a_row_that_marks_a_draft_as_purlis_does_not_is_skipped() {
    let plane = plane();
    let root = plane.path();
    let level = Level::project();
    let draft = drafted_as(root, "skills/deploy/SKILL.md", EDITED);
    let plan = planned(root, &draft, None, &[]).unwrap();
    let mut row = row_of("01draftgrade", &draft, &plan);
    row.draft.as_mut().unwrap().digest = "not a digest".to_owned();
    super::super::record(root, &level, &row, "device").unwrap();

    assert!(of(root, &draft).latest.is_none());
}
