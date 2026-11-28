// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tracing_subscriber::layer::SubscriberExt;

use super::*;

/// A layer whose lines are kept here and whose clock a test moves.
struct Watched {
    layer: AnswerSpans,
    written: Arc<Mutex<Vec<Slow>>>,
    clock: Arc<AtomicU64>,
}

const START: u64 = 1_800_000_000_000;

fn watched() -> Watched {
    let written = Arc::new(Mutex::new(Vec::new()));
    let clock = Arc::new(AtomicU64::new(START));
    let layer = AnswerSpans::writing_to(
        {
            let written = Arc::clone(&written);
            move |slow| written.lock().unwrap().push(slow)
        },
        {
            let clock = Arc::clone(&clock);
            move || clock.load(Ordering::SeqCst)
        },
    );
    Watched {
        layer,
        written,
        clock,
    }
}

impl Watched {
    /// `f`, with this layer the only one listening.
    fn listening<T>(&self, f: impl FnOnce() -> T) -> T {
        let subscriber = tracing_subscriber::registry().with(self.layer.clone());
        tracing::subscriber::with_default(subscriber, f)
    }

    /// One answer to `command` that waited `queued_ms` and worked `work_ms`, recorded as the
    /// app's door records it: the times after the span is opened.
    fn answer(&self, command: &str, queued_ms: u64, work_ms: u64) {
        self.listening(|| {
            let span = tracing::info_span!(
                "answer",
                command,
                class = "model",
                queued_ms = tracing::field::Empty,
                work_ms = tracing::field::Empty,
            );
            span.record("queued_ms", queued_ms);
            span.in_scope(|| {});
            span.record("work_ms", work_ms);
        });
    }

    fn written(&self) -> Vec<Slow> {
        self.written.lock().unwrap().clone()
    }
}

#[test]
fn an_answer_span_line_carries_its_command_and_class_and_never_an_argument() {
    // ADR 0075: a span may carry more than the four fields the layer reads, an argument a
    // caller added by mistake among them; only the four reach the log.
    let watched = watched();
    watched.listening(|| {
        let span = tracing::info_span!(
            "answer",
            command = "plane_sidebar",
            class = "model",
            queued_ms = 40_u64,
            work_ms = tracing::field::Empty,
            plane = "/Users/someone/secret-project",
            subjects = ?["persona:secret"],
        );
        span.record("work_ms", 300_u64);
    });
    let written = watched.written();
    assert_eq!(
        written,
        vec![Slow::AnswerSpan {
            command: "plane_sidebar".into(),
            class: "model".into(),
            queued_ms: 40,
            work_ms: 300,
            at: START - 340,
        }]
    );
    let line = serde_json::to_value(&written[0]).unwrap();
    let mut keys: Vec<&str> = line
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["at", "class", "command", "kind", "queued_ms", "work_ms"]
    );
    assert!(!line.to_string().contains("secret"), "{line}");
}

#[test]
fn a_quick_answer_writes_no_line_of_its_own_and_is_counted_in_the_summary() {
    let watched = watched();
    // Under the kind's threshold in all, though each half alone is not far off it.
    watched.answer("sandbox_state", 100, 99);
    assert_eq!(watched.written(), Vec::new());
    watched.answer("sandbox_state", 100, 100);
    assert_eq!(watched.written().len(), 1, "{:?}", watched.written());
    let summary = watched.layer.summary_now().expect("two answers closed");
    let Slow::AnswerSummary { commands, .. } = summary else {
        panic!("{summary:?}");
    };
    assert_eq!(commands[0].n, 2);
}

#[test]
fn a_span_of_another_name_or_one_never_worked_is_not_an_answer() {
    let watched = watched();
    watched.listening(|| {
        let other = tracing::info_span!(
            "reading",
            command = "plane_sidebar",
            class = "model",
            work_ms = 5_000_u64,
        );
        drop(other);
        // Its work never ended: the thread it ran on panicked before recording it.
        let unfinished = tracing::info_span!(
            "answer",
            command = "plane_sidebar",
            class = "model",
            queued_ms = 5_000_u64,
            work_ms = tracing::field::Empty,
        );
        drop(unfinished);
    });
    assert_eq!(watched.written(), Vec::new());
    assert_eq!(watched.layer.summary_now(), None);
}

#[test]
fn the_summary_is_written_by_the_first_answer_an_hour_after_the_last_with_each_commands_spread() {
    let watched = watched();
    for ms in 1..=100 {
        watched.answer("sandbox_state", 0, ms);
    }
    watched.answer("plane_sidebar", 0, 7);
    let spans = watched.written().len();
    // A minute short of the hour, nothing yet.
    watched
        .clock
        .store(START + HOUR_MS - 60_000, Ordering::SeqCst);
    watched.answer("plane_sidebar", 0, 9);
    assert_eq!(watched.written().len(), spans);

    watched.clock.store(START + HOUR_MS, Ordering::SeqCst);
    watched.answer("plane_sidebar", 0, 11);
    let written = watched.written();
    assert_eq!(written.len(), spans + 1);
    assert_eq!(
        written.last(),
        Some(&Slow::AnswerSummary {
            at: START + HOUR_MS,
            since: START,
            commands: vec![
                Spread {
                    command: "plane_sidebar".into(),
                    class: "model".into(),
                    n: 3,
                    p50_ms: 9,
                    p99_ms: 11,
                },
                Spread {
                    command: "sandbox_state".into(),
                    class: "model".into(),
                    n: 100,
                    p50_ms: 50,
                    p99_ms: 99,
                },
            ],
        })
    );
    // Counted again from there: the next hour holds none of these.
    watched.answer("plane_sidebar", 0, 3);
    let Some(Slow::AnswerSummary {
        since, commands, ..
    }) = watched.layer.summary_now()
    else {
        panic!("one answer closed since");
    };
    assert_eq!(since, START + HOUR_MS);
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].n, 1);
}

#[test]
fn a_spread_of_long_answers_is_read_within_a_hundredth() {
    let mut spreads = Spreads::since(0);
    for ms in [12_345, 12_345, 12_345, 250_000] {
        spreads.add("finished_turns", "unmeasured", ms);
    }
    let Some(Slow::AnswerSummary { commands, .. }) = spreads.take(1) else {
        panic!("answers were added");
    };
    let within = |read: u32, was: u32| read.abs_diff(was) * 100 <= was;
    assert!(within(commands[0].p50_ms, 12_345), "{commands:?}");
    assert!(within(commands[0].p99_ms, 250_000), "{commands:?}");
}
