// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A forward's lock never outlives its ssh** (ADR 0123, amended 2026-11-17): what another
//! process of the person's is handed is a socket that answers, or a forward of its own.

#![cfg(unix)]

use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::super::{HELD, HostKey, Route};
use super::*;

/// The environment variable [`stands_in_for_ssh`] reads the forward's `-L` from.
const FORWARDING: &str = "PURLIS_TEST_SSH_FORWARDING";

/// **This test binary as `ssh -L`**, run by [`an_ssh`]'s script: it listens on the forward's
/// local socket until its standard input closes, as the forward's session ends with purlis's
/// pipe. Run any other way, it does nothing.
#[test]
#[ignore = "run in a child, as ssh, by the two tests below"]
fn stands_in_for_ssh() {
    let Some(forwarding) = crate::envvar::var(FORWARDING) else {
        return;
    };
    let local = forwarding
        .split_once(':')
        .map_or(forwarding.as_str(), |(local, _)| local);
    // As the forward's `StreamLocalBindUnlink=yes` does.
    let _ = std::fs::remove_file(local);
    let listener = UnixListener::bind(local).expect("the forward's socket");
    std::thread::spawn(move || for _ in listener.incoming() {});
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
}

/// An `ssh` in `folder` that reads the person's settings as naming a machine (`-G`), and
/// otherwise forwards as [`stands_in_for_ssh`] does.
fn an_ssh(folder: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let ssh = folder.join("ssh");
    let exe = std::env::current_exe().expect("the test binary");
    let script = format!(
        "#!/bin/sh\n\
         forwarding=\n\
         while [ $# -gt 0 ]; do\n\
           case \"$1\" in\n\
             -G) printf 'hostname box\\nuser me\\n'; exit 0;;\n\
             -L) forwarding=$2;;\n\
           esac\n\
           shift\n\
         done\n\
         {FORWARDING}=\"$forwarding\" exec '{}' --exact \
         sshroute::forward::tests::stands_in_for_ssh --ignored --quiet\n",
        exe.display()
    );
    // Written while nothing forks, so no child holds it open for writing as it is run (ETXTBSY).
    crate::forklock::while_descriptors_are_made(|| {
        std::fs::write(&ssh, script).unwrap();
        std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();
    });
    ssh
}

fn key() -> HostKey {
    HostKey::parse(
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINiGzfjDDJsgYLYCmdkf8Fel9sE6Ma14Y2+c33Y/qZ2n",
    )
    .unwrap()
}

/// The test's own look at a socket, apart from the one under test.
fn something_answers(socket: &Path) -> bool {
    UnixStream::connect(socket).is_ok()
}

/// Ends the process `pid` outright, as a lost connection or a kill ends an ssh.
fn kill(pid: u32) {
    let killed = crate::forklock::status(
        std::process::Command::new("kill").args(["-KILL", &pid.to_string()]),
    )
    .expect("kill runs");
    assert!(killed.success(), "{killed:?}");
}

#[test]
fn a_holder_whose_ssh_ends_lets_go_so_another_process_opens_a_forward_that_answers() {
    let dir = tempfile::tempdir().unwrap();
    let ssh = an_ssh(dir.path());
    let route = Route::parse("ssh://me@box/var/run/docker.sock").unwrap();
    let key = key();
    let (links, settings) = (dir.path().join("l"), dir.path().join("s"));
    let opening = Opening {
        ssh: &ssh,
        route: &route,
        pool: "lost",
        key: &key,
        engine_links: &links,
        settings_in: &settings,
    };
    // The window opens the pool's forward, and keeps it.
    let Held::Own(mut first) = open(&opening).expect("it opens") else {
        panic!("the first to open it holds it");
    };
    assert!(something_answers(first.socket()));

    // Its ssh ends, and the window does not look at the pool again. A kill is only sent: the
    // ssh is gone once nothing answers on its socket.
    kill(first.ssh());
    let until = std::time::Instant::now() + Duration::from_secs(10);
    while something_answers(first.socket()) {
        assert!(
            std::time::Instant::now() < until,
            "the killed ssh still answers"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    // Another process of the person's (the session host's connector) asks for the pool.
    let second = open(&opening).expect("another process gets a forward");
    assert!(
        something_answers(second.socket()),
        "another process was handed a socket nothing answers"
    );
    assert!(
        matches!(second, Held::Own(_)),
        "the forward whose ssh ended is opened anew by the one that asked"
    );
    assert!(!first.alive());
    // The first lets go of its forward without taking the socket the second made.
    drop(first);
    assert!(something_answers(second.socket()));
}

#[test]
fn a_forward_another_process_holds_is_used_only_while_its_socket_answers() {
    let dir = tempfile::tempdir().unwrap();
    let ssh = an_ssh(dir.path());
    let route = Route::parse("ssh://me@box/var/run/docker.sock").unwrap();
    let key = key();
    let (links, settings) = (dir.path().join("l"), dir.path().join("s"));
    let opening = Opening {
        ssh: &ssh,
        route: &route,
        pool: "stale",
        key: &key,
        engine_links: &links,
        settings_in: &settings,
    };
    // Another process holds the pool's forward: its lock, and a socket that answers.
    let socket = super::super::socket_of(&links, "stale").unwrap();
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    let lock = std::fs::File::create(socket.parent().unwrap().join(HELD)).unwrap();
    lock.lock().unwrap();
    let listener = UnixListener::bind(&socket).unwrap();
    assert_eq!(hold(&opening).unwrap(), socket, "its socket is used");
    assert!(!dropped("stale"));

    // Its ssh ends; it keeps the lock a while, and the socket's file stays.
    drop(listener);
    assert!(socket.exists());
    assert!(
        dropped("stale"),
        "a socket nothing answers is a dropped forward"
    );

    // It lets go a moment later; meanwhile nothing is handed the dead socket.
    let let_go_of = Arc::new(AtomicBool::new(false));
    let letting_go = std::thread::spawn({
        let let_go_of = Arc::clone(&let_go_of);
        move || {
            std::thread::sleep(Duration::from_millis(500));
            let_go_of.store(true, Ordering::SeqCst);
            drop(lock);
        }
    });
    let held = hold(&opening).expect("a forward of its own");
    assert!(
        let_go_of.load(Ordering::SeqCst),
        "waited for the holder to let go"
    );
    letting_go.join().unwrap();
    assert!(something_answers(&held), "handed a socket nothing answers");
    assert!(ssh_of("stale").is_some(), "this process opened it anew");
    let_go("stale");
}

/// **One pool's forward is held apart from every other's** (#7862, T6 Q7): while a forward to one
/// machine is slow to open, a look at another pool's forward answers at once, never after the
/// slow one's wait.
/// Seen red: the look at the other pool waited out the slow forward's five seconds, behind one
/// table of every pool's forwards.
#[test]
fn a_forward_slow_to_open_holds_up_no_other_pools() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().unwrap();
    // An ssh that names the machine (`-G`), and otherwise makes no socket for five seconds.
    let ssh = dir.path().join("ssh");
    crate::forklock::while_descriptors_are_made(|| {
        std::fs::write(
            &ssh,
            "#!/bin/sh\nfor a in \"$@\"; do [ \"$a\" = -G ] && { printf 'hostname box\\nuser me\\n'; \
             exit 0; }; done\nsleep 5\n",
        )
        .unwrap();
        std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();
    });
    let route = Route::parse("ssh://me@box/var/run/docker.sock").unwrap();
    let key = key();
    let (links, settings) = (dir.path().join("l"), dir.path().join("s"));
    let opening = Opening {
        ssh: &ssh,
        route: &route,
        pool: "slow-7862",
        key: &key,
        engine_links: &links,
        settings_in: &settings,
    };
    let started = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            started.store(true, Ordering::Release);
            assert!(hold(&opening).is_err(), "the slow forward never opens");
        });
        while !started.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        // Its ssh is running, waited on for its socket.
        std::thread::sleep(Duration::from_millis(700));
        let began = std::time::Instant::now();
        assert!(!dropped("another-7862"));
        assert_eq!(ssh_of("another-7862"), None);
        let_go("another-7862");
        assert!(
            began.elapsed() < Duration::from_secs(2),
            "another pool's forward waited {:?} on the slow one",
            began.elapsed()
        );
    });
}
