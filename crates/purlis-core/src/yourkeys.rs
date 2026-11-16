// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Your keys: the only place a model key is read from** (R-169, R-619; spec C-23, T1a-09).
//!
//! In 1a each entry is one system keychain item, service `purlis/@your-keys` ([`SERVICE`]),
//! account the entry's id (a ULID), under the `@` convention [`crate::linkkey`] follows, with no
//! legacy `charter/` read. The keychain cannot be asked what it holds, so each entry has one
//! line in **the index**, `<config>/model-keys/index.json` ([`index_path`]): its id, its `kind`
//! (`keychain`), the `product` it is a key for (the declaration's `{ "from", "id" }` reference),
//! its `approval` setting and whether it was `checked` at save. Never its value.
//!
//! - **Written by the window alone** (R-173): [`add`], [`replace`] and [`set_approval`] are called from the
//!   app's window commands (`app/src-tauri/src/yourkeyscmds.rs`) and from nothing a CLI verb, a
//!   session-protocol method or a chat tool reaches. `nothing_but_the_window_writes_your_keys`
//!   walks every crate and the app's Rust, and only this file and that one may name a writer or
//!   [`SERVICE`].
//! - **Read by one function** (R-169, R-172): [`read_key`] reads the index and the entry's item,
//!   and nothing else: no project vault, whatever it holds under the same name, and no persona's
//!   grant, so any chat on a profile naming the entry gets the same key in any project.
//! - **The write order** (C-23, H-4): the index line first, then the item; a failed item write
//!   removes the line again and takes out the item it may have left, as does a write the
//!   keychain could not hold to the app. A crash in between leaves at worst a line with no item.
//! - **Recovery** (C-23): a line with no item lists as unreadable ([`Listed::readable`]); an item
//!   with no line is ignored and listed nowhere; a missing index is empty; an index that does not
//!   parse is refused with its error and nothing is written over it.
//! - **One key per product in 1a** (U9): a second entry for a product the index holds is not
//!   written; [`Saved::ReplaceOffered`] names the entry held, for the form's W9. A line this
//!   build can't read that names the product, or whose product can't be read, blocks it too.
//! - **The approval setting** (R-030, R-621): fixed off, since adding the key in the window is
//!   its approval. Any other setting is refused with W39.
//!
//! - **The mark** ([`mark`], T1a-10): an entry is used as a model key only while its sealed
//!   record says so. [`replace`] seals it again over the new value; any other write unmarks it.
//!
//! The index is on the Machine tier, device-bound, backed up; the items are on the Keyring tier
//! and not backed up (H-18). The whole `model-keys` folder is denied to every chat's sandbox,
//! reading and writing (`crate::sandbox`).

pub mod mark;

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ulid::Ulid;

use crate::seam::{Credentials, Files};
use crate::secrets::keyring::{Held, Presence, Secret};

/// The keychain service every entry's item is under; the account is the entry's id.
pub const SERVICE: &str = "purlis/@your-keys";

/// Your keys as a vault name: what a command, a tool, a fill or a runner would hand over to reach
/// an entry. Never a vault ([`crate::secrets::registry::name_ok`] refuses it).
pub const AS_A_VAULT: &str = "@your-keys";

/// Whether `name` names your keys, as [`AS_A_VAULT`] or as [`SERVICE`], ignoring case and the
/// spaces around it: what every existing secret path refuses (T1a-72, R-772,
/// [`crate::secrets::registry::your_keys_refusal`]).
pub fn named_by(name: &str) -> bool {
    let name = name.trim();
    name.eq_ignore_ascii_case(AS_A_VAULT) || name.eq_ignore_ascii_case(SERVICE)
}

/// The folder of the model keys' stores, in purlis's directory of the config home.
pub const DIR: &str = "model-keys";

/// The index, in [`DIR`].
pub const INDEX: &str = "index.json";

/// The index's own lock, in the session host's folder beside the other stores' locks: never the
/// profiles store's.
pub const LOCK: &str = "model-keys-index.json.lock";

/// The version the index is written at. One written at a newer version is never written over.
const VERSION: u64 = 1;

/// The most the index may be: the profiles store's cap.
const MOST_BYTES: u64 = crate::profilestore::MOST_BYTES;

/// Where the index is under the config home `config_root`.
pub fn index_path(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join(DIR).join(INDEX)
}

/// Where the index's lock is under the config home `config_root`.
pub fn lock_path(config_root: &Path) -> PathBuf {
    crate::machine::charterd(config_root).join(LOCK)
}

/// What an entry is a key for: a model provider declaration, by where it comes from and its id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Product {
    /// `built-in`, `project` or `yours`.
    pub from: String,
    pub id: String,
}

impl Product {
    /// The Anthropic built-in, the one product 1a ships.
    pub fn anthropic() -> Self {
        Self {
            from: "built-in".to_owned(),
            id: "anthropic".to_owned(),
        }
    }
}

/// Where an entry's value is: a keychain item, the only kind in 1a.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    #[serde(rename = "keychain")]
    Keychain,
}

/// An entry's V15 setting: off, the only one until V15 is built for vaults (R-030, R-621).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Approval {
    #[serde(rename = "off")]
    Off,
}

/// What the check at save found (V10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Checked {
    #[serde(rename = "passed")]
    Passed,
    #[serde(rename = "not checked yet")]
    NotCheckedYet,
    #[serde(rename = "refused at save")]
    RefusedAtSave,
}

/// One entry's index line. Never its value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: Ulid,
    pub kind: Kind,
    pub product: Product,
    pub approval: Approval,
    pub checked: Checked,
    /// Fields this build does not know, written back as they were.
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

/// An entry as Your keys lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub entry: Entry,
    /// False where the keychain provably holds no item for the line (W55's *Remove*).
    pub readable: bool,
}

/// What a save did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Saved {
    /// The entry is in your keys, under this id.
    Added(Ulid),
    /// Your keys already holds a key for the product, under `held`, and nothing was written: the
    /// form offers to replace it (W9).
    ReplaceOffered { held: Ulid },
}

/// Why [`read_key`] has no key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// No index line has the id.
    NotInYourKeys,
    /// The line is there and the keychain holds no item for it.
    Unreadable,
    /// The index could not be read.
    Index(String),
    /// The keychain could not be read.
    Store(String),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInYourKeys => f.write_str("the key is not in your keys"),
            Self::Unreadable => f.write_str("the key's item is missing from your keychain"),
            Self::Index(why) | Self::Store(why) => f.write_str(why),
        }
    }
}

/// The index as read: the lines this build reads, the ones it can't (kept as written), and the
/// rest of the file.
#[derive(Debug, Clone, Default, PartialEq)]
struct Index {
    version: u64,
    entries: Vec<Entry>,
    unread: Vec<Value>,
    rest: Map<String, Value>,
}

/// The file as written.
#[derive(Default, Serialize, Deserialize)]
struct OnDisk {
    #[serde(default)]
    version: u64,
    #[serde(default)]
    entries: Vec<Value>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

/// The index from its bytes, or why they are not one.
fn parse(at: &Path, bytes: &[u8]) -> Result<Index, String> {
    if bytes.len() as u64 > MOST_BYTES {
        return Err(format!(
            "{} is larger than your keys' index can be",
            at.display()
        ));
    }
    let on_disk: OnDisk = serde_json::from_slice(bytes)
        .map_err(|e| format!("{} is not your keys' index: {e}", at.display()))?;
    let mut index = Index {
        version: on_disk.version,
        rest: on_disk.rest,
        ..Index::default()
    };
    for value in on_disk.entries {
        match serde_json::from_value::<Entry>(value.clone()) {
            Ok(entry) => index.entries.push(entry),
            Err(_) => index.unread.push(value),
        }
    }
    Ok(index)
}

/// The index on disk, or an empty one where there is none.
fn read(config_root: &Path) -> Result<Index, String> {
    let at = index_path(config_root);
    let file = match std::fs::File::open(&at) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Index::default()),
        Err(e) => return Err(format!("purlis cannot read {}: {e}", at.display())),
    };
    let mut bytes = Vec::new();
    file.take(MOST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("purlis cannot read {}: {e}", at.display()))?;
    parse(&at, &bytes)
}

fn write(config_root: &Path, index: &Index) -> Result<(), String> {
    let at = index_path(config_root);
    let fail = |e: &dyn std::fmt::Display| format!("purlis cannot write {}: {e}", at.display());
    let mut entries = Vec::with_capacity(index.entries.len() + index.unread.len());
    for entry in &index.entries {
        entries.push(serde_json::to_value(entry).map_err(|e| fail(&e))?);
    }
    entries.extend(index.unread.iter().cloned());
    let text = serde_json::to_string_pretty(&OnDisk {
        version: VERSION,
        entries,
        rest: index.rest.clone(),
    })
    .map_err(|e| fail(&e))?
        + "\n";
    let dir = crate::machine::private_dir(config_root).map_err(|e| fail(&e))?;
    crate::profiletrust::private_dir(&dir.join(DIR)).map_err(|e| fail(&e))?;
    crate::machine::write_beside(config_root, &at, text.as_bytes()).map_err(|e| fail(&e))
}

/// **Whether your keys may hold a model key** under the config home `config_root`: any line in
/// the index, one this build reads or not, or an index purlis cannot read. What every chat's
/// sandbox counts as a keyring vault, so the system's credential service is denied to each chat
/// whenever a model key exists (decisions.md D6640-T3 item 7). A missing index holds none.
pub fn may_hold_a_key(config_root: &Path) -> bool {
    read(config_root).map_or(true, |index| {
        !index.entries.is_empty() || !index.unread.is_empty()
    })
}

/// The index under its lock, refused where it can't be read or a newer purlis wrote it.
fn read_to_write(config_root: &Path) -> Result<Index, String> {
    let index = read(config_root)?;
    if index.version > VERSION {
        return Err(format!(
            "{} was written by a newer purlis (version {}; this one writes version {VERSION}), \
             so this one leaves it as it is",
            index_path(config_root).display(),
            index.version
        ));
    }
    Ok(index)
}

/// **Adds a key to your keys** (the window's save): the index line, then the keychain item under
/// [`SERVICE`] and the new entry's id. A product your keys already holds a key for is not
/// written ([`Saved::ReplaceOffered`]), nor one an unread line may already hold a key for. An
/// item write that fails, or leaves the item the calling program's rather than held to the app
/// ([`Held::NotYet`]), takes the item out again (best effort) and removes the line, and the
/// error says why. Only the window calls this (R-173).
pub fn add(
    config_root: &Path,
    credentials: &dyn Credentials,
    product: Product,
    checked: Checked,
    value: &str,
) -> Result<Saved, String> {
    let _held = crate::machine::Lock::named(config_root, LOCK);
    let mut index = read_to_write(config_root)?;
    if let Some(held) = index.entries.iter().find(|e| e.product == product) {
        return Ok(Saved::ReplaceOffered { held: held.id });
    }
    if index.unread.iter().any(|line| may_be_for(line, &product)) {
        return Err(format!(
            "{} holds a line this purlis can't read that may be a key for {}, so it adds no \
             second one: remove that line, or open your keys in the purlis that wrote it",
            index_path(config_root).display(),
            product.id
        ));
    }
    let id = Ulid::generate();
    index.entries.push(Entry {
        id,
        kind: Kind::Keychain,
        product,
        approval: Approval::Off,
        checked,
        rest: Map::new(),
    });
    write(config_root, &index)?;
    let account = id.to_string();
    let why = match credentials.set(SERVICE, &account, value) {
        // Held to the app, or by a store that keeps no rule (the stub, or off macOS).
        Ok(Held::ToTheApp | Held::NoRule) => return Ok(Saved::Added(id)),
        // Outside the app on macOS: the item is the calling program's, which only the window's
        // write may never leave.
        Ok(Held::NotYet) => "the item couldn't be held to the purlis app; add the key in \
                             the purlis window"
            .to_owned(),
        Err(why) => why.to_string(),
    };
    // The item may have landed: it is taken out again, best effort, with its line.
    let mut said = format!("purlis couldn't keep the key in your keychain: {why}");
    if let Err(left) = credentials.delete(SERVICE, &account) {
        said += &format!("; and an item for it may be left in your keychain: {left}");
    }
    index.entries.retain(|e| e.id != id);
    if let Err(left) = write(config_root, &index) {
        said += &format!("; and its line is left in your keys: {left}");
    }
    Err(said)
}

/// Whether an index line this build can't read may be a key for `product`: it names the
/// product, or its product can't be read at all, so a second key is never added beside it.
fn may_be_for(line: &Value, product: &Product) -> bool {
    line.get("product")
        .and_then(|p| serde_json::from_value::<Product>(p.clone()).ok())
        .is_none_or(|named| named == *product)
}

/// **Your keys as listed**: every entry the index holds, each readable unless the keychain
/// provably holds no item for it. Asked of the items' attributes, never their values. An item
/// with no line is never listed.
pub fn list(config_root: &Path, credentials: &dyn Credentials) -> Result<Vec<Listed>, String> {
    Ok(read(config_root)?
        .entries
        .into_iter()
        .map(|entry| {
            let readable = credentials.exists(SERVICE, &entry.id.to_string()) != Presence::Absent;
            Listed { entry, readable }
        })
        .collect())
}

/// **Sets an entry's approval** (V15): only off is taken, which every entry already has, so
/// nothing is written; any other setting is refused with W39. Only the window calls this (R-173).
pub fn set_approval(config_root: &Path, id: Ulid, setting: &str) -> Result<(), String> {
    use crate::window_copy::{fill, line};
    if setting != "off" {
        return Err(fill(line::W39, &[("setting", setting)]));
    }
    let _held = crate::machine::Lock::named(config_root, LOCK);
    let index = read_to_write(config_root)?;
    if !index.entries.iter().any(|e| e.id == id) {
        return Err(ReadError::NotInYourKeys.to_string());
    }
    Ok(())
}

/// **The one way a model key is read** (R-169, R-172): the entry's line from the index, through
/// `files`, and its keychain item, through `credentials`. Nothing else is read: no project vault
/// and no persona's grant, so the key is the same for a chat in any project under any persona.
/// A start reads it as a model key through [`mark::read_model_key`], which also verifies its
/// mark (R-177).
pub fn read_key(
    config_root: &Path,
    files: &dyn Files,
    credentials: &dyn Credentials,
    id: Ulid,
) -> Result<Secret, ReadError> {
    let entry = read_entry(config_root, files, id)?;
    read_item(credentials, &entry)
}

/// Entry `id`'s line, read through `files`.
fn read_entry(config_root: &Path, files: &dyn Files, id: Ulid) -> Result<Entry, ReadError> {
    let at = index_path(config_root);
    let index = match files.read(&at) {
        Ok(bytes) => parse(&at, &bytes).map_err(ReadError::Index)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Index::default(),
        Err(e) => {
            return Err(ReadError::Index(format!(
                "purlis cannot read {}: {e}",
                at.display()
            )));
        }
    };
    index
        .entries
        .into_iter()
        .find(|e| e.id == id)
        .ok_or(ReadError::NotInYourKeys)
}

/// Entry's value, from its keychain item through `credentials`.
fn read_item(credentials: &dyn Credentials, entry: &Entry) -> Result<Secret, ReadError> {
    match credentials.get(SERVICE, &entry.id.to_string()) {
        Ok(Some(value)) => Ok(value),
        Ok(None) => Err(ReadError::Unreadable),
        Err(why) => Err(ReadError::Store(why.to_string())),
    }
}

/// What a *Replace* left the entry as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Replaced {
    /// The new value is in, and its mark is sealed again over it: a rotated key keeps its mark.
    Marked,
    /// The new value is in, and the entry is not marked: it wasn't before, or its value had been
    /// changed outside purlis, so the person marks it again.
    Unmarked,
}

/// **Replaces an entry's value** (the window's *Replace*, R-176): where the entry's mark
/// verifies against the value it holds now, the new value is written and the mark sealed again
/// over it, through the mark's one writer, so the rotated key keeps its mark. A mark that
/// doesn't verify is never sealed again: the value is written and the entry stays unmarked.
/// Only the window calls this (R-173).
pub fn replace(
    config_root: &Path,
    credentials: &dyn Credentials,
    id: Ulid,
    value: &str,
) -> Result<Replaced, String> {
    let _held = crate::machine::Lock::named(config_root, LOCK);
    let index = read_to_write(config_root)?;
    let entry = index
        .entries
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| ReadError::NotInYourKeys.to_string())?;
    let kept = match read_item(credentials, entry) {
        Ok(old) => mark::binding_for(config_root, credentials, entry, &old).ok(),
        Err(ReadError::Unreadable) => None,
        Err(why) => return Err(why.to_string()),
    };
    match credentials.set(SERVICE, &id.to_string(), value) {
        Ok(Held::ToTheApp | Held::NoRule) => {}
        Ok(Held::NotYet) => {
            return Err(
                "the key couldn't be held to the purlis app; replace it in the purlis \
                        window"
                    .to_owned(),
            );
        }
        Err(why) => {
            return Err(format!(
                "purlis couldn't keep the key in your keychain: {why}"
            ));
        }
    }
    let Some(binding) = kept else {
        return Ok(Replaced::Unmarked);
    };
    mark::mark(config_root, credentials, id, &binding)
        .map_err(|why| format!("the key was replaced and couldn't be marked again: {why}"))?;
    Ok(Replaced::Marked)
}
