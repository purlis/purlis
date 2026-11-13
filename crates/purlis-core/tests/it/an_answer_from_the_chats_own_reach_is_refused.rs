// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat answering its own asks can never be switched on** (R-036, V16a). A merge is pressed
//! only by an [`Answerer`], and an answerer is made only from a connection the host admitted as
//! the window or `charter inbox`: never from a chat's own socket or token, and no setting,
//! policy key or line a chat sends makes one. What a chat sends on its socket can ask, and
//! nothing it sends answers.
//!
//! A pin of shipped behaviour (Q208): it is seen red by letting [`Admitted::Chat`] answer.

use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::hookwire::Ask;

#[test]
fn an_answer_from_the_chats_own_reach_is_refused() {
    purlis_core::unsteered!();
    // The chat's own reach: its hook socket and its token, and every client scope a process
    // of its may reach. None of them is an answerer, so none can press a merge.
    for reach in [
        Admitted::Chat,
        Admitted::Terminal,
        Admitted::FleetMcp,
        Admitted::Editor,
    ] {
        assert_eq!(Answerer::admitted(reach), None, "{reach:?}");
    }
    // Only the person's own clients answer.
    assert!(Answerer::admitted(Admitted::LocalUi).is_some());
    assert!(Answerer::admitted(Admitted::Approval).is_some());

    // A line on the chat's socket that names an answer to a merge ask is no ask the app takes.
    for line in [
        r#"{"merge_press":{"chat":4,"repo":"o/r","number":12}}"#,
        r#"{"merge":{"chat":4,"request":{"number":12},"cwd":".","answer":"merge"}}"#,
        r#"{"answer":{"chat":4,"ask":"merge:4:o/r#12","option":"merge"}}"#,
    ] {
        assert!(serde_json::from_str::<Ask>(line).is_err(), "{line}");
    }
}
