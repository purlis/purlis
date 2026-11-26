// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Held forge acts, in the app** (one login F2; ADR 0128 D8; `purlis_core::heldact`).
//!
//! A chat holds no forge credential. A tag, a release, a push that changes CI files or a fork it
//! asks for is refused on its forge route and **asked of the person**: Asks lists each ask with
//! exactly what the core read of the forge and the clone ([`HeldActShown`]), and the person's
//! press makes the act **with their own sign-in**.
//!
//! - **Two presses** ([`press_held_act`]): the first reads the forge again and asks once more,
//!   handing back a nonce; the second names it and performs, only if nothing the row showed has
//!   changed between them. A double click of the first performs nothing.
//! - **Don't** ([`dont_held_act`]) puts the ask away and does nothing.
//! - **Window only** (`purlis_session_protocol::ui::WINDOW_ONLY`): each press is made as the
//!   window, the one client that sends these commands. No chat, `charter inbox`, shell or socket
//!   answers one, and nothing here remembers an answer: **a held act is never a rule**.
//! - **The row is the digest it was drawn with**: a press hands back the digest of what the
//!   window showed ([`HeldActShown::digest`]), and performs only while the ask still shows that.
//!
//! **The forge and the clone** an ask reads come from the project's [`HeldContext`] (seam "J":
//! the slice that turns the proxy on per chat sets it, from the person's sign-in and the chat's
//! clone). Until it is set, no act is asked, and a press says so and does nothing.
//!
//! **The person is asked in Asks** (#7685): each ask, as it is raised, is told to the project's
//! window ([`InTheWindow`]): the asks registry lists it (`crate::asking::held_act`), so the ✋
//! counts it and the notifier tells of it, and the window reads its asks again. An ask of a chat
//! the window does not have open is withdrawn, and the chat is told nobody was asked.
//!
//! **Tier** (ADR 0069): memory only; a relaunch holds none and the chat's next try asks again.

use std::sync::{Arc, Mutex, PoisonError, Weak};

use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::heldact::{
    Act, AsksWindow, Handoff, HeldActs, HeldContext, NotDone, Pressed, Shown, World,
};

use crate::planes::{Held, PlaneId, Planes};

/// The held acts a project's chats asked for, and the context they read the forge through.
#[derive(Default)]
pub struct HeldActState {
    acts: Arc<HeldActs>,
    context: Mutex<Option<Arc<dyn HeldContext>>>,
}

impl std::fmt::Debug for HeldActState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeldActState").finish_non_exhaustive()
    }
}

/// **One line of an ask**, as the window draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct HeldRowShown {
    pub label: String,
    pub value: String,
}

/// **A long text an ask shows whole**: a release's notes, a CI file's diff.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct HeldBodyShown {
    pub label: String,
    pub text: String,
}

/// **A held act the person is asked about**: exactly what the core read, and the digest of it
/// that a press hands back.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct HeldActShown {
    pub ask: u32,
    /// The chat that asked.
    pub session: u32,
    /// `tag`, `release`, `CI-file push` or `fork`.
    pub kind: String,
    pub repo: String,
    pub rows: Vec<HeldRowShown>,
    pub body: Option<HeldBodyShown>,
    /// What a press names, so it performs only what was drawn.
    pub digest: String,
}

/// **What a press did.**
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "step", rename_all = "kebab-case")]
pub enum HeldPress {
    /// The first press: the forge was read again and the ask is as shown. Press again, naming
    /// `nonce`, to perform it.
    AsksOnceMore { shown: HeldActShown, nonce: String },
    /// The second press: the act was performed. `said` is what was made.
    Done { said: String },
}

fn shown_of(ask: u64, session: u32, shown: &Shown, digest: String) -> Option<HeldActShown> {
    Some(HeldActShown {
        ask: u32::try_from(ask).ok()?,
        session,
        kind: shown.kind.to_owned(),
        repo: shown.repo.clone(),
        rows: shown
            .rows
            .iter()
            .map(|row| HeldRowShown {
                label: row.label.clone(),
                value: row.value.clone(),
            })
            .collect(),
        body: shown.body.as_ref().map(|body| HeldBodyShown {
            label: body.label.clone(),
            text: body.text.clone(),
        }),
        digest,
    })
}

/// The window, as the answerer of every press here.
fn the_window() -> Result<Answerer, String> {
    Answerer::admitted(Admitted::LocalUi).ok_or_else(|| "only the window answers".to_owned())
}

/// The sentence for what a performed act made.
fn made(act: &Act, forked: Option<&purlis_core::heldact::Forked>) -> String {
    let at =
        |sha: &purlis_core::mergehold::HeadSha| sha.as_str().chars().take(12).collect::<String>();
    match act {
        Act::Tag { repo, name, target } => format!(
            "Made the tag {} at {} in {}.",
            name.as_str(),
            at(target),
            repo.as_str()
        ),
        Act::Release { repo, tag, .. } => {
            format!("Made the release of {} in {}.", tag.as_str(), repo.as_str())
        }
        Act::CiPush {
            repo, branch, tip, ..
        } => format!(
            "Pushed {} of {} as one commit on its base. The tip it was made from was {}.",
            branch.as_str(),
            repo.as_str(),
            at(tip)
        ),
        Act::Fork { source, .. } => match forked {
            Some(fork) => format!("Forked {} into {}.", source.as_str(), fork.path),
            None => format!("Forked {}.", source.as_str()),
        },
    }
}

impl HeldActState {
    /// The project's context for held acts: set by the slice that turns the proxy on per chat
    /// (seam "J"), so nothing in this build calls it yet.
    #[allow(dead_code)]
    pub fn set_context(&self, context: Arc<dyn HeldContext>) {
        *self.context.lock().unwrap_or_else(PoisonError::into_inner) = Some(context);
    }

    fn context(&self) -> Option<Arc<dyn HeldContext>> {
        self.context
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// **The proxy's sink** for this project's chats, where a context is set (seam "J"): each
    /// ask it raises is told to `window`.
    pub fn handoff(&self, window: Arc<dyn AsksWindow>) -> Option<Handoff> {
        Some(Handoff::new(self.acts.clone(), self.context()?, window))
    }

    /// Chat `session` has closed: none of its held acts waits.
    pub fn forget(&self, session: u32) {
        self.acts.forget(session);
    }

    /// Whether a held act of chat `session` waits: its pane draws none, Asks alone does.
    pub fn waits_for(&self, session: u32) -> bool {
        self.acts.every().iter().any(|one| one.session == session)
    }

    /// Whether a held act of chat `session` in `repo` waits.
    pub fn waits_in(&self, session: u32, repo: &str) -> bool {
        self.waiting()
            .iter()
            .any(|one| one.session == session && one.repo.eq_ignore_ascii_case(repo))
    }

    /// **Every held act that waits**, oldest first, as drawn.
    pub fn waiting(&self) -> Vec<HeldActShown> {
        self.waiting_since()
            .into_iter()
            .map(|(shown, _)| shown)
            .collect()
    }

    /// [`Self::waiting`], each with when it was asked (seconds since 1970): what the asks
    /// registry lists.
    pub fn waiting_since(&self) -> Vec<(HeldActShown, u64)> {
        self.acts
            .every()
            .iter()
            .filter_map(|one| {
                let (shown, digest) = self.acts.shown(one.id)?;
                Some((shown_of(one.id, one.session, &shown, digest)?, one.since))
            })
            .collect()
    }

    /// **The person's press** of ask `ask`, of the row whose digest is `digest`.
    pub fn press(&self, ask: u32, digest: &str, nonce: Option<&str>) -> Result<HeldPress, String> {
        let by = the_window()?;
        let id = u64::from(ask);
        let session = self
            .acts
            .every()
            .iter()
            .find(|one| one.id == id)
            .map(|one| (one.session, one.prepared.act.repo().clone()))
            .ok_or_else(|| NotDone::NotHeld.to_string())?;
        let context = self.context().ok_or_else(|| {
            "held acts are not turned on for this project, so nothing was done".to_owned()
        })?;
        let world = context.world(session.0, &session.1)?;
        let borrowed: World<'_> = world.as_world();
        match self
            .acts
            .press(&borrowed, id, digest, nonce, by)
            .map_err(|why| why.to_string())?
        {
            Pressed::AsksOnceMore { nonce, .. } => {
                let (shown, digest) = self
                    .acts
                    .shown(id)
                    .ok_or_else(|| NotDone::NotHeld.to_string())?;
                let shown = shown_of(id, session.0, &shown, digest)
                    .ok_or_else(|| "the ask's number is out of range".to_owned())?;
                Ok(HeldPress::AsksOnceMore { shown, nonce })
            }
            Pressed::Done(done) => Ok(HeldPress::Done {
                said: made(&done.act, done.forked.as_ref()),
            }),
        }
    }

    /// **Don't**: the ask is put away and nothing is done. Whether it was waiting.
    pub fn dont(&self, ask: u32) -> Result<bool, String> {
        let by = the_window()?;
        self.acts
            .dont(u64::from(ask), by)
            .map_err(|why| why.to_string())
    }
}

/// **The person's window, as the held acts' [`AsksWindow`]** (#7685): the project's Asks, where
/// an ask of a chat the window has open is listed. Weak, as the project holds the registry.
pub struct InTheWindow(pub Weak<Held>);

impl AsksWindow for InTheWindow {
    fn raised(&self, session: u32, _: u64) -> Result<(), String> {
        let held = self
            .0
            .upgrade()
            .ok_or_else(|| "this project is not open in the window".to_owned())?;
        shown_in(held.as_ref(), session)
    }
}

/// **Whether chat `session`'s held act is shown to the person in project `held`**, and the
/// window told to read its asks again where it is. A chat the window has not open, or one it
/// draws nowhere (a grade's), has no Asks to show it in, and nor has a project whose window
/// hears nothing of its asks moving: the ask would be listed and never read.
pub(crate) fn shown_in(held: &Held, session: u32) -> Result<(), String> {
    let chats = held.chats();
    if !chats.open_now().iter().any(|open| open.session == session) {
        return Err(format!("chat {session} is not open in the window"));
    }
    if chats.kept_from_the_window(session) {
        return Err(format!("chat {session} is not drawn in the window"));
    }
    if !chats.told_asks_moved() {
        tracing::warn!(
            session,
            "a held act was raised where no window hears its asks move"
        );
        return Err("the window hears nothing of this project's asks".to_owned());
    }
    Ok(())
}

/// **The held acts a project's chats asked for**, for Asks.
#[tauri::command]
#[specta::specta]
pub async fn held_acts_waiting(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<HeldActShown>, String> {
    let held = planes.held(&plane)?;
    Ok(held.held_acts().waiting())
}

/// **The person's press** of a held act, as the window: the first press asks once more, the second
/// (naming the nonce the first returned) performs, with the person's own sign-in, exactly the act
/// the row showed (`digest`). Where the forge refuses, the ask waits again and the refusal is
/// said.
#[tauri::command]
#[specta::specta]
pub async fn press_held_act(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    ask: u32,
    digest: String,
    nonce: Option<String>,
) -> Result<HeldPress, String> {
    let held = planes.held(&plane)?;
    let moved = held.clone();
    let pressed = tauri::async_runtime::spawn_blocking(move || {
        held.held_acts().press(ask, &digest, nonce.as_deref())
    })
    .await
    .map_err(|why| why.to_string())?;
    // The ask moved either way: done, refused, or read again.
    moved.chats().asks_may_have_moved();
    pressed
}

/// **Don't**, on a held act: the ask is put away and nothing is done.
#[tauri::command]
#[specta::specta]
pub async fn dont_held_act(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    ask: u32,
) -> Result<bool, String> {
    let held = planes.held(&plane)?;
    let was = held.held_acts().dont(ask)?;
    held.chats().asks_may_have_moved();
    Ok(was)
}

#[cfg(test)]
mod tests;
