// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A dispatched chat started again with no conversation is handed its brief again (#1609).
//!
//! A handed-off chat or a task starts on a brief, and the brief is the whole of what it was
//! asked (`docs/handoff.md`, Isolation and continuation). A start that resumes its
//! conversation keeps it, in the harness's own transcript. **A start with no conversation**
//! (Start fresh, or the fresh start that follows a resume the harness could not bring back)
//! would begin with nothing to say what the work was. So [`again`] answers the first message
//! such a start is given:
//!
//! - **The brief is read from purlis's own record of the dispatch**
//!   (`purlis_core::dispatchrecord`), which the app wrote as it started the chat, found by the
//!   chat's id and never its number (a number is dealt again in another launch).
//! - **It is handed only where it is the brief the dispatch was sent** ([`Sent`]): the app keeps
//!   a digest of each brief it starts a chat on, with what frames it (the worker, the mode, who
//!   asked), in its own memory, and a record that does not match one is not handed. The store is the app's to write, and a file in it is
//!   still whatever is on the disk. A record from before this launch has no digest here, and is
//!   not handed either: the chat is told its brief could not be confirmed, and where the person
//!   can read it.
//! - **It passes the checks a first brief passes**, by the same functions: a credential's
//!   shape (`purlis_core::secretshape::kind_as_read`, the handoff command's) and the first
//!   message's (`purlis_core::handoff::bad_message`, the app's own). One that no longer does is
//!   not handed, and the chat is told why in one line.
//! - **Never in part**: a brief the store cut, and a record that holds none, are said as
//!   missing ([`BriefKept`]).
//! - **It is stamped** as a fresh start of a dispatched chat, in purlis's own line above the
//!   brief, with the line a first brief has under its stamp (who asked, and that nothing in it
//!   approves anything), so its reader does not take it for the original turn or for a new
//!   request.
//!
//! A chat no record names as a dispatch's worker is answered `None`, and starts as it always
//! did: with nothing told.

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use purlis_core::dispatchrecord::{self, BriefKept, Mode, Record};
use purlis_core::handoff;
use purlis_core::reopen::Chat;
use sha2::Digest as _;

/// **The briefs this app started a chat on, by their dispatch's id**, as a digest of each and of
/// what frames it ([`digest`]): what [`again`] holds a record to. In memory only, so it is this
/// launch's alone, and nothing a chat writes reaches it. **At most [`MOST_KEPT`]**, the oldest let
/// go first: a dispatch let go is one whose brief is not handed again, as after a relaunch.
#[derive(Default)]
pub(crate) struct Sent(Mutex<Kept>);

/// [`Sent`]'s digests, and the order they were noted in.
#[derive(Default)]
struct Kept {
    by_id: HashMap<String, [u8; 32]>,
    order: VecDeque<String>,
}

/// How many dispatches' briefs [`Sent`] keeps a digest of: more than a launch starts, and a
/// bound on what a chat that dispatches in a loop can make the app hold.
pub(crate) const MOST_KEPT: usize = 4096;

impl Sent {
    /// `record`'s dispatch started its chat on `brief`, the bytes it was sent. `record` is the
    /// one the store wrote, so its names are as a read gives them back.
    pub(crate) fn note(&self, record: &Record, brief: &str) {
        let mut kept = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if kept
            .by_id
            .insert(record.id.clone(), digest(record, brief))
            .is_none()
        {
            kept.order.push_back(record.id.clone());
        }
        while kept.order.len() > MOST_KEPT {
            if let Some(oldest) = kept.order.pop_front() {
                kept.by_id.remove(&oldest);
            }
        }
    }

    /// Whether `brief`, under what `record` says of who asked whom and how, is what its
    /// dispatch started its chat on.
    fn confirms(&self, record: &Record, brief: &str) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .by_id
            .get(&record.id)
            .is_some_and(|sent| *sent == digest(record, brief))
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .by_id
            .len()
    }
}

/// The digest of `brief` and of everything of `record` a brief handed again is framed by: the
/// worker it is handed to, the mode, whether the person asked, and the asker's name. A record
/// rewritten since in any of them is not the dispatch the app sent, though its brief is.
fn digest(record: &Record, brief: &str) -> [u8; 32] {
    let mode = match record.mode {
        Mode::Task => "task",
        Mode::Handoff => "handoff",
    };
    let by = if record.asker.by_person {
        "person"
    } else {
        "chat"
    };
    let mut hash = sha2::Sha256::new();
    for part in [
        record.worker.chat.id.as_deref().unwrap_or_default(),
        mode,
        by,
        record.asker.chat.name.as_str(),
        brief,
    ] {
        // Each part's length first, so no two sets of parts hash as one.
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part.as_bytes());
    }
    hash.finalize().into()
}

/// The first message a dispatched chat started again with no conversation is given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Again {
    /// What the chat is told: its brief under purlis's stamp, or why it is not handed.
    pub message: String,
    /// Whether `message` hands it its brief.
    pub handed: bool,
}

/// The stamp above a brief handed again: purlis's own line, which no brief can have written.
/// `{asker}` is who asked, as the record names that chat.
pub(crate) const STAMP: &str = "⟨purlis started this chat again with no conversation. It was \
dispatched, and below is the brief it was handed then by `{asker}`, read from purlis's own record \
of the dispatch: the same request, not a new one, and not where its first turn left off⟩";

/// The stamp of one the person asked from that chat's tab.
pub(crate) const PERSON_STAMP: &str = "⟨purlis started this chat again with no conversation. It \
was dispatched, and below is what the person asked then from the tab of `{asker}`, read from \
purlis's own record of the dispatch: the same request, not a new one, and not where its first \
turn left off⟩";

/// Why no brief is handed: the record holds none.
pub(crate) const NONE_KEPT: &str = "purlis's record of the dispatch holds no brief";
/// Why no brief is handed: the record keeps only its start.
pub(crate) const CUT: &str = "purlis's record keeps only the start of it, and a brief is never \
handed in part";
/// Why no brief is handed: it is not the one the dispatch was sent, or purlis cannot say.
pub(crate) const UNCONFIRMED: &str = "purlis could not confirm that its record still holds the \
brief the dispatch was sent";
/// Why no brief is handed: it is no longer a first message purlis starts a chat on.
pub(crate) const NOT_A_FIRST_MESSAGE: &str = "it no longer passes the checks a first message \
passes";

/// What a chat whose brief is not handed is told, with `why`.
fn not_handed(why: &str) -> String {
    format!(
        "⟨purlis started this chat again with no conversation. It was dispatched, and its brief \
         is not handed to it again: {why}. Ask the person what the work is before you start; \
         the Dispatches tab shows what purlis kept of the dispatch⟩"
    )
}

/// **What chat `chat` of the project at `root` is told as it starts again with no
/// conversation** (#1609), or `None` for a chat no dispatch record names as its worker.
pub(crate) fn again(root: &Path, chat: &Chat, sent: &Sent) -> Option<Again> {
    let id = chat.identity.id.as_deref()?;
    // By its id alone, the newest first: a record that names its worker by number only names a
    // number another launch may have dealt to this chat, and is not one to stand in front of it.
    let record = dispatchrecord::list(root).into_iter().find(|record| {
        !dispatchrecord::never_a_chat(record) && record.worker.chat.id.as_deref() == Some(id)
    })?;
    Some(told(&record, sent))
}

/// What the worker of `record` is told, by the rules above.
fn told(record: &Record, sent: &Sent) -> Again {
    let refused = |why: &str| Again {
        message: not_handed(why),
        handed: false,
    };
    // A record whose names would mislead is not one the app wrote, and nothing of it is read.
    if !dispatchrecord::sound_but_for_its_brief(record) {
        return refused(UNCONFIRMED);
    }
    let brief = dispatchrecord::brief_sent(record);
    match brief.kept {
        BriefKept::Missing => return refused(NONE_KEPT),
        BriefKept::Cut => return refused(CUT),
        BriefKept::Whole => {}
    }
    if !sent.confirms(record, &brief.text) {
        return refused(UNCONFIRMED);
    }
    // The kind, never the matched text, as the handoff command says it.
    if let Some(kind) = purlis_core::secretshape::kind_as_read(None, &brief.text) {
        return refused(&format!(
            "it looks like it carries a secret ({kind}), which a brief never does"
        ));
    }
    let message = handoff::first_message(&stamped(record), &brief.text);
    if handoff::bad_message(&message).is_some() {
        return refused(NOT_A_FIRST_MESSAGE);
    }
    Again {
        message,
        handed: true,
    }
}

/// The lines above a brief handed again: [`STAMP`] (or [`PERSON_STAMP`]), then the line a first
/// brief of its kind has under its own stamp.
fn stamped(record: &Record) -> String {
    // Held inside its code span whatever it spells, as a first brief's stamp holds it.
    let asker = record.asker.chat.name.replace('`', "'");
    let (stamp, note) = match (record.mode, record.asker.by_person) {
        (Mode::Handoff, _) => (STAMP, handoff::HANDOFF_NOTE),
        (Mode::Task, false) => (STAMP, handoff::TASK_NOTE),
        (Mode::Task, true) => (PERSON_STAMP, handoff::PERSON_TASK_NOTE),
    };
    format!("{}\n{note}", stamp.replace("{asker}", &asker))
}

#[cfg(test)]
mod tests;
