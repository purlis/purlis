// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Each answer's span** (FS-M1a, #7893): the door's, and a synchronous command's, as the slow
//! log's layer reads them ([`purlis_core::slowlog::answers`]).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_core::slowlog::Slow;
use purlis_core::slowlog::answers::AnswerSpans;
use tracing_subscriber::layer::SubscriberExt;

/// What the layer wrote while `f` ran, and the summary it holds after: `f`'s spans the only
/// ones it hears.
fn answered<T>(f: impl FnOnce() -> T) -> (T, Vec<Slow>, Option<Slow>) {
    let written = Arc::new(Mutex::new(Vec::new()));
    let layer = AnswerSpans::writing_to(
        {
            let written = Arc::clone(&written);
            move |slow| written.lock().unwrap().push(slow)
        },
        || 1_800_000_000_000,
    );
    let subscriber = tracing_subscriber::registry().with(layer.clone());
    let done = tracing::subscriber::with_default(subscriber, f);
    let written = written.lock().unwrap().clone();
    (done, written, layer.summary_now())
}

/// The one command the summary `summary` holds, as `(command, class, n)`.
fn summarised(summary: Option<Slow>) -> Vec<(String, String, u32)> {
    let Some(Slow::AnswerSummary { commands, .. }) = summary else {
        return Vec::new();
    };
    commands
        .into_iter()
        .map(|one| (one.command, one.class, one.n))
        .collect()
}

#[test]
fn the_door_tells_its_commands_answer_with_its_class_its_time_in_line_and_its_work() {
    // An argument the work holds, which the span must never carry (ADR 0075).
    let plane = String::from("/Users/someone/secret-project");
    let (answer, written, _) = answered(|| {
        tauri::async_runtime::block_on(crate::answer("plane_sidebar", "reading the sidebar", {
            move || {
                std::thread::sleep(Duration::from_millis(250));
                Ok(plane.len())
            }
        }))
    });
    assert_eq!(answer, Ok(29));
    let [
        Slow::AnswerSpan {
            command,
            class,
            work_ms,
            ..
        },
    ] = written.as_slice()
    else {
        panic!("one answer span: {written:?}");
    };
    assert_eq!(
        (command.as_str(), class.as_str()),
        ("plane_sidebar", "model")
    );
    assert!(*work_ms >= 250, "{work_ms}");
    let line = serde_json::to_string(&written[0]).unwrap();
    assert!(!line.contains("secret"), "{line}");
}

#[test]
fn an_answer_through_the_unnamed_door_is_told_as_unnamed() {
    let (_, _, summary) = answered(|| {
        tauri::async_runtime::block_on(crate::off_the_window("reading the sidebar", || Ok(())))
    });
    assert_eq!(
        summarised(summary),
        [("unnamed".to_owned(), "unmeasured".to_owned(), 1)]
    );
}

#[test]
fn a_synchronous_commands_answer_is_told_where_it_runs_and_an_async_ones_only_by_its_door() {
    // `send_input` answers on the window's thread; `plane_sidebar`'s handler only hands its work
    // to the door, whose span tells it, so a span here would count a dispatch as an answer.
    let (_, _, summary) = answered(|| {
        crate::mainwatch::answered_here("send_input", || ());
        crate::mainwatch::answered_here("plane_sidebar", || ());
    });
    assert_eq!(
        summarised(summary),
        [("send_input".to_owned(), "window".to_owned(), 1)]
    );
}
