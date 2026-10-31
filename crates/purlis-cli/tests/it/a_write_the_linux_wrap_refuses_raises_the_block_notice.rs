// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A write purlis's own wrap on Linux refuses reaches the app as the chat's sandbox block**
//! (#2344): the same block, naming the same target, that macOS's kernel line of the same refusal
//! becomes, so the window raises the same Notice with the same ways on. The write stays refused
//! whether or not the app hears of it.
//!
//! A Codex profile at the full sandbox, built as a start builds it for a chat in a workspace's
//! folder, applied by this build's own `purlis _sandbox` to a shell that writes another
//! workspace's file, with the environment the app starts a chat with and the app's hook socket
//! listening.

#![cfg(target_os = "linux")]

use crate::support;

use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

use purlis_core::harness::Harness;
use purlis_core::hookwire::{
    CHAT_ENV, HARNESS_ENV, Hearing, Listener, SANDBOXED_ENV, SOCKET_ENV, SandboxBlocked, TOKEN_ENV,
};
use purlis_core::sandbox::{self, Decided, Form, Machine, Os};
use purlis_core::sandboxblock::denial::Denial;
use purlis_core::sandboxblock::{Block, CHAT_DIR_ENV, Kind, Operation, Place};

/// What one wrapped shell running `script` in `chat` printed, and the blocks the app heard.
struct Ran {
    said: String,
    blocked: Vec<SandboxBlocked>,
}

fn wrapped(profile: &str, chat: &Path, home: &Path, script: &str, listening: bool) -> Ran {
    let (out, blocked) = telling(profile, chat, home, script, listening, |command| {
        command.output().expect("purlis ran")
    });
    assert!(
        out.status.success(),
        "{:?}: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    Ran {
        said: String::from_utf8_lossy(&out.stdout).trim().to_owned(),
        blocked,
    }
}

/// `run` given the command that wraps `sh -c script` in `chat` under `profile`, with the
/// environment the app starts a chat with and its hook socket bound (and read, where
/// `listening`); what it answered, and the blocks the app heard.
fn telling<T>(
    profile: &str,
    chat: &Path,
    home: &Path,
    script: &str,
    listening: bool,
    run: impl FnOnce(&mut Command) -> T,
) -> (T, Vec<SandboxBlocked>) {
    let sockets = tempfile::tempdir().expect("a folder for the socket");
    let path = sockets.path().join("hooks.sock");
    let listener = Listener::bind(sockets.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let (blocks, heard) = mpsc::channel();
    let blocks = Mutex::new(blocks);
    let _reading = listening.then(|| {
        listener.hear(Hearing {
            serve: purlis_core::hookwire::serve::refusing(),
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
    let mut command = support::purlis();
    command
        .args(&words)
        .current_dir(chat)
        .env_clear()
        .envs(support::carrying(support::unsteered_with(&[
            ("HOME", home.as_os_str()),
            (SANDBOXED_ENV, "1".as_ref()),
            (SOCKET_ENV, path.as_os_str()),
            (CHAT_ENV, "7".as_ref()),
            (TOKEN_ENV, token.expose().as_ref()),
            (HARNESS_ENV, "codex".as_ref()),
            (CHAT_DIR_ENV, chat.as_os_str()),
        ])));
    let answered = run(&mut command);
    let blocked = std::iter::from_fn(|| heard.recv_timeout(Duration::from_secs(2)).ok()).collect();
    (answered, blocked)
}

/// A project at `<out>/project`, the folder of a chat in its workspace `a`, and the Codex
/// profile the chat starts under at the full sandbox, with `home` as the home.
fn a_codex_chat(out: &Path, home: &Path) -> (PathBuf, PathBuf, String) {
    let root = out.join("project");
    let chat = root.join("workspaces/a/repo");
    let tmp = out.join("tmp");
    fs::create_dir_all(root.join(".purlis")).expect("the state folder");
    fs::create_dir_all(&chat).expect("the chat's folder");
    fs::create_dir_all(root.join("workspaces/b")).expect("another workspace");
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
    let profile = sandbox::codex::profile(wrap, &chat, &tmp, &[4040], None).expect("a profile");
    (root, chat, profile)
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
    let (root, chat, profile) = a_codex_chat(&out, &home);
    let elsewhere = root.join("workspaces/b/notes.md");

    let ran = wrapped(
        &profile,
        &chat,
        &home,
        &format!(
            "if touch {} 2>/dev/null; then echo yes; else echo no; fi",
            elsewhere.display()
        ),
        true,
    );
    assert_eq!(ran.said, "no", "the write is refused");
    assert!(!elsewhere.exists());

    let place = Place {
        root: &root,
        chat: &chat,
        cwd: &chat,
        home: Some(&home),
    };
    let line = format!(
        "Sandbox: touch(4120) deny(1) file-write-create {}",
        elsewhere.display()
    );
    let (as_on_macos, named) = Denial::of_line(&line).expect("a kernel line").block(&place);
    assert_eq!(
        as_on_macos,
        Block {
            operation: Operation::Write,
            kind: Kind::ProjectFiles,
            ours: false,
        }
    );
    assert_eq!(named, Some(elsewhere.display().to_string()));
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
    let (root, chat, profile) = a_codex_chat(&out, &home);
    let elsewhere = root.join("workspaces/b/notes.md");
    let script = format!(
        "for n in 1 2 3 4 5 6 7 8 9 10 11 12; do touch {} 2>/dev/null && echo yes; done; echo done",
        elsewhere.display()
    );

    let ran = wrapped(&profile, &chat, &home, &script, true);
    assert_eq!(ran.said, "done", "every write is refused");
    assert_eq!(ran.blocked.len(), 1, "{:?}", ran.blocked);

    let unheard = wrapped(&profile, &chat, &home, &script, false);
    assert_eq!(unheard.said, "done", "refused with nobody listening");
    assert!(unheard.blocked.is_empty());
    assert!(!elsewhere.exists());
}

/// #2423: a program that connects straight to internet addresses, past the proxy, raises one
/// quiet block a minute, naming the first address, and each connection stays refused.
#[test]
fn addresses_dialled_past_the_proxy_are_one_block_naming_the_first() {
    purlis_core::unsteered!();
    landlock();
    let scratch = tempfile::tempdir().expect("a folder");
    let out = scratch.path().canonicalize().expect("a real folder");
    let home = out.join("home");
    fs::create_dir_all(&home).expect("a home");
    let (_, chat, profile) = a_codex_chat(&out, &home);
    // Addresses of the documentation range: nothing answers there, and the wrap refuses
    // before anything is sent.
    let script = "for n in 1 2 3 4 5; do bash -c \"exec 3<>/dev/tcp/192.0.2.$n/443\" \
                  2>/dev/null && echo yes; done; echo done";

    let ran = wrapped(&profile, &chat, &home, script, true);
    assert_eq!(ran.said, "done", "every connection is refused");
    assert_eq!(
        ran.blocked,
        vec![SandboxBlocked {
            chat: 7,
            sandbox_blocked: Block {
                operation: Operation::Connect,
                kind: Kind::Address,
                ours: false,
            },
            harness: Some("codex".to_owned()),
            target: Some("192.0.2.1:443".to_owned()),
            read_back: false,
            suggested: None,
        }]
    );
}

/// Whether this machine refuses the call the wrap takes a chat's socket with, on another process
/// of ours as the chat is to the wrap. Docker's default seccomp profile does without
/// `CAP_SYS_PTRACE` (`docs/runner-image.md`), so the nightly `fedora` job's container does and
/// the Ubuntu jobs, on the runner itself, do not (#3440). The answer is the machine's, read apart
/// from the wrap, so a wrap that stopped taking sockets where it could still fails below.
fn the_machine_refuses_taking_a_socket() -> bool {
    use rustix::io::Errno;
    use rustix::process::{Pid, PidfdFlags, PidfdGetfdFlags, pidfd_getfd, pidfd_open};
    let mut other = Command::new("sleep")
        .arg("60")
        .stdin(Stdio::null())
        .spawn()
        .expect("sleep runs");
    let answered = pidfd_open(Pid::from_child(&other), PidfdFlags::empty())
        .and_then(|pidfd| pidfd_getfd(&pidfd, 0, PidfdGetfdFlags::empty()));
    let _ = other.kill();
    let _ = other.wait();
    matches!(answered, Err(Errno::PERM | Errno::ACCESS | Errno::NOSYS))
}

/// A server the wrap refuses, a socket that listens with no bind of its own, is told as the
/// block macOS's kernel line of a refused bind is, and stays refused. Where the machine refuses
/// the call that takes the socket, the wrap never reads the socket, and says that instead: no port
/// can be read without it (one bound and not listening is not in `/proc/net/tcp`), and none
/// would help, since no Allow makes that listen work. The app's words for it are
/// `a_server_whose_socket_could_not_be_taken_is_told_why_and_what_to_do`.
#[test]
fn a_listen_the_linux_wrap_refuses_is_told_as_macos_s_refused_bind_is() {
    purlis_core::unsteered!();
    landlock();
    let scratch = tempfile::tempdir().expect("a folder");
    let out = scratch.path().canonicalize().expect("a real folder");
    let home = out.join("home");
    fs::create_dir_all(&home).expect("a home");
    let (root, chat, profile) = a_codex_chat(&out, &home);
    let script = "perl -MSocket -e 'socket(my $s, PF_INET, SOCK_STREAM, 0) or die; \
                  print listen($s, 1) ? \"yes\\n\" : \"no\\n\"'";

    let ran = wrapped(&profile, &chat, &home, script, true);
    assert_eq!(ran.said, "no", "the listen is refused");
    let place = Place {
        root: &root,
        chat: &chat,
        cwd: &chat,
        home: Some(&home),
    };
    let (as_on_macos, named) = Denial::of_line("Sandbox: perl(4120) deny(1) network-bind *:0")
        .expect("a kernel line")
        .block(&place);
    let target = if the_machine_refuses_taking_a_socket() {
        Some(purlis_core::sandbox::localport::SOCKET_NOT_TAKEN.to_owned())
    } else {
        named
    };
    assert_eq!(
        ran.blocked,
        vec![SandboxBlocked {
            chat: 7,
            sandbox_blocked: as_on_macos,
            harness: Some("codex".to_owned()),
            target,
            read_back: false,
            suggested: None,
        }],
        "one block, as macOS's kernel line of a refused bind is"
    );
}

/// The signals the wrap leaves to its one waiter, which passes an ending on to the chat: INT,
/// QUIT, TERM and HUP, as a `SigBlk` mask.
const LEFT_TO_THE_WAITER: u64 = bit(1) | bit(2) | bit(3) | bit(15);

/// Signal `signal`'s bit in a `SigBlk` mask: HUP 1, INT 2, QUIT 3, TERM 15.
const fn bit(signal: u32) -> u64 {
    1 << (signal - 1)
}

/// Each thread of process `pid` now: whether it has no new privileges, and its blocked signals.
fn threads_of(pid: u32) -> Vec<(bool, u64)> {
    let Ok(tasks) = fs::read_dir(format!("/proc/{pid}/task")) else {
        return Vec::new();
    };
    tasks
        .flatten()
        .filter_map(|task| fs::read_to_string(task.path().join("status")).ok())
        .map(|status| {
            let field = |name: &str| {
                status
                    .lines()
                    .find_map(|line| line.strip_prefix(name))
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_owned()
            };
            (
                field("NoNewPrivs:") == "1",
                u64::from_str_radix(&field("SigBlk:"), 16).unwrap_or(0),
            )
        })
        .collect()
}

#[test]
fn with_telling_on_every_thread_of_the_wrap_is_held_as_the_wrap_is() {
    purlis_core::unsteered!();
    landlock();
    let scratch = tempfile::tempdir().expect("a folder");
    let out = scratch.path().canonicalize().expect("a real folder");
    let home = out.join("home");
    fs::create_dir_all(&home).expect("a home");
    let (root, chat, profile) = a_codex_chat(&out, &home);
    let script = format!(
        "touch {} 2>/dev/null; echo ready; sleep 3",
        root.join("workspaces/b/notes.md").display()
    );
    let (seen, blocked) = telling(&profile, &chat, &home, &script, true, |command| {
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("purlis runs");
        // Once the chat runs, and so the wrap holds itself: read while the refusal is told,
        // and after, every thread the wrap has meanwhile.
        let mut said = BufReader::new(child.stdout.take().expect("its output"));
        let mut ready = String::new();
        said.read_line(&mut ready).expect("a line");
        assert_eq!(ready.trim(), "ready");
        let until = Instant::now() + Duration::from_secs(2);
        let mut seen = Vec::new();
        while Instant::now() < until {
            seen.push(threads_of(child.id()));
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = child.wait();
        seen
    });
    assert_eq!(blocked.len(), 1, "the refusal was told: {blocked:?}");
    assert!(seen.iter().any(|threads| threads.len() > 2), "{seen:?}");
    for threads in &seen {
        assert!(
            threads.iter().all(|(held, _)| *held),
            "a thread of the wrap without no-new-privileges: {threads:?}"
        );
        let unblocked = threads
            .iter()
            .filter(|(_, blocked)| blocked & LEFT_TO_THE_WAITER != LEFT_TO_THE_WAITER)
            .count();
        assert!(
            unblocked <= 1,
            "more than the waiter takes the signals it passes on: {threads:x?}"
        );
    }
}

#[test]
fn with_telling_on_an_ending_asked_of_the_wrap_reaches_the_chat_every_time() {
    purlis_core::unsteered!();
    landlock();
    let scratch = tempfile::tempdir().expect("a folder");
    let out = scratch.path().canonicalize().expect("a real folder");
    let home = out.join("home");
    fs::create_dir_all(&home).expect("a home");
    let (root, chat, profile) = a_codex_chat(&out, &home);
    let script = format!(
        "trap 'echo trapped; exit 0' TERM; touch {} 2>/dev/null; echo ready; \
         while :; do sleep 0.05; done",
        root.join("workspaces/b/notes.md").display()
    );
    for round in 0..20 {
        let ((said, ended), _) = telling(&profile, &chat, &home, &script, true, |command| {
            let mut child = command
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .expect("purlis runs");
            let mut said = BufReader::new(child.stdout.take().expect("its output"));
            let mut ready = String::new();
            said.read_line(&mut ready).expect("a line");
            assert_eq!(ready.trim(), "ready", "round {round}");
            // While the refusal is being told.
            std::thread::sleep(Duration::from_millis(20));
            let killed = Command::new("kill")
                .args(["-TERM", &child.id().to_string()])
                .status()
                .expect("kill runs");
            assert!(killed.success());
            let mut rest = String::new();
            let _ = said.read_to_string(&mut rest);
            (rest, child.wait().expect("the wrap ends"))
        });
        assert_eq!(said.trim(), "trapped", "round {round}: {ended:?}");
        assert!(ended.success(), "round {round}: {ended:?}");
    }
}
