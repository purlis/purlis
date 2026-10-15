// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The keeper: the small process that ends an engine's process group when its host goes.
//!
//! macOS has no parent-death signal, and nothing of the host's runs after it is killed, so
//! something outside the host has to notice. The keeper is the first member and leader of each
//! engine's process group, and its stdin is the read end of a pipe whose only write end the
//! host holds (the *lifeline*). However the host ends (it returns, it crashes, it is killed
//! outright), the kernel closes that write end, the keeper reads end of file, waits
//! [`GRACE`] for the engine to finish on its own (its DevTools pipe closed at the same moment),
//! and then kills the whole group, itself included, with `SIGKILL`, which a hung or stopped
//! engine cannot refuse. So the engine is gone within a second of its host, on Linux and macOS
//! alike, with no signal handler and no `unsafe`.
//!
//! The keeper never kills a group it does not lead: started any other way it exits at once,
//! so a mistake cannot end its host's own group.

use std::io::{self, Read};
use std::time::Duration;

/// How long the engine has to end by itself after its host goes, before the group is killed.
pub const GRACE: Duration = Duration::from_millis(400);

/// What the keeper writes to its stdout once it guards its group.
pub const READY: &[u8] = b"ready\n";

/// The keeper's whole program: wait for the lifeline to end, then end the group. Called by
/// `purlis-engine-keeper`, and by any binary that runs the keeper as a mode of its own.
#[cfg(unix)]
pub fn run() -> ! {
    use nix::sys::signal::{SigSet, SigmaskHow, Signal, killpg, sigprocmask};
    use nix::unistd::{getpgrp, getpid};

    let group = getpgrp();
    if group != getpid() {
        // Not a group of its own: killing it would kill whoever started the keeper.
        std::process::exit(2);
    }
    // When the host goes, this group is orphaned, and an orphaned group with a stopped member
    // (a hung engine) is sent SIGHUP: held blocked, so the keeper lives to end the group.
    let mut held = SigSet::empty();
    for signal in [
        Signal::SIGHUP,
        Signal::SIGINT,
        Signal::SIGTERM,
        Signal::SIGQUIT,
    ] {
        held.add(signal);
    }
    let _ = sigprocmask(SigmaskHow::SIG_BLOCK, Some(&held), None);
    // Ready: the host waits for this byte before it starts the engine, so there is no moment
    // when the engine runs and its keeper does not yet guard it.
    {
        use std::io::Write;
        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(READY).and_then(|()| stdout.flush());
    }
    let mut stdin = io::stdin().lock();
    let mut sink = [0_u8; 256];
    loop {
        match stdin.read(&mut sink) {
            Ok(0) => break,
            Ok(_) => {}
            Err(why) if why.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    std::thread::sleep(GRACE);
    let _ = killpg(group, Signal::SIGKILL);
    std::process::exit(0)
}
