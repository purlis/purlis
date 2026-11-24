// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The timed grant and the Allowed list, for the window and the host** (M75 slice 21, #7030;
//! spec §8.8; Q312).
//!
//! - [`ask_offered`] says what the person is offered at a chat's ask: the scopes, the timed
//!   grant's lengths, and, at a deny, the one way out ("Propose a project change…").
//! - [`give_timed_grant`] is **the person's press**: "Allow for this chat, for…". The host decides
//!   the ask itself and grants only what it decides ([`crate::decisions::timed_grants`]), so a
//!   window cannot name a rule or a length the policy does not offer. It is the window's alone
//!   (`session_protocol::ui::WINDOW_ONLY`), never a link's: a chat's request is not the person's.
//! - [`allowed_answers`] and [`remove_allowed_answer`] are the Allowed list's data: the sealed
//!   grants and timed grants, with reason, countdown and Remove. Removing only narrows, so it
//!   takes no press, but it is still the window's.
//! - [`sweep`] and [`chat_closed`] **end a timed grant** at its time or its chat's close: the
//!   entry leaves the sealed store and one quiet line goes to the Journal (the host's event log),
//!   once, by whichever process took the entry out.

use std::path::Path;

use purlis_core::policy::grants::{self, Span};
use purlis_core::secrets::answers::{self, Standing};

use crate::hooks::Hooks;
use crate::planes::PlaneId;

/// **Takes the ended timed grants out of the store and writes their Journal lines.** The line is
/// written for an entry only where this commit is what took it out, so two sweeps that read the
/// same store write one line between them. A store that cannot be changed (the deny state, no
/// keyring) keeps the entry, which serves nothing past its end all the same, and is tried again.
pub fn end(root: &Path, hooks: &Hooks, number: Option<u32>, ended: &[grants::Ended]) {
    if ended.is_empty() {
        return;
    }
    let mut taken: Vec<&grants::Ended> = Vec::new();
    let committed = answers::commit(root, |draft| {
        for one in ended {
            if draft.entries().iter().any(|entry| *entry == one.entry) {
                draft.remove_narrowing(answers::GRANT, &one.entry.text);
                taken.push(one);
            }
        }
    });
    if let Err(why) = committed {
        tracing::warn!("purlis: ended timed grants were not taken out of the store: {why}");
        return;
    }
    for one in taken {
        if let Err(why) = hooks.record_timed_grant_ended(root, number, one) {
            tracing::warn!("purlis: a timed grant's end was not written to the Journal: {why}");
        }
    }
}

/// **Ends the timed grants of `standing` that have run out** by `now`, as a decision found them.
pub fn sweep(root: &Path, hooks: &Hooks, standing: &Standing, now: u64) {
    let ended = grants::ended(&standing.entries, None, now);
    end(root, hooks, None, &ended);
}

/// **Ends chat `id`'s timed grants, and any that have run out, with the chat's close**: called
/// where no session of the chat is left open (a restart keeps the chat, so its grants).
pub fn chat_closed(root: &Path, hooks: &Hooks, number: u32, id: Option<&str>) {
    let standing = answers::standing(root);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let ended = grants::ended(&standing.entries, id, now);
    end(root, hooks, Some(number), &ended);
}

/// What the person is offered at an ask, as the window draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct OfferShown {
    /// The grant scopes, narrowest first, in the words `@keep` uses.
    pub scopes: Vec<String>,
    /// The timed grant's lengths (`15m`, `1h`), shortest first; none where it is not offered.
    pub timed: Vec<String>,
    /// Whether Don't allow is offered.
    pub dont_allow: bool,
    /// The scopes a Never can be given at.
    pub never: Vec<String>,
    /// What a deny offers instead of the rest: the button's words.
    pub way_out: Option<String>,
}

fn offer_shown(offer: &grants::Offer) -> OfferShown {
    let words = |scopes: &[grants::Scope]| scopes.iter().map(|s| s.word().to_owned()).collect();
    OfferShown {
        scopes: words(&offer.scopes),
        timed: offer.timed.iter().map(|s| s.word().to_owned()).collect(),
        dont_allow: offer.dont_allow,
        never: words(&offer.never),
        way_out: offer.way_out.map(|way| way.label().to_owned()),
    }
}

/// **What chat `chat`'s ask offers the person**, decided by the host now. The window's alone.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ask_offered(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    chat: u32,
    action: String,
    kind: String,
    resource: String,
) -> Result<OfferShown, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading what an ask offers", move || {
        crate::decisions::offered(&held, chat, &action, (&kind, &resource))
            .map(|offer| offer_shown(&offer))
            .ok_or_else(|| format!("chat {chat} is not one this app has open."))
    })
    .await
}

/// What a timed grant given says.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct TimedGiven {
    /// The rules it answers (one grant each).
    pub rules: Vec<String>,
    /// When it ends, in seconds since 1970.
    pub until: u32,
}

/// **The person allows chat `chat`'s ask for a while**, with a reason: one timed grant for each
/// rule that decides it, sealed on this machine. Made here and nowhere else: the press is the
/// person's in their window. Refused, with what to do, at a deny, at what is not an ask, with no
/// reason and for a length the policy does not offer.
fn give(
    held: &crate::planes::Held,
    press: &answers::Press,
    (chat, action, kind, resource): (u32, &str, &str, &str),
    (length, reason): (&str, &str),
) -> Result<TimedGiven, String> {
    let span = Span::from_word(length).ok_or_else(|| {
        format!("{length:?} is not a length of timed grant: choose 15 minutes or 1 hour.")
    })?;
    let made = crate::decisions::timed_grants(held, chat, action, (kind, resource), (span, reason))
        .ok_or_else(|| format!("chat {chat} is not one this app has open."))?
        .map_err(|refused| refused.said())?;
    answers::commit(held.root(), |draft| {
        for grant in &made {
            draft.add(press, answers::GRANT, &grant.text());
        }
    })
    .map_err(|why| format!("The grant was not kept, so nothing is allowed: {why}"))?;
    Ok(TimedGiven {
        rules: made.iter().map(|grant| grant.rule.clone()).collect(),
        until: made
            .first()
            .and_then(|grant| grant.timed.as_ref())
            .map_or(0, |timed| u32::try_from(timed.until()).unwrap_or(u32::MAX)),
    })
}

/// **"Allow for this chat, for…"**: [`give`], on the person's press. The window's alone.
#[tauri::command]
#[specta::specta]
#[allow(
    clippy::disallowed_methods,
    reason = "the window's command layer is the one place that makes the person's press"
)]
pub(crate) async fn give_timed_grant(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    chat: u32,
    action: String,
    kind: String,
    resource: String,
    length: String,
    reason: String,
) -> Result<TimedGiven, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("giving a timed grant", move || {
        give(
            &held,
            &answers::Press::of_the_persons_press(),
            (chat, &action, &kind, &resource),
            (&length, &reason),
        )
    })
    .await
}

/// One row of the Allowed list: a grant or a timed grant.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct AllowedAnswer {
    /// What Remove names it by.
    pub id: String,
    /// `once`, `timed`, `this chat`, `this workspace` or `everywhere`.
    pub scope: String,
    /// The rule it covers.
    pub rule: String,
    /// The action.
    pub action: String,
    /// What it covers.
    pub resource: String,
    /// The chat's ULID, for Once, timed and This chat.
    pub chat: Option<String>,
    /// The workspace, for This workspace.
    pub workspace: Option<String>,
    /// The reason, for a timed grant.
    pub reason: Option<String>,
    /// When a timed grant ends, in seconds since 1970.
    pub until: Option<u32>,
    /// How many seconds it has left as this list was read.
    pub left: Option<u32>,
}

fn shown(row: grants::Row) -> AllowedAnswer {
    let secs = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    AllowedAnswer {
        id: row.id,
        scope: row.scope.word().to_owned(),
        rule: row.rule,
        action: row.action,
        resource: row.resource,
        chat: row.chat,
        workspace: row.workspace,
        reason: row.reason,
        until: row.until.map(secs),
        left: row.left.map(secs),
    }
}

/// The Allowed list's rows for the project at `root`, ending what has run out first.
fn list(root: &Path, hooks: &Hooks) -> Vec<AllowedAnswer> {
    let standing = answers::standing(root);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    sweep(root, hooks, &standing, now);
    grants::rows(&standing.entries, now)
        .into_iter()
        .map(shown)
        .collect()
}

/// **The grants and timed grants the project's chats hold**, for the Allowed list. The window's
/// alone.
#[tauri::command]
#[specta::specta]
pub(crate) async fn allowed_answers(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
) -> Result<Vec<AllowedAnswer>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the Allowed list", move || {
        Ok(list(held.root(), held.hooks()))
    })
    .await
}

/// Takes the grant `id` out of the store of the project at `root`. A grant's removal only narrows
/// what a chat may do, so it needs no press.
fn remove(root: &Path, id: &str) -> Result<(), String> {
    let mut found = false;
    answers::commit(root, |draft| {
        let hit: Vec<_> = draft
            .entries()
            .iter()
            .filter(|entry| entry.kind == answers::GRANT && grants::row_id(entry) == id)
            .cloned()
            .collect();
        found = !hit.is_empty();
        for entry in hit {
            draft.remove_narrowing(answers::GRANT, &entry.text);
        }
    })
    .map_err(|why| format!("It was not removed: {why}"))?;
    if found {
        Ok(())
    } else {
        Err("That is not on the list any more.".to_owned())
    }
}

/// **Remove**, on the Allowed list. The window's alone.
#[tauri::command]
#[specta::specta]
pub(crate) async fn remove_allowed_answer(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    id: String,
) -> Result<(), String> {
    planes.held(&plane)?;
    let root = plane.root().to_path_buf();
    crate::off_the_window("removing an allowed answer", move || remove(&root, &id)).await
}
