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

    // Its ssh ends, and the window does not look at the pool again.
    kill(first.ssh());

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
