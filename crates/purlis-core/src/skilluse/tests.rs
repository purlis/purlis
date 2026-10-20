// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_skills::runs::StartedBy;

const RUN: &str = "01JB0000000000000000000RUN";

fn started<'a>(skill: &'a str, persona: Option<&'a str>) -> Started<'a> {
    Started {
        run: RUN,
        skill,
        persona,
        started_by: StartedBy::SkillTool,
    }
}

fn may() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-05-04T11:32:17+00:00")
        .unwrap()
        .with_timezone(&chrono::Utc)
}

#[test]
fn a_skill_runs_start_is_logged_with_the_persona_or_null() {
    let dir = tempfile::tempdir().unwrap();
    let when = may();
    let p = record(
        dir.path(),
        &started("charter:persona", Some("ops")),
        when,
        "box",
    )
    .unwrap();
    record(dir.path(), &started("x", None), when, "box").unwrap();
    assert_eq!(record(dir.path(), &started(" ", None), when, "box"), None);
    assert_eq!(
        std::fs::read_to_string(p).unwrap(),
        "{\"persona\": \"ops\", \"run\": \"01JB0000000000000000000RUN\", \"skill\": \
         \"charter:persona\", \"started_by\": \"skill-tool\", \"ts\": \
         \"2026-05-04T11:32:17+00:00\"}\n{\"persona\": null, \"run\": \
         \"01JB0000000000000000000RUN\", \"skill\": \"x\", \"started_by\": \"skill-tool\", \
         \"ts\": \"2026-05-04T11:32:17+00:00\"}\n"
    );
}

#[test]
fn no_skill_use_is_logged_in_a_project_this_purlis_may_not_write() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(crate::plane::MANIFEST),
        "schema = 2\nrequires = [{ feature = \"from-the-future\", since = \"9.0.0\" }]\n",
    )
    .unwrap();
    let when = chrono::Utc::now();
    assert_eq!(
        record(
            dir.path(),
            &started("charter:persona", Some("ops")),
            when,
            "box"
        ),
        None
    );
    assert!(!dir.path().join("personas").exists());
}

#[test]
fn a_runs_start_and_end_read_back_as_one_run_with_its_signals_and_never_a_verdict() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(crate::plane::MANIFEST), "schema = 2\n").unwrap();
    let when = may();
    record(dir.path(), &started("deploy", Some("ops")), when, "box").unwrap();
    let mut spans = purlis_skills::runs::Spans::default();
    spans.open(
        "chat-run",
        purlis_skills::runs::Invoked {
            skill: "deploy",
            id: RUN,
            started_by: StartedBy::SkillTool,
            at_ms: 1_000,
            tokens: None,
        },
    );
    spans.heard(
        "chat-run",
        purlis_skills::runs::Heard {
            call: Some(Some("toolu_1")),
            refused: true,
            ..purlis_skills::runs::Heard::default()
        },
    );
    let closed = spans.close("chat-run", purlis_skills::runs::Ended::Turn, 4_500);
    ended(
        dir.path(),
        &Ending {
            run: &closed[0],
            tokens: Some(1200),
        },
        when,
        "box",
    )
    .unwrap();

    let runs = runs(dir.path());

    assert_eq!(
        runs,
        vec![Run {
            id: Some(RUN.to_owned()),
            skill: "deploy".to_owned(),
            persona: Some("ops".to_owned()),
            started_by: Some("skill-tool".to_owned()),
            started: "2026-05-04T11:32:17+00:00".to_owned(),
            device: "box".to_owned(),
            end: Some(End {
                ended: "turn".to_owned(),
                at: "2026-05-04T11:32:17+00:00".to_owned(),
                ms: 3_500,
                tool_calls: 1,
                tool_errors: 0,
                refusals: 1,
                asks: 0,
                tokens: Some(1200),
                parent: None,
            }),
        }]
    );
}

#[test]
fn a_row_written_before_runs_reads_as_a_run_of_its_own_with_no_start_and_no_signals() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("personas").join(DIR_NAME);
    std::fs::create_dir_all(&log).unwrap();
    std::fs::write(
        log.join("2026-03.laptop.jsonl"),
        "{\"persona\": \"ops\", \"skill\": \"charter:secrets\", \"ts\": \"2026-03-02T09:00:00+00:00\"}\n\
         {\"persona\": null, \"skill\": \"x\", \"ts\": \"2026-03-03T09:00:00+00:00\"}\n",
    )
    .unwrap();

    let runs = runs(dir.path());

    assert_eq!(runs.len(), 2, "{runs:#?}");
    assert_eq!(runs[0].skill, "x", "newest first");
    assert_eq!(runs[0].persona, None);
    assert_eq!(runs[1].skill, "charter:secrets");
    assert_eq!(runs[1].persona.as_deref(), Some("ops"));
    assert_eq!(runs[1].device, "laptop");
    for run in &runs {
        assert_eq!(
            (run.id.as_deref(), run.started_by.as_deref(), &run.end),
            (None, None, &None),
            "recorded before runs: {run:#?}"
        );
    }
}
