// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A write purlis's own wrap on Linux refuses reaches the app as the chat's sandbox block**
//! (#2344): the same block, naming the same target, that macOS's kernel line of the same refusal
//! becomes, so the window raises the same Notice with the same ways on. The write stays refused
//! whether or not the app hears of it.
//!
//! A Codex profile at the full sandbox, built as a start builds it, applied by this build's own
//! `purlis _sandbox` to a shell that writes outside the project, with the environment the app
//! starts a chat with and the app's hook socket listening.

#![cfg(target_os = "linux")]

mod support;

use std::fs;
use std::path::Path;
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use purlis_core::harness::Harness;
use purlis_core::hookwire::{
    CHAT_ENV, HARNESS_ENV, Hearing, Listener, SANDBOXED_ENV, SOCKET_ENV, SandboxBlocked, TOKEN_ENV,
};
use purlis_core::sandbox::{self, Decided, Form, Machine, Os};
use purlis_core::sandboxblock::denial::Denial;
use purlis_core::sandboxblock::{Block, CHAT_DIR_ENV, Kind, Operation, Place};

/// What one wrapped shell running `script` in `root` printed, and the blocks the app heard.
struct Ran {
    said: String,
    blocked: Vec<SandboxBlocked>,
}

fn wrapped(profile: &str, root: &Path, home: &Path, script: &str, listening: bool) -> Ran {
    let sockets = tempfile::tempdir().expect("a folder for the socket");
    let path = sockets.path().join("hooks.sock");
    let listener = Listener::bind(sockets.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let (blocks, heard) = mpsc::channel();
    let blocks = Mutex::new(blocks);
    let _reading = listening.then(|| {
        listener.hear(Hearing {
            secret_exec: Box::new(|_, _, writer| {
                purlis_core::secrets::brokered::not_answered(writer)
            }),
            each: Box::new(|_| Ok(())),
            answer: Box::new(|_, _| panic!("no ask")),
            noticed: Box::new(|_| {}),
            saved: Box::new(|_| {}),
            refused: Box::new(|_| Ok(())),
            tool: Box::new(|_| Ok(())),
            doing: Box::new(|_| {}),
            output: Box::new(|_| {}),
            touching: Box::new(|_| {}),
            blocked: Box::new(move |blocked| blocks.lock().unwrap().send(blocked).unwrap()),
            permission: Box::new(|_| None),
        })
    });
    let words = purlis_linux_sandbox::line(
        profile,
        "/bin/sh".to_owned(),
        vec!["-c".to_owned(), script.to_owned()],
    );
    let out = support::purlis()
        .args(&words)
        .current_dir(root)
        .env("HOME", home)
        .env(SANDBOXED_ENV, "1")
        .env(SOCKET_ENV, &path)
        .env(CHAT_ENV, "7")
        .env(TOKEN_ENV, token.expose())
        .env(HARNESS_ENV, "codex")
        .env(CHAT_DIR_ENV, root)
        .output()
        .expect("purlis ran");
    assert!(
        out.status.success(),
        "{:?}: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let blocked = std::iter::from_fn(|| heard.recv_timeout(Duration::from_secs(2)).ok()).collect();
    Ran {
        said: String::from_utf8_lossy(&out.stdout).trim().to_owned(),
        blocked,
    }
}

/// A project at `<out>/project` and the Codex profile a chat there starts under at the full
/// sandbox, with `home` as the home.
fn a_codex_chat(out: &Path, home: &Path) -> (std::path::PathBuf, String) {
    let root = out.join("project");
    let tmp = out.join("tmp");
    fs::create_dir_all(root.join(".purlis")).expect("the state folder");
    fs::create_dir_all(&tmp).expect("a temporary folder");
    fs::write(
        root.join("purlis.toml"),
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n",
    )
    .expect("the manifest");
    let bin = out.join("bin");
    fs::create_dir_all(&bin).expect("a folder on the PATH");
    let machine = Machine {
        env: purlis_core::secrets::Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(home.to_path_buf()),
        os: Os::Linux,
    };
    let decided = sandbox::decide(Harness::Codex, &root, &machine, &|_| true, None, None)
        .expect("decided")
        .expect("a sandbox in force");
    let Decided::Sandboxed(applied) = decided else {
        panic!("not sandboxed: {decided:?}");
    };
    let Form::Codex(wrap) = applied.form() else {
        panic!("Codex is wrapped");
    };
    let profile = sandbox::codex::profile(wrap, &root, &tmp, &[4040], None).expect("a profile");
    (root, profile)
}

fn landlock() {
    assert!(
        purlis_linux_sandbox::abi() >= purlis_linux_sandbox::MIN_ABI,
        "this kernel's Landlock is version {}, and purlis's Linux sandbox needs {}",
        purlis_linux_sandbox::abi(),
        purlis_linux_sandbox::MIN_ABI
    );
}

#[test]
fn a_write_the_linux_wrap_refuses_is_told_as_the_block_macos_s_kernel_line_of_it_is() {
    purlis_core::unsteered!();
    landlock();
    let scratch = tempfile::tempdir().expect("a folder");
    let out = scratch.path().canonicalize().expect("a real folder");
    let home = out.join("home");
    fs::create_dir_all(&home).expect("a home");
    let (root, profile) = a_codex_chat(&out, &home);
    let outside = home.join("outside");

    let ran = wrapped(
        &profile,
        &root,
        &home,
        &format!(
            "if touch {} 2>/dev/null; then echo yes; else echo no; fi",
            outside.display()
        ),
        true,
    );
    assert_eq!(ran.said, "no", "the write is refused");
    assert!(!outside.exists());

    let place = Place {
        root: &root,
        chat: &root,
        cwd: &root,
        home: Some(&home),
    };
    let line = format!(
        "Sandbox: touch(4120) deny(1) file-write-create {}",
        outside.display()
    );
    let (as_on_macos, named) = Denial::of_line(&line).expect("a kernel line").block(&place);
    assert_eq!(
        as_on_macos,
        Block {
            operation: Operation::Write,
            kind: Kind::Home,
            ours: false,
        }
    );
    assert_eq!(named, Some(outside.display().to_string()));
    assert_eq!(
        ran.blocked,
        vec![SandboxBlocked {
            chat: 7,
            sandbox_blocked: as_on_macos,
            harness: Some("codex".to_owned()),
            target: named,
            read_back: false,
            suggested: None,
        }],
        "one block, as macOS's kernel line of it is"
    );
}

#[test]
fn a_write_refused_again_and_again_is_one_block_and_stays_refused_with_no_app_listening() {
    purlis_core::unsteered!();
    landlock();
    let scratch = tempfile::tempdir().expect("a folder");
    let out = scratch.path().canonicalize().expect("a real folder");
    let home = out.join("home");
    fs::create_dir_all(&home).expect("a home");
    let (root, profile) = a_codex_chat(&out, &home);
    let outside = home.join("outside");
    let script = format!(
        "for n in 1 2 3 4 5 6 7 8 9 10 11 12; do touch {} 2>/dev/null && echo yes; done; echo done",
        outside.display()
    );

    let ran = wrapped(&profile, &root, &home, &script, true);
    assert_eq!(ran.said, "done", "every write is refused");
    assert_eq!(ran.blocked.len(), 1, "{:?}", ran.blocked);

    let unheard = wrapped(&profile, &root, &home, &script, false);
    assert_eq!(unheard.said, "done", "refused with nobody listening");
    assert!(unheard.blocked.is_empty());
    assert!(!outside.exists());
}
