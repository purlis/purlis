// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Try it sees a chat on a pool start by its harness's start hook** (#4483, ADR 0076): a chat on
//! a real Docker pool's runner reports `sessionstart` as Claude Code does at its launch, the line
//! crosses from the runner to the window, and what Try it waits on hears it. Nothing it prints is
//! read. Run by the parity job.

use super::*;

#[test]
fn in_a_docker_pool() {
    let Some(target) = super::in_a_docker_pool() else {
        return;
    };
    let runner = target.runner.as_ref().expect("a runner's target");
    // The hook waits for the watch, so the test can't miss the line it waits for.
    let id = a_chat_on_the_board(
        &target,
        &a_shell(&format!(
            "printf 'waiting.'; until [ -e /tmp/try-go ]; do sleep 0.05; done; \
             PURLIS_HARNESS=claude '{}' hook sessionstart </dev/null; printf 'started.'; \
             read line",
            purlis()
        )),
    );
    Pane::on(&target, id).shows("waiting.");
    let watching = target.hooks.skill_runs();
    let heard = watching.watch(id);
    runner.prints(": > /tmp/try-go");
    let started = crate::poolsetuptab::waited_for_its_start(
        &heard,
        "the pool",
        Duration::from_secs(120),
        || true,
        || false,
    );
    watching.unwatch(id);
    assert_eq!(started, Ok(()), "Try it heard the chat's start hook");
    Pane::on(&target, id).shows("started.");
    target.sessions.close(id).expect("it closes");
}
