// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use nix::sys::signal::Signal;
use nix::sys::wait::WaitStatus;
use nix::unistd::Pid;

use super::{engine_log, exit_of};
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

#[test]
fn an_engines_log_is_this_users_alone_even_where_an_earlier_one_was_looser() {
    use std::os::unix::fs::PermissionsExt;
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("b1.engine.log");
    std::fs::write(&path, "an earlier engine's lines").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    drop(engine_log(&path).unwrap());

    let meta = std::fs::metadata(&path).unwrap();
    assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    assert_eq!(meta.len(), 0);
    let fresh = folder.path().join("b2.engine.log");
    drop(engine_log(&fresh).unwrap());
    assert_eq!(
        std::fs::metadata(&fresh).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
