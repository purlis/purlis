// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The pairs the person said never to** (#1503): `app/dispatch-never.json`, a file of its own
//! beside this machine's other records of the project.
//!
//! # Why a file of its own
//!
//! Every other record of dispatch on this machine is an allow, so one that does not read is
//! read as empty and nothing is let through by it. A never is the one deny: read as empty, it
//! lets through what the person refused. So it is kept where nothing else is written:
//!
//! - **No other record's fault unreads it.** A key of `app/sandbox.json` this build cannot
//!   read costs that file its grants, and costs this one nothing.
//! - **No older build rewrites it.** A build from before this file drops the keys of
//!   `app/sandbox.json` it does not know at its next chat start. It never opens this file.
//!
//! # It fails closed
//!
//! [`read`] answers [`Nevers::Unread`] for a file that is there and is not the shape below, a
//! link, or anything this process cannot read. **That is never "no nevers".** While it is the
//! answer, no grant covers any pair ([`crate::dispatchgrant::Covers::Unread`]): the person is
//! asked, and told why, and a chat nobody is at is refused. No file at all is no nevers.
//!
//! **A write never drops what it could not read**: [`add`] and [`lift`] refuse on a file that
//! does not read, with [`unread_said`], and leave its bytes as they are. Mending it is the
//! person's: fix the file, or delete it, which says never to nothing.
//!
//! # The shape
//!
//! ```json
//! {"never": [{"asking": "steward", "target": "devops", "at": 1760000000, "chat": "steward 1"}]}
//! ```
//!
//! Each entry is matched as it is spelled. An entry that is not two strings makes the whole
//! file unread, since what it meant to refuse is unknown. A key this build does not know is
//! kept as it is.
//!
//! **When it was said, and on which chat's question** (#1464): `at` (seconds since 1970) and
//! `chat` (the asking chat's name as its tab showed it) are what Settings' table says of a
//! never, as the Granted list says of a grant. Both are optional and refuse nothing: an entry
//! without them, or with one that is not a number or a string, is the same never.
//!
//! # Where it is, and who writes it
//!
//! Under `app/` in the project's state folder, which the integrity class denies every
//! sandboxed chat writing ([`crate::sandbox::Class::Integrity`]). The app writes it on the
//! person's press. A chat that runs without the sandbox runs as the person and can write it,
//! as it can write anything of theirs; nothing here is a boundary against that chat.
//!
//! # What an earlier build left
//!
//! One dev build kept nevers as `dispatch_never` in `app/sandbox.json`. [`read`] moves what is
//! there into this file the first time it finds any, and counts them until the move is done.

use std::io;
use std::path::{Path, PathBuf};

/// The file, relative to the project's state folder ([`path`]).
pub const IN_STATE: &str = "app/dispatch-never.json";

/// The key that holds the pairs.
const KEY: &str = "never";

/// The file in the project at `root`.
pub fn path(root: &Path) -> PathBuf {
    crate::names::state(root).join(IN_STATE)
}

/// What [`read`] answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nevers {
    /// The pairs the person said never to, each as asking persona and target persona, as the
    /// file spells them. Empty where there is no file.
    Read(Vec<(String, String)>),
    /// There is a record and it does not read: what the person refused is unknown.
    Unread,
}

/// **When a never was said, and on which chat's question** (#1464): what Settings' table says
/// of it. Neither is part of the never: a never with neither refuses the same.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Said {
    /// When the person said it, in seconds since 1970.
    pub at: Option<u64>,
    /// The asking chat's name as its tab showed it then; none where it was said on no chat's
    /// question (an item of the needs-you list).
    pub chat: Option<String>,
}

/// One entry of the file: the pair, and what is known of when it was said. Two entries are the
/// same never whatever their seals say.
#[derive(Debug, Clone)]
pub struct Entry {
    pub asking: String,
    pub target: String,
    pub said: Said,
    /// Its seal and the set's counter when the app sealed it (ticket T1.1-73); none for an
    /// entry the app did not write, which still refuses.
    seal: Option<(String, u64)>,
    /// Every other key of the entry, kept as it is written.
    rest: serde_json::Map<String, serde_json::Value>,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.asking == other.asking
            && self.target == other.target
            && self.said == other.said
            && self.rest == other.rest
    }
}

impl Eq for Entry {}

impl Entry {
    fn pair(&self) -> (String, String) {
        (self.asking.clone(), self.target.clone())
    }

    fn is(&self, asking: &str, target: &str) -> bool {
        self.asking == asking && self.target == target
    }

    /// The never as its seal covers it.
    fn text(&self) -> String {
        format!("{} -> {}", self.asking, self.target)
    }

    /// A new entry for `asking` to `target`, said as `said`.
    fn new(asking: &str, target: &str, said: Said) -> Self {
        Self {
            asking: asking.to_owned(),
            target: target.to_owned(),
            said,
            seal: None,
            rest: serde_json::Map::new(),
        }
    }

    fn json(&self) -> serde_json::Value {
        let mut entry = serde_json::json!({"asking": self.asking, "target": self.target});
        if let Some(at) = self.said.at {
            entry["at"] = at.into();
        }
        if let Some(chat) = &self.said.chat {
            entry["chat"] = chat.as_str().into();
        }
        if let Some((mac, generation)) = &self.seal {
            entry["mac"] = mac.as_str().into();
            entry["generation"] = (*generation).into();
        }
        if let serde_json::Value::Object(keys) = &mut entry {
            for (key, value) in &self.rest {
                keys.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
        entry
    }
}

/// One entry, or `None` for an entry that is not `{asking, target}` strings. Its `at` and
/// `chat` are read where they are a number and a string, and are none otherwise.
fn entry_of(entry: &serde_json::Value) -> Option<Entry> {
    let entry = entry.as_object()?;
    let mut rest = entry.clone();
    for known in ["asking", "target", "mac", "generation"] {
        rest.remove(known);
    }
    // A when or a chat this build cannot read is kept as it is written, as any key is.
    for (key, readable) in [
        ("at", entry.get("at").is_some_and(serde_json::Value::is_u64)),
        (
            "chat",
            entry.get("chat").is_some_and(serde_json::Value::is_string),
        ),
    ] {
        if readable {
            rest.remove(key);
        }
    }
    let seal = entry
        .get("mac")
        .and_then(serde_json::Value::as_str)
        .zip(entry.get("generation").and_then(serde_json::Value::as_u64))
        .map(|(mac, generation)| (mac.to_owned(), generation));
    Some(Entry {
        rest,
        seal,
        asking: entry.get("asking")?.as_str()?.to_owned(),
        target: entry.get("target")?.as_str()?.to_owned(),
        said: Said {
            at: entry.get("at").and_then(serde_json::Value::as_u64),
            chat: entry
                .get("chat")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        },
    })
}

/// A list of entries, or `None` where it is not a list or holds one that is no pair.
fn entries_of(list: &serde_json::Value) -> Option<Vec<Entry>> {
    list.as_array()?.iter().map(entry_of).collect()
}

/// A list of entries as pairs, or `None` where it is not a list or holds one that is no pair.
pub(crate) fn pairs_of(list: &serde_json::Value) -> Option<Vec<(String, String)>> {
    Some(entries_of(list)?.iter().map(Entry::pair).collect())
}

/// The entries of the file.
type Pairs = Vec<Entry>;

/// The file's whole object, with every key this build does not know.
type Whole = serde_json::Map<String, serde_json::Value>;

/// `text` as the file's object and its pairs, or `None` where it is not this shape.
fn parsed(text: &str) -> Option<(Whole, Pairs)> {
    let serde_json::Value::Object(whole) = serde_json::from_str(text).ok()? else {
        return None;
    };
    let pairs = match whole.get(KEY) {
        None => Vec::new(),
        Some(list) => entries_of(list)?,
    };
    Some((whole, pairs))
}

/// The file's entries as they are on disk, with nothing moved into it; `None` where it is
/// there and does not read.
fn entries_on_disk(root: &Path) -> Option<Vec<Entry>> {
    whole_on_disk(root).map(|(_, entries)| entries)
}

/// The file's whole object and its entries as they are on disk (an empty object where there is
/// no file); `None` where it is there and does not read.
fn whole_on_disk(root: &Path) -> Option<(Whole, Pairs)> {
    let path = path(root);
    match std::fs::symlink_metadata(&path) {
        Err(none) if none.kind() == io::ErrorKind::NotFound => {
            Some((serde_json::Map::new(), Vec::new()))
        }
        // Never through a link, and never a folder or a device.
        Ok(there) if there.is_file() => std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| parsed(&text)),
        _ => None,
    }
}

/// The file as it is on disk, with nothing moved into it.
fn on_disk(root: &Path) -> Nevers {
    entries_on_disk(root).map_or(Nevers::Unread, |entries| {
        Nevers::Read(entries.iter().map(Entry::pair).collect())
    })
}

/// What a write is refused with on a record that does not read, and what the person is told.
pub fn unread_said(root: &Path) -> String {
    let path = path(root);
    let shown = path
        .strip_prefix(root)
        .unwrap_or(&path)
        .display()
        .to_string();
    format!(
        "purlis could not read the list of pairs you said never to ({shown} in this project), \
         or it was changed or removed by something other than purlis's app, so it changed \
         nothing there and no dispatch grant counts until it reads. Restore that file as \
         purlis last wrote it."
    )
}

/// Reads the file, changes its pairs with `how` and writes it back, under purlis's lock on its
/// directory. **Refuses a file that does not read**, and leaves it as it is.
fn change(root: &Path, how: impl FnOnce(&mut Vec<Entry>)) -> io::Result<()> {
    let unread = || io::Error::new(io::ErrorKind::InvalidData, unread_said(root));
    if on_disk(root) == Nevers::Unread {
        return Err(unread());
    }
    let path = path(root);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let anchor = crate::sandbox::local::anchor_key(root);
    crate::rewrite::update(root, &path, |now| {
        let (mut whole, mut pairs) = match now {
            None => (serde_json::Map::new(), Vec::new()),
            Some(text) => parsed(text).ok_or_else(unread)?,
        };
        let before: Vec<String> = pairs.iter().map(Entry::text).collect();
        how(&mut pairs);
        if crate::secrets::sealed_set::decides() {
            // The app seals what the person said just now, and keeps every other seal in step
            // with the list: a never taken out loses its seal, and one the app did not write
            // stays unsealed and still refuses (ticket T1.1-73).
            let adds: Vec<String> = pairs
                .iter()
                .map(Entry::text)
                .filter(|text| !before.contains(text))
                .collect();
            seal_in_step(&mut whole, &mut pairs, &anchor, &adds)?;
        }
        whole.insert(KEY.to_owned(), pairs.iter().map(Entry::json).collect());
        serde_json::to_string_pretty(&whole)
            .map(|text| Some(format!("{text}\n")))
            .map_err(io::Error::other)
    })?;
    Ok(())
}

/// The store the nevers' sealed set is anchored as: its keyring anchor is the presence marker.
const STORE: &str = "dispatch-nevers";

/// The kind of every entry of the set.
const NEVER: &str = "never";

/// The kind the file's whole-text seal was kept as by the build before the set (#6992): read
/// once, at the import, so a file a chat changed since then is not imported as it stands.
const WHOLE_KIND: &str = "dispatch-nevers";

/// The sealed set the file holds: its counter and live list, and each entry the app sealed.
fn set_of(whole: &Whole, entries: &[Entry]) -> crate::secrets::sealed_set::Set {
    crate::secrets::sealed_set::Set {
        generation: whole
            .get("generation")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        live: whole
            .get("live")
            .and_then(serde_json::Value::as_array)
            .map(|live| {
                live.iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        entries: entries
            .iter()
            .filter_map(|entry| {
                let (mac, generation) = entry.seal.clone()?;
                Some(crate::secrets::sealed_set::Entry {
                    kind: NEVER.to_owned(),
                    text: entry.text(),
                    generation,
                    mac,
                })
            })
            .collect(),
    }
}

/// Rewrites the file's set from what verifies, sealing `adds`, and puts each seal on its entry:
/// [`crate::secrets::sealed_set::Set::rewrite`].
fn seal_in_step(
    whole: &mut Whole,
    entries: &mut [Entry],
    anchor: &str,
    adds: &[String],
) -> io::Result<()> {
    let mut set = set_of(whole, entries);
    let texts: Vec<String> = entries.iter().map(Entry::text).collect();
    let adds: Vec<(&str, &str)> = adds.iter().map(|text| (NEVER, text.as_str())).collect();
    set.rewrite(
        STORE,
        anchor,
        |_, text| texts.iter().any(|t| t == text),
        &adds,
    )?;
    for entry in entries.iter_mut() {
        entry.seal = set
            .entries
            .iter()
            .find(|sealed| sealed.text == entry.text())
            .map(|sealed| (sealed.mac.clone(), sealed.generation));
    }
    whole.insert("generation".to_owned(), set.generation.into());
    whole.insert(
        "live".to_owned(),
        set.live
            .iter()
            .map(|mac| serde_json::Value::from(mac.as_str()))
            .collect::<Vec<_>>()
            .into(),
    );
    Ok(())
}

/// What [`sealed_standing`] found of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sealed {
    /// Every never the app sealed is here, or the seals do not decide in this process.
    Holds,
    /// The app never sealed a set here: the nevers there are imported once (Q320).
    Import,
    /// A sealed never is gone, the file is an older copy or deleted, or the key cannot be read
    /// while nevers are present: what the person refused is unknown.
    Unread,
}

/// **Whether the file holds every never the app sealed** (the review of #6992, S4; ticket
/// T1.1-73): its sealed set's anchor in the keyring is the presence marker, so a nevers file
/// deleted, emptied, put back from before, or with a sealed never taken out — by a chat that
/// runs without a sandbox, say — reads as [`Nevers::Unread`], which refuses every dispatch
/// across personas until the person mends it. A never something else added has no seal and
/// still refuses. Only the app verifies it.
fn sealed_standing(root: &Path) -> Sealed {
    use crate::secrets::sealed_set::{Set, Standing};
    if !crate::secrets::sealed_set::decides() {
        return Sealed::Holds;
    }
    // An acceptance of the project's `[policy]` that was kept and is not here now (the M75
    // review): the nevers it may have carried are unknown, so every dispatch across personas is
    // refused until the person presses again.
    if crate::enforcement::broken(root) {
        return Sealed::Unread;
    }
    let Some((whole, entries)) = whole_on_disk(root) else {
        return Sealed::Unread;
    };
    let anchor = crate::sandbox::local::anchor_key(root);
    let verified = set_of(&whole, &entries).verify(STORE, &anchor);
    match verified.standing {
        Standing::Holds if verified.missing == 0 => Sealed::Holds,
        Standing::Holds | Standing::Broken | Standing::Unanchored => Sealed::Unread,
        // The key does not read: nevers present, by the file or by a marker the keyring keeps,
        // are unknown; none at all is none. A keyring that did not start keeps no marker only
        // where no sealed record says one was used (#7153).
        Standing::Unverifiable => {
            let none_kept = crate::sandbox::local::no_keyring_at(root)
                || (!Set::anchored(STORE, &anchor)
                    && !crate::secrets::seal::kept_exists(WHOLE_KIND, &anchor));
            if entries.is_empty() && none_kept {
                Sealed::Holds
            } else {
                Sealed::Unread
            }
        }
        // Never sealed as a set: the file a build before it sealed whole must be as it left it.
        Standing::Fresh => {
            let text = std::fs::read_to_string(path(root)).unwrap_or_default();
            match crate::secrets::seal::kept_holds(WHOLE_KIND, &anchor, &text) {
                Some(false) => Sealed::Unread,
                _ if entries.is_empty() => Sealed::Holds,
                _ => Sealed::Import,
            }
        }
    }
}

/// **Whether the nevers file at `root` carries a seal the app made** (#7153): a set sealed at
/// least once, or any entry with its seal. A file that is there and does not read may carry one.
pub(crate) fn sealed_here(root: &Path) -> bool {
    match whole_on_disk(root) {
        Some((whole, entries)) => {
            let set = set_of(&whole, &entries);
            set.generation > 0 || !set.is_empty()
        }
        None => true,
    }
}

/// **The person's nevers from before the set, sealed once as they stand** (Q320): they only
/// tighten, so they are carried over. Refused where it cannot seal, which leaves them unread.
fn import(root: &Path) -> io::Result<()> {
    let path = path(root);
    let anchor = crate::sandbox::local::anchor_key(root);
    crate::rewrite::update(root, &path, |now| {
        let Some((mut whole, mut entries)) = now.and_then(parsed) else {
            return Ok(None);
        };
        let adds: Vec<String> = entries.iter().map(Entry::text).collect();
        seal_in_step(&mut whole, &mut entries, &anchor, &adds)?;
        whole.insert(KEY.to_owned(), entries.iter().map(Entry::json).collect());
        serde_json::to_string_pretty(&whole)
            .map(|text| Some(format!("{text}\n")))
            .map_err(io::Error::other)
    })
    .map(|_| ())
}

/// **The pairs the person said never to in the project at `root`**, or that the record does
/// not read. What an earlier build left in `app/sandbox.json` is moved here first, and is
/// counted either way.
pub fn read(root: &Path) -> Nevers {
    match sealed_standing(root) {
        Sealed::Holds => {}
        Sealed::Unread => return Nevers::Unread,
        Sealed::Import => {
            if let Err(why) = import(root) {
                tracing::warn!("purlis: the nevers were not sealed ({why})");
                return Nevers::Unread;
            }
        }
    }
    let Some(left) = crate::sandbox::local::legacy_dispatch_nevers(root) else {
        return Nevers::Unread;
    };
    let moved = left.is_empty()
        || (change(root, |pairs| {
            for (asking, target) in &left {
                if !pairs.iter().any(|one| one.is(asking, target)) {
                    pairs.push(Entry::new(asking, target, Said::default()));
                }
            }
        })
        .is_ok()
            && crate::sandbox::local::drop_legacy_dispatch_nevers(root).is_ok());
    match on_disk(root) {
        Nevers::Unread => Nevers::Unread,
        Nevers::Read(mut pairs) => {
            if !moved {
                for one in left {
                    if !pairs.contains(&one) {
                        pairs.push(one);
                    }
                }
            }
            Nevers::Read(pairs)
        }
    }
}

/// Records the person's never for `asking` to `target`, with nothing said of when: [`add_said`].
pub fn add(root: &Path, asking: &str, target: &str) -> io::Result<()> {
    add_said(root, asking, target, Said::default())
}

/// Records the person's never for `asking` to `target`: the grant Notice's "Never for this
/// pair", never a chat. `said` is when, and on which chat's question; a never said again keeps
/// when it was first said. Refused where the record does not read.
pub fn add_said(root: &Path, asking: &str, target: &str, said: Said) -> io::Result<()> {
    if read(root) == Nevers::Unread {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            unread_said(root),
        ));
    }
    change(root, |pairs| {
        if !pairs.iter().any(|one| one.is(asking, target)) {
            pairs.push(Entry::new(asking, target, said));
        }
    })
}

/// **Every never with when it was said** (#1464), in the file's order: what Settings' table
/// draws. Empty where the record does not read, which [`read`] says; read after [`read`], so
/// what an earlier build left is moved first.
pub fn entries(root: &Path) -> Vec<Entry> {
    if read(root) == Nevers::Unread {
        return Vec::new();
    }
    entries_on_disk(root).unwrap_or_default()
}

/// Lifts that never: Settings, never a chat. Answers whether there was one. Refused where the
/// record does not read.
pub fn lift(root: &Path, asking: &str, target: &str) -> io::Result<bool> {
    let Nevers::Read(now) = read(root) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            unread_said(root),
        ));
    };
    if !now.iter().any(|(from, to)| from == asking && to == target) {
        return Ok(false);
    }
    change(root, |pairs| {
        pairs.retain(|one| !one.is(asking, target));
    })?;
    Ok(true)
}

#[cfg(test)]
#[path = "dispatchnever_tests.rs"]
mod tests;
