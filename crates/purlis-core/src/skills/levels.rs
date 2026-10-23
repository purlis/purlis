// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which workspace and persona levels a running sandboxed chat could have made** (SK-9,
//! #2772). A sandboxed chat is denied writing every level's skills folder, including those of
//! levels made later (`sandbox::skills_held`). A backend may only be able to name the folders
//! that were there when the chat started, though. So purlis also hands no chat a skill from a
//! workspace or persona folder made while a sandboxed chat was running, since that chat could
//! have made it. That skill is left out with a reason that says so.
//!
//! - **The host notes each sandboxed chat's start** ([`note`]): `skill-levels/<chat id>.json`
//!   in this machine's state, holding the workspace and persona folders there at that moment.
//!   A chat cannot write the state folder.
//! - **A record counts while its chat is open**: while the app's record of open chats
//!   (`reopen`) lists its id. Each time the app writes that record it sweeps every record no
//!   chat on it holds ([`sweep`]), so a closed chat's goes, and at a launch every one left.
//! - **A folder the person makes through purlis is vouched for** ([`vouch`]): it is added to
//!   every record. A chat's own `purlis workspace create` cannot write the records.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The folder of this machine's state the records are kept in.
pub const DIR: &str = "skill-levels";

/// One running sandboxed chat's record.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Record {
    /// The chat's name, as its tab says it, for the reason a skill is left out.
    name: String,
    /// The workspace and persona folders there when it started, relative to the plane:
    /// `workspaces/ops`, `personas/steward`.
    levels: BTreeSet<String>,
}

/// Every workspace and persona folder of the plane at `root`, relative to it.
fn owners(root: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for parent in ["workspaces", "personas"] {
        let Ok(entries) = std::fs::read_dir(root.join(parent)) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            if !name.starts_with('.') && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                out.insert(format!("{parent}/{name}"));
            }
        }
    }
    out
}

/// Whether `id` can name a record's file: a ULID, as every chat id is.
fn an_id(id: &str) -> bool {
    crate::reopen::a_ulid(id).is_some()
}

fn rel_of(id: &str) -> String {
    format!("{DIR}/{id}.json")
}

fn read(state: &Path, id: &str) -> Option<Record> {
    let bytes = crate::proposed::read_pinned(state, Path::new(&rel_of(id))).ok()??;
    serde_json::from_slice(&bytes).ok()
}

fn write(state: &Path, id: &str, was: Option<&Record>, now: &Record) -> Result<(), String> {
    let base = match was {
        Some(was) => serde_json::to_vec(was)
            .ok()
            .map(|bytes| crate::proposed::base_of(Some(&bytes)).unwrap_or_default()),
        None => None,
    };
    let text = serde_json::to_string(now).map_err(|why| why.to_string())?;
    crate::proposed::write_pinned_making(
        state,
        Path::new(&rel_of(id)),
        base.as_deref(),
        &text,
        &|_, _| true,
    )
    .map_err(|_| "a chat's record of the project's levels could not be written".to_owned())
}

/// **Notes that the sandboxed chat `id`, named `name`, starts now** in the plane at `root`: the
/// workspace and persona folders there now are ones it did not make. A record already there for
/// the chat (a restart) is kept as it is, so a folder made during its earlier run stays one it
/// could have made.
pub fn note(root: &Path, id: &str, name: &str) -> Result<(), String> {
    if !an_id(id) {
        return Err("a chat with no id of its own is not noted".to_owned());
    }
    let state = crate::plane::state_dir(root);
    std::fs::create_dir_all(&state).map_err(|why| why.to_string())?;
    crate::profiletrust::private_dir(&state).map_err(|why| why.to_string())?;
    if read(&state, id).is_some() {
        return Ok(());
    }
    write(
        &state,
        id,
        None,
        &Record {
            name: name.to_owned(),
            levels: owners(root),
        },
    )
}

/// The records of the chats the app lists as open in the plane at `root`.
fn open_records(root: &Path) -> Vec<(String, Record)> {
    let state = crate::plane::state_dir(root);
    let Ok(record) = crate::reopen::read_or_refusal(root) else {
        return Vec::new();
    };
    record
        .chats
        .iter()
        .filter_map(|chat| chat.identity.id.clone())
        .filter(|id| an_id(id))
        .filter_map(|id| read(&state, &id).map(|one| (id, one)))
        .collect()
}

/// **Why the skills of the workspace or persona folder `owner` (absolute, in the plane at
/// `root`) reach no chat now**, or `None` where they do: a sandboxed chat that is still open
/// started before the folder was there, so it could have made it.
pub type MadeSince = Box<dyn Fn(&Path) -> Option<String> + Sync>;

/// [`MadeSince`] for the plane at `root`, as its records say now.
pub fn made_since(root: &Path) -> MadeSince {
    let records = open_records(root);
    let root = root.to_path_buf();
    Box::new(move |owner: &Path| {
        let rel = owner.strip_prefix(&root).ok()?;
        let rel = rel
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        records
            .iter()
            .find(|(_, record)| !record.levels.contains(&rel))
            .map(|(_, record)| {
                format!(
                    "its folder was made while chat {} was running, which could have made it: \
                     its skills reach chats once that chat has closed",
                    record.name
                )
            })
    })
}

/// **Vouches for the workspace or persona folder `owner`** (relative to the plane at `root`),
/// which purlis made for the person: added to every record, so no open chat is taken to have
/// made it. A chat cannot write the records, so its own `purlis workspace create` vouches for
/// nothing.
pub fn vouch(root: &Path, owner: &str) {
    let state = crate::plane::state_dir(root);
    for (id, was) in open_records(root) {
        let mut now = was.clone();
        if now.levels.insert(owner.to_owned())
            && let Err(why) = write(&state, &id, Some(&was), &now)
        {
            tracing::debug!(target: "purlis::skills", "{owner} was not vouched for ({why})");
        }
    }
}

/// **Removes every record whose chat is not among `open`**, the ids on the app's record of open
/// chats, once it is older than [`super::verbs::drafts::SWEEP_GRACE`] (the time between the host
/// noting a start and the chat being on that record): the app's, each time it writes that record,
/// so a closed chat's goes, and at a launch every one a quit or a crash left.
pub fn sweep(root: &Path, open: &[String]) {
    let state = crate::plane::state_dir(root);
    let Ok(entries) = std::fs::read_dir(state.join(DIR)) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let Some(id) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
            .map(str::to_owned)
        else {
            continue;
        };
        let young = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|at| at.elapsed().ok())
            .is_some_and(|age| age < super::verbs::drafts::SWEEP_GRACE);
        if an_id(&id) && !open.contains(&id) && !young {
            remove(&state, &id);
        }
    }
}

/// Removes the record of `id` from `state`, through its folder's handle: a name this module made.
fn remove(state: &Path, id: &str) {
    #[cfg(unix)]
    if let Ok(at) = super::at::folder(&state.join(DIR)) {
        let _ = super::at::remove_file_in(&at, &format!("{id}.json"));
    }
    #[cfg(not(unix))]
    let _ = (state, id);
}

/// The folder a skill level at `dir` belongs to: `workspaces/<ws>` or `personas/<p>`, or `None`
/// for the project's own levels.
pub fn owner_of(plane: &Path, dir: &Path) -> Option<PathBuf> {
    let rel = dir.strip_prefix(plane).ok()?;
    let mut parts = rel.components();
    let (first, second) = (parts.next()?, parts.next()?);
    matches!(first.as_os_str().to_str(), Some("workspaces" | "personas"))
        .then(|| plane.join(first).join(second))
}

#[cfg(test)]
mod tests;
