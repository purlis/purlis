// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A router's grade (SK-15e, #3233): its library, its plan and consent, and its record.

use std::path::Path;

use purlis_skills::grade::routers::SkillPart;
use purlis_skills::grade::{ArmScore, GradeEnd, GradeReport, TriggerScore};

use super::*;

const TRIGGERS: &str = r#"[
    {"query": "ship it to production", "should_trigger": true},
    {"query": "what is production", "should_trigger": false}
]"#;

/// A plane with a workspace `ops` granted the project skills `deploy` (with trigger tests) and
/// `notes` (without), and a skill of its own, `rollback`, with trigger tests.
fn plane() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    for (dir, name, tests) in [
        ("skills", "deploy", true),
        ("skills", "notes", false),
        ("workspaces/ops/skills", "rollback", true),
    ] {
        let folder = root.join(dir).join(name);
        std::fs::create_dir_all(folder.join("evals")).unwrap();
        std::fs::write(
            folder.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nStep one.\n"),
        )
        .unwrap();
        if tests {
            std::fs::write(folder.join("evals/triggers.json"), TRIGGERS).unwrap();
        }
        if dir == "skills" {
            std::fs::write(
                root.join(dir).join(format!("{name}.toml")),
                "grant = \"all\"\n",
            )
            .unwrap();
        }
    }
    tmp
}

#[test]
fn the_library_is_what_a_chat_of_the_scope_is_handed_and_its_tested_skills_are_graded() {
    let tmp = plane();
    let root = tmp.path();

    let planned = planned(root, Some("ops"), None, "relevance").unwrap();

    assert_eq!(planned.library, 3, "{planned:?}");
    let graded: Vec<&str> = planned
        .skills
        .iter()
        .map(|one| one.skill.as_str())
        .collect();
    assert_eq!(graded.len(), 2);
    assert!(graded.contains(&"deploy") && graded.contains(&"rollback"));
    let request = planned.request.as_ref().expect("a request");
    assert_eq!(request.router, "relevance");
    assert!(
        request
            .skills
            .iter()
            .all(|one| one.arms == [Arm::Router, Arm::BuiltInRouter])
    );
    // Two skills, two tests each, two arms, three runs.
    assert_eq!(planned.plan.map(|plan| plan.chats), Some(24));
    assert!(planned.consent.is_some());
    assert_eq!(planned.commit, None, "nothing is committed here");
    assert_eq!(planned.copies.len(), 3);
}

#[test]
fn the_consent_holds_only_while_the_library_and_the_router_do() {
    let tmp = plane();
    let root = tmp.path();
    let first = planned(root, Some("ops"), None, "relevance").unwrap();
    assert_eq!(
        planned(root, Some("ops"), None, "relevance")
            .unwrap()
            .consent,
        first.consent
    );
    assert_ne!(
        planned(root, Some("ops"), None, "other").unwrap().consent,
        first.consent
    );
    // An untested copy of the library is handed to every chat, so it is consented to too.
    std::fs::write(
        root.join("skills/notes/SKILL.md"),
        "---\nname: notes\ndescription: Does notes, now otherwise.\n---\n",
    )
    .unwrap();
    assert_ne!(
        planned(root, Some("ops"), None, "relevance")
            .unwrap()
            .consent,
        first.consent
    );
}

#[test]
fn a_router_that_is_no_extension_s_id_is_refused() {
    let tmp = plane();
    assert!(planned(tmp.path(), Some("ops"), None, "../relevance").is_err());
}

fn part(skill: &str, router_passes: bool) -> SkillPart {
    let score = |arm: Arm, pass: bool| TriggerScore {
        query: "ship it to production".into(),
        should_trigger: true,
        arm,
        runs: 3,
        triggered: if pass { 3 } else { 0 },
        rate: Some(if pass { 1.0 } else { 0.0 }),
        pass: Some(pass),
        chats: vec![Some("01A".into()), Some("01B".into()), Some("01C".into())],
        asked: 0,
    };
    let arm = |arm: Arm| ArmScore {
        arm,
        pass_rate: None,
        tokens: None,
        seconds: None,
        chats: 3,
        not_run: 0,
    };
    SkillPart {
        skill: skill.to_owned(),
        report: GradeReport {
            grader: grade::BUILT_IN.to_owned(),
            triggers: vec![
                score(Arm::Router, router_passes),
                score(Arm::BuiltInRouter, true),
            ],
            arms: vec![arm(Arm::Router), arm(Arm::BuiltInRouter)],
            delta: None,
            ended: GradeEnd::Whole,
            not_started: None,
            cap: Some(200_000),
            spent_tokens: Some(6000),
            cost_cap_usd: None,
            spent_usd: None,
            counted_usd: None,
            priced: None,
        },
    }
}

fn graded(id: &str, ts: &str) -> RouterGraded {
    RouterGraded {
        v: VERSION,
        grade: id.to_owned(),
        router: "relevance".to_owned(),
        workspace: Some("ops".to_owned()),
        persona: None,
        commit: Some("a".repeat(40)),
        library: vec![RecordedCopy {
            skill: "deploy".to_owned(),
            level: Some("skills".to_owned()),
            tree: "b".repeat(40),
            commit: Some("a".repeat(40)),
        }],
        harness: Some("claude-code".to_owned()),
        profile: Some("claude".to_owned()),
        ts: ts.to_owned(),
        runs: 3,
        report: RouterGradeReport {
            grader: grade::BUILT_IN.to_owned(),
            router: "relevance".to_owned(),
            skills: vec![part("deploy", false), part("rollback", true)],
            ended: GradeEnd::Whole,
            not_started: None,
            cap: Some(200_000),
            spent_tokens: Some(12_000),
            cost_cap_usd: None,
            spent_usd: None,
            priced: None,
        },
        spent: Spent {
            chats: 12,
            tokens: Some(12_000),
            cost_usd: None,
            asked_for: None,
        },
        model: None,
    }
}

/// Writes `rows` as one month's committed file of routers' grades.
fn written(root: &Path, rows: &[serde_json::Value]) {
    let dir = root.join("personas/_grades/routers");
    std::fs::create_dir_all(&dir).unwrap();
    let text: String = rows.iter().map(|row| format!("{row}\n")).collect();
    std::fs::write(dir.join("2026-10.device.jsonl"), text).unwrap();
}

#[test]
fn a_router_s_grades_are_read_newest_first_and_a_row_it_cannot_take_is_skipped() {
    let tmp = plane();
    let root = tmp.path();
    let older = graded("01OLDER", "2026-10-01T10:00:00Z");
    let newer = graded("01NEWER", "2026-10-02T10:00:00Z");
    let mut forged = serde_json::to_value(graded("01FORGED", "2026-10-03T10:00:00Z")).unwrap();
    forged["library"][0]["skill"] = serde_json::json!("deploy\u{7}");
    let mut other = graded("01OTHER", "2026-10-04T10:00:00Z");
    other.router = "another".to_owned();
    other.report.router = "another".to_owned();
    written(
        root,
        &[
            serde_json::to_value(&older).unwrap(),
            serde_json::to_value(&newer).unwrap(),
            forged,
            serde_json::to_value(&other).unwrap(),
            serde_json::json!("not a row"),
        ],
    );

    let read = grades_of(root, "relevance", Some("ops"), None);
    assert_eq!(read, [newer.clone(), older]);
    assert_eq!(grades_of(root, "another", Some("ops"), None).len(), 1);
    // The same router chosen elsewhere is graded on another library: none of these is its.
    assert!(grades_of(root, "relevance", None, None).is_empty());
    assert!(grades_of(root, "relevance", None, Some("ops")).is_empty());
    let mut skill_arms = serde_json::to_value(&newer).unwrap();
    skill_arms["report"]["skills"][0]["report"]["arms"][0]["arm"] = serde_json::json!("with");
    skill_arms["report"]["skills"][0]["report"]["triggers"][0]["arm"] = serde_json::json!("with");
    written(root, &[skill_arms]);
    assert!(
        grades_of(root, "relevance", Some("ops"), None).is_empty(),
        "a router's row scored in a skill's arms is skipped"
    );
}

#[test]
fn a_grade_is_said_beside_the_library_s_commit_with_both_routers_passes() {
    let said = graded("01A", "2026-10-02T10:00:00Z").said();
    assert_eq!(
        said,
        "Graded on the library at aaaaaaaaaaaa, 2026-10-02: 1 of 2 trigger tests pass with the \
         router relevance, and 2 of 2 with the built-in router."
    );
    let mut uncommitted = graded("01A", "2026-10-02T10:00:00Z");
    uncommitted.commit = None;
    uncommitted.report.ended = GradeEnd::OverCap;
    assert!(
        uncommitted
            .said()
            .starts_with("Graded on a library with changes not committed, 2026-10-02 ("),
        "{}",
        uncommitted.said()
    );
}

#[test]
fn a_grade_is_recorded_in_the_committed_record_and_read_back() {
    let tmp = plane();
    let root = tmp.path();
    // At the plane's root, the library is the project's skills alone.
    let planned = planned(root, None, None, "relevance").unwrap();
    assert_eq!(planned.library, 2);
    let row = graded("01RECORDED", "2026-10-02T10:00:00Z");
    let path = record(root, &planned.copies, &row, "device").expect("recorded");
    assert!(
        path.starts_with(root.join("personas/_grades/routers")),
        "{path:?}"
    );
    assert_eq!(grades_of(root, "relevance", Some("ops"), None), [row]);
    // A skill's own grades never read a router's.
    assert!(super::super::grades_of(root, "deploy", None).is_empty());
}

#[test]
fn a_grade_of_a_library_with_a_copy_of_this_machine_s_own_stays_here() {
    let tmp = plane();
    let root = tmp.path();
    // The workspace `ops` is this machine's alone, and its skill `rollback` with it.
    let planned = planned(root, Some("ops"), None, "relevance").unwrap();
    let row = graded("01LOCAL", "2026-10-02T10:00:00Z");
    let path = record(root, &planned.copies, &row, "device").expect("recorded");
    assert!(
        path.starts_with(crate::appstate::dir(root).unwrap()),
        "{path:?}"
    );
    assert!(!root.join("personas/_grades/routers").exists());
    assert_eq!(grades_of(root, "relevance", Some("ops"), None), [row]);
}

#[test]
fn every_chat_of_a_router_s_grade_reads_the_pins_of_the_scope_that_chose_it() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::write(
        root.join("workspaces/ops/workspace.json"),
        r#"{"settings": {"skills": {"pinned": ["deploy"]}}}"#,
    )
    .unwrap();
    let pinned = planned(root, Some("ops"), None, "relevance").unwrap();
    assert_eq!(pinned.pins, ["deploy"]);
    // A pin is consented to: the plan without it is another plan.
    std::fs::write(root.join("workspaces/ops/workspace.json"), "{}").unwrap();
    assert_ne!(
        planned(root, Some("ops"), None, "relevance")
            .unwrap()
            .consent,
        pinned.consent
    );
    // And the arm laid out with it is read with it.
    let laid = crate::skills::verbs::drafts::lay_out_library_arm(
        root,
        &[],
        Some("relevance"),
        &pinned.pins,
    )
    .unwrap();
    let tried = crate::skills::verbs::drafts::tried(root, &laid.piece).unwrap();
    assert_eq!(crate::skills::pins_or_none(&tried, None, None), ["deploy"]);
}

/// #3616: a router's grade purlis stopped under before it scored a test says it was
/// interrupted, and no count of tests that passed.
#[test]
fn an_interrupted_grade_says_it_was_interrupted_and_no_count_3616() {
    let mut row = graded("01A", "2026-10-02T10:00:00Z");
    row.report = interrupted_report("relevance", 200_000, None, None);
    assert!(row.sound(chrono::Utc::now()), "{row:?}");
    assert_eq!(
        row.said(),
        "Interrupted on the library at aaaaaaaaaaaa, 2026-10-02: purlis stopped while it ran, \
         before any trigger test was scored."
    );
}
