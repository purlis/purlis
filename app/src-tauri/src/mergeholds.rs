// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The merge hold, in the app** (R-114, R-115, R-117, R-036; `purlis_core::mergehold`).
//!
//! A chat's hook refuses it a merge and hands the request over
//! (`purlis_core::hookwire::Ask::Merge`). Here the request is found in the project: the folder
//! the command ran in must be a clone in the plane, of a repo the project's inventory lists on
//! that forge and host (R-118), and a repo the command named must be that one. The app reads the
//! request from the forge with its own login ([`mergehold::read`]): its head and base, which
//! the one merge ask it holds is keyed on with the repo and the number, and the title, author,
//! checks and ruleset the ask shows, none of them the chat's (R-119, R-146). Asks lists the ask
//! ([`crate::asking`]), the event log records it as a hold, and only then is the chat answered
//! (Q239). Where it raises no ask, its answer says why, and the hook's refusal is a block.
//!
//! **Only the person's press merges** ([`merge_held_request`]), by the app's own forge call,
//! only at the head the ask was raised at, by the repo's own method, and through its merge queue
//! or train where it lands through one (`purlis_core::mergehold::ByForge`): a head that moved is
//! the forge's refusal, a base that moved is purlis's, and the ask waits again. A forge that sets
//! the request to merge later instead is an error the person sees. **Don't merge**
//! ([`dont_merge_held_request`]) merges nothing. Both are the window's alone (`purlis_session_protocol::ui::WINDOW_ONLY`), and each presses as the window,
//! which is the only answerer a command here can be: nothing a chat sends answers (V16a).
//!
//! **The person's press is bound to what they were shown** (T1-40; R-150, Q248): the window
//! re-reads the request before Merge asks once more ([`reread_held_merge`]), so the question is
//! asked of what the forge says now, with the time it was read; and a press re-reads it again,
//! then hands back the ruleset state its row showed, merging only while that is the state the
//! read gives. A re-read that finds the head moved retires the ask for one at the new head
//! (Q242), so nothing merges at a head the person was not shown.
//!
//! In memory only: a relaunch holds none, and the chat's next try asks again.

use std::path::Path;
use std::sync::{Mutex, PoisonError, Weak};

use purlis_core::eventlog::stops::{Hold, Stop, Target};
use purlis_core::forge::backend::{Caller, ForgeBackend};
use purlis_core::forge::pr::Repo as OnForge;
use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::hookwire::{Answer, MergeAsk};
use purlis_core::mergehold::{
    self, Base, ByForge, HeadSha, Holds, Invalid, MergeKey, Raised, Refreshed, Repo, Request, Seen,
};

use crate::asking::{MergeShown, RulesetShown};

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
    /// Request `number` of the repo `on` names, as the forge says it stands, read with the
    /// app's own login ([`mergehold::read`]) within [`mergehold::THE_FORGE_IS_READ_WITHIN`].
    fn read(&self, on: &OnForge, number: u64) -> Result<mergehold::Read, String>;
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

    fn read(&self, on: &OnForge, number: u64) -> Result<mergehold::Read, String> {
        mergehold::read(
            on.backend().into(),
            &on.path,
            number,
            mergehold::THE_FORGE_IS_READ_WITHIN,
            crate::sandboxing::now_secs(),
        )
    }

    fn record(&self, chat: u32, stop: Stop) {
        if let Err(why) = self.hooks().write_stop(chat, stop.clone()) {
            tracing::warn!("purlis: chat {chat}'s held merge was not recorded: {why}");
        }
        // Only a hold is recorded here, and a hold never counts toward the block loop (R-215):
        // told it all the same, so a stop the app writes has one road to the count. A raise
        // that failed writes nothing here (T1-11).
        crate::pausing::stopped(self, chat, &stop, None);
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
    let holds = project.holds();
    let resolve = |request: &Request| -> Result<(MergeKey, mergehold::Facts), String> {
        let read = project
            .read(&on, request.number)
            .map_err(|why| format!("purlis could not read it from the forge ({why})."))?;
        let key = MergeKey {
            repo: repo.clone(),
            number: request.number,
            head: HeadSha::try_from(read.at.sha.as_str()).map_err(|why| why.to_string())?,
            base: Base::try_from(read.at.base.as_str()).map_err(|why| why.to_string())?,
        };
        // Refused before it is raised, so a read on another forge never refreshes the ask.
        if holds.forge_of(chat, &key).is_some_and(|was| was != on) {
            return Err(format!(
                "chat {chat} has this merge waiting on the person already, on another forge \
                 than {}, so purlis does not ask it again here.",
                on.forge.host
            ));
        }
        Ok((key, read.facts))
    };
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
        // Still the one ask, at the head and base it was raised at, showing the new read's facts
        // (Q248). A moved head is `New`: the ask at the old head is gone (Q242).
        Raised::Already => {}
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

/// The repo and head a press names, checked as they are made: a refusal is the core's
/// [`Invalid`], which names the part and never the value.
fn key_of(repo: &str, head: &str) -> Result<(Repo, HeadSha), Invalid> {
    Ok((Repo::try_from(repo)?, HeadSha::try_from(head)?))
}

/// The window, as the answerer of every press here: the one client that sends these commands.
fn the_window() -> Result<Answerer, String> {
    Answerer::admitted(Admitted::LocalUi).ok_or_else(|| "only the window answers".to_owned())
}

/// **What a re-read of a held merge found** (Q248): the head the request is at now, and what its
/// row shows of it, with the time the forge was read.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct MergeReread {
    /// The head the request is at now: another than the ask's, and that ask is retired for one
    /// at this head (Q242).
    pub head: String,
    pub facts: MergeShown,
}

/// **Read chat `session`'s held merge again**, of request `number` of `repo` at `head`, before
/// Merge asks once more (Q248): the forge is read with the app's own login, and the ask shows
/// what it says now, with the time of the read. A head that moved retires the ask for one at the
/// new head. An ask that waits no more is not raised again.
#[tauri::command]
#[specta::specta]
pub async fn reread_held_merge(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    repo: String,
    number: u32,
    head: String,
) -> Result<MergeReread, String> {
    let held = planes.held(&plane)?;
    tauri::async_runtime::spawn_blocking(move || reread_shown(&held, session, &repo, number, &head))
        .await
        .map_err(|why| why.to_string())?
}

/// [`reread`] of chat `session`'s ask in `held`, as the window is told it.
fn reread_shown(
    held: &Held,
    session: u32,
    repo: &str,
    number: u32,
    head: &str,
) -> Result<MergeReread, String> {
    let (now, facts, _) = reread(held, session, repo, number, head)?;
    Ok(MergeReread {
        head: now.head.as_str().to_owned(),
        facts: MergeShown::of(&facts, now.base.as_str()),
    })
}

/// The re-read of chat `session`'s ask for request `number` of `repo` at `head`, in `project`:
/// the key the forge gives now, what it says of the request, and whether the ask moved.
pub(crate) fn reread(
    project: &dyn Project,
    session: u32,
    repo: &str,
    number: u32,
    head: &str,
) -> Result<(MergeKey, mergehold::Facts, Refreshed), String> {
    let holds = project.holds();
    let (was, on) = waiting(holds, session, repo, number, head)?;
    let read = project
        .read(&on, u64::from(number))
        .map_err(|why| format!("purlis could not read it from the forge again ({why})."))?;
    let now = key_at(&was, &read.at).map_err(|why| why.to_string())?;
    let refreshed = holds
        .holds
        .refresh(session, &was, now.clone(), read.facts.clone())
        .map_err(|why| why.to_string());
    // The ask at the new head is on the forge the old one was raised on.
    if refreshed == Ok(Refreshed::Moved) {
        holds.on_forge(session, &now, on);
    }
    holds.prune();
    project.moved();
    let refreshed = refreshed?;
    Ok((now, read.facts, refreshed))
}

/// The key of `was`'s request where the forge says it stands (`at`), checked as it is made: a
/// refusal is the core's [`Invalid`], which names the part and never the value.
fn key_at(was: &MergeKey, at: &purlis_core::forge::pr::HeadAt) -> Result<MergeKey, Invalid> {
    let head = HeadSha::try_from(at.sha.as_str())?;
    let base = Base::try_from(at.base.as_str())?;
    Ok(MergeKey {
        repo: was.repo.clone(),
        number: was.number,
        head,
        base,
    })
}

/// **Merge**, pressed on chat `session`'s merge ask: the app reads the request from the forge
/// again, then asks it, with its own login, to merge request `number` of `repo` at exactly
/// `head`, only while the ruleset state the row showed (`ruleset`) is the one it read (R-150).
/// The ask is answered at the press, so a second press merges nothing; where the forge refuses,
/// it waits again and the refusal is said.
#[tauri::command]
#[specta::specta]
pub async fn merge_held_request(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    repo: String,
    number: u32,
    head: String,
    ruleset: RulesetShown,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    tauri::async_runtime::spawn_blocking(move || {
        pressed(&held, session, &repo, number, &head, ruleset.into())
    })
    .await
    .map_err(|why| why.to_string())?
}

/// The person pressed Merge on chat `session`'s ask for request `number` of `repo` at `head`,
/// shown `seen`.
pub fn pressed(
    held: &Held,
    session: u32,
    repo: &str,
    number: u32,
    head: &str,
    seen: Seen,
) -> Result<(), String> {
    // A moved head or base is another ask, which the person has not been shown (Q242).
    let merged = reread(held, session, repo, number, head).and_then(|(_, _, refreshed)| {
        if refreshed == Refreshed::Moved {
            return Err(MOVED.to_owned());
        }
        press(
            held.merge_holds(),
            session,
            repo,
            number,
            head,
            seen,
            &OnForge::backend,
        )
    });
    held.chats().asks_may_have_moved();
    merged
}

/// What a press says where the re-read found the request at another head or base than its row's.
const MOVED: &str = "the request moved to another head or base since you were asked, so nothing \
                     was merged. It is asked again where it is now.";

/// The press of chat `session`'s ask in `holds`, shown `seen`, through the forge its repo is on,
/// which `backend_of` makes the client of: the forge's own, in the app.
fn press(
    holds: &MergeHolds,
    session: u32,
    repo: &str,
    number: u32,
    head: &str,
    seen: Seen,
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
        .press(session, &key, seen, the_window()?, &forge)
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
    let (repo, head) = key_of(repo, head).map_err(|why| why.to_string())?;
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
