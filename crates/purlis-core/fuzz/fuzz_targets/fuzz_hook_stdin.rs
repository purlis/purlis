// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis hook`'s stdin** (T1.1-47): the JSON a harness hands a hook, which a chat can shape
//! through its tool calls and their output. Each core reader `purlis hook` hands it to reads it
//! without panicking, and stays inside its bounds: a turn's last message cut to what it was asked
//! for, at most [`AT_MOST_PER_RESULT`] blocks from one result.

#![no_main]

use std::path::Path;

use libfuzzer_sys::fuzz_target;
use purlis_core::hookwire::{Output, Speaker};
use purlis_core::sandboxblock::{self, AT_MOST_PER_RESULT, Place};

fuzz_target!(|payload: &str| {
    let none = |_: &str| None;
    let _ = Speaker::read(payload, &none);
    for most in [0, 7, 64] {
        if let Some(Output::Final { text }) = Output::of_stop_within(payload, most) {
            assert!(text.len() <= most, "{} > {most}", text.len());
        }
    }
    let Ok(read) = serde_json::from_str::<serde_json::Value>(payload) else {
        return;
    };
    let place = Place {
        root: Path::new("/plane/project"),
        chat: Path::new("/plane/project/chat"),
        cwd: Path::new("/plane/project/chat"),
        home: Some(Path::new("/home/someone")),
    };
    let blocks = sandboxblock::detect(&read, &place);
    assert!(blocks.len() <= AT_MOST_PER_RESULT, "{}", blocks.len());
    let _ = sandboxblock::read_back(&read);
    let _ = sandboxblock::printed_port(&read);
});
