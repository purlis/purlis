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
//!   in this machine's app state of the project (`crate::appstate`), holding the workspace and
//!   persona folders there at that moment. A chat cannot write the app state.
//! - **A record counts until the app sweeps it**, each time it writes its record of open chats
//!   (`reopen`), for a chat no longer on it ([`sweep`]). The sweep moves every folder that chat
//!   could have made to the folders not accepted yet, which stay left out until the person
//!   accepts each in the Skill studio ([`accept`]), as a draft waits for its Accept.
//! - **A folder the person makes through purlis is vouched for** ([`vouch`]): it is added to
//!   every record. A chat's own `purlis workspace create` cannot write the records.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The folder of this machine's app state of the project the records are kept in.
pub const DIR: &str = "skill-levels";

/// The file in [`DIR`] that keeps the folders a chat that has closed could have made, until the
/// person accepts each in the Skill studio ([`accept`]).
const UNACCEPTED: &str = "unaccepted.json";

/// The folders a closed chat could have made, not accepted yet.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Unaccepted {
    levels: BTreeSet<String>,
}

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

/// The folders not accepted yet, or `Err` where the file is there and cannot be read.
fn unaccepted_read(state: &Path) -> Result<Unaccepted, ()> {
    // No folder of records yet: nothing waits.
    if std::fs::symlink_metadata(state.join(DIR))
        .is_err_and(|why| why.kind() == std::io::ErrorKind::NotFound)
    {
        return Ok(Unaccepted::default());
    }
    match crate::proposed::read_pinned(state, Path::new(&format!("{DIR}/{UNACCEPTED}"))) {
        Ok(None) => Ok(Unaccepted::default()),
        Ok(Some(bytes)) => serde_json::from_slice(&bytes).map_err(|_| ()),
        Err(_) => Err(()),
    }
}

/// The folders not accepted yet, or, where they cannot be read, every workspace and persona folder
/// of the plane at `root`: what cannot be read is never taken as accepted.
fn unaccepted_or_all(state: &Path, root: &Path) -> Unaccepted {
    unaccepted_read(state).unwrap_or_else(|()| Unaccepted {
        levels: owners(root),
    })
}

/// Writes `now` as the folders not accepted yet, over the file as it was just read. The app is
/// its one writer.
fn write_unaccepted(state: &Path, now: &Unaccepted) -> Result<(), String> {
    let rel = format!("{DIR}/{UNACCEPTED}");
    let there = crate::proposed::read_pinned(state, Path::new(&rel))
        .map_err(|_| "the folders not accepted yet could not be read".to_owned())?;
    let base = there
        .as_deref()
        .and_then(|bytes| crate::proposed::base_of(Some(bytes)));
    let text = serde_json::to_string(now).map_err(|why| why.to_string())?;
    crate::proposed::write_pinned_making(state, Path::new(&rel), base.as_deref(), &text, &|_, _| {
        true
    })
    .map_err(|_| "the folders not accepted yet could not be written".to_owned())
}

/// What a skill of a level a chat could have made is left out with, ending in its way out.
const ACCEPT_IT: &str = "accept it in the Skill studio";

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
    let noted = note_on_disk(root, id, name);
    if noted.is_err() {
        // Held in this process instead, so a folder the chat makes is still not accepted.
        unnoted_add(
            root,
            Record {
                name: name.to_owned(),
                levels: owners(root),
            },
        );
    }
    noted
}

/// The starts this process could not note on disk ([`note`]), by the clone key of their plane: each
/// counts as an open record for as long as the process runs.
fn unnoted() -> &'static std::sync::Mutex<std::collections::HashMap<String, Vec<Record>>> {
    static UNNOTED: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Vec<Record>>>,
    > = std::sync::OnceLock::new();
    UNNOTED.get_or_init(Default::default)
}

fn unnoted_add(root: &Path, record: Record) {
    if let Ok(mut held) = unnoted().lock() {
        held.entry(crate::plane::CloneKey::of(root).to_string())
            .or_default()
            .push(record);
    }
}

fn unnoted_of(root: &Path) -> Vec<(String, Record)> {
    unnoted()
        .lock()
        .ok()
        .and_then(|held| held.get(crate::plane::CloneKey::of(root).as_str()).cloned())
        .unwrap_or_default()
        .into_iter()
        .map(|record| (String::new(), record))
        .collect()
}

/// **Why a sandboxed chat in the plane at `root` is not started**, where this machine keeps no app
/// state of it (`crate::appstate::dir`): no record of the folders there at its start could be kept,
/// so a folder it made could not be told apart. `None` where there is app state.
pub fn refusal_for_a_sandboxed_start(root: &Path) -> Option<String> {
    refusal_with(crate::appstate::dir(root).as_deref())
}

/// [`refusal_for_a_sandboxed_start`], with `state` the project's app state.
fn refusal_with(state: Option<&Path>) -> Option<String> {
    state.is_none().then(|| {
        format!(
            "this sandboxed chat was not started: {}",
            crate::appstate::NO_DATA_HOME
        )
    })
}

fn note_on_disk(root: &Path, id: &str, name: &str) -> Result<(), String> {
    if !an_id(id) {
        return Err("a chat with no id of its own is not noted".to_owned());
    }
    let state = crate::appstate::made(root)?;
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
    match crate::appstate::dir(root) {
        Some(state) => records_in(&state).unwrap_or_default(),
        None => Vec::new(),
    }
}

/// The records in the app state `state`, or `Err` where the folder or a record in it cannot be
/// read.
fn records_in(state: &Path) -> Result<Vec<(String, Record)>, ()> {
    // Every record there is, whether or not its chat is still on the app's record: one whose chat
    // closed holds until the app's sweep moves what it could have made to the folders not
    // accepted yet ([`sweep`]), so no moment between the two lets such a folder through.
    let entries = match std::fs::read_dir(state.join(DIR)) {
        Ok(entries) => entries,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(()),
    };
    let mut out: Vec<(String, Record)> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| ())?;
        let Some(id) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
            .filter(|id| an_id(id))
            .map(str::to_owned)
        else {
            continue;
        };
        out.push((id.clone(), read(state, &id).ok_or(())?));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// **Why the skills of the workspace or persona folder `owner` (absolute, in the plane at
/// `root`) reach no chat now**, or `None` where they do: a sandboxed chat that is still open
/// started before the folder was there, so it could have made it.
pub type MadeSince = Box<dyn Fn(&Path) -> Option<String> + Sync>;

/// [`MadeSince`] for the plane at `root`, as its records say now: a folder an open sandboxed
/// chat could have made, or one a closed chat could have, that the person has not accepted. It
/// stays left out until the person accepts it in the Skill studio ([`accept`]), whether or not
/// the chat has closed.
pub fn made_since(root: &Path) -> MadeSince {
    made_since_in(root, crate::appstate::dir(root))
}

/// [`made_since`], read from the app state `state`: a test's seam for a data home of its own.
#[cfg(test)]
pub(crate) fn made_since_in_for(root: &Path, state: &Path) -> MadeSince {
    made_since_in(root, Some(state.to_path_buf()))
}

/// What a skill of a level is left out with where which folders were accepted cannot be read.
const UNREAD: &str = "purlis could not read which of this project's folders you accepted: \
     accept it in the Skill studio";

/// [`made_since`], with `state` the project's app state, or `None` where there is none: then no
/// sandboxed chat could have started there ([`refusal_for_a_sandboxed_start`]), and only the starts
/// this process could not note hold anything. A record or the folders not accepted yet that
/// cannot be read hold every workspace and persona folder.
fn made_since_in(root: &Path, state: Option<PathBuf>) -> MadeSince {
    let mut records = unnoted_of(root);
    let mut held = Unaccepted::default();
    let mut unread = false;
    if let Some(state) = &state {
        match records_in(state) {
            Ok(found) => records.extend(found),
            Err(()) => unread = true,
        }
        match unaccepted_read(state) {
            Ok(found) => held = found,
            Err(()) => unread = true,
        }
    }
    let root = root.to_path_buf();
    Box::new(move |owner: &Path| {
        let rel = rel_in(&root, owner)?;
        if unread && an_owner(&rel) {
            return Some(UNREAD.to_owned());
        }
        if let Some((_, record)) = records
            .iter()
            .find(|(_, record)| !record.levels.contains(&rel))
        {
            return Some(format!(
                "its folder was made while chat {} was running, which could have made it: \
                 {ACCEPT_IT}",
                record.name
            ));
        }
        held.levels
            .contains(&rel)
            .then(|| format!("its folder was made by a chat: {ACCEPT_IT}"))
    })
}

/// `owner` relative to the plane at `root`, `/` between the parts.
fn rel_in(root: &Path, owner: &Path) -> Option<String> {
    let rel = owner.strip_prefix(root).ok()?;
    Some(
        rel.components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

/// Whether `owner` names a workspace's or a persona's folder: `workspaces/<name>`,
/// `personas/<name>`.
fn an_owner(owner: &str) -> bool {
    let parts: Vec<&str> = owner.split('/').collect();
    matches!(parts.as_slice(), ["workspaces" | "personas", name]
        if !name.is_empty() && !name.starts_with('.') && !name.contains('\\'))
}

/// **Whether the workspace or persona folder `owner`** (relative to the plane at `root`) **is
/// there**, asked just before a person's create, which vouches only for a folder it made
/// ([`vouch_if_made`]).
pub fn there(root: &Path, owner: &str) -> bool {
    std::fs::symlink_metadata(root.join(owner)).is_ok()
}

/// **Vouches for `owner` after the person's own create, only where the create made it**. A folder
/// that was there before could be one a chat made for the create to take up: it is not vouched
/// for, and where its skills wait for the person's Accept in the Skill studio, the sentence that
/// says so is answered.
pub fn vouch_if_made(root: &Path, owner: &str, was_there: bool) -> Option<String> {
    if !was_there {
        vouch(root, owner);
        return None;
    }
    made_since(root)(&root.join(owner)).map(|_| {
        format!(
            "{owner} was there before it was created, so its skills reach chats only once you \
             accept its folder in the Skill studio, where a chat could have made it"
        )
    })
}

/// **The person accepts the workspace or persona folder `owner`** (relative to the plane at
/// `root`) in the Skill studio: its skills reach chats from now on. Vouched for in every open
/// record, and taken off the folders not accepted yet.
pub fn accept(root: &Path, owner: &str) -> Result<(), String> {
    if !an_owner(owner) || !root.join(owner).is_dir() {
        return Err("that folder is no workspace or persona of this project".to_owned());
    }
    vouch(root, owner);
    let Some(state) = crate::appstate::dir(root) else {
        return Ok(());
    };
    let was = unaccepted_or_all(&state, root);
    if !was.levels.contains(owner) {
        return Ok(());
    }
    let mut now = was.clone();
    now.levels.remove(owner);
    write_unaccepted(&state, &now)
}

/// [`accept`]'s folder for a skill left out in `folder` of the plane at `plane`, where a chat could
/// have made its level: what the studio's Accept names.
pub fn to_accept(plane: &Path, folder: &Path) -> Option<String> {
    owner_of(plane, folder).and_then(|owner| rel_in(plane, &owner))
}

/// **Vouches for the workspace or persona folder `owner`** (relative to the plane at `root`),
/// which purlis made for the person: added to every record, so no open chat is taken to have
/// made it. A chat cannot write the records, so its own `purlis workspace create` vouches for
/// nothing.
pub fn vouch(root: &Path, owner: &str) {
    let Some(state) = crate::appstate::dir(root) else {
        return;
    };
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
    let Some(state) = crate::appstate::dir(root) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(state.join(DIR)) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let Some(id) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
            .filter(|name| *name != UNACCEPTED.trim_end_matches(".json"))
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
            // What it could have made stays left out until the person accepts it.
            if let Some(record) = read(&state, &id) {
                let was = unaccepted_or_all(&state, root);
                let mut now = was.clone();
                now.levels
                    .extend(owners(root).difference(&record.levels).cloned());
                if now != was && write_unaccepted(&state, &now).is_err() {
                    continue;
                }
            }
            remove(&state, &id);
        }
    }
}

/// **Every workspace and persona folder of the project at `root` is held until the person
/// accepts it again** in the Skill studio ([`accept`]): what the move of an older purlis's records
/// out of the plane's state does (`crate::appstate`). Those records are not carried forward, so
/// no folder is taken as vouched for on their word.
pub(crate) fn ask_again(root: &Path) -> Result<(), String> {
    ask_again_in(&crate::appstate::made(root)?, root)
}

/// [`ask_again`], in the project's app state `state`.
pub(crate) fn ask_again_in(state: &Path, root: &Path) -> Result<(), String> {
    let was = unaccepted_or_all(state, root);
    let mut now = was.clone();
    now.levels.extend(owners(root));
    if now == was {
        return Ok(());
    }
    write_unaccepted(state, &now)
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
