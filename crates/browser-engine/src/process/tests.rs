// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use nix::sys::signal::Signal;
use nix::sys::wait::WaitStatus;
use nix::unistd::Pid;

use super::{Engine, Keeper, engine_log, exit_of};
use crate::ending::EngineExit;
use crate::launch::{Launch, ReachProxy};

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

#[test]
fn a_stop_of_an_engine_that_never_reads_returns_within_its_grace() {
    // #2123: the engine answers its first call and never reads again; a call too large for the
    // pipe fills it, and the stop's own close cannot be written.
    let dir = tempfile::tempdir().unwrap();
    let program = stand_in::program(
        dir.path(),
        "engine",
        "#!/bin/sh\n\
         head -c 1 <&3 >/dev/null\n\
         printf '{\"id\":1,\"result\":{\"product\":\"stand-in\"}}\\000' >&4\n\
         exec sleep 600\n",
    );
    // A keeper that says it guards the group and holds its lifeline: the stop ends the group.
    let keeper = stand_in::program(
        dir.path(),
        "keeper",
        "#!/bin/sh\nprintf 'ready\\n'\nexec cat >/dev/null\n",
    );
    let launch = Launch::for_a_test(
        &program,
        dir.path().join("profile"),
        ReachProxy("127.0.0.1:9".parse().unwrap()),
    );
    let engine =
        Engine::start(&launch, &Keeper::program(&keeper)).unwrap_or_else(|why| panic!("{why}"));
    let (done, stopped) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = engine.devtools().call_within(
            "Page.setDocumentContent",
            serde_json::json!({ "html": "x".repeat(4 << 20) }),
            None,
            std::time::Duration::from_millis(100),
        );
        let started = std::time::Instant::now();
        engine.stop(std::time::Duration::from_millis(500));
        let _ = done.send(started.elapsed());
    });
    let took = stopped
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("the stop never returned");
    assert!(took < std::time::Duration::from_millis(900), "{took:?}");
}
