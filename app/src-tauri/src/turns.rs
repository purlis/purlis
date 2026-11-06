// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The app's half of [`purlis_core::files::turns`] (N3, #5405): as a chat's own harness starts a
//! turn and ends one, or its program ends, the branch's state is recorded as that turn's refs.
//!
//! **Never on the hook's thread.** A hook waits for its line to be taken and has 50 ms (the spec's
//! limit), and staging a folder can take longer. So a boundary is queued and recorded on a thread
//! of the chat's own, in the order heard, and a slow folder holds up no other chat. The model has
//! to answer before a harness writes anything, which is what keeps a turn's start ahead of its
//! first edit. A boundary that cannot be recorded is said in the log: the chat is not held up, and
//! the turn shows no diff.
//!
//! **Only what ran here.** A chat on a runner works in a folder of the runner's: what the folder
//! here holds says nothing of its turn, so it records none.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::files::{Branch, turns};
use purlis_core::hookwire::Report;
use purlis_core::state::Event;

/// What a chat's thread does next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Boundary {
    Began,
    Ended,
    /// The chat is removed: its refs go, after whatever was queued before.
    Removed,
}

/// One queued boundary.
struct Queued {
    boundary: Boundary,
    cwd: PathBuf,
}

/// The chats with boundaries queued, by id. A chat is here only while its thread runs.
#[derive(Clone, Default)]
pub struct TurnRecorder {
    queues: Arc<Mutex<HashMap<String, VecDeque<Queued>>>>,
}

impl TurnRecorder {
    /// Chat `chat`, working in `cwd`, was reported: a turn's boundary when the report is its own
    /// harness's prompt or `Stop`, and nothing otherwise. A helper's end is no end of it.
    pub fn heard(&self, plane: &Path, chat: &str, cwd: &Path, report: &Report) {
        if report.agent.is_some() {
            return;
        }
        match report.event {
            Event::UserPromptSubmit => self.queue(plane, chat, cwd, Boundary::Began),
            Event::Stop => self.queue(plane, chat, cwd, Boundary::Ended),
            _ => {}
        }
    }

    /// Chat `chat`'s program ended: a turn still open ended with it.
    pub fn program_ended(&self, plane: &Path, chat: &str, cwd: &Path) {
        self.queue(plane, chat, cwd, Boundary::Ended);
    }

    /// Chat `chat` is removed, and its turns' refs with it.
    pub fn removed(&self, plane: &Path, chat: &str, cwd: &Path) {
        self.queue(plane, chat, cwd, Boundary::Removed);
    }

    fn queue(&self, plane: &Path, chat: &str, cwd: &Path, boundary: Boundary) {
        let queued = Queued {
            boundary,
            cwd: cwd.to_path_buf(),
        };
        let mut queues = self.held();
        if let Some(queue) = queues.get_mut(chat) {
            queue.push_back(queued);
            return;
        }
        queues.insert(chat.to_owned(), VecDeque::from([queued]));
        drop(queues);
        let (me, plane_at, id) = (self.clone(), plane.to_path_buf(), chat.to_owned());
        let spawned = std::thread::Builder::new()
            .name("turn-refs".to_owned())
            .spawn(move || me.drain(&plane_at, &id));
        if let Err(why) = spawned {
            self.held().remove(chat);
            tracing::warn!("purlis: no thread to record a turn of chat {chat} ({why})");
        }
    }

    /// Whether a boundary of `chat` is queued or being recorded: its latest turn's refs may not
    /// be written yet, so an undo waits until they are (#5411).
    pub fn recording(&self, chat: &str) -> bool {
        self.held().contains_key(chat)
    }

    /// Records what is queued for `chat` until nothing is. Taking the next and giving up the
    /// queue are one hold, so what is queued after the last is seen by a new thread, and the chat
    /// stays here until its last boundary is recorded ([`Self::recording`]).
    fn drain(&self, plane: &Path, chat: &str) {
        loop {
            let next = {
                let mut queues = self.held();
                let next = queues.get_mut(chat).and_then(VecDeque::pop_front);
                if next.is_none() {
                    queues.remove(chat);
                }
                next
            };
            let Some(Queued { boundary, cwd }) = next else {
                return;
            };
            if let Err(why) = record(plane, chat, &cwd, boundary) {
                tracing::warn!("purlis: a turn of chat {chat} was not recorded: {why}");
            }
        }
    }

    fn held(&self) -> std::sync::MutexGuard<'_, HashMap<String, VecDeque<Queued>>> {
        self.queues.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// One boundary, where `cwd` is a folder of a workspace's repo. A chat working anywhere else has
/// no branch, so it has no turn to record.
fn record(
    plane: &Path,
    chat: &str,
    cwd: &Path,
    boundary: Boundary,
) -> Result<(), purlis_core::files::Refused> {
    let Some(place) = turns::place_of(plane, cwd) else {
        return Ok(());
    };
    let branch = match place.piece.as_deref() {
        Some(piece) => Branch::piece(&place.ws, &place.repo, piece),
        None => Branch::repo(&place.ws, &place.repo),
    };
    match boundary {
        Boundary::Began => turns::began(plane, branch, chat).map(drop),
        Boundary::Ended => turns::ended(plane, branch, chat).map(drop),
        Boundary::Removed => turns::forget(plane, &place.ws, &place.repo, chat),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use purlis_core::files::{FileDiff, InTurn, TurnChanged};

    const CHAT: &str = "01J9CHAT0000000000000000TN";

    fn report(event: Event, payload: &str) -> Report {
        Report::read(event, payload, &|name| match name {
            purlis_core::hookwire::CHAT_ENV => Some("7".to_owned()),
            purlis_core::hookwire::HARNESS_ENV => Some("claude-code".to_owned()),
            _ => None,
        })
        .expect("a report of chat 7")
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let done = purlis_core::forklock::output(
            std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(["-c", "commit.gpgSign=false"])
                .args(args),
        )
        .expect("git runs");
        assert!(done.status.success(), "git {args:?}: {done:?}");
        String::from_utf8_lossy(&done.stdout).into_owned()
    }

    fn refs(dir: &Path) -> String {
        git(
            dir,
            &["for-each-ref", "--format=%(refname)", "refs/purlis/turns/"],
        )
    }

    fn until(what: &str, done: impl Fn() -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !done() {
            assert!(
                std::time::Instant::now() < deadline,
                "{what} never happened"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// The boundaries as a chat's reports bring them, off the caller's thread and in order, read
    /// as the file diff reads them. The whole chat, through its hooks, is
    /// `behaviour::a_turn_is_recorded_by_its_hooks_and_its_diff_is_what_it_edited`.
    #[test]
    fn a_prompt_and_a_stop_record_a_turn_the_file_diff_reads_and_removal_takes_it() {
        let plane = tempfile::tempdir().expect("a plane");
        let plane = std::fs::canonicalize(plane.path())
            .map(|path| (plane, path))
            .expect("it resolves");
        let folder = plane.1.join("workspaces/ws/app");
        std::fs::create_dir_all(&folder).expect("the repo's folder");
        git(&folder, &["init", "-q", "-b", "main"]);
        std::fs::write(folder.join("a.txt"), "one\n").expect("a file");
        git(&folder, &["add", "."]);
        git(&folder, &["commit", "-qm", "first"]);
        let recorder = TurnRecorder::default();

        // A helper's end is no end of the chat's turn: nothing is queued for it.
        let helper = report(Event::Stop, r#"{"agent_id":"helper"}"#);
        assert!(helper.agent.is_some());
        recorder.heard(&plane.1, CHAT, &folder, &helper);
        assert!(recorder.held().is_empty());
        recorder.heard(
            &plane.1,
            CHAT,
            &folder,
            &report(Event::UserPromptSubmit, "{}"),
        );
        until("the turn's start", || refs(&folder).contains("/1/start"));
        std::fs::write(folder.join("a.txt"), "one\nTWO\n").expect("the edit");
        recorder.heard(&plane.1, CHAT, &folder, &report(Event::Stop, "{}"));
        // Recording until the end is written, which an undo waits on (#5411).
        assert!(recorder.recording(CHAT));
        until("the turn's end", || !recorder.recording(CHAT));
        assert!(refs(&folder).contains("/1/end"));

        let read = purlis_core::files::turn_changed(
            &crate::reader(),
            &plane.1,
            Branch::repo("ws", "app"),
            "a.txt",
            None,
        )
        .expect("it reads");
        let TurnChanged::Of {
            number: 1,
            file: InTurn::Changed(shown),
            ..
        } = read
        else {
            panic!("a.txt is what the turn changed: {read:?}");
        };
        let FileDiff::Text { base, head, .. } = shown.diff else {
            panic!("a.txt is text");
        };
        assert_eq!((base.as_str(), head.as_str()), ("one\n", "one\nTWO\n"));

        // The program's end after the turn's own moves nothing.
        let at = format!("refs/purlis/turns/{CHAT}/1/end");
        let end = git(&folder, &["rev-parse", &at]);
        std::fs::write(folder.join("a.txt"), "one\nafter the turn\n").expect("a later edit");
        recorder.program_ended(&plane.1, CHAT, &folder);
        until("the program's end to be heard", || {
            recorder.held().is_empty()
        });
        assert_eq!(git(&folder, &["rev-parse", &at]), end);

        recorder.removed(&plane.1, CHAT, &folder);
        until("the chat's refs to go", || refs(&folder).is_empty());
    }

    #[test]
    fn a_chat_working_above_any_repo_records_nothing_and_says_nothing_is_wrong() {
        let plane = tempfile::tempdir().expect("a plane");
        assert!(
            record(
                plane.path(),
                CHAT,
                &plane.path().join("workspaces/ws"),
                Boundary::Began
            )
            .is_ok()
        );
    }
}
