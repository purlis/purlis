// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A human edit is told at the chat's next turn** (#5409; R8, V34a, ADR 0084 §8): a real
//! discard in a real branch folder, read back as the line its chat's hook is handed.
//!
//! The whole chat is the repo's fake harness, whose hook is `purlis hook userpromptsubmit` as a
//! harness runs it: it binds a socket, which a sandboxed agent cannot, so it first runs on CI.

use purlis_core::eventlog::{self, Entry, Event as Logged};
use purlis_core::files::{self, Against, Branch, Discard, FileDiff, Target};
use purlis_core::hookwire::{Conversation, Report};
use purlis_core::state::Detail;
use std::time::{Duration, Instant};

use crate::humanedits::{DiscardAnswer, discard_in};

use super::*;

const FILE: &str = "src/lib.rs";
const GONE_LINE: &str = "the agent's line that is discarded";

fn git(at: &Path, args: &[&str]) {
    let mut git = std::process::Command::new("git");
    git.arg("-C").arg(at).args(args);
    git.args::<[&str; 0], &str>([])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .env_remove("GIT_DIR");
    let out = purlis_core::forklock::output(&mut git).expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Workspace `alpha`'s repo `svc`, and a branch folder of it whose file the agent has changed.
fn a_folder_with_the_agents_change(root: &Path) -> PathBuf {
    let clone = root.join("workspaces").join("alpha").join("svc");
    std::fs::create_dir_all(clone.join("src")).expect("the repo's folder");
    git(&clone, &["init", "-q", "-b", "main"]);
    std::fs::write(clone.join(FILE), "one\ntwo\nthree\n").expect("the file");
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-q", "-m", "files"]);
    let piece = purlis_core::worktree::add(root, "alpha", "svc", "piece", None)
        .expect("a piece is cut")
        .path;
    std::fs::write(piece.join(FILE), format!("one\n{GONE_LINE}\nthree\n")).expect("the change");
    piece
}

/// The operator discards the file's hunk from the window.
fn the_operator_discards_the_hunk(held: &Held) {
    let at = Branch::piece("alpha", "svc", "piece");
    let shown =
        files::what_changed(&crate::reader(), held.root(), at, FILE).expect("the file's diff");
    let FileDiff::Text { hunks, .. } = shown.diff else {
        panic!("a text file's diff");
    };
    let ask = Discard {
        path: FILE.to_owned(),
        target: Target::Hunk(hunks[0]),
        against: Against::Opened(shown.stamp.expect("a stamp")),
    };
    let answer = discard_in(held, at, &ask, true).expect("the discard");
    assert!(
        matches!(
            answer,
            DiscardAnswer::Discarded {
                unrecorded: None,
                ..
            }
        ),
        "{answer:?}"
    );
}

/// The line chat `chat`'s hook is handed as its turn begins.
fn told(held: &Held, id: &PlaneId, chat: u32) -> Option<String> {
    match asks(held, id, chat, What::HumanEdits) {
        Answer::Task(answered) => match *answered {
            Answered::HumanEdits { line } => line,
            other => panic!("the line, not {other:?}"),
        },
        other => panic!("the line, not {other:?}"),
    }
}

/// Chat `chat`'s prompt reaches the log, as the hook reports it once it has printed.
fn its_prompt_is_recorded(events: &crate::hooks::Events, root: &Path, chat: u32) {
    events
        .record(Entry::Report {
            plane: root.to_path_buf(),
            report: Report {
                chat,
                event: Event::UserPromptSubmit,
                conversation: Conversation::Unknown,
                pid: None,
                agent: None,
                detail: Detail::default(),
            },
            followed: eventlog::Followed::No,
            begun: None,
        })
        .expect("the report is recorded");
}

/// A chat working in `cwd`, on the pretend host.
fn a_chat_in(held: &Held, cwd: &Path) -> u32 {
    let chat = Chat {
        program: "harness".to_owned(),
        name: "1".to_owned(),
        cwd: Some(cwd.to_path_buf()),
        ..Default::default()
    };
    held.chats().start(&chat, STARTING).expect("it runs")
}

#[test]
fn a_discarded_hunk_is_told_once_at_each_chat_in_the_folders_next_turn_by_its_path_alone() {
    use purlis_core::eventlog::{ArgsKey, Log, Recorder};
    let plane = Plane::new();
    let host = Pretend::default();
    let logs = plane.root.join("events");
    let events = crate::hooks::Events::here(Recorder::new(
        Log::open(&logs, "DEVICE").expect("a log"),
        ArgsKey::open(&logs).expect("a key"),
    ));
    let planes = planes_on(&host).recording_events(Some(events.clone()));
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let folder = a_folder_with_the_agents_change(&plane.root);
    let first = a_chat_in(&held, &folder);
    let second = a_chat_in(&held, &folder);
    let elsewhere = a_chat_in(&held, &plane.root);
    its_prompt_is_recorded(&events, held.root(), first);
    assert_eq!(told(&held, &id, first), None);

    the_operator_discards_the_hunk(&held);

    let line = told(&held, &id, first).expect("the first chat is told");
    assert_eq!(
        line,
        "The operator edited these files by hand since your last turn. Read them again before \
         you change them: `src/lib.rs` in `svc`."
    );
    assert!(!line.contains(GONE_LINE));
    // Each chat in the folder has a next turn of its own, and no other chat is told.
    assert_eq!(told(&held, &id, second).as_deref(), Some(line.as_str()));
    assert_eq!(told(&held, &id, elsewhere), None);

    its_prompt_is_recorded(&events, held.root(), first);

    assert_eq!(told(&held, &id, first), None);
    assert_eq!(told(&held, &id, second).as_deref(), Some(line.as_str()));
}

/// How many times the log says chat `chat`'s prompt was heard.
fn prompts_of(logs: &Path, chat: &str) -> usize {
    eventlog::read(logs)
        .expect("the log reads")
        .iter()
        .filter(|event: &&Logged| {
            event.kind == "hook.userpromptsubmit" && event.chat.as_deref() == Some(chat)
        })
        .count()
}

fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !done() {
        assert!(Instant::now() < deadline, "{what} never happened");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The ticket's whole chat: prompt, discard a hunk, next turn, and the turn after.
#[cfg(unix)]
#[test]
fn a_whole_chat_is_told_of_a_discarded_hunk_at_its_next_turn_and_not_at_the_one_after() {
    let plane = Plane::new();
    let logs = plane.root.join("events");
    let planes = planes_recording_events(&logs);
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let folder = a_folder_with_the_agents_change(&plane.root);
    let scratch = tempfile::tempdir().expect("a folder for what the harness is told");
    let told = scratch.path().join("told");
    let purlis = crate::behaviour::built_beside("purlis", "purlis-cli");
    let harness = crate::behaviour::built_beside("fake-harness", "fake-harness");
    let hook = format!(
        "printf '%s' '{{\"session_id\":\"s-1\",\"prompt\":\"carry on\"}}' \
         | '{}' hook userpromptsubmit >> '{}'",
        purlis.display(),
        told.display()
    );
    let chat = Chat {
        program: harness.display().to_string(),
        args: [
            "--hook",
            hook.as_str(),
            "--interactive",
            "--again",
            "next turn",
        ]
        .map(str::to_owned)
        .to_vec(),
        name: "fake".to_owned(),
        cwd: Some(folder),
        ..Default::default()
    };
    let session = held
        .chats()
        .start(&chat, STARTING)
        .expect("the chat starts");
    let chat_id = held.chats().id_of(session).expect("the chat's id");
    let line_times = || {
        std::fs::read_to_string(&told)
            .unwrap_or_default()
            .matches("`src/lib.rs` in `svc`")
            .count()
    };
    // The first prompt: its report is what the edit comes after.
    until("the first prompt's report", || {
        prompts_of(&logs, &chat_id) == 1
    });
    assert_eq!(line_times(), 0);

    the_operator_discards_the_hunk(&held);

    let next_turn = || {
        held.chats()
            .sessions()
            .input(session, b"next turn\r")
            .expect("typed");
    };
    next_turn();
    until("the next prompt's report", || {
        prompts_of(&logs, &chat_id) == 2
    });
    assert_eq!(line_times(), 1, "the next turn's briefing names the file");
    assert!(
        !std::fs::read_to_string(&told)
            .unwrap_or_default()
            .contains(GONE_LINE)
    );

    next_turn();
    until("the third prompt's report", || {
        prompts_of(&logs, &chat_id) == 3
    });
    assert_eq!(line_times(), 1, "the turn after is not told again");
}
