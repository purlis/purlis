// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The engine inherits descriptors 0 to 4 and nothing else: not even a descriptor its host holds
//! open across exec. The engine here is a stand-in that writes which descriptors it holds, then
//! answers its first DevTools call as a real one would.

#![cfg(unix)]

use crate::common;

use std::time::{Duration, Instant};

use nix::fcntl::{FcntlArg, fcntl};
use purlis_browser_engine::launch::{Launch, ReachProxy};
use purlis_browser_engine::process::{Engine, Keeper};

#[test]
fn the_engine_inherits_only_its_pipe_and_its_standard_streams() {
    // The descriptor below stays open for the rest of its process, where every other test's
    // child would inherit it: under `cargo test`, which runs this binary's tests as threads of
    // one process, the test runs in a process of its own (#3613).
    if crate::alone() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    // A descriptor the host holds open across exec, as one it inherited itself would be.
    // `F_DUPFD` gives a copy above every descriptor the engine is meant to have, and without
    // close-on-exec. It stays open for the rest of this test process.
    let held = std::fs::File::open("/dev/null").unwrap();
    let leaked = fcntl(&held, FcntlArg::F_DUPFD(60)).unwrap();
    let seen = dir.path().join("seen");
    let program = stand_in::program(
        dir.path(),
        "engine",
        &format!(
            "#!/bin/sh\n\
             for n in 0 1 2 3 4 {leaked}; do [ -e /dev/fd/$n ] && printf '%s ' $n; done > '{seen}.part'\n\
             mv '{seen}.part' '{seen}'\n\
             head -c 1 <&3 >/dev/null\n\
             printf '{{\"id\":1,\"result\":{{\"product\":\"stand-in\"}}}}\\000' >&4\n\
             exec sleep 600\n",
            seen = seen.display()
        ),
    );
    let launch = Launch::new(
        &common::fixture::chosen(&program),
        dir.path().join("profile"),
        ReachProxy("127.0.0.1:9".parse().unwrap()),
    );
    let engine = Engine::start(
        &launch,
        &Keeper::program(env!("CARGO_BIN_EXE_purlis-engine-keeper")),
    )
    .unwrap_or_else(|why| panic!("{why}"));
    let started = Instant::now();
    while !seen.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the stand-in wrote nothing"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let held_by_engine = std::fs::read_to_string(&seen).unwrap();
    assert_eq!(
        held_by_engine.trim(),
        "0 1 2 3 4",
        "descriptor {leaked} was leaked"
    );
    drop(engine);
    drop(held);
}
