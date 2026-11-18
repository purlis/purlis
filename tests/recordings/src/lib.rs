// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Recordings of the real harnesses** (T1.1-03, #6850): the fidelity anchor the fake harness
//! reproduces (ticket 4). Each harness, at the version its file in
//! `tests/fixtures/harness-versions` pins, runs a covering script under purlis's wrap at Full
//! against a stand-in model, with its hook entries naming the recorder in place of `purlis`.
//!
//! - [`recorder`]: the program the hook entries name, which writes every hook call.
//! - [`standin`]: the stand-in model, in the Messages, Responses and Chat Completions APIs.
//! - [`script`]: the covering script, written once for every harness.
//! - [`projection`]: what the model was shown, from the requests the stand-in received.
//! - [`recording`]: a recording as checked in, and the check that each pinned version has one.
//! - [`mask`]: what may differ between two recordings of one script, a closed list.
//! - [`turns`]: the turn script that reproduces a recording in the fake harness.
//! - [`drift`]: the nightly re-record's diff, and the issue it opens.

pub mod drift;
pub mod mask;
pub mod projection;
pub mod recorder;
pub mod recording;
pub mod script;
pub mod standin;
pub mod turns;

use std::path::PathBuf;

/// This crate's folder: `tests/recordings` in the repo.
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The repo's root.
pub fn repo() -> PathBuf {
    root()
        .parent()
        .and_then(|tests| tests.parent())
        .map(PathBuf::from)
        .expect("tests/recordings is two folders below the repo's root")
}
