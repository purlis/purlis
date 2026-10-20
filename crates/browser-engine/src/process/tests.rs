// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use nix::sys::signal::Signal;
use nix::sys::wait::WaitStatus;
use nix::unistd::Pid;

use super::exit_of;
use crate::ending::EngineExit;

#[test]
fn an_exit_is_its_code_and_a_signal_is_named_as_the_system_names_it() {
    let pid = Pid::from_raw(42);
    assert_eq!(
        exit_of(WaitStatus::Exited(pid, 3)),
        Some(EngineExit::Code(3))
    );
    assert_eq!(
        exit_of(WaitStatus::Signaled(pid, Signal::SIGSEGV, false)),
        Some(EngineExit::Signal("SIGSEGV".into()))
    );
    assert_eq!(exit_of(WaitStatus::StillAlive), None);
}
