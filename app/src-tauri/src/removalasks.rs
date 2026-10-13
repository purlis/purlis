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
//! - **Remove** is the window's own remove ([`crate::workspaces::remove_for_the_person`]) and
//!   nothing else: the core's guard runs inside it, never forced from here, so a workspace
//!   that holds work nothing else has, or has a chat running in it, is refused in the core's
//!   own words as it is from the sidebar.
//! - **Keep** puts the question away and removes nothing.
//!
//! **Only the person's press removes anything.** What a chat hands over is a name
//! (`purlis_core::hookwire::RemovalAsk`), and the name is looked up in the project's own
//! listing when it is asked and again when Remove is pressed
//! (`purlis_core::removalask::named`). A second ask of one chat about one workspace is the
//! same question, never a second one; a question about a workspace that is gone is dropped;
//! both answers are the window's alone (`purlis_session_protocol::ui::WINDOW_ONLY`).
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

/// The most workspaces one chat has a question waiting about at once: the oldest goes first.
pub const AT_MOST_PER_CHAT: usize = 3;

/// One question that waits: the chat that asked, the workspace, and when it began to wait, in
/// seconds since 1970.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waits {
    pub session: u32,
    pub workspace: String,
    pub since: u64,
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

    /// Notes that chat `session` asked about `workspace` at `now`, and answers whether that
    /// is news: one question per chat and workspace, however often the chat asks, waiting
    /// since the first.
    pub fn note(&self, session: u32, workspace: &str, now: u64) -> bool {
        let mut held = self.lock();
        if held
            .iter()
            .any(|one| one.session == session && one.workspace == workspace)
        {
            return false;
        }
        held.push(Waits {
            session,
            workspace: workspace.to_owned(),
            since: now,
        });
        let mine = held.iter().filter(|one| one.session == session).count();
        if mine > AT_MOST_PER_CHAT
            && let Some(oldest) = held.iter().position(|one| one.session == session)
        {
            held.remove(oldest);
        }
        true
    }

    /// Every question that waits, oldest first.
    pub fn every(&self) -> Vec<Waits> {
        self.lock().clone()
    }

    /// Whether chat `session` has a question waiting about `workspace`.
    pub fn holds(&self, session: u32, workspace: &str) -> bool {
        self.lock()
            .iter()
            .any(|one| one.session == session && one.workspace == workspace)
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
        Ok(workspace) => {
            let news = asked.note(session, &workspace, now);
            (removalask::asked_said(&workspace), news)
        }
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

/// What is said when Remove or Keep is pressed on a question that waits no more: answered
/// elsewhere, or its chat closed.
fn waits_no_more(workspace: &str) -> String {
    format!(
        "Nothing is asking to remove {} any more, so nothing was removed.",
        purlis_core::shown::short(workspace)
    )
}

/// **Remove** on a chat's question: the window's own remove of `workspace`, with its guard and
/// never forced, for a question chat `session` has waiting about that workspace.
///
/// `workspace` is what the question the person pressed showed. It is taken only where it is
/// the workspace that chat's question is about now, and is one the project lists now: a
/// question that changed under the pointer, or a name that is no workspace's, removes nothing.
/// A refusal is the core's own, with what it refused over, as the sidebar's remove hands it
/// back; the question still waits then.
#[tauri::command]
#[specta::specta]
pub async fn remove_asked_workspace(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    heard: tauri::State<'_, crate::heard::Heard>,
    plane: PlaneId,
    session: u32,
    workspace: String,
) -> Result<Vec<String>, crate::workspaces::Refused> {
    let unread = |said: String| crate::workspaces::Refused {
        said,
        at_risk: Vec::new(),
    };
    let held = planes.held(&plane).map_err(unread)?;
    if !held.removal_asks().holds(session, &workspace) {
        return Err(unread(waits_no_more(&workspace)));
    }
    // Looked up again as it is pressed: the listing is the project's, never the question's.
    let named = match removalask::named(held.root(), &workspace) {
        Ok(named) => named,
        Err(_) => {
            held.removal_asks().gone(&workspace);
            held.chats().asks_may_have_moved();
            return Err(unread(format!(
                "This project has no workspace {} any more, so nothing was removed.",
                purlis_core::shown::short(&workspace)
            )));
        }
    };
    crate::workspaces::remove_for_the_person(&app, &held, &heard, plane, named, false).await
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
