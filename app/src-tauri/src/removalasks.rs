// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A workspace a chat asked the person to remove** (qw356).
//!
//! A chat whose sandbox holds a removal is refused `purlis workspace remove` before anything
//! is deleted, and that used to be where it ended, even in the chat purlis's own Safe remove
//! opens. Here the refusal becomes a question in the asks registry (`crate::asking`), drawn on
//! the asking chat's tab and in the Inbox: which workspace, what removing it deletes, and
//! **Remove** and **Keep**.
//!
//! - **Remove…** opens the window's own Delete workspace dialog and removes nothing: what
//!   goes is read there, and the person's press in it is the window's own remove
//!   ([`crate::workspaces::workspace_remove`]), with the core's guard. No command here removes
//!   a workspace: a removal is for good and it is a chat that raised the question, so it is
//!   never one press on a Notice or on an Inbox row.
//! - **Keep** puts the question away and removes nothing.
//!
//! **Only the person's press removes anything.** What a chat hands over is a name
//! (`purlis_core::hookwire::RemovalAsk`), and the name is looked up in the project's own
//! listing when it is asked (`purlis_core::removalask::named`). A second ask of one chat about
//! one workspace is the same question, never a second one; a chat with its few questions
//! waiting is not asked another, and none that waits is put away for it; a question about a
//! workspace that is gone is dropped; Keep is the window's alone
//! (`purlis_session_protocol::ui::WINDOW_ONLY`).
//!
//! **The chat is told what the person decided on its next turn**, in purlis's fixed words,
//! on the road the person's other answers take
//! (`purlis_core::dispatchtalk::Talk::removal_word`): from the app's memory, never a file. A
//! workspace the person removes from the sidebar while a chat's question about it waits
//! answers that question too.
//!
//! In memory only: a relaunch starts with none, and the chat's next try asks again.

use std::path::Path;
use std::sync::{Mutex, PoisonError};

use purlis_core::dispatchtalk::RemovalWord;
use purlis_core::hookwire::{Answer, RemovalAsk, Removing};
use purlis_core::removalask;

use crate::planes::{Held, PlaneId, Planes};

/// The most workspaces one chat has a question waiting about at once: one more is not asked.
pub const AT_MOST_PER_CHAT: usize = 3;

/// One question that waits: the chat that asked, the workspace, and when it began to wait, in
/// seconds since 1970.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waits {
    pub session: u32,
    pub workspace: String,
    pub since: u64,
}

/// What became of an ask that was noted ([`Asked::note`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Noted {
    /// A question began to wait.
    News,
    /// The chat's question about that workspace waits already.
    Already,
    /// The chat has as many questions waiting as one chat may: nothing was asked.
    TooMany,
}

/// The workspaces each of a project's chats asked the person to remove, until they answer.
#[derive(Debug, Default)]
pub struct Asked {
    held: Mutex<Vec<Waits>>,
}

impl Asked {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Waits>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Notes that chat `session` asked about `workspace` at `now`: one question per chat and
    /// workspace, however often the chat asks, waiting since the first, and
    /// [`AT_MOST_PER_CHAT`] of one chat's at most. **A question that waits is never put away
    /// for a later one**: what the person is reading stays where it is, and a chat going round
    /// many names raises nothing new.
    pub fn note(&self, session: u32, workspace: &str, now: u64) -> Noted {
        let mut held = self.lock();
        if held
            .iter()
            .any(|one| one.session == session && one.workspace == workspace)
        {
            return Noted::Already;
        }
        if held.iter().filter(|one| one.session == session).count() >= AT_MOST_PER_CHAT {
            return Noted::TooMany;
        }
        held.push(Waits {
            session,
            workspace: workspace.to_owned(),
            since: now,
        });
        Noted::News
    }

    /// Every question that waits, oldest first.
    pub fn every(&self) -> Vec<Waits> {
        self.lock().clone()
    }

    /// Chat `session`'s question about `workspace` was answered: it waits no more. Answers
    /// whether it was waiting.
    pub fn answered(&self, session: u32, workspace: &str) -> bool {
        let mut held = self.lock();
        let before = held.len();
        held.retain(|one| !(one.session == session && one.workspace == workspace));
        held.len() != before
    }

    /// `workspace` is gone: every question about it waits no more. The chats that asked.
    pub fn gone(&self, workspace: &str) -> Vec<u32> {
        let mut held = self.lock();
        let asked = held
            .iter()
            .filter(|one| one.workspace == workspace)
            .map(|one| one.session)
            .collect();
        held.retain(|one| one.workspace != workspace);
        asked
    }

    /// Chat `session` has closed: nothing of its waits.
    pub fn forget(&self, session: u32) {
        self.lock().retain(|one| one.session != session);
    }

    /// Drops every question about a workspace `there` does not list: one removed elsewhere, in
    /// a terminal say. Answers whether any was dropped.
    pub fn only(&self, there: &dyn Fn(&str) -> bool) -> bool {
        let mut held = self.lock();
        let before = held.len();
        held.retain(|one| there(&one.workspace));
        held.len() != before
    }
}

/// What the app answers chat `session`'s ask about `name`, in the project at `root`, and
/// whether a question began to wait: the sentence that says the person has been asked, or why
/// they were not. **Nothing is removed here, whatever is asked.**
pub fn asked_in(root: &Path, asked: &Asked, session: u32, name: &str, now: u64) -> (String, bool) {
    match removalask::named(root, name) {
        Ok(workspace) => match asked.note(session, &workspace, now) {
            Noted::News => (removalask::asked_said(&workspace), true),
            Noted::Already => (removalask::asked_said(&workspace), false),
            Noted::TooMany => (removalask::too_many_said(&workspace), false),
        },
        Err(why) => (why, false),
    }
}

/// Chat `ask.chat` ran `purlis workspace remove` where its sandbox holds the removal, and its
/// command asks the app to ask the person.
///
/// The chat the line names is the one whose token it carries, which the listener checks
/// before this is asked. Each chat is held to the rate its brokered writes are
/// (`purlis_core::brokered::Rate`), so a chat cannot fill the person's Inbox by asking.
pub fn asked(held: &Held, ask: &RemovalAsk) -> Answer {
    let no = |why: String| Answer::No { why };
    if !held
        .chats()
        .open_now()
        .iter()
        .any(|open| open.session == ask.chat)
    {
        return no(format!("chat {} is not one this app has open", ask.chat));
    }
    if let Err(why) = held.brokered().allow(ask.chat, std::time::Instant::now()) {
        return no(why);
    }
    let Removing::Workspace { name } = &ask.removing;
    let (said, news) = asked_in(
        held.root(),
        held.removal_asks(),
        ask.chat,
        name,
        crate::sandboxing::now_secs(),
    );
    if news {
        tracing::info!(
            "purlis: chat {} asked the person to remove a workspace; nothing is removed until \
             they answer in the window",
            ask.chat
        );
        held.chats().asks_may_have_moved();
    }
    no(said)
}

/// The questions project `held` has waiting, each about a workspace the project still has: one
/// about a workspace that is gone is dropped here, as it is read.
pub fn waiting(held: &Held) -> Vec<Waits> {
    let every = purlis_core::workspaces::Plane::open(held.root())
        .workspaces()
        .unwrap_or_default();
    held.removal_asks()
        .only(&|workspace| every.iter().any(|one| one == workspace));
    held.removal_asks().every()
}

/// The person removed `workspace` from the window, by whichever of its ways: every chat that
/// asked them to is told on its next turn, and its question waits no more.
pub fn removed(held: &Held, workspace: &str) {
    let asked = held.removal_asks().gone(workspace);
    if asked.is_empty() {
        return;
    }
    {
        let mut ledger = held.tasks().ledger();
        for session in &asked {
            ledger.talk.removal_word(
                *session,
                RemovalWord::Removed {
                    workspace: workspace.to_owned(),
                },
            );
        }
    }
    held.chats().asks_may_have_moved();
}

/// What is said when Keep is pressed on a question that waits no more: answered elsewhere, or
/// its chat closed.
fn waits_no_more(workspace: &str) -> String {
    format!(
        "Nothing is asking to remove {} any more, so nothing was removed.",
        purlis_core::shown::short(workspace)
    )
}

/// **Keep** on a chat's question: puts it away and removes nothing. The chat is told on its
/// next turn that the person kept the workspace, and not to ask again unless they ask.
#[tauri::command]
#[specta::specta]
pub fn keep_asked_workspace(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    workspace: String,
) -> Result<(), String> {
    kept(&*planes.held(&plane)?, session, &workspace)
}

/// The person kept `workspace`, which chat `session` asked them to remove.
pub fn kept(held: &Held, session: u32, workspace: &str) -> Result<(), String> {
    if !held.removal_asks().answered(session, workspace) {
        return Err(waits_no_more(workspace));
    }
    held.tasks().ledger().talk.removal_word(
        session,
        RemovalWord::Kept {
            workspace: workspace.to_owned(),
        },
    );
    held.chats().asks_may_have_moved();
    Ok(())
}

#[cfg(test)]
#[path = "removalasks_tests.rs"]
mod tests;
