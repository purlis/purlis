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

/// Whether the plane at `root` has any workspace or persona folder.
pub(crate) fn has_owners(root: &Path) -> bool {
    !owners(root).is_empty()
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
    note_on_disk(root, id, name)
}

/// The record kept where a sandboxed chat's start could not be noted under its own id
/// ([`note_or_hold`]): an open record no sweep removes, so every folder made since stays left out
/// until the person accepts it, which adds it to this record as to every other ([`vouch`]).
const UNNOTED: &str = "unnoted";

/// **[`note`], or, where it cannot be noted, a record no sweep removes** ([`UNNOTED`]), so no
/// folder made after the start reaches a chat unasked, and the person's Accept in the Skill
/// studio lets each go as usual. `Ok(None)` where it was noted, `Ok(Some(why))` where it was not
/// and the record holds, and `Err` where neither could be written: such a chat is not started.
pub fn note_or_hold(root: &Path, id: &str, name: &str) -> Result<Option<String>, String> {
    let Err(why) = note(root, id, name) else {
        return Ok(None);
    };
    let held = crate::appstate::made(root).and_then(|state| {
        if read(&state, UNNOTED).is_some() {
            return Ok(());
        }
        write(
            &state,
            UNNOTED,
            None,
            &Record {
                name: name.to_owned(),
                levels: owners(root),
            },
        )
    });
    held.map_err(|also| {
        format!(
            "this sandboxed chat was not started: purlis could not keep the record of its \
             project's folders ({why}; {also})"
        )
    })?;
    Ok(Some(why))
}

/// **Why a sandboxed chat in the plane at `root` is not started**, where this machine keeps no app
/// state of it (`crate::appstate::dir`): no record of the folders there at its start could be kept,
/// so a folder it made could not be told apart. `None` where there is app state.
pub fn refusal_for_a_sandboxed_start(root: &Path) -> Option<String> {
    refusal_with(crate::appstate::kept_here(root))
}

/// [`refusal_for_a_sandboxed_start`], with `kept` whether the machine can keep its app state.
fn refusal_with(kept: bool) -> Option<String> {
    (!kept).then(|| {
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
    // The record of starts that could not be noted, where there is one ([`UNNOTED`]).
    match read(state, UNNOTED) {
        Some(record) => out.push((UNNOTED.to_owned(), record)),
        None if std::fs::symlink_metadata(state.join(rel_of(UNNOTED)))
            .is_err_and(|why| why.kind() == std::io::ErrorKind::NotFound) => {}
        None => return Err(()),
    }
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
        match read(state, &id) {
            Some(record) => out.push((id, record)),
            // One removed while the folder was listed is no record.
            None if std::fs::symlink_metadata(state.join(rel_of(&id)))
                .is_err_and(|why| why.kind() == std::io::ErrorKind::NotFound) => {}
            None => return Err(()),
        }
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

/// What every workspace and persona folder is left out with where purlis cannot confirm which of
/// them the person accepted ([`confirmed`]).
pub const UNCONFIRMED: &str = "purlis can't confirm which of this project's folders you accepted \
     before: open the project once in the purlis app and press Accept the folders in the Skill \
     studio's Needs you";

/// [`made_since`], with `state` the project's app state, or `None` where there is none. **Held by
/// default**: unless `state` holds levels records this purlis confirmed for this project
/// (`crate::appstate::levels_confirmed`), every workspace and persona folder is left out until the
/// person accepts them again ([`confirm`]); so is every one where a record or the folders not
/// accepted yet cannot be read.
fn made_since_in(root: &Path, state: Option<PathBuf>) -> MadeSince {
    let mut records = Vec::new();
    let mut held = Unaccepted::default();
    let mut unread = false;
    let unconfirmed = match state.as_deref() {
        Some(state) => !crate::appstate::levels_confirmed(state, root),
        // A project purlis has never kept state for starts confirmed at its first write.
        None => !crate::appstate::never_kept(root),
    };
    let mut accepted = Unaccepted::default();
    if let Some(state) = &state {
        match records_in(state) {
            Ok(found) => records.extend(found),
            Err(()) => unread = true,
        }
        match unaccepted_read(state) {
            Ok(found) => held = found,
            Err(()) => unread = true,
        }
        if unconfirmed {
            accepted = accepted_one_by_one(state);
        }
    }
    let on_disk = owners(root);
    let known = std::sync::OnceLock::new();
    let root = root.to_path_buf();
    Box::new(move |owner: &Path| {
        let identity = |rel: &str| folder_identity(&root, rel);
        let rel = as_the_disk_spells(&on_disk, rel_in(&root, owner)?, &identity, &known);
        if unread && an_owner(&rel) {
            return Some(UNREAD.to_owned());
        }
        // A chat's own reason first: no Accept of the project as a whole lets a folder through
        // that a running chat could have made.
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
        if unconfirmed && an_owner(&rel) && !accepted.levels.contains(&rel) {
            return Some(UNCONFIRMED.to_owned());
        }
        held.levels
            .contains(&rel)
            .then(|| format!("its folder was made by a chat: {ACCEPT_IT}"))
    })
}

/// A folder's identity on its disk: its device and inode.
type Identity = (u64, u64);

/// **`rel` as the disk spells it**: the workspace or persona folder among `on_disk` that is the
/// folder `rel` opens, where `rel` names it only in another spelling (a disk that folds case, or
/// one spelling of a letter, opens both names as one folder). Matched by the folders' `identity`,
/// never by folding the name; `known` keeps `on_disk`'s identities once read. `rel` itself where
/// it is one of them, or opens none.
fn as_the_disk_spells(
    on_disk: &BTreeSet<String>,
    rel: String,
    identity: &dyn Fn(&str) -> Option<Identity>,
    known: &std::sync::OnceLock<std::collections::BTreeMap<Identity, String>>,
) -> String {
    if on_disk.contains(&rel) || !an_owner(&rel) {
        return rel;
    }
    // No port reads a folder's identity there yet: the name is folded instead.
    #[cfg(not(unix))]
    {
        let lower = rel.to_lowercase();
        if let Some(folder) = on_disk.iter().find(|folder| folder.to_lowercase() == lower) {
            return folder.clone();
        }
    }
    let Some(asked) = identity(&rel) else {
        return rel;
    };
    known
        .get_or_init(|| {
            on_disk
                .iter()
                .filter_map(|folder| identity(folder).map(|id| (id, folder.clone())))
                .collect()
        })
        .get(&asked)
        .cloned()
        .unwrap_or(rel)
}

/// The identity of the folder `rel` of the plane at `root`, following no link: a link is a folder
/// of its own, never the one it points at.
fn folder_identity(root: &Path, rel: &str) -> Option<Identity> {
    let path = root.join(rel);
    std::fs::symlink_metadata(&path)
        .is_ok_and(|meta| meta.is_dir())
        .then(|| crate::pypath::link_identity(&path))
        .flatten()
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
    if !confirmed(root) {
        // That folder alone, while the rest wait for the project's one Accept.
        let state = crate::appstate::made(root)?;
        repaired(&state)?;
        let mut now = accepted_one_by_one(&state);
        if now.levels.insert(owner.to_owned()) {
            write_list(&state, ACCEPTED, &now)?;
        }
    } else if let Some(state) = crate::appstate::dir(root) {
        repaired(&state)?;
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
            // A record that cannot be read could have named any of them: every one is held. One
            // removed meanwhile is no record.
            let read_now = read(&state, &id);
            if read_now.is_none()
                && std::fs::symlink_metadata(state.join(rel_of(&id)))
                    .is_err_and(|why| why.kind() == std::io::ErrorKind::NotFound)
            {
                continue;
            }
            let levels = read_now.map(|record| record.levels).unwrap_or_default();
            let was = unaccepted_or_all(&state, root);
            let mut now = was.clone();
            now.levels.extend(owners(root).difference(&levels).cloned());
            if now != was && write_unaccepted(&state, &now).is_err() {
                continue;
            }
            remove(&state, &id);
        }
    }
}

/// **Each chat on the plane at `root`'s record of open chats noted in the app state `state`**, its
/// levels the workspace and persona folders there now, where it has no record there yet: what the
/// move of an older purlis's records out of the plane does, so a chat that started before the
/// upgrade and runs on holds every folder it makes after it, as a chat noted at its start does.
/// Nothing of the plane's own records is read. Where the record of open chats is there and cannot
/// be read, or names a running chat with no id of its own, one record no sweep removes holds them
/// all ([`UNNOTED`]).
pub(crate) fn noted_again(state: &Path, root: &Path) -> Result<(), String> {
    noted_again_with(state, root, &owners(root))
}

/// [`noted_again`], each new record holding `now`, every workspace and persona folder as the
/// caller listed them: so [`confirm`]'s records and its folders not accepted yet are of one
/// listing, and a folder made after it is held by each new record.
fn noted_again_with(state: &Path, root: &Path, now: &BTreeSet<String>) -> Result<(), String> {
    let unnoted = || (UNNOTED.to_owned(), "a chat".to_owned());
    let chats: Vec<(String, String)> = match crate::reopen::read_strictly(root) {
        Ok(None) => Vec::new(),
        Ok(Some(record)) => record
            .chats
            .iter()
            .filter_map(|chat| {
                match chat.identity.id.clone().filter(|id| an_id(id)) {
                    Some(id) => Some((id, chat.name.clone())),
                    // A running chat with no id of its own is held by one record for all.
                    None if chat.pid.is_some_and(crate::process::alive) => Some(unnoted()),
                    None => None,
                }
            })
            .collect(),
        // A record this purlis cannot read could name any chat: one record holds them all.
        Err(_) => vec![unnoted()],
    };
    for (id, name) in chats {
        if read(state, &id).is_some() {
            continue;
        }
        write(
            state,
            &id,
            None,
            &Record {
                name,
                levels: now.clone(),
            },
        )?;
    }
    Ok(())
}

/// **Every workspace and persona folder of the project at `root` is held until the person
/// accepts it again** in the Skill studio ([`accept`]): what the move of an older purlis's records
/// out of the plane's state does (`crate::appstate`), with `state` the project's app state. Those
/// records are not carried forward, so no folder is taken as vouched for on their word.
pub(crate) fn ask_again_in(state: &Path, root: &Path) -> Result<(), String> {
    let was = unaccepted_or_all(state, root);
    let mut now = was.clone();
    now.levels.extend(owners(root));
    if now == was {
        return Ok(());
    }
    write_unaccepted(state, &now)
}

/// **Whether purlis can confirm which of the plane at `root`'s folders the person accepted**: its
/// app state holds levels records this purlis confirmed for it.
pub fn confirmed(root: &Path) -> bool {
    match crate::appstate::dir(root) {
        Some(state) => crate::appstate::levels_confirmed(&state, root),
        None => crate::appstate::never_kept(root),
    }
}

/// **What the studio asks the person once for the plane at `root`**, where purlis cannot confirm
/// which of its folders they accepted and it has some: the sentence ([`UNCONFIRMED`]) and the
/// folders its Accept accepts, each there now and not accepted one by one yet. `None` otherwise.
pub fn to_confirm(root: &Path) -> Option<(&'static str, Vec<String>)> {
    if confirmed(root) {
        return None;
    }
    let accepted = crate::appstate::dir(root)
        .map(|state| accepted_one_by_one(&state))
        .unwrap_or_default();
    let folders: Vec<String> = owners(root)
        .into_iter()
        .filter(|folder| !accepted.levels.contains(folder))
        .collect();
    (!folders.is_empty()).then_some((UNCONFIRMED, folders))
}

/// The file in [`DIR`] that keeps the folders the person accepted one by one while purlis could
/// not confirm the rest ([`accept`]): taken up into the project's one Accept ([`confirm`]).
const ACCEPTED: &str = "accepted.json";

/// The folders accepted one by one while unconfirmed; none where the file cannot be read.
fn accepted_one_by_one(state: &Path) -> Unaccepted {
    crate::proposed::read_pinned(state, Path::new(&format!("{DIR}/{ACCEPTED}")))
        .ok()
        .flatten()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// `now` written as `name` in [`DIR`], beside it first and renamed into its place.
fn write_list(state: &Path, name: &str, now: &Unaccepted) -> Result<(), String> {
    let folder = state.join(DIR);
    crate::profiletrust::private_dir(&folder).map_err(|why| why.to_string())?;
    let text = serde_json::to_string(now).map_err(|why| why.to_string())?;
    crate::rewrite::replace(
        &folder,
        &folder.join(name),
        text.as_bytes(),
        crate::rewrite::Mode::Private,
    )
    .map_err(|why| why.to_string())
}

/// **The person's one Accept for the plane at `root`** (#3718), of the folders `folders` the
/// studio listed ([`to_confirm`]): its levels records are written bound to it, every record that
/// cannot be read is replaced, and each listed folder there now is taken off the folders not
/// accepted yet; every other folder there now goes on them. No chat's record is changed, so a
/// folder a running chat could have made stays left out for that reason.
pub fn confirm(root: &Path, folders: &[String]) -> Result<(), String> {
    let state = crate::appstate::made(root)?;
    std::fs::create_dir_all(state.join(DIR)).map_err(|why| why.to_string())?;
    repaired(&state)?;
    let now = owners(root);
    // The listed folders, and those the person accepted one by one before.
    let listed: BTreeSet<String> = folders
        .iter()
        .cloned()
        .chain(accepted_one_by_one(&state).levels)
        .filter(|folder| now.contains(folder))
        .collect();
    let was = unaccepted_or_all(&state, root);
    let mut kept: BTreeSet<String> = was.levels.difference(&listed).cloned().collect();
    kept.extend(now.difference(&listed).cloned());
    let kept = Unaccepted { levels: kept };
    if kept != was || unaccepted_read(&state).is_err() {
        write_unaccepted(&state, &kept)?;
    }
    // Every chat on the record of open chats keeps its own record through the Accept, holding
    // every folder of the one listing above.
    noted_again_with(&state, root, &now)?;
    crate::appstate::mark_levels_confirmed(&state, root)?;
    // Emptied last: where a step above fails, the folders accepted one by one stay accepted. Once
    // the mark is written the Accept is done, and the list is read only while the project is
    // unconfirmed, so one left behind is said and not taken as the Accept failing.
    if let Err(why) = write_list(&state, ACCEPTED, &Unaccepted::default()) {
        tracing::warn!(
            target: "purlis::skills",
            "the folders accepted one by one were not emptied ({why})"
        );
    }
    Ok(())
}

/// **Every record in the app state `state` that is there and cannot be read, replaced by one that
/// still holds every folder**: a record naming no folder, so each is left out until the person
/// accepts it, and Accept can then let them go one by one ([`vouch`]).
fn repaired(state: &Path) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(state.join(DIR)) else {
        return Ok(());
    };
    let ids: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_suffix(".json"))
                .filter(|id| *id == UNNOTED || an_id(id))
                .map(str::to_owned)
        })
        .collect();
    for id in ids {
        if read(state, &id).is_some() {
            continue;
        }
        // The new record is written beside the old and renamed over it: it is never gone first.
        let text = serde_json::to_string(&Record {
            name: "a chat".to_owned(),
            levels: BTreeSet::new(),
        })
        .map_err(|why| why.to_string())?;
        let folder = state.join(DIR);
        crate::rewrite::replace(
            &folder,
            &state.join(rel_of(&id)),
            text.as_bytes(),
            crate::rewrite::Mode::Private,
        )
        .map_err(|why| why.to_string())?;
    }
    Ok(())
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
