// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat on another machine comes back by itself once its SSH connection is killed** (#6266,
//! ADR 0123 §8): a live chat on the remote parity job's runner, reached through the pool's SSH
//! forward; the ssh carrying that forward is killed outright, as a lost connection ends it;
//! the chat reads as reconnecting, and then, with nobody pressing anything, is linked again and
//! answers what is typed. Run by the remote parity job; elsewhere it runs nothing.

use super::*;

/// How long the dropped forward has to be noticed and the chat dimmed: the link's own check,
/// with the keep-alive's minute as the last resort.
const NOTICED_WITHIN: Duration = Duration::from_secs(120);

/// How long the forward and the link have to come back by themselves, over the job's latency
/// and loss, a failed try's wait among them.
const BACK_WITHIN: Duration = Duration::from_secs(300);

/// Waits up to `within` for `done`, asking every 20 ms so a short dim is not missed.
fn until(within: Duration, what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + within;
    while !done() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn in_a_remote_docker_pool() {
    let Some(target) = super::in_a_remote_docker_pool() else {
        return;
    };
    let id = target
        .sessions
        .open(
            None,
            &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
            &|_| {},
        )
        .expect("it starts");
    let pane = Pane::on(&target, id);
    pane.shows("HARNESS-READY");

    // This pool's SSH connection ends, as a lost connection ends it: its ssh is killed outright.
    // Only this pool's: the job's other tests run beside it over connections of their own.
    let pool = &target.runner.as_ref().expect("a pool's runner").pool;
    let ssh = purlis_core::sshroute::forward::ssh_of(pool).expect("this process holds the forward");
    let killed = purlis_core::forklock::status(
        std::process::Command::new("kill").args(["-KILL", &ssh.to_string()]),
    )
    .expect("kill runs");
    assert!(killed.success(), "{killed:?}");

    until(
        NOTICED_WITHIN,
        "the chat reads as reconnecting once its connection is killed",
        || target.sessions.reconnecting().contains(&id),
    );
    until(BACK_WITHIN, "the chat comes back by itself", || {
        !target.sessions.reconnecting().contains(&id)
    });

    // The same chat, still running there, answers what is typed now.
    target
        .sessions
        .input(id, b"after the drop\r")
        .expect("typed");
    pane.shows_within("you said: after the drop", BACK_WITHIN);
    target.sessions.close(id).expect("it closes");
}
