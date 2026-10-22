// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_skills::runs::{Call, Heard, Spans, Start};

const RUN: &str = "01JB0000000000000000000RUN";

fn may() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-05-04T11:32:17+00:00")
        .unwrap()
        .with_timezone(&chrono::Utc)
}

fn a_plane() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(crate::plane::MANIFEST), "schema = 2\n").unwrap();
    dir
}

/// A run of `skill`, opened in a span of its own.
fn opened(spans: &mut Spans, skill: &str) -> Open {
    spans
        .open(
            "chat-run",
            Start {
                skill,
                id: RUN,
                started_by: StartedBy::SkillTool,
                whose: &purlis_skills::runs::Whose::default(),
                at_ms: 1_000,
                tokens: None,
            },
        )
        .cloned()
        .unwrap()
}

#[test]
fn a_skill_runs_start_is_logged_with_the_persona_or_null() {
    let dir = a_plane();
    let when = may();
    let mut spans = Spans::default();
    let p = started(
        dir.path(),
        &Starting::from(&opened(&mut spans, "charter:persona")),
        Some("ops"),
        when,
        "box",
    )
    .unwrap();
    started(
        dir.path(),
        &Starting::from(&opened(&mut Spans::default(), "x")),
        None,
        when,
        "box",
    )
    .unwrap();
    assert_eq!(
        started(
            dir.path(),
            &Starting::from(&opened(&mut Spans::default(), " ")),
            None,
            when,
            "box"
        ),
        None
    );
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
fn no_skill_run_is_logged_in_a_project_this_purlis_may_not_write() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(crate::plane::MANIFEST),
        "schema = 2\nrequires = [{ feature = \"from-the-future\", since = \"9.0.0\" }]\n",
    )
    .unwrap();
    let open = opened(&mut Spans::default(), "charter:persona");
    assert_eq!(
        started(
            dir.path(),
            &Starting::from(&open),
            Some("ops"),
            chrono::Utc::now(),
            "box"
        ),
        None
    );
    assert!(!dir.path().join("personas").exists());
}

#[test]
fn a_runs_start_and_end_read_back_as_one_run_with_its_signals_and_never_a_verdict() {
    let dir = a_plane();
    let when = may();
    let mut spans = Spans::default();
    let open = opened(&mut spans, "deploy");
    started(dir.path(), &Starting::from(&open), Some("ops"), when, "box").unwrap();
    spans.heard(
        "chat-run",
        Heard {
            call: Some(Call::Id("toolu_1")),
            refused: true,
            ..Heard::default()
        },
    );
    let closed = spans.close("chat-run", Ended::Turn, 4_500);
    ended(dir.path(), &closed[0], Some(1200), when, "box").unwrap();

    let runs = runs(dir.path());

    assert_eq!(
        runs,
        vec![Run {
            id: Some(RUN.to_owned()),
            skill: "deploy".to_owned(),
            persona: Some("ops".to_owned()),
            started_by: Some(StartedBy::SkillTool),
            started: "2026-05-04T11:32:17+00:00".to_owned(),
            device: "box".to_owned(),
            copy: None,
            end: Some(End {
                ended: Ended::Turn,
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
fn a_row_written_before_runs_were_recorded_reads_as_a_run_of_its_own_with_no_start_or_signals() {
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
            (run.id.as_deref(), run.started_by, &run.end),
            (None, None, &None),
            "recorded before runs were: {run:#?}"
        );
    }
}
