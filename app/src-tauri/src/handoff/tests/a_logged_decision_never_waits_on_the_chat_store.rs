// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A dispatch decided under `log` returns** (#7524). The decision is made while the chat
//! store's `open` lock is held (`Chats::deciding_over`), and under `log` the core records a
//! would-have-blocked line from inside it. The app finds that line's chat by the same lock, so
//! a write made there waited on itself: the decision never returned, and each hook connection
//! behind it was held until the channel's cap. The scenario "every Blocking ask, found from its
//! project's tab" hung on it.
//!
//! Red without `decisions::write_line_apart` (a write in place waits on the lock for ever),
//! green with it: the call comes back, and the line is in the log once the writer is joined.

use purlis_core::eventlog::stops::{WOULD_BLOCK, WouldBlock};

use super::*;

#[test]
fn a_decision_made_under_log_returns_and_its_would_block_line_is_written() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let log = plane.root.join("events");
    let planes = planes_recording_events(&log);
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    // A chat the store has no number for: the writer still looks it up under the store's lock,
    // which is what waited, and the line is kept under no chat (S3).
    let chat_id = purlis_core::enforcement::UNKNOWN_CHAT.to_owned();
    let line = WouldBlock {
        rules: vec!["dispatch-ask".to_owned()],
        level: None,
        family: "dispatch".to_owned(),
        host: None,
    };

    let inside = held.clone();
    // As `dispatchdecision` runs it: with the store's lock held. The call returns the writer's
    // handle at once; were the write made in place, it would wait on the lock for ever.
    let writer = std::thread::spawn(move || {
        inside.chats().deciding_over(|_, _| {
            crate::decisions::write_line_apart(inside.clone(), &chat_id, &line)
        })
    });
    // Joined, not slept on: the lock is free once the decision returned, so the writer ends.
    writer
        .join()
        .expect("a decision that records a would-block line under the store's lock returns")
        .join()
        .expect("the writer thread finished");

    let wrote = purlis_core::eventlog::read(&log)
        .map(|events| events.iter().filter(|e| e.kind == WOULD_BLOCK).count())
        .unwrap_or(0);
    assert_eq!(wrote, 1, "the line is written once the writer is joined");
}
