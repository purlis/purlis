// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Where an ask stands, and what an app that is there answers** (#6995, review round 2).
//!
//! [`reach::under_in`] reads the kernel through [`reach::Kernel`], so each cell here gives it a
//! chain of processes and takes each program for what a shipped build takes it for
//! ([`reach::shipped_program_of`]): a test build's own rule ([`reach::program_of`]) is never the
//! one under test. Then an app that cannot tell which chat is asking says so, never that the
//! asker is in none of its chats. A full app above a chat is `brokered_writes`' cell in
//! `purlis-cli`, where the app is a process above the one asking, as it must be.
#![cfg(unix)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use purlis_core::hookwire::reach::{self, Kernel, Ran, Under, Unreached};
use purlis_core::hookwire::{self, Answer, CHAT_ENV, Listener, Program, SOCKET_ENV};

/// A chain of processes, as a cell says the kernel shows it.
struct Shown {
    parents: HashMap<u32, u32>,
    programs: HashMap<u32, PathBuf>,
    owners: HashMap<u32, u32>,
    sockets: HashMap<u32, Vec<PathBuf>>,
    /// Whether listing the sockets fails, as an `lsof` that cannot run does.
    unlisted: bool,
    namespace: bool,
    sandboxed: bool,
}

/// The process asking.
const ME: u32 = 100;
/// The user it runs as.
const USER: u32 = 501;

impl Shown {
    /// `ME` below `above`, parent first, each `(pid, program, owner)`: a program of `None` is
    /// one that cannot be read. The topmost's parent is `top`.
    fn chain(above: &[(u32, Option<&str>, u32)], top: u32) -> Self {
        let mut shown = Self {
            parents: HashMap::new(),
            programs: HashMap::new(),
            owners: [(ME, USER)].into_iter().collect(),
            sockets: HashMap::new(),
            unlisted: false,
            namespace: true,
            sandboxed: false,
        };
        let mut below = ME;
        for (pid, program, owner) in above {
            shown.parents.insert(below, *pid);
            if let Some(program) = program {
                shown.programs.insert(*pid, PathBuf::from(program));
            }
            shown.owners.insert(*pid, *owner);
            below = *pid;
        }
        shown.parents.insert(below, top);
        shown
    }

    fn holding(mut self, pid: u32, socket: &str) -> Self {
        self.sockets
            .entry(pid)
            .or_default()
            .push(PathBuf::from(socket));
        self
    }
}

impl Kernel for Shown {
    fn parent_of(&self, pid: u32) -> Option<u32> {
        self.parents.get(&pid).copied()
    }
    fn chain(&self, pid: u32) -> Vec<u32> {
        let mut chain = Vec::new();
        let mut at = pid;
        while let Some(up) = self.parent_of(at).filter(|up| *up > 1) {
            chain.push(up);
            at = up;
        }
        chain
    }
    fn in_the_system_s_pid_namespace(&self) -> bool {
        self.namespace
    }
    fn programs_of(&self, pids: &[u32]) -> Vec<Option<PathBuf>> {
        pids.iter()
            .map(|pid| self.programs.get(pid).cloned())
            .collect()
    }
    fn owner_of(&self, pid: u32) -> Option<u32> {
        self.owners.get(&pid).copied()
    }
    fn this_user(&self) -> u32 {
        USER
    }
    fn sockets_held_by(&self, pids: &[u32]) -> std::io::Result<Vec<Vec<PathBuf>>> {
        if self.unlisted {
            return Err(std::io::Error::other("lsof did not run"));
        }
        Ok(pids
            .iter()
            .map(|pid| self.sockets.get(pid).cloned().unwrap_or_default())
            .collect())
    }
    fn held_by_a_sandbox(&self) -> bool {
        self.sandboxed
    }
}

/// [`reach::under_in`] by the naming rule a shipped build reads.
fn shipped(shown: &Shown) -> Under {
    reach::under_in(shown, ME, &reach::shipped_program_of)
}

const APP: &str = "/Applications/purlis.app/Contents/MacOS/purlis-app";
const HOST: &str = "/Applications/purlis.app/Contents/MacOS/purlis";
const HARNESS: &str = "/usr/local/bin/claude";
const SHELL: &str = "/bin/zsh";
const TERMINAL: &str = "/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal";

#[test]
fn the_first_process_of_a_pid_namespace_of_its_own_is_a_sandbox_s_boundary() {
    purlis_core::unsteered!();
    // `unshare -p`: the chain reads to a top whose parent is 0, as the system's first
    // process's is, every program is read, and none is the app's.
    let mut shown = Shown::chain(&[(2, Some(SHELL), USER)], 0);
    assert_eq!(shipped(&shown), Under::NoApp, "the system's own namespace");
    shown.namespace = false;
    assert_eq!(shipped(&shown), Under::Boxed);
}

#[test]
fn a_shipped_build_takes_the_app_by_its_program_s_name_and_purlis_only_where_it_serves() {
    purlis_core::unsteered!();
    assert_eq!(reach::shipped_program_of(APP.as_ref()), Ran::App);
    assert_eq!(reach::shipped_program_of(HOST.as_ref()), Ran::Purlis);
    assert_eq!(reach::shipped_program_of(HARNESS.as_ref()), Ran::Other);
    // The window (40) and its session host (30) above a harness (20) and its shell (10).
    let chat = [
        (10, Some(SHELL), USER),
        (20, Some(HARNESS), USER),
        (30, Some(HOST), USER),
        (40, Some(APP), USER),
    ];
    assert_eq!(shipped(&Shown::chain(&chat, 1)), Under::App(vec![40, 30]));

    // `purlis serve` with no window: the app only while it holds a plane's hook socket.
    let serve = [(10, Some(SHELL), USER), (20, Some(HOST), USER)];
    assert_eq!(
        shipped(&Shown::chain(&serve, 1).holding(20, "/plane/.purlis/app/hooks.sock")),
        Under::App(vec![20])
    );
    assert_eq!(shipped(&Shown::chain(&serve, 1)), Under::NoApp);
    assert_eq!(
        shipped(&Shown::chain(&serve, 1).holding(20, "/plane/.purlis/app/other.sock")),
        Under::NoApp
    );
}

#[test]
fn a_purlis_above_whose_sockets_cannot_be_listed_is_a_doubt_never_no_app() {
    purlis_core::unsteered!();
    // `purlis serve` above a shell: whether it serves is in its sockets, and a listing that
    // fails says nothing of them, so this process is never taken for the person's terminal.
    let serve = [(10, Some(SHELL), USER), (20, Some(HOST), USER)];
    let mut unlisted = Shown::chain(&serve, 1).holding(20, "/plane/.purlis/app/hooks.sock");
    unlisted.unlisted = true;
    assert_eq!(shipped(&unlisted), Under::Boxed);
    // With no `purlis` above, nothing is listed, and nothing fails.
    let mut terminal = Shown::chain(&[(10, Some(SHELL), USER)], 1);
    terminal.unlisted = true;
    assert_eq!(shipped(&terminal), Under::NoApp);
}

#[test]
fn a_shipped_build_never_takes_a_chat_s_copy_of_the_app_s_program_for_the_app() {
    purlis_core::unsteered!();
    // A chat copies the app's program into a folder of its own and runs it below its harness,
    // holding a hook socket of its own making: the real app above is the app, and the copy is
    // none of its processes, so what listens there is never believed.
    let tmp = tempfile::tempdir().unwrap();
    let copy = tmp.path().join(purlis_core::names::APP_BINARY.write);
    std::fs::write(&copy, b"").unwrap();
    let copy = copy.to_str().unwrap();
    let chat = Shown::chain(
        &[
            (10, Some(copy), USER),
            (20, Some(HARNESS), USER),
            (30, Some(HOST), USER),
            (40, Some(APP), USER),
        ],
        1,
    )
    .holding(10, &format!("{}/hooks.sock", tmp.path().display()));

    assert_eq!(shipped(&chat), Under::App(vec![40, 30]));

    // A `purlis serve` the chat starts below its harness is not the app's either.
    let serve = Shown::chain(
        &[
            (10, Some(HOST), USER),
            (20, Some(HARNESS), USER),
            (30, Some(HOST), USER),
            (40, Some(APP), USER),
        ],
        1,
    )
    .holding(10, "/chat/.purlis/app/hooks.sock");
    assert_eq!(shipped(&serve), Under::App(vec![40, 30]));
}

#[test]
fn a_listener_of_a_chat_s_own_copy_of_the_app_is_refused() {
    purlis_core::unsteered!();
    // This process listens, as the chat's copy would: the app it stands under is another
    // process, so the copy's socket is refused, never asked.
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("app").join("hooks.sock");
    let _listener = Listener::bind(tmp.path(), &socket).expect("a socket");
    let hinted = socket.display().to_string();
    let chat = move |name: &str| match name {
        CHAT_ENV => Some("3".to_owned()),
        SOCKET_ENV => Some(hinted.clone()),
        _ => None,
    };

    let unreached = reach::reach_from(
        Under::App(vec![u32::MAX - 1]),
        tmp.path(),
        &chat,
        Duration::from_secs(2),
    )
    .unwrap_err();

    assert!(
        matches!(unreached, Unreached::NotTheApp(_)),
        "{unreached:?}"
    );
}

#[test]
fn a_person_s_terminal_climbing_through_root_s_login_is_not_a_sandbox_s_boundary() {
    purlis_core::unsteered!();
    // zsh, then `login` (root's, its program unreadable to this user), then Terminal.
    let terminal = [
        (10, Some(SHELL), USER),
        (20, None, 0),
        (30, Some(TERMINAL), USER),
    ];
    assert_eq!(shipped(&Shown::chain(&terminal, 1)), Under::NoApp);
    // `sshd`, root's, above the person's shell, with its own root parent.
    let ssh = [(10, Some(SHELL), USER), (20, None, 0), (30, None, 0)];
    assert_eq!(shipped(&Shown::chain(&ssh, 1)), Under::NoApp);
}

#[test]
fn an_unreadable_ancestor_of_this_user_s_or_with_none_of_its_own_below_is_a_boundary() {
    purlis_core::unsteered!();
    // This user's own process, its program hidden: what a sandbox hides.
    let hidden = [(10, Some(SHELL), USER), (20, None, USER)];
    assert_eq!(shipped(&Shown::chain(&hidden, 1)), Under::Boxed);
    // Root's, with no process of this user's read below it.
    let bare = [(10, None, 0)];
    assert_eq!(shipped(&Shown::chain(&bare, 1)), Under::Boxed);
    // Whose owner cannot be read.
    let mut unknown = Shown::chain(&[(10, Some(SHELL), USER), (20, None, 0)], 1);
    unknown.owners.remove(&20);
    assert_eq!(shipped(&unknown), Under::Boxed);
}

#[test]
fn an_orphan_a_sandbox_still_holds_is_boxed_and_a_person_s_terminal_is_not() {
    purlis_core::unsteered!();
    // A chat's process that left its tree: its parent is `launchd` or `init`, nothing is
    // above it to read, and its sandbox came with it.
    let mut orphan = Shown::chain(&[], 1);
    assert_eq!(shipped(&orphan), Under::NoApp, "no sandbox holds it");
    orphan.sandboxed = true;
    assert_eq!(shipped(&orphan), Under::Boxed);
    // Under the app, the app is asked whatever holds this process.
    let mut chat = Shown::chain(&[(10, Some(HOST), USER), (20, Some(APP), USER)], 1);
    chat.sandboxed = true;
    assert_eq!(shipped(&chat), Under::App(vec![20, 10]));
}

/// What the app at `socket` answers a `whoami` from this process.
fn whoami_at(socket: &std::path::Path) -> String {
    let mut stream = UnixStream::connect(socket).expect("connects");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(b"{\"whoami\":true}\n").unwrap();
    let mut said = String::new();
    BufReader::new(stream).read_line(&mut said).unwrap();
    said
}

#[test]
fn the_app_says_it_cannot_tell_rather_than_in_no_chat_while_a_chat_s_program_is_unrecorded() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("hooks.sock");
    let listener = Listener::bind(tmp.path(), &socket).expect("a socket");
    // Chat 3's program is another process: this one runs inside no chat.
    let mut other =
        purlis_core::forklock::spawn(std::process::Command::new("sleep").arg("30")).unwrap();
    let tokens = listener.tokens();
    tokens.issue(3).expect("a token");
    tokens.bind(3, Program::of(other.id()).expect("its program"));
    let _reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(|_, _| Answer::No {
            why: hookwire::NOTHING_ANSWERS.to_owned(),
        }),
    );
    let said = |line: &str| match serde_json::from_str::<Answer>(line) {
        Ok(Answer::No { why }) => why,
        _ => panic!("not a refusal: {line}"),
    };

    let before = whoami_at(&socket);
    // Chat 4 has started, and its program is not recorded yet: this process may be inside it.
    tokens.issue(4).expect("a token");
    let after = whoami_at(&socket);
    let _ = other.kill();
    let _ = other.wait();

    assert_eq!(said(&before), reach::IN_NO_CHAT);
    assert_eq!(said(&after), reach::IN_DOUBT);
}

#[test]
fn an_asker_that_cannot_be_read_is_in_doubt_never_in_no_chat() {
    purlis_core::unsteered!();
    let tokens = hookwire::ChatTokens::default();
    assert_eq!(tokens.chat_of(&[]), hookwire::ChatOf::InDoubt);
}
