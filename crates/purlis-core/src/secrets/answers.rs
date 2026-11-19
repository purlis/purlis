// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The one sealed answer store** (M75 spec §8.10, ticket M75-07a, #7015; rulings Q306, Q308,
//! Q309, Q320): every answer the person gave this machine that decides what a chat may do, each
//! sealed on its own, under a sealed, monotonic manifest, and anchored in the app's keyring
//! item. This module keeps the store, its anchors and the registry of anchored planes, and
//! **decides which state a plane is in** ([`standing`]); slice 07b refuses on it.
//!
//! # The store
//!
//! `<config home>/purlis/app/answers/<clone key>/manifest.json` and
//! `<kind>/<digest>.json` beside it; the Everywhere nevers in `answers/everywhere/`, keyed by
//! neither plane nor clone ([`Place`]).
//!
//! - **Each entry is sealed on its own** ([`super::seal`]), bound to its kind and the clone key.
//! - **The manifest lists every entry** (kind, digest, seal), the plane's id and a counter that
//!   only rises, and is itself sealed. It is the list: **an entry loads only if the manifest
//!   lists it**, never because its file is in the folder ([`load`]). A sealed file the manifest
//!   does not list is ignored, with a Notice: a removed grant, or a used Once grant, put back
//!   counts for nothing.
//! - **The counter rises on every write**, a removal and a Once grant's use included
//!   ([`commit`]).
//!
//! # The anchors
//!
//! The app's keyring item ([`super::seal::SERVICE`], outside purlis's own prefixes so the
//! one-item writer refuses it) holds, per plane, keyed by **the plane's path as it is reached**
//! ([`crate::sandbox::local::anchor_key`]: never the clone key, which a chat can change by
//! swapping `.git`): the **presence anchor**, the **high-water counter** and the counts of
//! nevers and accepted-floor versions. **The counter is written there before the manifest**, so
//! a restored state folder does not replay. The Everywhere entry has its own anchor.
//!
//! The **registry** (one sealed item) lists the planes the app opened: each one's id, real path
//! and the path as opened; and the **device id**, sealed unconfirmed at the plane's first open,
//! until the person confirms it in the window ([`context_machine`]).
//!
//! # Which state a plane is in
//!
//! [`standing`]: **deny** (a [`Deny`] cause each), **fresh** (no anchor, no store), or **holds**,
//! where only a grant that is missing or fails its seal reads as "no answer"
//! ([`Standing::no_answer`]) and any other entry that does is the deny state.
//!
//! # Where it is weaker
//!
//! As the seal is: only the app seals and verifies. On Linux and Windows any of the person's
//! programs can read the keyring, so there the anchor is only as strong as the declared seal gap
//! (Q320b R2-4); a process that is not the app is [`State::Unverifiable`].

use super::seal::{self, Item, KeyState};
use crate::sandbox::local::{anchor_key, clone_key};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

/// The accepted policy, all its data.
pub const ACCEPTED: &str = "accepted";
/// An acceptance (of a project's grants, say).
pub const ACCEPTANCE: &str = "acceptance";
/// A grant, of any scope. **The one kind that reads as "no answer" where it fails.**
pub const GRANT: &str = "grant";
/// A never.
pub const NEVER: &str = "never";
/// The nevers' presence marker: present whenever any never is, so a nevers list taken away
/// never reads as "no nevers".
pub const NEVER_MARKER: &str = "never-marker";
/// An env-var pass.
pub const PASS: &str = "pass";
/// A version of the accepted floor.
pub const FLOOR: &str = "floor";

/// The kinds a store holds.
pub const KINDS: [&str; 7] = [
    ACCEPTED,
    ACCEPTANCE,
    GRANT,
    NEVER,
    NEVER_MARKER,
    PASS,
    FLOOR,
];

/// What `bind` an Everywhere entry is sealed to: neither a plane nor a clone.
const EVERYWHERE: &str = "everywhere";

/// The keyring account of the registry.
const REGISTRY_ACCOUNT: &str = "registry";

/// The keyring account of the Everywhere anchor.
const EVERYWHERE_ACCOUNT: &str = "everywhere";

/// The keyring account of the anchor of the plane reached as `anchor`.
fn plane_account(anchor: &str) -> String {
    format!("anchor:{anchor}")
}

// ---- what is kept -----------------------------------------------------------------------------

/// One answer, as it loads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Its kind (one of [`KINDS`]).
    pub kind: String,
    /// The answer, as its owner spells it.
    pub text: String,
}

impl Entry {
    /// An answer of `kind`.
    pub fn new(kind: &str, text: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            text: text.to_owned(),
        }
    }
}

/// The file of one entry.
#[derive(Debug, Serialize, Deserialize)]
struct EntryFile {
    kind: String,
    text: String,
    mac: String,
}

/// One line of the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Listed {
    kind: String,
    digest: String,
    mac: String,
}

/// The manifest: the whole store, sealed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Manifest {
    /// The plane's stable id (a ULID); empty for the Everywhere entry.
    plane_id: String,
    /// The clone key (or `everywhere`) it was sealed for.
    bind: String,
    /// One past the last write's; only rises.
    counter: u64,
    /// Whether the person's existing nevers were imported once (Q320).
    #[serde(default)]
    imported_nevers: bool,
    entries: Vec<Listed>,
    /// Its seal, over every other field.
    #[serde(default)]
    mac: String,
}

/// What the keyring's anchor of a plane (or of the Everywhere entry) holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Anchor {
    /// The high-water counter.
    counter: u64,
    /// How many nevers the store held at it.
    nevers: u64,
    /// How many accepted-floor versions it held.
    floors: u64,
}

/// The anchor as the keyring item holds it, sealed.
#[derive(Debug, Serialize, Deserialize)]
struct AnchorItem {
    #[serde(flatten)]
    anchor: Anchor,
    mac: String,
}

/// One plane the registry lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registered {
    /// Its stable id, kept in its sealed manifest too.
    pub id: String,
    /// Its anchor ([`anchor_key`]): a digest of the path as it is reached.
    pub anchor: String,
    /// Its real path.
    pub real: PathBuf,
    /// The path as it was opened.
    pub opened: PathBuf,
    /// The clone key of its last sealed write (empty before one).
    #[serde(default)]
    pub clone: String,
    /// Whether anything was ever sealed for it.
    #[serde(default)]
    pub sealed: bool,
}

/// What the window says where there is no keyring: the shipped default holds.
pub const NO_KEYRING_NOTICE: &str = "This machine has no keyring purlis can use, so it cannot seal \
     your answers and the shipped default applies. Nothing you answer is kept.";

/// What doctor says of the answer store.
///
/// The declared seal gap on Linux and Windows is the keyring's own ([`super::keyring::Held`]);
/// the old answer files are safe to delete (Q320).
pub fn doctor_lines() -> Vec<String> {
    let mut lines = Vec::new();
    if context_machine().is_none() {
        lines.push(DEVICE_UNSET.to_owned());
    }
    lines
}

/// The device id the registry seals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSealed {
    /// The random ULID `machine.json` held when it was sealed.
    pub id: String,
    /// Whether the person confirmed it in the window.
    pub confirmed: bool,
}

/// The registry of anchored planes, and the device id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Registry {
    planes: Vec<Registered>,
    device: Option<DeviceSealed>,
    #[serde(default)]
    mac: String,
}

// ---- seals ------------------------------------------------------------------------------------

fn entry_digest(kind: &str, text: &str) -> String {
    use sha2::Digest;
    let mut hash = sha2::Sha256::new();
    for part in [kind, text] {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part.as_bytes());
    }
    let mut hex = crate::extension::hex(&hash.finalize());
    hex.truncate(32);
    hex
}

fn entry_kind(kind: &str) -> String {
    format!("answers/{kind}")
}

fn manifest_text(manifest: &Manifest) -> String {
    let mut bare = manifest.clone();
    bare.mac = String::new();
    serde_json::to_string(&bare).unwrap_or_default()
}

fn anchor_text(anchor: Anchor) -> String {
    format!("{};{};{}", anchor.counter, anchor.nevers, anchor.floors)
}

fn registry_text(registry: &Registry) -> String {
    let mut bare = registry.clone();
    bare.mac = String::new();
    serde_json::to_string(&bare).unwrap_or_default()
}

// ---- where ------------------------------------------------------------------------------------

/// Which store: a plane's, or the Everywhere entry's.
#[derive(Debug, Clone)]
struct Place {
    /// What its entries are sealed to: the clone key, or `everywhere`.
    bind: String,
    /// Its folder.
    folder: PathBuf,
    /// Its keyring account.
    account: String,
    /// What its anchor's seal is bound to.
    anchor_bind: String,
}

/// The folder every store is in; none where this machine has no config home.
fn answers_folder() -> Option<PathBuf> {
    let home = crate::sandbox::local::config_home()?;
    Some(crate::machine::dir(&home).join("app").join("answers"))
}

fn plane_place(root: &Path) -> Option<Place> {
    let bind = clone_key(root);
    let anchor = anchor_key(root);
    Some(Place {
        folder: answers_folder()?.join(&bind),
        bind,
        account: plane_account(&anchor),
        anchor_bind: anchor,
    })
}

fn everywhere_place() -> Option<Place> {
    Some(Place {
        folder: answers_folder()?.join(EVERYWHERE),
        bind: EVERYWHERE.to_owned(),
        account: EVERYWHERE_ACCOUNT.to_owned(),
        anchor_bind: EVERYWHERE.to_owned(),
    })
}

// ---- the window -------------------------------------------------------------------------------

static WINDOW_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
thread_local! {
    static TEST_WINDOW: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// **A window is open in this process** (the app calls it as its first window opens): only a
/// press in a window accepts (spec §9.4), so a windowless host (`purlis serve`) adds nothing.
pub fn window_opened() {
    WINDOW_OPEN.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// **A test's window**, on this thread: open (the default) or not.
#[cfg(test)]
pub(crate) fn set_window_for_this_test(open: bool) {
    TEST_WINDOW.with(|held| held.set(open));
}

fn window_is_open() -> bool {
    #[cfg(test)]
    {
        TEST_WINDOW.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        WINDOW_OPEN.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// What a windowless host says to a press it cannot take.
pub const NO_WINDOW: &str = "purlis accepts nothing until a window is open: only a press in a \
     window accepts, and this host has none.";

// ---- the keyring items ------------------------------------------------------------------------

/// What the keyring's item `account` held, read.
enum Read<T> {
    Value(T),
    /// No such item: never written.
    Missing,
    /// There, and not what the app sealed: edited, or made by another program.
    Broken,
    /// The keyring did not answer.
    Unreadable,
}

fn read_anchor(account: &str, bind: &str) -> Read<Anchor> {
    match seal::item_read(account) {
        Item::Missing => Read::Missing,
        Item::Unreadable | Item::NoKeyring => Read::Unreadable,
        Item::Value(text) => match serde_json::from_str::<AnchorItem>(&text) {
            Ok(item)
                if seal::holds(&item.mac, "answers/anchor", bind, &anchor_text(item.anchor)) =>
            {
                Read::Value(item.anchor)
            }
            _ => Read::Broken,
        },
    }
}

fn write_anchor(account: &str, bind: &str, anchor: Anchor) -> io::Result<()> {
    let mac = seal::sealed("answers/anchor", bind, &anchor_text(anchor))
        .ok_or_else(|| io::Error::other("purlis could not seal the answers' anchor"))?;
    let value = serde_json::to_string(&AnchorItem { anchor, mac }).map_err(io::Error::other)?;
    if seal::item_write(account, &value) {
        Ok(())
    } else {
        Err(io::Error::other(
            "purlis could not keep the answers' anchor in the keyring, so nothing was changed",
        ))
    }
}

fn read_registry() -> Read<Registry> {
    match seal::item_read(REGISTRY_ACCOUNT) {
        Item::Missing => Read::Missing,
        Item::Unreadable | Item::NoKeyring => Read::Unreadable,
        Item::Value(text) => match serde_json::from_str::<Registry>(&text) {
            Ok(registry)
                if seal::holds(
                    &registry.mac,
                    "answers/registry",
                    "machine",
                    &registry_text(&registry),
                ) =>
            {
                Read::Value(registry)
            }
            _ => Read::Broken,
        },
    }
}

fn write_registry(registry: &mut Registry) -> io::Result<()> {
    registry.mac = seal::sealed("answers/registry", "machine", &registry_text(registry))
        .ok_or_else(|| io::Error::other("purlis could not seal the registry of planes"))?;
    let value = serde_json::to_string(registry).map_err(io::Error::other)?;
    if seal::item_write(REGISTRY_ACCOUNT, &value) {
        Ok(())
    } else {
        Err(io::Error::other(
            "purlis could not keep the registry of planes in the keyring",
        ))
    }
}

/// One process at a time reads, changes and writes the registry.
static REGISTRY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn locked() -> std::sync::MutexGuard<'static, ()> {
    REGISTRY_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ---- the registry -----------------------------------------------------------------------------

/// What [`register`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Registration {
    /// The plane is in the registry, under this id.
    Registered(String),
    /// Nothing could be kept: no keyring, or a process that is not the app.
    NotKept,
}

/// **Registers the plane at `root` at its first open**, before anything is sealed (spec §8.10): a
/// process inside it is never plane-less. Its id, real path and the path as opened go into the
/// registry; **the device id is sealed there too, unconfirmed** ([`context_machine`]). Opening
/// again changes nothing. A registry that does not read is never replaced.
pub fn register(root: &Path) -> io::Result<Registration> {
    let _one = locked();
    if seal::key_state() != KeyState::Usable {
        return Ok(Registration::NotKept);
    }
    let mut registry = match read_registry() {
        Read::Value(registry) => registry,
        Read::Missing => Registry::default(),
        Read::Broken | Read::Unreadable => return Ok(Registration::NotKept),
    };
    let anchor = anchor_key(root);
    let mut changed = false;
    if registry.device.is_none()
        && let Some(home) = crate::sandbox::local::config_home()
        && let Ok(id) = crate::machine::device_id(&home)
    {
        registry.device = Some(DeviceSealed {
            id,
            confirmed: false,
        });
        changed = true;
    }
    let id = match registry.planes.iter().find(|plane| plane.anchor == anchor) {
        Some(plane) => plane.id.clone(),
        None => {
            let id = ulid::Ulid::generate().to_string();
            registry.planes.push(Registered {
                id: id.clone(),
                anchor,
                real: crate::sandbox::real(root),
                opened: std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf()),
                clone: String::new(),
                sealed: false,
            });
            changed = true;
            id
        }
    };
    if changed {
        write_registry(&mut registry)?;
    }
    Ok(Registration::Registered(id))
}

/// **The registered plane whose folder holds `path`**, by its real path or the path it was
/// opened by: what slice 07b's "outside every anchored plane" test reads. `None` where the
/// registry is empty or unreadable: callers that must fail closed ask [`registry_reads`] too.
pub fn plane_containing(path: &Path) -> Option<Registered> {
    let Read::Value(registry) = read_registry() else {
        return None;
    };
    let real = crate::sandbox::real(path);
    registry
        .planes
        .into_iter()
        .find(|plane| real.starts_with(&plane.real) || path.starts_with(&plane.opened))
}

/// Whether the registry reads (or is not there yet): false where a caller cannot tell what is
/// anchored.
pub fn registry_reads() -> bool {
    matches!(read_registry(), Read::Value(_) | Read::Missing)
}

// ---- the device id ----------------------------------------------------------------------------

/// What the registry says of this machine's device id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Device {
    /// Sealed, and the person confirmed it in the window.
    Confirmed(String),
    /// Sealed at the plane's first open, not yet confirmed.
    Unconfirmed(String),
    /// No confirmed id: no keyring, an unreadable registry, or no plane opened yet. Doctor
    /// declares it ([`DEVICE_UNSET`]).
    None,
}

/// What doctor says where `context.machine` is unset.
pub const DEVICE_UNSET: &str = "this machine has no confirmed device id, so no rule can name it: \
     purlis seals the id in its keyring when the app first opens a project, and you confirm it \
     in the window. Without a keyring, or with a registry that does not read, it stays unset.";

/// **The device id the registry holds**, never `machine.json`'s.
pub fn device() -> Device {
    match read_registry() {
        Read::Value(Registry {
            device: Some(device),
            ..
        }) if seal::key_state() == KeyState::Usable => {
            if device.confirmed {
                Device::Confirmed(device.id)
            } else {
                Device::Unconfirmed(device.id)
            }
        }
        _ => Device::None,
    }
}

/// **`context.machine`: the device id, once confirmed.** Unset (`None`) until then, with no
/// keyring, or with a registry that does not read. Editing `machine.json` changes nothing here.
pub fn context_machine() -> Option<String> {
    match device() {
        Device::Confirmed(id) => Some(id),
        Device::Unconfirmed(_) | Device::None => None,
    }
}

/// **The person confirms the device id in the window.** Refused where no window is open, and in
/// a process that cannot seal. (Refusing a chat's confirmation is slice 07b's.)
pub fn confirm_device() -> Result<(), String> {
    if !window_is_open() {
        return Err(NO_WINDOW.to_owned());
    }
    let _one = locked();
    let Read::Value(mut registry) = read_registry() else {
        return Err("purlis could not read its registry of planes".to_owned());
    };
    let Some(device) = registry.device.as_mut() else {
        return Err("no device id is sealed yet: open a project first".to_owned());
    };
    device.confirmed = true;
    write_registry(&mut registry).map_err(|why| why.to_string())
}

// ---- the state ---------------------------------------------------------------------------------

/// What was lost, counted from the anchor against what the store holds now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Lost {
    /// Writes the store is behind the anchor by.
    pub writes: u64,
    /// Nevers the anchor counted that the store does not hold.
    pub nevers: u64,
    /// Accepted-floor versions likewise.
    pub floors: u64,
}

/// Why a plane is in the deny state. One cause each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deny {
    /// The anchor is present and the plane's manifest is missing.
    ManifestMissing(Lost),
    /// The manifest is older than the anchor's counter: an older copy put back.
    ManifestOlder(Lost),
    /// The manifest is newer than the anchor: the anchor was taken away and made again.
    AnchorOlder,
    /// The anchored path's clone key has no manifest: `.git` was swapped.
    CloneKeySwapped,
    /// The manifest does not hold its seal, or is another clone's.
    ManifestFailsSeal,
    /// An entry the manifest lists is missing from the folder.
    EntryMissing {
        /// Its kind.
        kind: String,
        /// Its digest.
        digest: String,
    },
    /// An entry other than a grant fails its seal or the manifest's digest.
    EntryFailsSeal {
        /// Its kind.
        kind: String,
        /// Its digest.
        digest: String,
    },
    /// The nevers' presence marker and the nevers disagree, or the anchor's counts do.
    CountsDisagree,
    /// The plane's id is anchored at another path.
    PlaneMoved {
        /// The id.
        id: String,
        /// Where the registry has it.
        anchored_at: PathBuf,
    },
    /// A sealed store, and no anchor.
    StoreWithoutAnchor,
    /// An anchor that was kept once is gone, or does not hold its seal.
    AnchorGone,
    /// The registry does not read or does not list this plane.
    RegistryUnreadable,
    /// The Everywhere entry is missing, older or broken: every plane is in the deny state.
    EverywhereOlder(Lost),
    /// Slice 14 recorded a commit past the accepted floor's cap.
    PastTheFloorCap,
    /// The seal key stays locked on an anchored plane (a cancelled Hello, a wrong password).
    SealKeyLocked,
    /// A keyring that did not start where a store is kept.
    KeyringDidNotStart,
}

/// Where a plane stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// No anchor and no store: a fresh install; the shipped default applies (Q309 point 2).
    Fresh,
    /// No keyring at all, and nothing kept: the shipped default holds, with a Notice.
    NoKeyring,
    /// The store verified; the answers that count are in [`Standing::entries`].
    Holds,
    /// Not the app: nothing is sealed or verified, and an answer counts as none.
    Unverifiable,
    /// Nothing counts, and nothing the plane's chats ask is answered by the default.
    Deny(Vec<Deny>),
}

/// **A plane's standing**, which slice 07b refuses on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    /// The state.
    pub state: State,
    /// The plane's answers that count (empty unless [`State::Holds`]).
    pub entries: Vec<Entry>,
    /// The Everywhere nevers that count: whatever the plane's state.
    pub everywhere: Vec<Entry>,
    /// Grants that are missing or fail their seal: "no answer", each as kind and digest.
    pub no_answer: Vec<String>,
    /// What the window says: sealed files the manifest does not list, ignored.
    pub notices: Vec<String>,
}

impl Standing {
    fn of(state: State) -> Self {
        Self {
            state,
            entries: Vec::new(),
            everywhere: Vec::new(),
            no_answer: Vec::new(),
            notices: Vec::new(),
        }
    }

    /// Whether the plane is in the deny state.
    pub fn is_deny(&self) -> bool {
        matches!(self.state, State::Deny(_))
    }

    /// The causes, empty unless denied.
    pub fn causes(&self) -> &[Deny] {
        match &self.state {
            State::Deny(causes) => causes,
            _ => &[],
        }
    }

    /// **Slice 14 recorded a commit past the accepted floor's cap**: the deny state.
    #[must_use]
    pub fn with_commit_past_the_floor_cap(mut self, past: bool) -> Self {
        if past {
            self.state = match self.state {
                State::Deny(mut causes) => {
                    causes.push(Deny::PastTheFloorCap);
                    State::Deny(causes)
                }
                _ => State::Deny(vec![Deny::PastTheFloorCap]),
            };
            self.entries.clear();
        }
        self
    }
}

/// What reading one place found.
struct Found {
    manifest: Option<Manifest>,
    entries: Vec<Entry>,
    no_answer: Vec<String>,
    notices: Vec<String>,
    anchor: Option<Anchor>,
    denies: Vec<Deny>,
}

fn count(manifest: &Manifest, kind: &str) -> u64 {
    manifest.entries.iter().filter(|e| e.kind == kind).count() as u64
}

fn lost(anchor: Anchor, manifest: &Manifest) -> Lost {
    Lost {
        writes: anchor.counter.saturating_sub(manifest.counter),
        nevers: anchor.nevers.saturating_sub(count(manifest, NEVER)),
        floors: anchor.floors.saturating_sub(count(manifest, FLOOR)),
    }
}

fn read_manifest(place: &Place) -> Result<Option<Manifest>, ()> {
    let text = match std::fs::read_to_string(place.folder.join("manifest.json")) {
        Ok(text) => text,
        Err(why) if why.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    let manifest: Manifest = serde_json::from_str(&text).map_err(|_| ())?;
    // Sealed for the clone it names, which may be another one: a store moved or copied here.
    if seal::holds(
        &manifest.mac,
        "answers/manifest",
        &manifest.bind,
        &manifest_text(&manifest),
    ) {
        Ok(Some(manifest))
    } else {
        Err(())
    }
}

/// Whether anything of a store is in `place`'s folder.
fn store_exists(place: &Place) -> bool {
    std::fs::read_dir(&place.folder).is_ok_and(|mut dir| dir.next().is_some())
}

/// The entry `listed` names, **or why it does not load**.
fn load_entry(place: &Place, listed: &Listed) -> Result<Entry, bool> {
    // `Err(true)`: missing; `Err(false)`: present and failing.
    let file = place
        .folder
        .join(&listed.kind)
        .join(format!("{}.json", listed.digest));
    let text =
        std::fs::read_to_string(&file).map_err(|why| why.kind() == io::ErrorKind::NotFound)?;
    let held: EntryFile = serde_json::from_str(&text).map_err(|_| false)?;
    let ok = held.kind == listed.kind
        && held.mac == listed.mac
        && entry_digest(&held.kind, &held.text) == listed.digest
        && seal::holds(&held.mac, &entry_kind(&held.kind), &place.bind, &held.text);
    if ok {
        Ok(Entry {
            kind: held.kind,
            text: held.text,
        })
    } else {
        Err(false)
    }
}

/// Files in the kinds' folders the manifest does not list.
fn unlisted(place: &Place, manifest: &Manifest) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for kind in KINDS {
        let Ok(dir) = std::fs::read_dir(place.folder.join(kind)) else {
            continue;
        };
        for file in dir.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            let digest = name.strip_suffix(".json").unwrap_or(&name);
            if !manifest
                .entries
                .iter()
                .any(|listed| listed.kind == kind && listed.digest == digest)
            {
                out.push(file.path());
            }
        }
    }
    out
}

/// Reads `place` against its anchor. **Loads only what the manifest lists.**
fn read_place(place: &Place, plane: Option<&Registered>) -> Found {
    let mut found = Found {
        manifest: None,
        entries: Vec::new(),
        no_answer: Vec::new(),
        notices: Vec::new(),
        anchor: None,
        denies: Vec::new(),
    };
    let anchor = match read_anchor(&place.account, &place.anchor_bind) {
        Read::Value(anchor) => Some(anchor),
        Read::Missing => None,
        Read::Broken => {
            found.denies.push(Deny::AnchorGone);
            return found;
        }
        Read::Unreadable => {
            found.denies.push(Deny::SealKeyLocked);
            return found;
        }
    };
    found.anchor = anchor;
    let manifest = match read_manifest(place) {
        Ok(manifest) => manifest,
        Err(()) => {
            found.denies.push(Deny::ManifestFailsSeal);
            return found;
        }
    };
    let sealed_before = plane.is_some_and(|registered| registered.sealed);
    match (anchor, manifest) {
        (None, None) => {
            if store_exists(place) {
                found.denies.push(Deny::StoreWithoutAnchor);
            } else if sealed_before {
                found.denies.push(Deny::AnchorGone);
            }
            return found;
        }
        (None, Some(manifest)) => {
            found.denies.push(Deny::StoreWithoutAnchor);
            found.manifest = Some(manifest);
            return found;
        }
        (Some(anchor), None) => {
            let swapped = plane.is_some_and(|registered| {
                !registered.clone.is_empty() && registered.clone != place.bind
            });
            if anchor.counter == 0 && !store_exists(place) {
                return found;
            }
            found.denies.push(if swapped {
                Deny::CloneKeySwapped
            } else {
                Deny::ManifestMissing(Lost {
                    writes: anchor.counter,
                    nevers: anchor.nevers,
                    floors: anchor.floors,
                })
            });
            return found;
        }
        (Some(_), Some(manifest)) if manifest.bind != place.bind => {
            // Another clone's manifest where this clone's belongs: moved or copied here.
            found.denies.push(Deny::ManifestFailsSeal);
            found.manifest = Some(manifest);
            return found;
        }
        (Some(anchor), Some(manifest)) => {
            if manifest.counter < anchor.counter {
                found
                    .denies
                    .push(Deny::ManifestOlder(lost(anchor, &manifest)));
            } else if manifest.counter > anchor.counter {
                found.denies.push(Deny::AnchorOlder);
            }
            let counts_differ = manifest.counter == anchor.counter
                && (count(&manifest, NEVER) != anchor.nevers
                    || count(&manifest, FLOOR) != anchor.floors);
            let marker_differs =
                (count(&manifest, NEVER_MARKER) > 0) != (count(&manifest, NEVER) > 0);
            if counts_differ || marker_differs {
                found.denies.push(Deny::CountsDisagree);
            }
            for listed in &manifest.entries {
                match load_entry(place, listed) {
                    Ok(entry) => found.entries.push(entry),
                    Err(missing) if listed.kind == GRANT => {
                        found.no_answer.push(format!("grant {}", listed.digest));
                        let _ = missing;
                    }
                    Err(true) => found.denies.push(Deny::EntryMissing {
                        kind: listed.kind.clone(),
                        digest: listed.digest.clone(),
                    }),
                    Err(false) => found.denies.push(Deny::EntryFailsSeal {
                        kind: listed.kind.clone(),
                        digest: listed.digest.clone(),
                    }),
                }
            }
            for file in unlisted(place, &manifest) {
                found.notices.push(format!(
                    "purlis ignored {}: it is sealed, but the store's manifest does not list it, \
                     so a removed or used answer put back counts for nothing.",
                    file.display()
                ));
            }
            found.manifest = Some(manifest);
        }
    }
    found
}

fn nothing_in(found: &Found) -> bool {
    found.manifest.is_none() && found.anchor.is_none() && found.denies.is_empty()
}

/// **The Everywhere entry's standing**: its own manifest, anchor and counter.
fn read_everywhere() -> Found {
    match everywhere_place() {
        Some(place) => read_place(&place, None),
        None => Found {
            manifest: None,
            entries: Vec::new(),
            no_answer: Vec::new(),
            notices: Vec::new(),
            anchor: None,
            denies: Vec::new(),
        },
    }
}

/// **Which state the plane at `root` is in** (spec §4.6), and the answers that count.
pub fn standing(root: &Path) -> Standing {
    let key = seal::key_state();
    if key == KeyState::NotTheApp {
        return Standing::of(State::Unverifiable);
    }
    let Some(place) = plane_place(root) else {
        return Standing::of(State::NoKeyring);
    };
    let everywhere_place = everywhere_place();
    let on_disk = store_exists(&place) || everywhere_place.as_ref().is_some_and(store_exists);
    match key {
        KeyState::NoKeyring => {
            // A keyring that did not start where a store is kept is a keyring that failed
            // (#7153): nothing can be verified.
            let mut standing = Standing::of(if on_disk {
                State::Deny(vec![Deny::KeyringDidNotStart])
            } else {
                State::NoKeyring
            });
            if !on_disk {
                standing.notices.push(NO_KEYRING_NOTICE.to_owned());
            }
            return standing;
        }
        KeyState::Locked => return Standing::of(State::Deny(vec![Deny::SealKeyLocked])),
        KeyState::Usable | KeyState::NotTheApp => {}
    }
    let registry = read_registry();
    let registered = match &registry {
        Read::Value(registry) => registry
            .planes
            .iter()
            .find(|plane| plane.anchor == place.anchor_bind)
            .cloned(),
        _ => None,
    };
    let everywhere = read_everywhere();
    let mut denies = Vec::new();
    for cause in &everywhere.denies {
        denies.push(match cause {
            Deny::ManifestMissing(lost) | Deny::ManifestOlder(lost) => Deny::EverywhereOlder(*lost),
            other => other.clone(),
        });
    }
    let mut found = match registered.as_ref() {
        Some(plane) => read_place(&place, Some(plane)),
        None => read_place(&place, None),
    };
    if matches!(registry, Read::Broken | Read::Unreadable) && !(nothing_in(&found) && !on_disk) {
        found.denies.push(Deny::RegistryUnreadable);
    }
    if let (Some(manifest), Read::Value(registry)) = (&found.manifest, &registry) {
        match registered.as_ref() {
            Some(plane) if plane.id == manifest.plane_id => {}
            _ => match registry.planes.iter().find(|p| p.id == manifest.plane_id) {
                Some(elsewhere) => found.denies.push(Deny::PlaneMoved {
                    id: elsewhere.id.clone(),
                    anchored_at: elsewhere.real.clone(),
                }),
                None => found.denies.push(Deny::RegistryUnreadable),
            },
        }
    } else if found.manifest.is_some() && !matches!(registry, Read::Value(_)) {
        // Already said as `RegistryUnreadable`.
    }
    denies.extend(found.denies.iter().cloned());
    let mut standing = Standing::of(State::Fresh);
    standing.everywhere = if everywhere.denies.is_empty() {
        everywhere.entries.clone()
    } else {
        Vec::new()
    };
    standing.no_answer = found.no_answer;
    standing.notices = found.notices;
    standing.notices.extend(everywhere.notices);
    if !denies.is_empty() {
        standing.state = State::Deny(denies);
    } else if found.manifest.is_some() {
        standing.state = State::Holds;
        standing.entries = found.entries;
    }
    standing
}

/// **The Everywhere nevers that count**, with the deny causes where its entry is not whole.
pub fn everywhere_standing() -> Standing {
    let key = seal::key_state();
    if key == KeyState::NotTheApp {
        return Standing::of(State::Unverifiable);
    }
    if key == KeyState::NoKeyring {
        return Standing::of(State::NoKeyring);
    }
    if key == KeyState::Locked {
        return Standing::of(State::Deny(vec![Deny::SealKeyLocked]));
    }
    let found = read_everywhere();
    let mut standing = Standing::of(State::Fresh);
    standing.notices = found.notices;
    if !found.denies.is_empty() {
        standing.state = State::Deny(found.denies);
    } else if found.manifest.is_some() {
        standing.state = State::Holds;
        standing.entries = found.entries;
    }
    standing
}

// ---- writing ----------------------------------------------------------------------------------

/// What a commit changes: the entries as they stand, and what it adds or takes out.
#[derive(Debug, Clone, Default)]
pub struct Draft {
    entries: Vec<Entry>,
    imported_nevers: bool,
    added: bool,
}

impl Draft {
    /// The entries as they stand.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Adds an answer the person gave just now.
    pub fn add(&mut self, kind: &str, text: &str) {
        if !self
            .entries
            .iter()
            .any(|e| e.kind == kind && e.text == text)
        {
            self.entries.push(Entry::new(kind, text));
            self.added = true;
        }
    }

    /// Takes an answer out (a grant's removal, a Once grant's use).
    pub fn remove(&mut self, kind: &str, text: &str) {
        self.entries.retain(|e| !(e.kind == kind && e.text == text));
    }

    /// Marks the person's existing nevers as imported once (Q320).
    pub fn mark_nevers_imported(&mut self) {
        self.imported_nevers = true;
    }
}

fn manifest_of(found: &Found) -> Option<&Manifest> {
    found.manifest.as_ref()
}

/// **Commits a change to the store of the plane at `root`.** The counter rises, whatever the
/// change: a removal, a Once grant's use. Order: the new entries' files, **the anchor's counter
/// in the keyring, then the manifest**, then the removed files. Refused, with nothing changed,
/// where the plane is in the deny state ([`recommit`] is the person's press that mends it),
/// where this process cannot seal, and, for a change that adds an answer, where no window is
/// open. Returns the new counter.
pub fn commit(root: &Path, change: impl FnOnce(&mut Draft)) -> io::Result<u64> {
    let place = plane_place(root).ok_or_else(no_home)?;
    let here = standing(root);
    if here.is_deny() {
        return Err(io::Error::other(
            "the plane's answers are in the deny state, so nothing was added to them: accept again to mend it",
        ));
    }
    let id = match register(root)? {
        Registration::Registered(id) => id,
        Registration::NotKept => return Err(cannot_seal()),
    };
    let found = read_place(&place, None);
    let mut draft = Draft {
        entries: here.entries,
        imported_nevers: manifest_of(&found).is_some_and(|m| m.imported_nevers),
        added: false,
    };
    change(&mut draft);
    write(
        &place,
        Some((root, &id)),
        draft,
        found.anchor,
        manifest_of(&found).map_or(0, |m| m.counter),
    )
}

/// **The person's press that mends the deny state**: the plane's store starts again from
/// `entries`, at a counter past every one seen, so no older copy ever replays.
pub fn recommit(root: &Path, entries: &[Entry]) -> io::Result<u64> {
    let place = plane_place(root).ok_or_else(no_home)?;
    let id = match register(root)? {
        Registration::Registered(id) => id,
        Registration::NotKept => return Err(cannot_seal()),
    };
    let found = read_place(&place, None);
    let high = found
        .anchor
        .map_or(0, |a| a.counter)
        .max(manifest_of(&found).map_or(0, |m| m.counter));
    let mut draft = Draft::default();
    for entry in entries {
        draft.add(&entry.kind, &entry.text);
    }
    write(&place, Some((root, &id)), draft, found.anchor, high)
}

/// **Commits a change to the Everywhere entry** (the nevers every plane shares).
pub fn commit_everywhere(change: impl FnOnce(&mut Draft)) -> io::Result<u64> {
    let place = everywhere_place().ok_or_else(no_home)?;
    let here = everywhere_standing();
    if here.is_deny() {
        return Err(io::Error::other(
            "the Everywhere nevers are in the deny state, so nothing was added to them",
        ));
    }
    let found = read_place(&place, None);
    let mut draft = Draft {
        entries: here.entries,
        imported_nevers: manifest_of(&found).is_some_and(|m| m.imported_nevers),
        added: false,
    };
    change(&mut draft);
    write(
        &place,
        None,
        draft,
        found.anchor,
        manifest_of(&found).map_or(0, |m| m.counter),
    )
}

fn no_home() -> io::Error {
    io::Error::other("this machine has no config home to keep the answers in")
}

fn cannot_seal() -> io::Error {
    io::Error::other(
        "only purlis's app can seal an answer, and it could not here, so nothing was changed",
    )
}

fn write(
    place: &Place,
    plane: Option<(&Path, &str)>,
    draft: Draft,
    anchor: Option<Anchor>,
    manifest_counter: u64,
) -> io::Result<u64> {
    if seal::key_state() != KeyState::Usable {
        return Err(cannot_seal());
    }
    if draft.added && !window_is_open() {
        return Err(io::Error::other(NO_WINDOW));
    }
    // The marker follows the nevers, so a list taken away is never "no nevers".
    let mut entries = draft.entries;
    entries.retain(|e| e.kind != NEVER_MARKER);
    if entries.iter().any(|e| e.kind == NEVER) {
        entries.push(Entry::new(NEVER_MARKER, "present"));
    }
    let counter = anchor.map_or(0, |a| a.counter).max(manifest_counter) + 1;
    let mut listed = Vec::new();
    for entry in &entries {
        let digest = entry_digest(&entry.kind, &entry.text);
        let mac = seal::sealed(&entry_kind(&entry.kind), &place.bind, &entry.text)
            .ok_or_else(cannot_seal)?;
        write_file(
            &place
                .folder
                .join(&entry.kind)
                .join(format!("{digest}.json")),
            &serde_json::to_string(&EntryFile {
                kind: entry.kind.clone(),
                text: entry.text.clone(),
                mac: mac.clone(),
            })
            .map_err(io::Error::other)?,
        )?;
        listed.push(Listed {
            kind: entry.kind.clone(),
            digest,
            mac,
        });
    }
    listed.sort_by(|a, b| (&a.kind, &a.digest).cmp(&(&b.kind, &b.digest)));
    let mut manifest = Manifest {
        plane_id: plane.map(|(_, id)| id.to_owned()).unwrap_or_default(),
        bind: place.bind.clone(),
        counter,
        imported_nevers: draft.imported_nevers,
        entries: listed,
        mac: String::new(),
    };
    manifest.mac = seal::sealed("answers/manifest", &place.bind, &manifest_text(&manifest))
        .ok_or_else(cannot_seal)?;
    // The counter first: a state folder put back from before cannot replay past it.
    write_anchor(
        &place.account,
        &place.anchor_bind,
        Anchor {
            counter,
            nevers: count(&manifest, NEVER),
            floors: count(&manifest, FLOOR),
        },
    )?;
    if let Some((root, id)) = plane {
        mark_sealed(root, id, &place.bind)?;
    }
    write_file(
        &place.folder.join("manifest.json"),
        &serde_json::to_string_pretty(&manifest).map_err(io::Error::other)?,
    )?;
    // What the manifest no longer lists goes, after it is written.
    for kind in KINDS {
        let Ok(dir) = std::fs::read_dir(place.folder.join(kind)) else {
            continue;
        };
        for file in dir.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            let digest = name.strip_suffix(".json").unwrap_or(&name).to_owned();
            if !manifest
                .entries
                .iter()
                .any(|l| l.kind == kind && l.digest == digest)
            {
                let _ = std::fs::remove_file(file.path());
            }
        }
    }
    Ok(counter)
}

/// The registry notes that something was sealed for the plane, and under which clone key.
fn mark_sealed(root: &Path, id: &str, clone: &str) -> io::Result<()> {
    let _one = locked();
    let Read::Value(mut registry) = read_registry() else {
        return Err(io::Error::other(
            "purlis could not read its registry of planes",
        ));
    };
    let anchor = anchor_key(root);
    let Some(plane) = registry
        .planes
        .iter_mut()
        .find(|plane| plane.anchor == anchor && plane.id == id)
    else {
        return Err(io::Error::other("the plane is not in the registry"));
    };
    if plane.sealed && plane.clone == clone {
        return Ok(());
    }
    plane.sealed = true;
    plane.clone = clone.to_owned();
    write_registry(&mut registry)
}

/// A file written whole or not at all, in a folder only the person reads.
fn write_file(path: &Path, text: &str) -> io::Result<()> {
    use std::io::Write;
    let dir = path.parent().ok_or_else(no_home)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.write_all(text.as_bytes())?;
    file.persist(path).map_err(|why| why.error)?;
    Ok(())
}

#[cfg(test)]
#[path = "answers_tests.rs"]
mod tests;
