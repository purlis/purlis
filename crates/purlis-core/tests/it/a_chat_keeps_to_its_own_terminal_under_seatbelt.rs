// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat on macOS uses its own terminal, under its own compiled profile, for real.** An
//! interactive shell in a session, inside the profile, as a chat is: its terminal by name and as
//! `/dev/tty`, a command that sets it, a terminal it makes itself, and another session's, which
//! it does not open.

#![cfg(target_os = "macos")]

use std::time::{Duration, Instant};

use purlis_core::sandbox::seatbelt;
use purlis_terminal::engine::{AlacrittyEngine, Size};
use purlis_terminal::session::{Session, Spec};

const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
const SIZE: Size = Size {
    rows: 40,
    columns: 160,
};

/// Each line the shell is asked to answer, its answer computed so the echoed line never matches.
fn typed(other: &str) -> String {
    format!(
        "t=$(tty); printf a >\"$t\" && echo name-$((1+1)); \
         printf b >/dev/tty && echo devtty-$((1+1)); \
         stty size >/dev/null && echo stty-$((1+1)); \
         script -q /dev/null /usr/bin/true && echo made-$((1+1)); \
         (printf x >{other}) 2>/dev/null || echo nowrite-$((1+1)); \
         (: <{other}) 2>/dev/null || echo noread-$((1+1)); \
         echo end-$((1+1))\n"
    )
}

fn session(spec: Spec) -> Session {
    Session::spawn(spec, Box::new(AlacrittyEngine::new(SIZE, 1000))).expect("the session starts")
}

fn screen_until(session: &Session, text: &str) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let lines = session.screen().lines;
        if lines
            .iter()
            .any(|line| line.starts_with(text) || line.contains(&format!(" {text}")))
        {
            return lines;
        }
        assert!(Instant::now() < deadline, "never saw {text}: {lines:#?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Whether this process can apply a profile at all: not where it is already sandboxed.
fn can_apply() -> bool {
    purlis_core::forklock::status(std::process::Command::new(SANDBOX_EXEC).args([
        "-p",
        "(version 1)(allow default)",
        "/usr/bin/true",
    ]))
    .is_ok_and(|status| status.success())
}

fn held_to_its_own_terminal(profile: &str, cwd: &std::path::Path) {
    let other = session(Spec::new("/bin/sh", SIZE).args(["-c", "tty; exec sleep 120"]));
    let lines = screen_until(&other, "/dev/ttys");
    let other_name = lines
        .iter()
        .find(|line| line.starts_with("/dev/ttys"))
        .expect("its name")
        .trim()
        .to_owned();
    let mut spec = Spec::new(SANDBOX_EXEC, SIZE).args(["-p", profile, "/bin/sh", "-i"]);
    spec.cwd = Some(cwd.to_path_buf());
    let chat = session(spec);
    chat.write(typed(&other_name).as_bytes()).expect("typed");
    let lines = screen_until(&chat, "end-2");
    for answer in [
        "name-2",
        "devtty-2",
        "stty-2",
        "made-2",
        "nowrite-2",
        "noread-2",
    ] {
        assert!(
            lines.iter().any(|line| line.contains(answer)),
            "{answer}: {lines:#?}"
        );
    }
    chat.end();
    other.end();
}

#[test]
fn a_chat_keeps_to_its_own_terminal() {
    purlis_core::unsteered!();
    if !can_apply() {
        // On CI nothing sandboxes the test, so a skip there is a failure.
        assert!(
            std::env::var_os("CI").is_none(),
            "this process cannot apply a Seatbelt profile on CI"
        );
        return;
    }
    let cwd = tempfile::tempdir().expect("a folder");
    let tmp = tempfile::tempdir().expect("a temp folder");
    let full = seatbelt::profile(
        &[],
        &seatbelt::Own::default(),
        cwd.path(),
        tmp.path(),
        &[],
        None,
    )
    .expect("a profile");
    held_to_its_own_terminal(&full, cwd.path());
    let floor =
        seatbelt::floor_profile(&[], &[], &seatbelt::Own::default(), &[], None).expect("a profile");
    held_to_its_own_terminal(&floor, cwd.path());
}
