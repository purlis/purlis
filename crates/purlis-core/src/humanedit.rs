// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A human edit** (ADR 0084 §8, as widened by ADR 0081's amendment of 2026-11-05): a change the
//! operator made to a file of a chat's branch from purlis. One [`HumanEdit`] is one event in the
//! event log (FD-9), and it holds which files and lines, never their contents.
//!
//! It is the one record every kind of edit writes and the one thing that reads them back goes
//! through: the announcement at the agent's next turn (R8, V34a) reads the events since the
//! chat's last turn with [`HumanEdit::of`]. It adds no store: its tier is the event log's
//! (Machine, device-bound; ADR 0069 row 63).

use serde::{Deserialize, Serialize};

use crate::eventlog::Event;

/// The event's kind in the log.
pub const KIND: &str = "human.edit";

/// What the operator did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EditKind {
    /// An edit typed and saved in the light editor (RC-10).
    Typed,
    /// One hunk of a file's diff put back as it was, or a binary file's change whole.
    DiscardedHunk,
    /// Every file a turn changed put back as it was before it (**Undo a turn**).
    UndoneTurn,
}

/// Lines of a file as the edit left it, in `git diff -U0`'s numbers: counted from 1, and a range
/// of no lines naming the line the removal comes after (`0` for the top).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditedLines {
    pub start: u32,
    pub count: u32,
}

/// One file an edit changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditedFile {
    /// Its path relative to the repo.
    pub path: String,
    /// The lines it changed; none for a binary file, which has no lines.
    pub lines: Vec<EditedLines>,
}

/// One human edit, as its event's body holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanEdit {
    pub kind: EditKind,
    /// The repo the paths are relative to, as the agent's line names it.
    pub repo: String,
    /// The branch folder it was made in; none for the repo's own folder.
    pub piece: Option<String>,
    pub files: Vec<EditedFile>,
    /// Whether a chat working in the folder was not idle when the edit was made (ADR 0084 §8).
    pub busy: bool,
    /// The number of every chat working in the folder, each of which is told at its next turn.
    /// The event's envelope is under the first of them.
    pub chats: Vec<u32>,
}

impl HumanEdit {
    /// The event's body.
    pub fn body(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    /// The edit an event records; `None` for an event of any other kind.
    pub fn of(event: &Event) -> Option<Self> {
        (event.kind == KIND)
            .then(|| serde_json::from_value(event.body.clone()).ok())
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: &str, body: serde_json::Value) -> Event {
        Event {
            v: 1,
            device_id: "device".into(),
            seq: 1,
            ulid: "01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
            chat: Some("chat".into()),
            run: Some("run".into()),
            parent_run: None,
            kind: kind.into(),
            body,
        }
    }

    #[test]
    fn an_edit_comes_back_from_its_event_and_says_its_kind_in_words() {
        let edit = HumanEdit {
            kind: EditKind::DiscardedHunk,
            repo: "svc".into(),
            piece: Some("fix-it".into()),
            files: vec![EditedFile {
                path: "src/lib.rs".into(),
                lines: vec![EditedLines { start: 9, count: 1 }],
            }],
            busy: true,
            chats: vec![3, 4],
        };

        assert_eq!(edit.body()["kind"], "discarded-hunk");
        assert_eq!(HumanEdit::of(&event(KIND, edit.body())), Some(edit));
    }

    #[test]
    fn the_log_takes_one_edit_as_one_event_and_gives_it_back() {
        use crate::eventlog::{self, ArgsKey, Entry, Log, Recorder};
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("events");
        let mut recorder = Recorder::new(
            Log::open(&logs, "DEVICE").unwrap(),
            ArgsKey::open(&logs).unwrap(),
        );
        let edit = HumanEdit {
            kind: EditKind::DiscardedHunk,
            repo: "svc".into(),
            piece: None,
            files: vec![EditedFile {
                path: "src/lib.rs".into(),
                lines: Vec::new(),
            }],
            busy: false,
            chats: Vec::new(),
        };

        recorder
            .apply(Entry::HumanEdit {
                plane: dir.path().to_path_buf(),
                number: None,
                edit: edit.clone(),
            })
            .unwrap();

        let edits: Vec<_> = eventlog::read(&logs)
            .unwrap()
            .iter()
            .filter_map(HumanEdit::of)
            .collect();
        assert_eq!(edits, [edit]);
    }

    #[test]
    fn an_event_of_another_kind_is_no_edit() {
        let body = serde_json::json!({ "kind": "typed" });
        assert_eq!(HumanEdit::of(&event("trust.proposed.apply", body)), None);
    }

    #[test]
    fn each_kind_has_the_word_the_briefing_will_read() {
        for (kind, word) in [
            (EditKind::Typed, "typed"),
            (EditKind::DiscardedHunk, "discarded-hunk"),
            (EditKind::UndoneTurn, "undone-turn"),
        ] {
            assert_eq!(serde_json::to_value(kind).unwrap(), word);
        }
    }
}
