// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The merge hold, in the app** (R-114, R-115, R-117, R-036; `purlis_core::mergehold`).
//!
//! A chat's hook refuses it a merge and hands the request over
//! (`purlis_core::hookwire::Ask::Merge`). Here the request is found in the project: the folder
//! the command ran in must be a clone in the plane, of a repo the project's inventory lists on
//! that forge and host, and a repo the command named must be that one. The app reads the
//! request's head and base from the forge with its own login, holds one merge ask keyed on the
//! repo, the number, that head and that base, which Asks lists ([`crate::asking`]) and the event
//! log records as a hold, and only then answers the chat (Q239). Where it raises no ask, its
//! answer says why, and the hook's refusal is a block.
//!
//! **Only the person's press merges** ([`merge_held_request`]), by the app's own forge call,
//! only at the head the ask was raised at, by the repo's own method, and through its merge queue
//! or train where it lands through one (`purlis_core::mergehold::ByForge`): a head that moved is
//! the forge's refusal, a base that moved is purlis's, and the ask waits again. A forge that sets
//! the request to merge later instead is an error the person sees. **Don't merge**
//! ([`dont_merge_held_request`]) merges nothing. Both are the window's alone (`purlis_session_protocol::ui::WINDOW_ONLY`), and each presses as the window,
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
use purlis_core::forge::backend::{Caller, ForgeBackend};
use purlis_core::forge::pr::{HeadAt, Pr, Repo as OnForge};
use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::hookwire::{Answer, MergeAsk};
use purlis_core::mergehold::{
    self, Base, ByForge, HeadSha, Holds, MergeKey, Raised, Repo, Request,
};

use crate::planes::{Held, PlaneId, Planes};

/// **The merges a project's chats' hooks held**: the asks, and the forge each one was raised
/// on, by its host and path (Q242), for its press. Two forges' `o/r` in one project are two
/// repos, so a press reaches the forge its own ask was raised on, never another's.
#[derive(Debug, Default)]
pub struct MergeHolds {
    holds: Holds,
    on: Mutex<Vec<(u32, MergeKey, OnForge)>>,
}

impl MergeHolds {
    /// Every merge ask that waits, oldest first.
    pub fn every(&self) -> Vec<mergehold::Waits> {
        self.holds.every()
    }

    /// Chat `session` has closed: none of its merge asks waits.
    pub fn forget(&self, session: u32) {
        self.holds.forget(session);
        self.prune();
    }

    /// The forge chat `session`'s ask for `key` was raised on.
    fn forge_of(&self, session: u32, key: &MergeKey) -> Option<OnForge> {
        self.on
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|(of, at, _)| *of == session && at == key)
            .map(|(_, _, on)| on.clone())
    }

    /// Chat `session`'s ask for `key` was raised on `on`.
    fn on_forge(&self, session: u32, key: &MergeKey, on: OnForge) {
        let mut every = self.on.lock().unwrap_or_else(PoisonError::into_inner);
        every.retain(|(of, at, _)| !(*of == session && at == key));
        every.push((session, key.clone(), on));
    }

    /// Only the forges of asks that still wait are kept.
    fn prune(&self) {
        let waiting = self.holds.every();
        self.on
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|(session, key, _)| {
                waiting
                    .iter()
                    .any(|one| one.session == *session && one.key == *key)
            });
    }
}

/// **What a held merge's answer needs of its project**: the seam it is tested at, where a test
/// stands in for the app's project and the forge.
pub(crate) trait Project {
    /// The project's root.
    fn root(&self) -> &Path;
    /// Whether chat `chat` is one this app has open.
    fn is_open(&self, chat: u32) -> bool;
    /// Whether chat `chat` may ask now, at the rate its brokered writes are held to.
    fn allow(&self, chat: u32) -> Result<(), String>;
    /// The merge asks the project holds.
    fn holds(&self) -> &MergeHolds;
    /// Where request `number` of the repo `on` names stands, read with the app's own login
    /// within [`mergehold::THE_FORGE_IS_READ_WITHIN`].
    fn read(&self, on: &OnForge, number: u64) -> Result<HeadAt, String>;
    /// Writes one stop of chat `chat` into the event log.
    fn record(&self, chat: u32, stop: Stop);
    /// The asks may have moved.
    fn moved(&self);
}

impl Project for Held {
    fn root(&self) -> &Path {
        Held::root(self)
    }

    fn is_open(&self, chat: u32) -> bool {
        self.chats()
            .open_now()
            .iter()
            .any(|open| open.session == chat)
    }

    fn allow(&self, chat: u32) -> Result<(), String> {
        self.brokered().allow(chat, std::time::Instant::now())
    }

    fn holds(&self) -> &MergeHolds {
        self.merge_holds()
    }

    fn read(&self, on: &OnForge, number: u64) -> Result<HeadAt, String> {
        let (tell, heard) = std::sync::mpsc::channel();
        let on = on.clone();
        std::thread::Builder::new()
            .name("merge-hold".to_owned())
            .spawn(move || {
                let pr = Pr {
                    number,
                    url: String::new(),
                };
                let read = on.backend().head(&Caller::window(), &on.path, &pr);
                let _ = tell.send(read.map_err(|why| why.to_string()));
            })
            .map_err(|why| why.to_string())?;
        heard
            .recv_timeout(mergehold::THE_FORGE_IS_READ_WITHIN)
            .map_err(|_| "the forge did not answer in time".to_owned())?
    }

    fn record(&self, chat: u32, stop: Stop) {
        if let Err(why) = self.hooks().write_stop(chat, stop.clone()) {
            tracing::warn!("purlis: chat {chat}'s held merge was not recorded: {why}");
        }
        // A hold never counts toward the block loop; a block written for a raise that failed
        // does (R-207, R-215).
        crate::pausing::stopped(self, chat, &stop);
    }

    fn moved(&self) {
        self.chats().asks_may_have_moved();
    }
}

/// The repo `ask` is of, found in the project at `root`, or why nobody is asked: the folder it
/// ran in is a clone in the project, of a repo the project's inventory lists on the same forge,
/// host and path ([`mergehold::listed_in`]), and a repo the command named is that one.
pub(crate) fn found(root: &Path, ask: &MergeAsk) -> Result<(OnForge, Repo), String> {
    let cwd = Path::new(&ask.cwd);
    if !cwd.is_absolute() {
        return Err("purlis could not tell which clone this ran in.".into());
    }
    purlis_core::inventory::in_plane(root, cwd)?;
    let on = OnForge::of_clone(root, cwd)?;
    let repo = Repo::try_from(on.path.as_str()).map_err(|why| why.to_string())?;
    if let Some(named) = &ask.request.repo
        && *named != repo
    {
        return Err(format!(
            "the command names {}, but it ran in a clone of {}.",
            named.as_str(),
            repo.as_str()
        ));
    }
    if !mergehold::listed_in(root, &on.forge, &on.path) {
        return Err(format!(
            "{} on {} is not one of this project's repos, so purlis does not merge in it.",
            repo.as_str(),
            on.forge.host
        ));
    }
    Ok((on, repo))
}

/// **Chat `ask.chat`'s hook held a merge** and hands its request over. The request is found in
/// the project, read from the forge, and its ask raised **before** the hook is answered (Q239),
/// so the chat is told the person is asked ([`Answer::MergeHeld`]) only where an ask waits; any
/// other answer is a block the hook's deny line records.
///
/// The chat the line names is the one whose token it carries, which the listener checks before
/// this is asked. Each chat is held to the rate its brokered writes are, **shared with them**: a
/// merge ask is one more thing a chat puts before the person, and a separate allowance would
/// let a chat fill the person's Asks twice as fast. A refusal by the rate is said and recorded.
pub fn asked(held: &Weak<Held>, ask: &MergeAsk) -> Answer {
    match held.upgrade() {
        Some(held) => answer(held.as_ref(), ask),
        None => Answer::No {
            why: "this project has been closed.".to_owned(),
        },
    }
}

/// [`asked`], of `project`.
pub(crate) fn answer(project: &dyn Project, ask: &MergeAsk) -> Answer {
    match raise(project, ask) {
        Ok(said) => Answer::MergeHeld { said },
        Err(why) => {
            tracing::warn!(
                "purlis: chat {}'s held merge raised no ask: {why}",
                ask.chat
            );
            Answer::No { why }
        }
    }
}

/// Raises chat `ask.chat`'s merge ask, and answers what the chat is told; or why nobody was
/// asked.
fn raise(project: &dyn Project, ask: &MergeAsk) -> Result<String, String> {
    let chat = ask.chat;
    if !project.is_open(chat) {
        return Err(format!("chat {chat} is not one this app has open."));
    }
    project.allow(chat)?;
    let (on, repo) = found(project.root(), ask)?;
    let request = Request {
        repo: Some(repo.clone()),
        number: ask.request.number,
    };
    let resolve = |request: &Request| -> Result<MergeKey, String> {
        let at = project
            .read(&on, request.number)
            .map_err(|why| format!("purlis could not read it from the forge ({why})."))?;
        Ok(MergeKey {
            repo: repo.clone(),
            number: request.number,
            head: HeadSha::try_from(at.sha.as_str()).map_err(|why| why.to_string())?,
            base: Base::try_from(at.base.as_str()).map_err(|why| why.to_string())?,
        })
    };
    let holds = project.holds();
    let (key, raised) = mergehold::asked(
        &holds.holds,
        chat,
        &request,
        &resolve,
        crate::sandboxing::now_secs(),
    )?;
    match raised {
        Raised::New => {
            holds.on_forge(chat, &key, on);
            holds.prune();
            project.record(
                chat,
                Stop::Hold(Hold {
                    tool: "Bash".to_owned(),
                    target: Target::guard_arm(mergehold::REASON).ok(),
                    args: None,
                }),
            );
            tracing::info!(
                "purlis: chat {chat}'s merge is held; nothing is merged until the person presses \
                 Merge"
            );
        }
        // Still the one ask, at the head and base it was raised at. A moved head is `New`: the
        // ask at the old head is gone (Q242).
        Raised::Already => {
            if holds.forge_of(chat, &key).as_ref() != Some(&on) {
                return Err(format!(
                    "chat {chat} has this merge waiting on the person already, on another forge \
                     than {}, so purlis does not ask it again here.",
                    on.forge.host
                ));
            }
        }
        Raised::TooMany => {
            return Err(format!(
                "chat {chat} has {} merges waiting on the person already, which is as many as \
                 one chat may.",
                mergehold::AT_MOST_PER_CHAT
            ));
        }
    }
    project.moved();
    Ok(mergehold::handed_said(&request))
}

/// The repo and head a press names, checked as they are made.
fn key_of(repo: &str, head: &str) -> Result<(Repo, HeadSha), String> {
    Ok((
        Repo::try_from(repo).map_err(|why| why.to_string())?,
        HeadSha::try_from(head).map_err(|why| why.to_string())?,
    ))
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
    let merged = press(
        held.merge_holds(),
        session,
        repo,
        number,
        head,
        &OnForge::backend,
    );
    held.chats().asks_may_have_moved();
    merged
}

/// The press of chat `session`'s ask in `holds`, through the forge its repo is on, which
/// `backend_of` makes the client of: the forge's own, in the app.
fn press(
    holds: &MergeHolds,
    session: u32,
    repo: &str,
    number: u32,
    head: &str,
    backend_of: &dyn Fn(&OnForge) -> Box<dyn ForgeBackend>,
) -> Result<(), String> {
    let (key, on) = waiting(holds, session, repo, number, head)?;
    let backend = backend_of(&on);
    let forge = ByForge {
        backend: backend.as_ref(),
        caller: Caller::window(),
        path: on.path.clone(),
    };
    let pressed = holds
        .holds
        .press(session, &key, the_window()?, &forge)
        .map_err(|why| why.to_string());
    holds.prune();
    let (_, merged) = pressed?;
    tracing::info!("purlis: chat {session}'s held merge was pressed: {merged:?}");
    Ok(())
}

/// The waiting ask a press names, and the forge its repo is on.
fn waiting(
    holds: &MergeHolds,
    session: u32,
    repo: &str,
    number: u32,
    head: &str,
) -> Result<(MergeKey, OnForge), String> {
    let (repo, head) = key_of(repo, head)?;
    let not_held = || mergehold::NotMerged::NotHeld.to_string();
    let key = holds
        .holds
        .key_of(session, &repo, u64::from(number), &head)
        .ok_or_else(not_held)?;
    let on = holds.forge_of(session, &key).ok_or_else(not_held)?;
    Ok((key, on))
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
    let (key, _) = waiting(held.merge_holds(), session, &repo, number, &head)?;
    if !held
        .merge_holds()
        .holds
        .dont_merge(session, &key, the_window()?)
    {
        return Err(mergehold::NotMerged::NotHeld.to_string());
    }
    held.merge_holds().prune();
    held.chats().asks_may_have_moved();
    Ok(())
}

#[cfg(test)]
#[path = "mergeholds/tests.rs"]
mod tests;
