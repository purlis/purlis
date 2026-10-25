// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::super::StudioRun;
use super::*;
use crate::skilluse::{End, Run};
use purlis_skills::runs::{Ended, StartedBy};

fn run(id: Option<&str>, started_by: Option<StartedBy>, end: Option<End>) -> StudioRun {
    StudioRun {
        inside: None,
        ran: Vec::new(),
        run: Run {
            id: id.map(str::to_owned),
            skill: "deploy".to_owned(),
            persona: id.map(|_| "ops".to_owned()),
            started_by,
            started: "2026-10-20T10:00:05+00:00".to_owned(),
            device: "d".to_owned(),
            copy: None,
            harness: id.map(|_| "claude".to_owned()),
            workspace: None,
            content: None,
            end,
        },
    }
}

fn ended(tokens: Option<u64>) -> End {
    End {
        ended: Ended::Interrupted,
        at: "2026-10-20T10:00:08+00:00".to_owned(),
        ms: 3500,
        tool_calls: 7,
        tool_errors: 1,
        refusals: 2,
        asks: 1,
        tokens,
        parent: None,
    }
}

#[test]
fn the_runs_panel_is_a_table_of_each_runs_signals_and_never_a_verdict() {
    let runs = [
        run(Some("01JB"), Some(StartedBy::Read), Some(ended(None))),
        run(None, None, None),
    ];
    let blocks = runs_blocks(&runs, "deploy");
    let [Block::Note { text, .. }, Block::Table(table)] = blocks.as_slice() else {
        panic!("{blocks:?}");
    };
    assert!(text.contains("never a verdict"), "{text}");
    assert_eq!(table.caption, "Runs of deploy");
    assert_eq!(table.columns.len(), 12);
    assert_eq!(
        table.rows[0].cells,
        [
            "2026-10-20 10:00 UTC",
            "ops",
            "Claude Code",
            "The chat read the skill",
            "Interrupted",
            "7",
            "1",
            "2",
            "1",
            "Not reported",
            "3 s",
            "",
        ]
    );
    assert_eq!(table.rows[0].key, "01JB");
    assert_eq!(table.rows[1].key, "before-1");
    assert_eq!(table.rows[1].cells[3], "From before runs were recorded");
    assert_eq!(table.rows[1].cells[4], "Not recorded");
    let every: String = table.rows.iter().flat_map(|it| it.cells.clone()).collect();
    let every = every.to_lowercase();
    assert!(
        !every.contains("pass") && !every.contains("fail"),
        "{every}"
    );
}

#[test]
fn a_skill_with_no_runs_says_so_and_how_one_starts() {
    let blocks = runs_blocks(&[], "deploy");
    let [Block::Table(table)] = blocks.as_slice() else {
        panic!("{blocks:?}");
    };
    assert!(table.rows.is_empty());
    assert_eq!(table.empty.headline, "No runs of deploy yet");
}

#[test]
fn the_runs_panel_answers_as_every_studio_panel_does() {
    let plane = tempfile::tempdir().expect("a project");
    let runs = RunsPanel {
        plane: plane.path(),
    };
    assert_eq!(runs.key(), PanelKey::Runs);
    assert_eq!(runs.by(), None);
    let answered = Answered::of(&runs, "deploy");
    assert_eq!(answered.title, "Runs");
    assert!(matches!(answered.blocks.as_deref(), Ok([Block::Table(_)])));
}

#[test]
fn a_time_and_a_stamp_read_as_a_person_reads_them() {
    assert_eq!(time_said(850), "850 ms");
    assert_eq!(time_said(12_000), "12 s");
    assert_eq!(time_said(184_000), "3 min 4 s");
    assert_eq!(said_at("2026-10-20T10:00:05+00:00"), "2026-10-20 10:00 UTC");
    assert_eq!(said_at("yesterday"), "yesterday");
    assert_eq!(
        started_by_said(Some(StartedBy::McpLoad)),
        "The chat loaded it through purlis"
    );
    assert_eq!(ended_said(None, false), "Running, or not recorded");
}

#[test]
fn without_a_studio_the_templates_are_purlis_s_own() {
    use purlis_skills::change::Template;
    let templates = Templates { studio: None };
    let ids: Vec<TemplateId> = templates.offered().into_iter().map(|it| it.id).collect();
    assert_eq!(
        ids,
        [
            TemplateId::BuiltIn(Template::Blank),
            TemplateId::BuiltIn(Template::Steps)
        ]
    );
    let text = templates
        .text(
            &TemplateId::BuiltIn(Template::Steps),
            "deploy",
            "Deploys the service when asked to.",
        )
        .expect("a built-in");
    assert!(text.contains("## Steps"), "{text}");
    let elsewhere = TemplateId::Extension {
        extension: "ext".to_owned(),
        template: "one".to_owned(),
    };
    assert!(templates.text(&elsewhere, "deploy", "D").is_err());
}

#[test]
fn an_empty_runs_panel_offers_a_new_chat() {
    let blocks = runs_blocks(&[], "deploy");
    let [Block::Table(table)] = blocks.as_slice() else {
        panic!("{blocks:?}");
    };
    assert_eq!(table.empty.offer.as_deref(), Some("chat.new"));
}

/// One grade of `deploy` whose arm with the skill passed every test and whose arm without it
/// passed half, on its commit.
fn a_grade() -> super::super::grades::Graded {
    use purlis_skills::grade::{Arm, ArmScore, GradeEnd, GradeReport, Stat, TriggerScore};
    let stat = |mean: f64, stddev: f64| Stat {
        mean,
        stddev,
        min: mean,
        max: mean,
    };
    let score = |arm: Arm, pass: bool| TriggerScore {
        query: "ship it".into(),
        should_trigger: true,
        arm,
        runs: 3,
        triggered: if pass { 3 } else { 0 },
        rate: Some(if pass { 1.0 } else { 0.0 }),
        pass: Some(pass),
        chats: Vec::new(),
    };
    let arm = |arm: Arm, mean: f64, stddev: f64| ArmScore {
        arm,
        pass_rate: Some(stat(mean, stddev)),
        tokens: None,
        seconds: None,
        chats: 6,
        not_run: 0,
    };
    super::super::grades::Graded {
        v: super::super::grades::VERSION,
        grade: "g1".into(),
        skill: "deploy".into(),
        level: Some("skills".into()),
        tree: "a".repeat(40),
        commit: Some("0123456789abcdef0123456789abcdef01234567".into()),
        harness: Some("claude".into()),
        profile: Some("work".into()),
        ts: "2026-10-25T10:00:05+00:00".into(),
        runs: 3,
        report: GradeReport {
            grader: purlis_skills::grade::BUILT_IN.into(),
            triggers: vec![score(Arm::With, true), score(Arm::Without, false)],
            arms: vec![arm(Arm::With, 1.0, 0.0), arm(Arm::Without, 0.5, 0.12)],
            delta: Some(0.5),
            ended: GradeEnd::Whole,
            not_started: None,
            cap: None,
            spent_tokens: None,
        },
        spent: super::super::grades::Spent {
            chats: 12,
            tokens: None,
            cost_usd: None,
        },
    }
}

#[test]
fn the_grades_panel_is_a_scorecard_of_each_grade_against_no_skill() {
    let blocks = grades_blocks(&[a_grade()], "deploy");
    let [Block::Note { text, .. }, Block::Table(table)] = blocks.as_slice() else {
        panic!("{blocks:?}");
    };
    assert!(text.contains("3 times each"), "{text}");
    assert_eq!(table.caption, "Grades of deploy");
    assert_eq!(
        table.rows[0].cells,
        [
            "2026-10-25 10:00 UTC",
            "0123456",
            "Claude Code",
            "1 of 1",
            "100%",
            "50% ± 12",
            "+50 points",
            "12 chats",
            "Finished",
        ]
    );
    assert_eq!(table.columns.len(), table.rows[0].cells.len());
}

#[test]
fn a_skill_with_no_grades_says_so_and_how_one_is_made() {
    let blocks = grades_blocks(&[], "deploy");
    let [Block::Table(table)] = blocks.as_slice() else {
        panic!("{blocks:?}");
    };
    assert_eq!(table.empty.headline, "No grades of deploy yet");
    assert!(table.empty.body.as_deref().unwrap().contains("Grade"));
}

#[test]
fn purlis_s_own_panels_are_its_grades_then_its_runs() {
    let plane = tempfile::tempdir().expect("a project");
    let keys: Vec<PanelKey> = own_panels(plane.path(), "deploy")
        .into_iter()
        .map(|panel| panel.key)
        .collect();
    assert_eq!(keys, [PanelKey::Grades, PanelKey::Runs]);
}
