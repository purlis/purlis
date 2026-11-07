// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #5718: a stopped grade is kept to resume, offered only while what decides it holds.

use std::collections::BTreeMap;

use purlis_skills::grade::{Arm, ChatEnd, ChatRan, Earlier, TriggerTest};

use super::*;
use crate::skilluse::Content;

fn request() -> GradeRequest {
    GradeRequest {
        skill: "deploy".into(),
        triggers: vec![TriggerTest {
            query: "ship it".into(),
            should_trigger: true,
        }],
        outcomes: Vec::new(),
        arms: vec![Arm::With, Arm::Without],
        runs: 3,
        shell: false,
    }
}

fn copy(skill_md: &str, triggers: &str) -> SkillCopy {
    let mut files = BTreeMap::new();
    files.insert("SKILL.md".to_owned(), skill_md.to_owned());
    files.insert("evals/triggers.json".to_owned(), triggers.to_owned());
    SkillCopy {
        snapshot: Snapshot {
            name: "deploy".into(),
            files,
        },
        content: Content::Uncommitted,
        commit: None,
        previous: None,
        previous_refused: None,
        inputs: BTreeMap::new(),
    }
}

fn on<'a>(profile: Option<&'a str>, program: Option<&'a str>, model: Option<&'a str>) -> On<'a> {
    On {
        profile,
        program,
        model,
    }
}

fn deciding_of(request: &GradeRequest, copy: &SkillCopy) -> Deciding {
    Deciding::of(request, copy, on(Some("work"), Some("claude"), None), None)
}

fn ran(end: ChatEnd, tokens: u64) -> ChatRan {
    ChatRan {
        chat: Some("01CHAT".into()),
        started: Vec::new(),
        tokens: Some(tokens),
        cost_usd: Some(0.01),
        ms: 1000,
        end,
        answer: None,
    }
}

/// A grade of [`request`] stopped by the person on its second run, with the skill.
fn stopped(grade: &str) -> Stopped {
    let done = || ran(ChatEnd::Turn, 1_000);
    Stopped {
        v: VERSION,
        grade: grade.into(),
        skill: "deploy".into(),
        level: Some("skills".into()),
        draft: None,
        draft_of: None,
        request: request(),
        kept: Kept {
            triggers: vec![vec![
                vec![done(), ran(ChatEnd::Cancelled, 500)],
                vec![done()],
            ]],
            outcomes: Vec::new(),
            earlier: Earlier {
                tokens: Some(2_500),
                usd: Some(0.03),
                ..Earlier::default()
            },
        },
        deciding: deciding_of(&request(), &copy("v1", "[]")),
        ended: GradeEnd::Cancelled,
        resets_at: None,
        ts: "2026-11-08T01:00:00+00:00".into(),
    }
}

#[test]
fn nothing_changed_is_nothing_to_say() {
    let then = deciding_of(&request(), &copy("v1", "[]"));
    assert!(then.changed(&then.clone()).is_empty());
    assert_eq!(changed_said(&[]), None);
}

#[test]
fn each_input_that_decides_the_result_is_named_where_it_changed() {
    let then = deciding_of(&request(), &copy("v1", "[]"));
    // The skill's text, and only that: its fixtures are its tests, read from the request.
    let edited = deciding_of(&request(), &copy("v2", "[]"));
    assert_eq!(then.changed(&edited), [Changed::Skill]);
    assert!(
        then.changed(&deciding_of(&request(), &copy("v1", "[1]")))
            .is_empty(),
        "a fixture file is no part of the skill's text"
    );
    let mut more_tests = request();
    more_tests.triggers.push(TriggerTest {
        query: "deploy now".into(),
        should_trigger: true,
    });
    assert_eq!(
        then.changed(&deciding_of(&more_tests, &copy("v1", "[]"))),
        [Changed::Tests]
    );
    let running_on = |profile: Option<&str>, program: Option<&str>, model: Option<&str>| {
        Deciding::of(
            &request(),
            &copy("v1", "[]"),
            on(profile, program, model),
            None,
        )
    };
    assert_eq!(
        then.changed(&running_on(Some("home"), Some("claude"), None)),
        [Changed::Program]
    );
    assert_eq!(
        then.changed(&running_on(Some("work"), Some("codex"), None)),
        [Changed::Program],
        "the same profile, set to start another program"
    );
    assert_eq!(
        then.changed(&running_on(
            Some("work"),
            Some("claude"),
            Some("claude-opus-5-5")
        )),
        [Changed::Model]
    );
    let mut more_runs = request();
    more_runs.runs = 5;
    assert_eq!(
        then.changed(&deciding_of(&more_runs, &copy("v1", "[]"))),
        [Changed::Settings]
    );
    let by_a_grader = Deciding::of(
        &request(),
        &copy("v1", "[]"),
        on(Some("work"), Some("claude"), None),
        Some("x"),
    );
    assert_eq!(then.changed(&by_a_grader), [Changed::Settings]);
}

#[test]
fn what_changed_is_said_as_a_person_lists_it() {
    assert_eq!(
        changed_said(&[Changed::Skill]).unwrap(),
        "Resume is not offered: the skill's text changed since it stopped. Grade again runs it \
         whole."
    );
    assert_eq!(
        changed_said(&[Changed::Skill, Changed::Tests, Changed::Model]).unwrap(),
        "Resume is not offered: the skill's text, its tests and the model changed since it \
         stopped. Grade again runs it whole."
    );
}

#[test]
fn a_stopped_grade_is_kept_read_back_and_forgotten() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let kept = stopped("01GRADE");
    keep(root, &kept).unwrap();
    assert_eq!(of(root, "01GRADE"), Some(kept));
    forget(root, "01GRADE");
    assert_eq!(of(root, "01GRADE"), None);
}

#[test]
fn a_name_that_is_not_plain_is_never_a_path() {
    let tmp = tempfile::tempdir().unwrap();
    let mut kept = stopped("../escape");
    assert!(keep(tmp.path(), &kept).is_err());
    kept.grade = "..".into();
    assert!(keep(tmp.path(), &kept).is_err());
    assert_eq!(of(tmp.path(), "../escape"), None);
}

#[test]
fn a_finished_grade_read_back_is_none_to_resume() {
    let tmp = tempfile::tempdir().unwrap();
    let mut whole = stopped("01WHOLE");
    whole.ended = GradeEnd::Whole;
    keep(tmp.path(), &whole).unwrap();
    assert_eq!(of(tmp.path(), "01WHOLE"), None);
}

#[test]
fn a_new_grade_of_the_copy_forgets_its_stopped_one_and_no_other() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    keep(root, &stopped("01OLD")).unwrap();
    let mut other = stopped("01OTHER");
    other.skill = "pdf".into();
    keep(root, &other).unwrap();
    let mut of_draft = stopped("01DRAFT");
    of_draft.draft = Some(draft::Mark {
        id: "01MINT".into(),
        digest: "d".into(),
        chats: 6,
    });
    keep(root, &of_draft).unwrap();
    let mut newest = all(root);
    assert_eq!(
        newest
            .iter()
            .map(|one| one.grade.as_str())
            .collect::<Vec<_>>()
            .len(),
        3
    );
    newest.retain(|one| one.grade == "01OLD");
    assert_eq!(newest.len(), 1);
    forget_of(root, "deploy", Some("skills"), None);
    assert_eq!(of(root, "01OLD"), None);
    assert!(of(root, "01OTHER").is_some());
    assert!(of(root, "01DRAFT").is_some(), "a draft's is its own");
    forget_of(root, "deploy", Some("skills"), Some("01MINT"));
    assert_eq!(of(root, "01DRAFT"), None);
}

#[test]
fn the_plan_of_a_resume_is_the_chats_left_at_what_each_spent_so_far() {
    let kept = stopped("01GRADE");
    // Six chats in all; two had their result, and the one cut off runs again.
    assert_eq!(kept.left(), 4);
    let cap = cap::Cap {
        tokens: 200_000,
        from: cap::From::Default,
    };
    let plan = plan(&kept, cap);
    assert_eq!(plan.left, 4);
    // 2,500 tokens over the three chats it asked for, for four more.
    assert_eq!(plan.tokens_left, Some(3_334));
    assert!((plan.usd_left.unwrap() - 0.04).abs() < 1e-9);
    assert_eq!(plan.cap.tokens, 200_000);
    assert_eq!(plan.earlier.tokens, Some(2_500));
}
