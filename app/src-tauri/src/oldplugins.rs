// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The old `[harness_plugins]` keys, said once** (#3467, the operator's rulings of
//! 2026-10-26/27 on spec #2762): purlis no longer turns another harness's plugins on or off per
//! project, so a project that still holds the keys starts as any other, with them ignored and
//! never rewritten behind the person's back
//! ([`purlis_core::harness_plugin::old`]).
//!
//! - **One Ask in the Inbox** ([`notice_of`], drawn by `asking::old_harness_plugins`, so it
//!   costs a project switch no read of its own) says which files hold them, until the person
//!   answers it once: the Ask's **Dismiss**, or an **Apply** or a **Dismiss** on a change its
//!   Remove held. The answer is kept in this machine's state for the project, where no chat
//!   writes, so the Ask never comes back for it.
//! - **Remove is an edit the person approves** ([`remove_old_harness_plugins`]): each file that
//!   holds them, the project's two and each workspace's manifest (#3486), gets a proposed change
//!   that takes the table out, held as purlis's own ([`crate::proposals::BY_PURLIS`]) and drawn,
//!   shown whole and applied as a chat's proposal is. Nothing is written until the person
//!   presses Apply on it, and a manifest is then written by its own writer, keeping its stamp.
//!   While one of those changes waits, the Ask stays out of the Inbox: the change is what is
//!   asked.
//! - **A change that went stale asks again** (#3569): one whose file moved before Apply, or that
//!   can no longer be made, is no answer. Once it is dismissed, the Ask comes back with Remove
//!   while its file still holds the keys ([`purlis_change_dismissed`]).

use std::path::{Path, PathBuf};

use purlis_core::harness_plugin::old;
use purlis_core::proposed::{Change, base_of};

use crate::planes::{Held, PlaneId, Planes};
use crate::proposals::{BY_PURLIS, Proposal, Proposals};

/// The Ask about the old keys, as the Inbox draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct OldHarnessPlugins {
    /// The sentence: which files hold the keys, that they are ignored, and what Remove reaches.
    pub said: String,
    /// The files that hold them, relative to the project.
    pub files: Vec<String>,
    /// Whether Remove has a change to propose: a file that holds them can be made without them.
    pub removable: bool,
}

/// Where this machine keeps that the project's Ask was answered: an empty file for all of it
/// (its Dismiss, and every answer kept before #3569), or one line per file whose change the
/// person applied or turned down.
fn answered_at(root: &Path) -> PathBuf {
    purlis_core::names::state(root)
        .join("app")
        .join("old-harness-plugins-answered")
}

/// What the person answered for the project's old keys.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Answered {
    /// The Ask itself: none of its files is asked about again.
    All,
    /// These files, each by what was done with its change.
    Files(std::collections::BTreeSet<String>),
}

impl Answered {
    fn covers(&self, file: &str) -> bool {
        match self {
            Self::All => true,
            Self::Files(files) => files.contains(file),
        }
    }
}

/// What [`answered_at`] keeps; nothing answered where it is not there.
fn answered(root: &Path) -> Answered {
    match std::fs::read_to_string(answered_at(root)) {
        Ok(text) if text.trim().is_empty() => Answered::All,
        Ok(text) => Answered::Files(
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect(),
        ),
        // A record that cannot be read keeps the Ask away, as an answer would.
        Err(why) if why.kind() != std::io::ErrorKind::NotFound => Answered::All,
        Err(_) => Answered::Files(std::collections::BTreeSet::new()),
    }
}

/// Keeps `answered`: none answered takes the record away.
fn keep(root: &Path, answered: &Answered) -> Result<(), String> {
    let at = answered_at(root);
    let text = match answered {
        Answered::All => String::new(),
        Answered::Files(files) if files.is_empty() => {
            return match std::fs::remove_file(&at) {
                Ok(()) => Ok(()),
                Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(why) => Err(format!("purlis could not keep your answer ({why})")),
            };
        }
        Answered::Files(files) => files.iter().map(|file| format!("{file}\n")).collect(),
    };
    let write = || -> std::io::Result<()> {
        if let Some(folder) = at.parent() {
            std::fs::create_dir_all(folder)?;
        }
        std::fs::write(&at, text)
    };
    write().map_err(|e| format!("purlis could not keep your answer ({e})"))
}

/// The files of the project at `root` that hold the keys and are still to be asked about: not
/// answered, and with no change of purlis's among `waiting`, the files of the project's
/// proposals purlis holds as its own ([`Proposals::paths_of`]).
fn asking_about(root: &Path, waiting: &[String]) -> Vec<old::Found> {
    let answered = answered(root);
    old::found(root)
        .into_iter()
        .filter(|one| !answered.covers(&one.file) && !waiting.contains(&one.file))
        .collect()
}

/// The Ask for the project at `root`, or none: no file holds the keys that was not answered and
/// has no change of purlis's among `waiting` ([`asking_about`]).
pub fn notice_of(root: &Path, waiting: &[String]) -> Option<OldHarnessPlugins> {
    let found = asking_about(root, waiting);
    if found.is_empty() {
        return None;
    }
    Some(OldHarnessPlugins {
        said: old::said(&found),
        files: found.iter().map(|one| one.file.clone()).collect(),
        removable: found.iter().any(|one| one.removed(root).is_ok()),
    })
}

/// Keeps that the whole Ask was answered, so it is said once: its Dismiss.
fn answer(root: &Path) -> Result<(), String> {
    keep(root, &Answered::All)
}

/// Whether `path` is a file the old keys can be in: one of the project's own files at its root,
/// or a workspace's manifest. A change of purlis's to anything else is none of the Ask's.
fn could_hold(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        [file] => file.ends_with(".toml"),
        ["workspaces", _, "workspace.json"] => true,
        _ => false,
    }
}

/// The Ask answered for `file`, or asked again for it: kept, said in the log where it cannot be.
fn answered_for(root: &Path, file: &str, yes: bool) {
    let Answered::Files(mut files) = answered(root) else {
        return;
    };
    let moved = if yes {
        files.insert(file.to_owned())
    } else {
        files.remove(file)
    };
    if moved && let Err(why) = keep(root, &Answered::Files(files)) {
        tracing::warn!("purlis: {why}");
    }
}

/// **purlis's own change `one` was applied** in the project at `root`: the person's yes, which
/// answers the Ask for its file.
pub fn purlis_change_applied(root: &Path, one: &Proposal) {
    if one.session == BY_PURLIS && could_hold(&one.path) {
        answered_for(root, &one.path, true);
    }
}

/// **purlis's own change `one` was dismissed** in the project at `root` (#3569). One that
/// still applied is the person's no, which answers the Ask for its file. One that went stale or
/// can no longer be made is no answer: while its file still holds the keys, the Ask is offered
/// again for it, with Remove, whatever its other files' changes do.
pub fn purlis_change_dismissed(root: &Path, one: &Proposal) {
    if one.session != BY_PURLIS || !could_hold(&one.path) {
        return;
    }
    let applies_still = matches!(
        crate::proposals::now_of(root, one),
        crate::proposals::Now::Made { .. }
    );
    answered_for(root, &one.path, applies_still);
}

/// One change Remove held: its proposal's id, and the file it changes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct HeldRemoval {
    pub id: u32,
    pub file: String,
}

/// **Remove**'s work in the project at `root`: a proposed change per file that holds the keys
/// (its own two, then each workspace's manifest), held in `proposals` at `now` as purlis's, in
/// file order. A file that cannot be made without them is left out. The Ask is not answered
/// here: it stays out of the Inbox while a change waits ([`notice_of`]), and is answered by what
/// the person does with the change.
pub fn removal_held(
    root: &Path,
    proposals: &Proposals,
    now: u64,
) -> Result<Vec<HeldRemoval>, String> {
    let mut held = Vec::new();
    for one in asking_about(root, &[]) {
        let Ok(content) = one.removed(root) else {
            continue;
        };
        let base = base_of(Some(one.text.as_bytes()));
        if let Some(id) = proposals.note(BY_PURLIS, &one.file, base, Change::Whole { content }, now)
        {
            held.push(HeldRemoval { id, file: one.file });
        }
    }
    if held.is_empty() {
        return Err("purlis found no change to propose, so nothing waits for you".to_owned());
    }
    Ok(held)
}

/// **Remove** on the Ask: each file's change held for the person to apply, each with its
/// file. Window-only: only the person proposes purlis's own change.
#[tauri::command]
#[specta::specta]
pub async fn remove_old_harness_plugins(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<HeldRemoval>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("proposing the old keys' removal", move || removed(&held)).await
}

/// [`remove_old_harness_plugins`]'s work, for project `held`.
fn removed(held: &Held) -> Result<Vec<HeldRemoval>, String> {
    let ids = removal_held(held.root(), held.proposals(), crate::sandboxing::now_secs())?;
    held.chats().asks_may_have_moved();
    Ok(ids)
}

/// **Dismiss** on the Ask: kept, so it is not said again. Window-only.
#[tauri::command]
#[specta::specta]
pub async fn dismiss_old_harness_plugins(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("dismissing the old harness plugin keys", move || {
        answer(held.root())
    })
    .await
}

#[cfg(test)]
mod tests;
