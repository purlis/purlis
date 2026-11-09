// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat proposes a pin; the person pins it** (#5963, ADR 0120 §1, spec #5948 story 13): the
//! window's half of [`purlis_core::artifact::proposal`].
//!
//! A chat's `artifact_propose_pin` tool, or `purlis artifact pin` run in it, reaches the app as
//! [`purlis_core::hookwire::Ask::ProposePin`]. It is held with the chat's other proposed changes
//! ([`crate::proposals`]), so it waits on the chat's tab and in the Inbox, with two answers:
//!
//! - **Review…** opens Pin to workspace (#5953) on it, which says who will see it, LIVE or
//!   LOCAL, and lets the person change its name and icon. Only that dialog's Pin pins it
//!   ([`pin_proposed`]); Cancel leaves it waiting.
//! - **Dismiss** pins nothing.
//!
//! **Apply never pins one** ([`ONLY_IN_ITS_DIALOG`]), whatever asks: the row offers no Apply,
//! and one asked for anyway is refused. The workspace is the app's record of the chat that
//! asked, never the chat's to name. What is pinned is judged again when the person pins it
//! ([`purlis_core::artifact::proposal::pin_as_answered`]): a folder whose pin changed since it
//! was shown is stale, and nothing is written. The chat hears pinned, dismissed or stale on its
//! next turn. In memory only, as every proposal is.

use std::path::Path;

use purlis_core::artifact::proposal::{self, Answered, Proposed};
use purlis_core::artifact::{self, Pin, Pinned};
use purlis_core::dispatchtalk::ProposalWord;
use purlis_core::hookwire::{Answer, ProposePinAsk};
use serde::Serialize;
use tauri::Emitter;

use crate::artifacts::{ArtifactPinAsk, ArtifactPinShown, ArtifactPinsMoved};
use crate::planes::{Held, PlaneId, Planes};
use crate::proposals::{AT_MOST_PER_CHAT, BY_PURLIS, Now, Proposal, Proposals};

/// **A pin a chat proposed, held** (#5963): its workspace, by the app's record of the chat; what
/// it proposed; and the folder's pin as it was when it was shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinChange {
    pub workspace: String,
    pub proposed: Proposed,
    /// [`proposal::Shown::state`] when it was held: another at the answer makes it stale.
    pub state: String,
}

/// What Apply says of a proposed pin: it pins nothing.
pub const ONLY_IN_ITS_DIALOG: &str = "A proposed pin is pinned only from its Pin to workspace \
                                      dialog, so nothing was pinned.";

/// What the chat is told once its proposal is held.
pub const HELD_SAID: &str = "shown to the person in purlis's window as a proposed pin: only they \
                             pin it. Wait for their answer.";

/// What Pin says of a proposal whose folder's pin changed since it was shown.
pub const STALE_SAID: &str = "This folder's pin changed since the pin was proposed, so nothing \
                              was pinned. Dismiss it; the chat is told it changed.";

/// The id of Review… on a proposed pin's row: it opens Pin to workspace, and answers nothing.
pub const REVIEW: &str = "review-pin";

/// **What a proposed pin's row says** (#5963), for [`crate::asking::proposed_change`].
pub fn says(one: &Proposal, now: &Now) -> String {
    match now {
        Now::Made { .. } => format!("Proposes to {}, which only you pin", one.path),
        Now::Stale => format!(
            "Proposed to {}, whose pin changed since: it can no longer be pinned as shown",
            one.path
        ),
        Now::Unprepared => format!("Proposed to {}, which can no longer be pinned", one.path),
    }
}

/// [`crate::proposals::now_of`] for a proposed pin: the folder's pin now and as it would be, or
/// why it cannot be.
pub fn now(root: &Path, pin: &PinChange) -> Now {
    let Some(at) = purlis_core::wscmd::workspace_dir(root, &pin.workspace) else {
        return Now::Unprepared;
    };
    match proposal::shown(&at, &pin.proposed) {
        Ok(shown) if shown.state != pin.state => Now::Stale,
        Ok(shown) => Now::Made {
            before: shown.pinned.as_ref().map(pin_line).unwrap_or_default(),
            after: checked_line(&pin.proposed.folder, &shown.checked),
        },
        Err(_) => Now::Unprepared,
    }
}

fn pin_line(pin: &Pin) -> String {
    checked_line(
        &pin.folder,
        &artifact::Checked {
            name: pin.name.clone(),
            icon: pin.icon.clone(),
            command: pin.command.clone(),
            cwd: pin.cwd.clone(),
        },
    )
}

/// One pin in words, as the row and the audit read it.
fn checked_line(folder: &str, checked: &artifact::Checked) -> String {
    let mut line = format!("{} ({}, icon {})", checked.name, folder, checked.icon);
    if let Some(command) = &checked.command {
        line.push_str(&format!("; Regenerate runs `{command}`"));
        if let Some(cwd) = &checked.cwd {
            line.push_str(&format!(" in {cwd}"));
        }
    }
    line.push('\n');
    line
}

/// **Chat `ask.chat` proposed a pin**: held for the person, or why not. **Nothing is pinned
/// here, whatever is asked.** Held to the rate a chat's brokered writes are.
pub fn asked(held: &Held, ask: &ProposePinAsk) -> Answer {
    let no = |why: String| Answer::No { why };
    if !held
        .chats()
        .open_now()
        .iter()
        .any(|open| open.session == ask.chat)
    {
        return no(format!("chat {} is not one this app has open", ask.chat));
    }
    // Where it works is the app's record of the chat, never the line's.
    let workspace = held
        .chats()
        .drafting()
        .into_iter()
        .find(|one| one.session == ask.chat)
        .and_then(|one| one.workspace);
    if let Err(why) = held.brokered().allow(ask.chat, std::time::Instant::now()) {
        return no(why);
    }
    match asked_in(
        held.root(),
        held.proposals(),
        ask,
        workspace.as_deref(),
        crate::sandboxing::now_secs(),
    ) {
        Ok(()) => {
            held.chats().asks_may_have_moved();
            Answer::Said {
                lines: vec![purlis_core::repocmd::Say::Done(HELD_SAID.into())],
                code: 0,
            }
        }
        Err(why) => no(why),
    }
}

/// [`asked`]'s judgement of `ask` in the project at `root`, for a chat working in `workspace`,
/// held in `proposals` at `now`.
pub fn asked_in(
    root: &Path,
    proposals: &Proposals,
    ask: &ProposePinAsk,
    workspace: Option<&str>,
    now: u64,
) -> Result<(), String> {
    // PC-14: a project where proposals are off holds none of these either.
    if let Some(why) = purlis_core::sandbox::policy::Locks::of(root).proposed_changes_refused() {
        return Err(why);
    }
    if ask.chat == BY_PURLIS {
        return Err(format!("chat {BY_PURLIS} is no chat that proposes"));
    }
    let workspace = workspace.ok_or(
        "this chat works at the project's root, which pins nothing: an artifact is pinned to a \
         workspace, so propose it from a chat in that workspace",
    )?;
    let at = purlis_core::wscmd::workspace_dir(root, workspace)
        .ok_or_else(|| format!("'{workspace}' is not a workspace purlis knows"))?;
    let shown = proposal::shown(&at, &ask.pin)?;
    proposals
        .note_pin(
            ask.chat,
            &ask.pin.title(workspace),
            PinChange {
                workspace: workspace.to_owned(),
                proposed: ask.pin.clone(),
                state: shown.state,
            },
            now,
        )
        .map(|_| ())
        .ok_or_else(|| {
            format!(
                "this chat has {AT_MOST_PER_CHAT} proposed changes waiting already: wait for the \
                 person's answers before proposing another"
            )
        })
}

/// **A proposed pin, as Pin to workspace offers it** (#5963): the dialog's ask, and the chat and
/// command it came with, which the dialog names.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ProposedPin {
    pub workspace: String,
    pub ask: ArtifactPinAsk,
    /// The chat that proposed it, by its name where it has one.
    pub by: String,
    /// The command Regenerate would run, and its working folder.
    pub command: Option<String>,
    pub cwd: Option<String>,
}

/// [`proposed_pin`]'s work in the project at `root`; `by` names the chat that proposed it.
pub fn offered_in(
    root: &Path,
    proposals: &Proposals,
    id: u32,
    by: &dyn Fn(u32) -> String,
) -> Result<ProposedPin, String> {
    let one = proposals.get(id).ok_or(crate::proposals::WAITS_NO_MORE)?;
    let pin = one.pin.as_ref().ok_or(NOT_A_PIN)?;
    let at = purlis_core::wscmd::workspace_dir(root, &pin.workspace)
        .ok_or_else(|| format!("'{}' is not a workspace purlis knows", pin.workspace))?;
    let shown = proposal::shown(&at, &pin.proposed)?;
    if one.stale || shown.state != pin.state {
        return Err(STALE_SAID.to_owned());
    }
    Ok(ProposedPin {
        workspace: pin.workspace.clone(),
        ask: ArtifactPinAsk {
            live: purlis_core::wscmd::live_workspaces(root).contains(&pin.workspace),
            folder: pin.proposed.folder.clone(),
            copied: false,
            name: shown.checked.name,
            icon: shown.checked.icon,
            pinned: shown.pinned.is_some(),
        },
        by: by(one.session),
        command: shown.checked.command,
        cwd: shown.checked.cwd,
    })
}

/// What a command about a proposed pin says of a proposal that is not one.
const NOT_A_PIN: &str = "That proposed change is not a pin.";

/// **A proposed pin, for its Pin to workspace dialog** (#5963). Window-only.
#[tauri::command]
#[specta::specta]
pub async fn proposed_pin(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: u32,
) -> Result<ProposedPin, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading a proposed pin", move || {
        offered_in(held.root(), held.proposals(), id, &|chat| {
            name_of(&held, chat)
        })
    })
    .await
}

/// The chat `chat`, by its name where it has one.
fn name_of(held: &Held, chat: u32) -> String {
    held.chats()
        .drafting()
        .into_iter()
        .find(|one| one.session == chat)
        .map(|one| one.name)
        .filter(|name| !name.trim().is_empty())
        .map_or_else(
            || format!("chat {chat}"),
            |name| purlis_core::dispatchgrant::inert(&name),
        )
}

/// What became of the person's Pin on a proposed pin.
#[derive(Debug)]
pub enum Made {
    /// Pinned: the proposal, the pin, and whether its workspace is LIVE.
    Pinned(Box<Proposal>, Box<Pin>, bool),
    /// The folder's pin changed since: nothing written, the proposal marked stale.
    Stale(Box<Proposal>),
}

/// [`pin_proposed`]'s work in the project at `root`: proposal `id` pinned as the person answered
/// it, with their `name` and `icon`, and taken out; or stale, and kept for Dismiss.
pub fn pinned_in(
    root: &Path,
    proposals: &Proposals,
    id: u32,
    name: &str,
    icon: &str,
) -> Result<Made, String> {
    let one = proposals.get(id).ok_or(crate::proposals::WAITS_NO_MORE)?;
    let pin = one.pin.as_ref().ok_or(NOT_A_PIN)?;
    if one.stale {
        return Ok(Made::Stale(Box::new(one)));
    }
    let at = purlis_core::wscmd::workspace_dir(root, &pin.workspace)
        .ok_or_else(|| format!("'{}' is not a workspace purlis knows", pin.workspace))?;
    let _held = proposals
        .applying()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match proposal::pin_as_answered(root, &at, &pin.proposed, &pin.state, name, icon)? {
        Answered::Stale => {
            proposals.stale(id);
            Ok(Made::Stale(Box::new(one)))
        }
        Answered::Pinned(pinned) => {
            proposals.take(id);
            let pin_now = match *pinned {
                Pinned::New(pin) | Pinned::Updated(pin) => pin,
                Pinned::Replaced { new, .. } => new,
            };
            let live = purlis_core::wscmd::live_workspaces(root).contains(&pin.workspace);
            Ok(Made::Pinned(Box::new(one), Box::new(pin_now), live))
        }
    }
}

/// **Pin on a proposed pin's Pin to workspace dialog** (#5963): pins it with the person's `name`
/// and `icon`, and the folder and command the proposal named, which the dialog showed. Recorded
/// as an applied proposal is (PC-4). Window-only.
#[tauri::command]
#[specta::specta]
pub async fn pin_proposed(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: u32,
    name: String,
    icon: String,
) -> Result<ArtifactPinShown, String> {
    let held = planes.held(&plane)?;
    let (shown, workspace) = crate::off_the_window("pinning a proposed pin", move || {
        let root = held.root();
        let session = held.proposals().get(id).map(|one| one.session);
        let persona = session
            .and_then(|chat| held.chats().recorded_chat(chat))
            .and_then(|chat| chat.persona);
        let made = pinned_in(root, held.proposals(), id, &name, &icon);
        let result = match made {
            Ok(Made::Pinned(one, pin, live)) => {
                let written = purlis_core::proposed::base_of(Some(pin_line(&pin).as_bytes()))
                    .unwrap_or_default();
                let recorded = held.hooks().record_proposed(
                    root,
                    one.session,
                    &purlis_core::proposed::Applied {
                        persona: persona.as_deref(),
                        path: &one.path,
                        written: &written,
                        outcome: purlis_core::proposed::Outcome::Applied,
                        why: None,
                    },
                );
                if let Err(why) = recorded {
                    tracing::warn!("purlis: a proposed pin was not recorded ({why})");
                }
                crate::proposals::tell(
                    &held,
                    one.session,
                    ProposalWord::PinApplied {
                        path: one.path.clone(),
                    },
                );
                let workspace = one.pin.map(|pin| pin.workspace).unwrap_or_default();
                Ok((
                    ArtifactPinShown {
                        id: pin.id,
                        folder: pin.folder,
                        name: pin.name,
                        icon: pin.icon,
                        missing: None,
                        live,
                        too_large: None,
                        fresh: None,
                    },
                    workspace,
                ))
            }
            Ok(Made::Stale(one)) => {
                if !one.stale {
                    crate::proposals::tell(
                        &held,
                        one.session,
                        ProposalWord::PinStale { path: one.path },
                    );
                }
                Err(STALE_SAID.to_owned())
            }
            Err(why) => Err(why),
        };
        held.chats().asks_may_have_moved();
        result
    })
    .await?;
    let _ = app.emit(
        crate::artifacts::ARTIFACT_PINS_MOVED,
        ArtifactPinsMoved { plane, workspace },
    );
    Ok(shown)
}

#[cfg(test)]
mod tests;
