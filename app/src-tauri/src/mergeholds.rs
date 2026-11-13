// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The merge hold, in the app** (R-114, R-115, R-117, R-036; `purlis_core::mergehold`).
//!
//! A chat's hook refuses it a merge and hands the request over
//! (`purlis_core::hookwire::Ask::Merge`). Here the request is found in the project: the folder
//! the command ran in must be a clone in the plane, of a repo the project's inventory lists, and
//! a repo the command named must be that one. The app answers the chat at once, then reads the
//! request's head from the forge with its own login and holds one merge ask, keyed on the repo,
//! the number and that head, which Asks lists ([`crate::asking`]) and the event log records as a
//! hold.
//!
//! **Only the person's press merges** ([`merge_held_request`]), by the app's own forge call,
//! only at the head the ask was raised at: a head that moved is the forge's refusal, and the ask
//! waits again. **Don't merge** ([`dont_merge_held_request`]) merges nothing. Both are the
//! window's alone (`purlis_session_protocol::ui::WINDOW_ONLY`), and each presses as the window,
//! which is the only answerer a command here can be: nothing a chat sends answers (V16a).
//!
//! **Debt** (the merge row, R-120): no window draws the ask's row yet. Asks lists it, as
//! `asks-only` (`app/src/askPlaces.guard.test.ts`), and the press waits on that row, which goes
//! through the settle guard as every press does (ADR 0097).
//!
//! In memory only: a relaunch holds none, and the chat's next try asks again.

use std::path::Path;
use std::sync::{Mutex, PoisonError, Weak};

use purlis_core::eventlog::stops::{Hold, Stop, Target};
use purlis_core::forge::backend::Caller;
use purlis_core::forge::pr::{MergeAs, Pr, Repo as OnForge};
use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::hookwire::{Answer, MergeAsk};
use purlis_core::mergehold::{self, HeadSha, Holds, MergeForge, MergeKey, Raised, Repo, Request};

use crate::planes::{Held, PlaneId, Planes};

/// **The merges a project's chats' hooks held**: the asks, and the forge each one's repo is on,
/// for its press.
#[derive(Debug, Default)]
pub struct MergeHolds {
    holds: Holds,
    on: Mutex<Vec<(MergeKey, OnForge)>>,
}

impl MergeHolds {
    /// Every merge ask that waits, oldest first.
    pub fn every(&self) -> Vec<mergehold::Waits> {
        self.holds.every()
    }

    /// Chat `session` has closed: none of its merge asks waits.
    pub fn forget(&self, session: u32) {
        self.holds.forget(session);
        let waiting = self.holds.every();
        self.on
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|(key, _)| waiting.iter().any(|one| one.key == *key));
    }

    fn forge_of(&self, key: &MergeKey) -> Option<OnForge> {
        self.on
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|(of, _)| of == key)
            .map(|(_, on)| on.clone())
    }
}

/// **The app's own forge call for a press**: `merge_at` with the window's login, never a chat's,
/// only while the request's head is the key's.
struct AppsLogin(OnForge);

impl MergeForge for AppsLogin {
    fn merge_at(&self, key: &MergeKey) -> Result<(), String> {
        let pr = Pr {
            number: key.number,
            url: String::new(),
        };
        let how = MergeAs {
            squash: false,
            title: format!("Merge request {} of {}", key.number, key.repo.as_str()),
            message: "Merged in purlis on the person's press.".to_owned(),
        };
        self.0
            .backend()
            .merge_at(
                &Caller::window(),
                &self.0.path,
                &pr,
                key.head.as_str(),
                &how,
            )
            .map(|_| ())
            .map_err(|why| why.to_string())
    }
}

/// The repo `ask` is of, found in the project at `root`, or why nobody is asked.
fn found(root: &Path, ask: &MergeAsk) -> Result<(OnForge, Repo), String> {
    let cwd = Path::new(&ask.cwd);
    if !cwd.is_absolute() {
        return Err("purlis could not tell which clone this ran in, so nobody was asked.".into());
    }
    purlis_core::inventory::in_plane(root, cwd)?;
    let on = OnForge::of_clone(root, cwd)?;
    let repo = Repo::try_from(on.path.as_str()).map_err(|why| why.to_string())?;
    if let Some(named) = &ask.request.repo
        && *named != repo
    {
        return Err(format!(
            "The command names {}, but it ran in a clone of {}, so nobody was asked.",
            named.as_str(),
            repo.as_str()
        ));
    }
    let listed = std::fs::read_to_string(purlis_core::inventory::path(root))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .map(|doc| purlis_core::inventory::listed(&doc))
        .unwrap_or_default();
    if !listed.iter().any(|row| {
        row.get("path_with_namespace")
            .and_then(serde_json::Value::as_str)
            == Some(&on.path)
    }) {
        return Err(format!(
            "{} is not one of this project's repos, so purlis does not merge in it and nobody \
             was asked.",
            repo.as_str()
        ));
    }
    Ok((on, repo))
}

/// **Chat `ask.chat`'s hook held a merge** and hands its request over. Answered at once with the
/// sentence the chat is told; the forge is read beside it, and the ask raised once it has been.
///
/// The chat the line names is the one whose token it carries, which the listener checks before
/// this is asked. Each chat is held to the rate its brokered writes are, so a chat cannot fill
/// the person's Asks by asking.
pub fn asked(held: &Weak<Held>, ask: &MergeAsk) -> Answer {
    let no = |why: String| Answer::No { why };
    let Some(strong) = held.upgrade() else {
        return no("this project has been closed".to_owned());
    };
    if !strong
        .chats()
        .open_now()
        .iter()
        .any(|open| open.session == ask.chat)
    {
        return no(format!("chat {} is not one this app has open", ask.chat));
    }
    if let Err(why) = strong.brokered().allow(ask.chat, std::time::Instant::now()) {
        return no(why);
    }
    let (on, repo) = match found(strong.root(), ask) {
        Ok(found) => found,
        Err(why) => return no(why),
    };
    let request = Request {
        repo: Some(repo),
        number: ask.request.number,
    };
    let said = mergehold::handed_said(&request);
    let (held, chat) = (held.clone(), ask.chat);
    let spawned = std::thread::Builder::new()
        .name("merge-hold".to_owned())
        .spawn(move || raise(&held, chat, &request, on));
    if spawned.is_err() {
        return no(
            "purlis could not read this request from the forge, so nobody was asked.".into(),
        );
    }
    no(said)
}

/// Reads `request`'s head from the forge and holds chat `chat`'s ask for it.
fn raise(held: &Weak<Held>, chat: u32, request: &Request, on: OnForge) {
    let pr = Pr {
        number: request.number,
        url: String::new(),
    };
    let head = on.backend().head(&Caller::window(), &on.path, &pr);
    let Some(held) = held.upgrade() else {
        return;
    };
    let resolve = |request: &Request| -> Result<MergeKey, String> {
        let head = head.as_ref().map_err(ToString::to_string)?;
        Ok(MergeKey {
            repo: request
                .repo
                .clone()
                .ok_or_else(|| "no repo was found".to_owned())?,
            number: request.number,
            head: HeadSha::try_from(head.as_str()).map_err(|why| why.to_string())?,
        })
    };
    let holds = held.merge_holds();
    match mergehold::asked(
        &holds.holds,
        chat,
        request,
        &resolve,
        crate::sandboxing::now_secs(),
    ) {
        Ok((key, Raised::New)) => {
            holds
                .on
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((key, on));
            let hold = Stop::Hold(Hold {
                tool: "Bash".to_owned(),
                target: Target::guard_arm(mergehold::REASON).ok(),
                args: None,
            });
            if let Err(why) = held.hooks().record_stop(held.root(), chat, hold) {
                tracing::warn!("purlis: chat {chat}'s held merge was not recorded: {why}");
            }
            tracing::info!(
                "purlis: chat {chat}'s merge is held; nothing is merged until the person presses \
                 Merge"
            );
            held.chats().asks_may_have_moved();
        }
        Ok((_, Raised::Already | Raised::TooMany)) => {}
        Err(why) => tracing::warn!("purlis: chat {chat}'s held merge raised no ask: {why}"),
    }
}

/// The key a press names, checked as it is made.
fn key_of(repo: &str, number: u32, head: &str) -> Result<MergeKey, String> {
    Ok(MergeKey {
        repo: Repo::try_from(repo).map_err(|why| why.to_string())?,
        number: u64::from(number),
        head: HeadSha::try_from(head).map_err(|why| why.to_string())?,
    })
}

/// The window, as the answerer of every press here: the one client that sends these commands.
fn the_window() -> Result<Answerer, String> {
    Answerer::admitted(Admitted::LocalUi).ok_or_else(|| "only the window answers".to_owned())
}

/// **Merge**, pressed on chat `session`'s merge ask: the app asks the forge, with its own login,
/// to merge request `number` of `repo` at exactly `head`. The ask is answered at the press, so a
/// second press merges nothing; where the forge refuses, it waits again and the refusal is said.
#[tauri::command]
#[specta::specta]
pub async fn merge_held_request(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    repo: String,
    number: u32,
    head: String,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    tauri::async_runtime::spawn_blocking(move || pressed(&held, session, &repo, number, &head))
        .await
        .map_err(|why| why.to_string())?
}

/// The person pressed Merge on chat `session`'s ask for request `number` of `repo` at `head`.
pub fn pressed(
    held: &Held,
    session: u32,
    repo: &str,
    number: u32,
    head: &str,
) -> Result<(), String> {
    let key = key_of(repo, number, head)?;
    let holds = held.merge_holds();
    let on = holds
        .forge_of(&key)
        .ok_or_else(|| mergehold::NotMerged::NotHeld.to_string())?;
    let merged = holds
        .holds
        .press(session, &key, the_window()?, &AppsLogin(on))
        .map(|_| ())
        .map_err(|why| why.to_string());
    held.chats().asks_may_have_moved();
    merged
}

/// **Don't merge**, pressed on chat `session`'s merge ask: it waits no more, and nothing is
/// merged.
#[tauri::command]
#[specta::specta]
pub fn dont_merge_held_request(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    repo: String,
    number: u32,
    head: String,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    let key = key_of(&repo, number, &head)?;
    if !held
        .merge_holds()
        .holds
        .dont_merge(session, &key, the_window()?)
    {
        return Err(mergehold::NotMerged::NotHeld.to_string());
    }
    held.chats().asks_may_have_moved();
    Ok(())
}
