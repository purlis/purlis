// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Proposed changes the person applies** (#1975, the operator's rulings PC-1..PC-14 of
//! 2026-10-14): the window's half of [`purlis_core::proposed`].
//!
//! A chat's edit of purlis's own configuration reaches the app as a proposal
//! (`purlis_core::hookwire::Ask::Propose`): the file, the base the change was worked out
//! against, and the change (the tool's own edits, or a whole new content). Here it is held for
//! the person in the asks registry (`crate::asking`), drawn on the chat's tab and in the Inbox:
//! the change shown whole, **Apply** and **Dismiss**.
//!
//! - **What is shown and what is written are the app's own** (PC-3): the file is read here, only
//!   as the one regular file its name says, the change is made to it here, and Apply writes
//!   exactly that, as the person, only while the file is still what the proposal was worked out
//!   against ([`purlis_core::proposed::write_pinned`]). Nothing the chat sent says what the file
//!   holds.
//! - **Stale** (PC-8): where the file changed since, Apply refuses, the proposal offers Dismiss
//!   alone, and the chat is told to read it again and propose again. Never merged.
//! - **One a file a chat** (PC-9): a newer proposal of a chat's replaces its older one for the
//!   same file; two chats' proposals are both held, and applying one makes the other stale.
//! - **The chat hears fixed words** (PC-10): applied, dismissed or stale, on its next turn
//!   (`purlis_core::dispatchtalk::Talk::proposal_word`). Its proposals end with it.
//! - **Recorded** (PC-4): every Apply is audited before the file is written.
//! - **Apply allows nothing** (PC-5) and there is **no "always apply"** (PC-13); policy's
//!   `sandbox.proposed-changes = "off"` holds none (PC-14).
//! - **Shown whole, or not applied here** (PC-7): a change too long for a row says so and
//!   offers Dismiss; its comparison tab with Apply is follow-up work.
//!
//! In memory only: a relaunch starts with none, and the chat's next edit proposes again.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use purlis_core::dispatchtalk::ProposalWord;
use purlis_core::hookwire::{Answer, ProposeAsk};
use purlis_core::proposed::{self, Change, NotMade};

use crate::planes::{Held, PlaneId, Planes};

/// The most proposals one chat has waiting at once: one more is not taken.
pub const AT_MOST_PER_CHAT: usize = 8;

/// One proposal that waits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// Its number in the project, for the window's answer.
    pub id: u32,
    pub session: u32,
    /// The file, relative to the project, as [`proposed::proposable`] took it.
    pub path: String,
    pub base: Option<String>,
    pub change: Change,
    /// When it began to wait, in seconds since 1970.
    pub since: u64,
    /// The file changed since it was proposed: Dismiss alone (PC-8).
    pub stale: bool,
}

/// The proposals of a project's chats, until the person answers.
#[derive(Debug, Default)]
pub struct Proposals {
    held: Mutex<(u32, Vec<Proposal>)>,
}

impl Proposals {
    fn lock(&self) -> std::sync::MutexGuard<'_, (u32, Vec<Proposal>)> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Holds chat `session`'s proposal to `path`: in place of its older one for that file
    /// (PC-9), or none where the chat has [`AT_MOST_PER_CHAT`] waiting already.
    pub fn note(
        &self,
        session: u32,
        path: &str,
        base: Option<String>,
        change: Change,
        now: u64,
    ) -> Option<u32> {
        let mut held = self.lock();
        let (next, list) = &mut *held;
        list.retain(|one| !(one.session == session && one.path == path));
        if list.iter().filter(|one| one.session == session).count() >= AT_MOST_PER_CHAT {
            return None;
        }
        *next = next.wrapping_add(1).max(1);
        let id = *next;
        list.push(Proposal {
            id,
            session,
            path: path.to_owned(),
            base,
            change,
            since: now,
            stale: false,
        });
        Some(id)
    }

    /// Every proposal that waits, oldest first.
    pub fn every(&self) -> Vec<Proposal> {
        self.lock().1.clone()
    }

    /// Proposal `id`, where it waits.
    pub fn get(&self, id: u32) -> Option<Proposal> {
        self.lock().1.iter().find(|one| one.id == id).cloned()
    }

    /// Takes proposal `id` out: it waits no more.
    pub fn take(&self, id: u32) -> Option<Proposal> {
        let mut held = self.lock();
        let at = held.1.iter().position(|one| one.id == id)?;
        Some(held.1.remove(at))
    }

    /// Marks proposal `id` stale. Answers whether it was not already.
    pub fn stale(&self, id: u32) -> bool {
        let mut held = self.lock();
        match held.1.iter_mut().find(|one| one.id == id) {
            Some(one) if !one.stale => {
                one.stale = true;
                true
            }
            _ => false,
        }
    }

    /// Every other proposal to `path` is stale now (PC-9): the chats newly told.
    pub fn others_stale(&self, path: &str) -> Vec<u32> {
        let mut held = self.lock();
        let mut told = Vec::new();
        for one in held
            .1
            .iter_mut()
            .filter(|one| one.path == path && !one.stale)
        {
            one.stale = true;
            told.push(one.session);
        }
        told
    }

    /// Chat `session` has closed: its proposals go with it (PC-10).
    pub fn forget(&self, session: u32) {
        self.lock().1.retain(|one| one.session != session);
    }
}

/// What the chat is told once its proposal is held: the person has it, and it waits.
pub const HELD_SAID: &str = "shown to the person as a proposed change: only they apply it, in \
                             the window. Wait for their answer.";

/// What a chat is told where the file changed since it read it.
pub const CHANGED_SINCE: &str = "the file changed since this chat read it, so nothing was \
                                 proposed: read it again, and propose the change again.";

/// **Chat `ask.chat` proposed a change** to one of purlis's own configuration files: held for
/// the person, or why not. **Nothing is written here, whatever is asked.**
///
/// The chat the line names is the one whose token it carries, which the listener checks before
/// this is asked; each chat is held to the rate its brokered writes are, so a chat cannot fill
/// the person's Inbox by proposing.
pub fn asked(held: &Held, ask: &ProposeAsk) -> Answer {
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
    match asked_in(
        held.root(),
        held.proposals(),
        ask,
        crate::sandboxing::now_secs(),
    ) {
        Ok(()) => {
            held.chats().asks_may_have_moved();
            Answer::Said {
                lines: vec![purlis_core::repocmd::Say::Done(HELD_SAID.to_owned())],
                code: 0,
            }
        }
        Err(why) => no(why),
    }
}

/// [`asked`]'s judgement of `ask` in the project at `root`, held in `proposals` at `now`.
pub fn asked_in(
    root: &Path,
    proposals: &Proposals,
    ask: &ProposeAsk,
    now: u64,
) -> Result<(), String> {
    if let Some(why) = purlis_core::sandbox::policy::Locks::of(root).proposed_changes_refused() {
        return Err(why);
    }
    if ask.change.carried() > proposed::MOST_BYTES {
        return Err(proposed::UNPREPARED.to_owned());
    }
    let rel = proposed::proposable(root, &root.join(&ask.path))?;
    // Made here, against the file as this app reads it, before it is held: one that does not
    // apply is never shown.
    match proposed::prepare(root, &rel, Some(ask.base.as_deref()), &ask.change) {
        Ok(_) => {}
        Err(NotMade::Stale) => return Err(CHANGED_SINCE.to_owned()),
        Err(NotMade::Unprepared) => return Err(proposed::UNPREPARED.to_owned()),
    }
    let path = rel.display().to_string();
    proposals
        .note(ask.chat, &path, ask.base.clone(), ask.change.clone(), now)
        .map(|_| ())
        .ok_or_else(|| {
            format!(
                "this chat has {AT_MOST_PER_CHAT} proposed changes waiting already: wait for the \
                 person's answers before proposing another"
            )
        })
}

/// What a proposal is now, as its row and its Notice draw it: the change made to the file as
/// it is, or why it cannot be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Now {
    /// The file before and after: shown whole where it fits a row.
    Made { before: String, after: String },
    /// The file changed since, or the proposal was marked so.
    Stale,
    /// It cannot be made at all any more.
    Unprepared,
}

/// Proposal `one` made to its file as it is now, in the project at `root`.
pub fn now_of(root: &Path, one: &Proposal) -> Now {
    if one.stale {
        return Now::Stale;
    }
    match proposed::prepare(
        root,
        Path::new(&one.path),
        Some(one.base.as_deref()),
        &one.change,
    ) {
        Ok(made) => Now::Made {
            before: made.before.unwrap_or_default(),
            after: made.after,
        },
        Err(NotMade::Stale) => Now::Stale,
        Err(NotMade::Unprepared) => Now::Unprepared,
    }
}

/// Every proposal of project `held` that waits, each with what it is now. One found stale here
/// is marked so, and its chat told.
pub fn waiting(held: &Held) -> Vec<(Proposal, Now)> {
    let root = held.root();
    held.proposals()
        .every()
        .into_iter()
        .map(|one| {
            let now = now_of(root, &one);
            if now == Now::Stale && held.proposals().stale(one.id) {
                tell(
                    held,
                    one.session,
                    ProposalWord::Stale {
                        path: one.path.clone(),
                    },
                );
            }
            (one, now)
        })
        .collect()
}

/// Tells chat `session` `word` on its next turn.
fn tell(held: &Held, session: u32, word: ProposalWord) {
    held.tasks().ledger().talk.proposal_word(session, word);
}

/// Why an answer found nothing to answer: answered elsewhere, or its chat closed.
const WAITS_NO_MORE: &str = "That proposed change waits no more: it was answered already, or \
                             its chat closed. Nothing was written.";

/// **Apply** on a proposed change (PC-3): written exactly as the window showed it, as the
/// person, only while the file is still what it was proposed against; audited first (PC-4).
/// Window-only.
#[tauri::command]
#[specta::specta]
pub async fn apply_proposed_change(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: u32,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("applying a proposed change", move || applied(&held, id)).await
}

/// [`apply_proposed_change`]'s work, for project `held`.
pub fn applied(held: &Held, id: u32) -> Result<String, String> {
    let one = held.proposals().get(id).ok_or(WAITS_NO_MORE)?;
    let root = held.root();
    let persona = held
        .chats()
        .recorded_chat(one.session)
        .and_then(|chat| chat.persona);
    let audit =
        |applied: &proposed::Applied<'_>| held.hooks().record_proposed(root, one.session, applied);
    match apply_in(root, &one, persona.as_deref(), &audit) {
        Ok(said) => {
            held.proposals().take(id);
            for session in held.proposals().others_stale(&one.path) {
                tell(
                    held,
                    session,
                    ProposalWord::Stale {
                        path: one.path.clone(),
                    },
                );
            }
            tell(
                held,
                one.session,
                ProposalWord::Applied {
                    path: one.path.clone(),
                },
            );
            held.chats().asks_may_have_moved();
            Ok(said)
        }
        Err(Refusal::Stale) => {
            if held.proposals().stale(id) {
                tell(
                    held,
                    one.session,
                    ProposalWord::Stale {
                        path: one.path.clone(),
                    },
                );
            }
            held.chats().asks_may_have_moved();
            Err(STALE_SAID.to_owned())
        }
        Err(Refusal::Not(why)) => Err(why),
    }
}

/// What Apply says of a proposal whose file changed since (PC-8).
pub const STALE_SAID: &str = "The file changed since this change was proposed, so nothing was \
                              written. Dismiss it; the chat is told to read the file again.";

/// Why Apply did not write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Stale,
    Not(String),
}

/// Applies `one` in the project at `root`: judged again, audited by `audit`, then written.
pub fn apply_in(
    root: &Path,
    one: &Proposal,
    persona: Option<&str>,
    audit: &dyn Fn(&proposed::Applied<'_>) -> Result<(), String>,
) -> Result<String, Refusal> {
    if one.stale {
        return Err(Refusal::Stale);
    }
    if let Some(why) = purlis_core::sandbox::policy::Locks::of(root).proposed_changes_refused() {
        return Err(Refusal::Not(why));
    }
    let rel: PathBuf = proposed::proposable(root, &root.join(&one.path)).map_err(Refusal::Not)?;
    let made = match proposed::prepare(root, &rel, Some(one.base.as_deref()), &one.change) {
        Ok(made) => made,
        Err(NotMade::Stale) => return Err(Refusal::Stale),
        Err(NotMade::Unprepared) => return Err(Refusal::Not(proposed::UNPREPARED.to_owned())),
    };
    let written = proposed::base_of(Some(made.after.as_bytes())).unwrap_or_default();
    audit(&proposed::Applied {
        persona,
        path: &one.path,
        written: &written,
    })
    .map_err(Refusal::Not)?;
    match proposed::write_pinned(root, &rel, one.base.as_deref(), &made.after) {
        Ok(()) => Ok(format!(
            "Applied to {}. It rides your next save.",
            purlis_core::shown::short(&one.path)
        )),
        Err(NotMade::Stale) => Err(Refusal::Stale),
        Err(NotMade::Unprepared) => Err(Refusal::Not(format!(
            "purlis could not write {}, so nothing was written.",
            purlis_core::shown::short(&one.path)
        ))),
    }
}

/// **Dismiss** on a proposed change: nothing is written, and the chat is told on its next turn.
/// Window-only.
#[tauri::command]
#[specta::specta]
pub fn dismiss_proposed_change(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: u32,
) -> Result<(), String> {
    dismissed(&*planes.held(&plane)?, id)
}

/// [`dismiss_proposed_change`]'s work, for project `held`.
pub fn dismissed(held: &Held, id: u32) -> Result<(), String> {
    let one = held.proposals().take(id).ok_or(WAITS_NO_MORE)?;
    // A stale one was told stale already; one put away fresh is the person's no.
    if !one.stale {
        tell(
            held,
            one.session,
            ProposalWord::Dismissed { path: one.path },
        );
    }
    held.chats().asks_may_have_moved();
    Ok(())
}

#[cfg(test)]
#[path = "proposals_tests.rs"]
mod tests;
