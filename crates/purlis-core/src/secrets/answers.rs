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
//!   ([`commit`]). **One commit at a time**, in this process and across them: a file lock under
//!   the store is held from reading the store to the last file written.
//! - **A marker by the plane's path** (`sealed-here/<anchor key>`) says a store was kept there,
//!   so a swapped `.git` where the keyring did not start is still a refusal, never the shipped
//!   default.
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
    /// The identity of its `.git` (device and inode) when it was registered, which a move on
    /// the same filesystem keeps: how a plane moved with no link left is found again
    /// ([`standing`]). Empty where unknown.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub git: String,
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
    /// Whether anything was ever sealed in the Everywhere entry: the backstop where its anchor
    /// and its folder are both gone.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    everywhere_sealed: bool,
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

/// **The marker that something was sealed for the plane reached as `root`**, keyed by the path
/// alone ([`anchor_key`]), never the clone key: where the keyring did not start and `.git` was
/// swapped, the clone key names no folder, and only the path still says a store was kept. An
/// interim of this module's own, until the sealed-planes marker of #7228 is on this branch.
fn sealed_here_marker(root: &Path) -> Option<PathBuf> {
    Some(answers_folder()?.join("sealed-here").join(anchor_key(root)))
}

/// Whether the marker for `root` is there.
fn sealed_here(root: &Path) -> bool {
    sealed_here_marker(root).is_some_and(|marker| marker.exists())
}

// ---- the lock ---------------------------------------------------------------------------------

/// How long a commit waits for another one before it refuses.
#[cfg(not(test))]
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(10);
#[cfg(test)]
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_millis(300);

/// One commit at a time in this process, where there is no `flock` to say it (non-unix).
#[cfg(not(unix))]
static COMMIT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// **What a commit holds from reading the store to the last file written**: an `flock` on
/// `answers/.lock` (taken on its own open file, so it holds between two commits of one process
/// as well as between two processes), so two commits (two windows, a second process) take turns
/// and never both write one past the same counter, the loser's entries deleted by the winner's
/// clean-up. Released on drop. Without `flock` (non-unix) a lock in this process only.
struct StoreLock {
    #[cfg(not(unix))]
    _process: std::sync::MutexGuard<'static, ()>,
    #[cfg(unix)]
    _file: std::fs::File,
}

fn lock_file() -> Option<PathBuf> {
    Some(answers_folder()?.join(".lock"))
}

fn store_lock() -> io::Result<StoreLock> {
    let deadline = std::time::Instant::now() + LOCK_WAIT;
    let busy = || {
        io::Error::other(
            "another change to the answers is in progress, so nothing was changed: try again",
        )
    };
    #[cfg(unix)]
    {
        let path = lock_file().ok_or_else(no_home)?;
        make_dir(path.parent().ok_or_else(no_home)?)?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)?;
        loop {
            match rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
                Ok(()) => return Ok(StoreLock { _file: file }),
                Err(rustix::io::Errno::WOULDBLOCK | rustix::io::Errno::INTR) => {
                    if std::time::Instant::now() >= deadline {
                        return Err(busy());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(why) => return Err(io::Error::from(why)),
            }
        }
    }
    #[cfg(not(unix))]
    loop {
        match COMMIT_LOCK.try_lock() {
            Ok(guard) => return Ok(StoreLock { _process: guard }),
            Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                return Ok(StoreLock {
                    _process: poisoned.into_inner(),
                });
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                if std::time::Instant::now() >= deadline {
                    return Err(busy());
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    }
}

// ---- the press --------------------------------------------------------------------------------

/// **The person's press in the window** (spec §9.4): only a press accepts, so everything that
/// adds an answer, takes out one that widens what a chat may do, mends the deny state or confirms
/// the device id takes one. It is a value, not a flag the process holds: it is made by one function
/// and passed to each call, so a host with no window has none to pass and the compiler refuses
/// the call.
#[derive(Debug)]
#[must_use = "a press is what lets an answer change"]
pub struct Press(());

impl Press {
    /// **Made on the person's press, and only then**: the app's window command layer calls this
    /// from the handler of a press the person made in the window (an accept, a confirm, a mend).
    /// **No handler that serves a chat (a hook, a door, a dispatch) may call it**, nor `purlis
    /// serve`, nor the CLI: a chat's request is never the person's press.
    pub fn of_the_persons_press() -> Self {
        Self(())
    }
}

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
/// again changes nothing. **A registry that does not read is never replaced, and a registry that
/// is missing is made only where nothing is kept** (no store, marker or anchor, this plane's or
/// any other's): where something is, replacing it is the person's press ([`recommit`]). Takes
/// the store's lock, so it never races a commit's read-change-write of the registry.
pub fn register(root: &Path) -> io::Result<Registration> {
    let _turn = store_lock()?;
    register_unlocked(root)
}

/// [`register`], for a caller that holds the store's lock already ([`commit`], [`recommit`]).
fn register_unlocked(root: &Path) -> io::Result<Registration> {
    let _one = locked();
    if seal::key_state() != KeyState::Usable {
        return Ok(Registration::NotKept);
    }
    let mut registry = match read_registry() {
        Read::Value(registry) => registry,
        Read::Missing => {
            if something_kept(root) {
                return Err(io::Error::other(
                    "purlis could not find its registry of planes where answers are kept, so it \
                     made no new one: accept again to mend it",
                ));
            }
            Registry::default()
        }
        Read::Broken | Read::Unreadable => return Ok(Registration::NotKept),
    };
    let anchor = anchor_key(root);
    let mut changed = seed_device(&mut registry);
    let git = git_identity(root);
    let id = match registry
        .planes
        .iter_mut()
        .find(|plane| plane.anchor == anchor)
    {
        Some(plane) => {
            if plane.git.is_empty() && !git.is_empty() {
                plane.git = git;
                changed = true;
            }
            plane.id.clone()
        }
        None => {
            let id = ulid::Ulid::generate().to_string();
            registry.planes.push(Registered {
                id: id.clone(),
                anchor,
                real: crate::sandbox::real(root),
                opened: std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf()),
                clone: String::new(),
                sealed: false,
                git,
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

/// **Whether anything is kept** that a registry should list: a store, marker or anchor of the
/// plane at `root`, the Everywhere entry's, or any other store in the answers' folder.
fn something_kept(root: &Path) -> bool {
    let mut places = vec![everywhere_place()];
    places.push(plane_place(root));
    let in_a_place = places.into_iter().flatten().any(|place| {
        store_exists(&place)
            || !matches!(
                read_anchor(&place.account, &place.anchor_bind),
                Read::Missing
            )
    });
    let in_the_folder = answers_folder()
        .and_then(|folder| std::fs::read_dir(folder).ok())
        .is_some_and(|dir| dir.flatten().any(|item| item.file_name() != ".lock"));
    in_a_place || in_the_folder || sealed_here(root)
}

/// Seals the device id into `registry` where it holds none: unconfirmed. Whether it changed.
fn seed_device(registry: &mut Registry) -> bool {
    if registry.device.is_none()
        && let Some(home) = crate::sandbox::local::config_home()
        && let Ok(id) = crate::machine::device_id(&home)
    {
        registry.device = Some(DeviceSealed {
            id,
            confirmed: false,
        });
        return true;
    }
    false
}

/// **The person's press replaces a registry that does not hold its seal, or is missing where
/// something is kept,** with a new one: no plane listed (each is registered again by its own
/// [`recommit`]), the device id sealed again and **unconfirmed**, the Everywhere flag kept where
/// its store or anchor is still there. Only where this process can seal and the registry was read
/// and failed (or answered that it is not there): a keyring that did not answer is never
/// replaced. What it cannot rebuild: which other planes had anything sealed, and the clone keys
/// of their last writes, so a plane whose anchor and store both go afterwards reads as fresh
/// until it is registered again. The caller holds the store's lock.
fn mend_registry(_press: &Press, root: Option<&Path>) -> io::Result<()> {
    let _one = locked();
    if seal::key_state() != KeyState::Usable {
        return Err(cannot_seal());
    }
    let missing_where_kept = matches!(read_registry(), Read::Missing)
        && root.map_or_else(
            || {
                everywhere_place().is_some_and(|place| {
                    store_exists(&place)
                        || !matches!(
                            read_anchor(&place.account, &place.anchor_bind),
                            Read::Missing
                        )
                })
            },
            something_kept,
        );
    if !matches!(read_registry(), Read::Broken) && !missing_where_kept {
        return Ok(());
    }
    let mut registry = Registry {
        everywhere_sealed: everywhere_place().is_some_and(|place| {
            store_exists(&place)
                || !matches!(
                    read_anchor(&place.account, &place.anchor_bind),
                    Read::Missing
                )
        }),
        ..Registry::default()
    };
    seed_device(&mut registry);
    write_registry(&mut registry)
}

/// The identity of `root`'s `.git` (device and inode), which a rename on one filesystem keeps.
/// Empty where there is none, or the platform does not say.
fn git_identity(root: &Path) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::symlink_metadata(root.join(".git")) {
            return format!("{}:{}", meta.dev(), meta.ino());
        }
    }
    let _ = root;
    String::new()
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

/// **The person confirms the device id in the window**: the one `id` the window showed, so an id
/// that changed since is never confirmed unseen. Takes the person's press ([`Press`]); refused
/// where the registry does not read.
pub fn confirm_device(_press: &Press, id: &str) -> Result<(), String> {
    let _turn = store_lock().map_err(|why| why.to_string())?;
    let _one = locked();
    let Read::Value(mut registry) = read_registry() else {
        return Err("purlis could not read its registry of planes".to_owned());
    };
    let Some(device) = registry.device.as_mut() else {
        return Err("no device id is sealed yet: open a project first".to_owned());
    };
    if device.id != id {
        return Err(
            "the device id is not the one that was shown, so nothing was confirmed".to_owned(),
        );
    }
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
    /// The nevers an earlier build kept are still in their old files and were not imported into
    /// the store (Q320), or do not read: what the person refused is unknown, so the old files stay
    /// authoritative and the plane stays denied until [`import_nevers`] has run.
    NeversNotImported,
}

impl Deny {
    /// What this cause says, in words, for doctor and the window.
    pub fn said(&self) -> String {
        match self {
            Self::ManifestMissing(_) => "the store's manifest is gone".to_owned(),
            Self::ManifestOlder(lost) => format!(
                "an older copy of the store was put back ({} writes, {} nevers and {} accepted \
                 versions are lost)",
                lost.writes, lost.nevers, lost.floors
            ),
            Self::AnchorOlder => "the keyring's anchor is older than the store".to_owned(),
            Self::CloneKeySwapped => "the project's .git was swapped for another".to_owned(),
            Self::ManifestFailsSeal => {
                "the store's manifest does not hold its seal, or is another clone's".to_owned()
            }
            Self::EntryMissing { kind, .. } => format!("a {kind} the manifest lists is missing"),
            Self::EntryFailsSeal { kind, .. } => format!("a {kind} fails its seal"),
            Self::CountsDisagree => {
                "the nevers or their marker disagree with the anchor".to_owned()
            }
            Self::PlaneMoved { anchored_at, .. } => format!(
                "the project was moved: its answers are anchored at {}",
                anchored_at.display()
            ),
            Self::StoreWithoutAnchor => "a sealed store has no anchor in the keyring".to_owned(),
            Self::AnchorGone => {
                "an anchor that was kept is gone or does not hold its seal".to_owned()
            }
            Self::RegistryUnreadable => {
                "the registry of projects does not read, or does not list this one".to_owned()
            }
            Self::EverywhereOlder(_) => {
                "the nevers that apply everywhere are missing or older than they were".to_owned()
            }
            Self::PastTheFloorCap => "a commit went past the accepted floor's cap".to_owned(),
            Self::SealKeyLocked => {
                "the seal key stays locked (a cancelled confirmation or a wrong password)"
                    .to_owned()
            }
            Self::KeyringDidNotStart => {
                "the keyring did not start where answers are kept".to_owned()
            }
            Self::NeversNotImported => {
                "the nevers you said before are still in their old files and are not imported yet"
                    .to_owned()
            }
        }
    }
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

/// **Whether `path` is still a folder of its own**: a link at it, which the app's move leaves
/// pointing at the new place until every record follows, is the plane gone from there.
fn still_a_folder(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| !meta.file_type().is_symlink())
}

fn nothing_in(found: &Found) -> bool {
    found.manifest.is_none() && found.anchor.is_none() && found.denies.is_empty()
}

/// **The Everywhere entry's standing**: its own manifest, anchor and counter. Where it has
/// none of them and the registry says something was sealed in it, the anchor is gone: the
/// backstop for a folder and an anchor taken away together.
fn read_everywhere() -> Found {
    let mut found = match everywhere_place() {
        Some(place) => read_place(&place, None),
        None => Found {
            manifest: None,
            entries: Vec::new(),
            no_answer: Vec::new(),
            notices: Vec::new(),
            anchor: None,
            denies: Vec::new(),
        },
    };
    if nothing_in(&found) && matches!(read_registry(), Read::Value(r) if r.everywhere_sealed) {
        found.denies.push(Deny::AnchorGone);
    }
    found
}

/// **Which state the plane at `root` is in** (spec §4.6), and the answers that count.
pub fn standing(root: &Path) -> Standing {
    standing_with(root, true)
}

/// [`standing`], where `import_counts` says whether nevers still in their old files, not yet
/// imported, are the deny state ([`Deny::NeversNotImported`]). The import itself asks without,
/// since it is what ends that cause.
fn standing_with(root: &Path, import_counts: bool) -> Standing {
    let key = seal::key_state();
    if key == KeyState::NotTheApp {
        return Standing::of(State::Unverifiable);
    }
    let Some(place) = plane_place(root) else {
        return Standing::of(State::NoKeyring);
    };
    let everywhere_place = everywhere_place();
    // By path as well as by clone key: a swapped `.git` names another clone key, which has no
    // folder, and only the path's marker still says a store was kept here.
    let plane_on_disk = store_exists(&place) || sealed_here(root);
    let on_disk = plane_on_disk || everywhere_place.as_ref().is_some_and(store_exists);
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
    let nothing_here = nothing_in(&found) && !plane_on_disk;
    let registry_trouble = match &registry {
        Read::Broken | Read::Unreadable => !(nothing_in(&found) && !on_disk),
        // A registry that was never written is fine only where nothing was ever kept: any
        // anchor or store, the Everywhere entry's included, was registered first.
        Read::Missing => !nothing_in(&found) || on_disk || !nothing_in(&everywhere),
        Read::Value(_) => false,
    };
    if registry_trouble {
        found.denies.push(Deny::RegistryUnreadable);
    }
    if let Read::Value(registry) = &registry
        && nothing_here
        && let git = git_identity(root)
        && !git.is_empty()
        && let Some(old) = registry.planes.iter().find(|plane| {
            plane.sealed
                && plane.anchor != place.anchor_bind
                && plane.git == git
                && !still_a_folder(&plane.real)
        })
    {
        // Nothing kept at this path, and a plane that had answers sealed, whose folder is gone,
        // had this very `.git`: the plane was moved, and no anchor speaks for its new path.
        found.denies.push(Deny::PlaneMoved {
            id: old.id.clone(),
            anchored_at: old.real.clone(),
        });
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
    }
    if import_counts && import::nevers_unsettled(root, &found) {
        found.denies.push(Deny::NeversNotImported);
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
        // A keyring that did not start where the Everywhere entry is kept is one that failed
        // (#7153), as [`standing`] reads it.
        let kept = everywhere_place().is_some_and(|place| store_exists(&place));
        return if kept {
            Standing::of(State::Deny(vec![Deny::KeyringDidNotStart]))
        } else {
            Standing::of(State::NoKeyring)
        };
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

/// What a commit changes: the entries as they stand.
#[derive(Debug, Clone, Default)]
pub struct Draft {
    entries: Vec<Entry>,
    imported_nevers: bool,
    /// Why the commit is refused: a kind that is not one of [`KINDS`], or a removal that is not
    /// narrowing and came without a press.
    refused: Option<String>,
}

impl Draft {
    /// The entries as they stand.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Adds an answer the person gave just now (their [`Press`]). A `kind` that is not one of
    /// [`KINDS`] (it names a file in the store's folder) adds nothing, and the commit is refused.
    pub fn add(&mut self, _press: &Press, kind: &str, text: &str) {
        if !KINDS.contains(&kind) {
            self.refused = Some(format!(
                "{kind:?} is not a kind of answer the store keeps, so nothing was changed"
            ));
            return;
        }
        if !self
            .entries
            .iter()
            .any(|e| e.kind == kind && e.text == text)
        {
            self.entries.push(Entry::new(kind, text));
        }
    }

    /// Takes any answer out, **the person's press** ([`Press`]): a never's or a floor's removal
    /// widens what a chat may do.
    pub fn remove(&mut self, _press: &Press, kind: &str, text: &str) {
        self.take_out(kind, text);
    }

    /// **Takes out an answer whose removal only narrows what a chat may do**: a grant's removal
    /// (a Once grant's use included), a pass's, an acceptance's withdrawal. Needs no press, so a
    /// windowless host records a Once grant's use. Any other kind (a never, a floor, the accepted
    /// policy) widens when it goes: it adds nothing and refuses the commit.
    pub fn remove_narrowing(&mut self, kind: &str, text: &str) {
        if ![GRANT, PASS, ACCEPTANCE].contains(&kind) {
            self.refused = Some(format!(
                "taking out {kind:?} widens what a chat may do, so only the person's press can: \
                 nothing was changed"
            ));
            return;
        }
        self.take_out(kind, text);
    }

    fn take_out(&mut self, kind: &str, text: &str) {
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
/// change: a removal, a Once grant's use. **One commit at a time, across processes**: the lock
/// is held from reading the store to the last file written. Order: **the anchor's counter in the
/// keyring**, the new entries' files, the manifest, then the removed files. Refused, with
/// nothing changed, where the plane is in the deny state ([`recommit`] is the person's press
/// that mends it), where this process cannot seal, and, for a change that adds or takes out an
/// answer. Returns the new counter. **A change that adds an answer, or takes out one that
/// widens, needs the person's [`Press`]** ([`Draft::add`], [`Draft::remove`]); a narrowing
/// removal needs none ([`Draft::remove_narrowing`]).
pub fn commit(root: &Path, change: impl FnOnce(&mut Draft)) -> io::Result<u64> {
    let place = plane_place(root).ok_or_else(no_home)?;
    let _turn = store_lock()?;
    let here = standing(root);
    if here.is_deny() {
        return Err(io::Error::other(
            "the plane's answers are in the deny state, so nothing was added to them: accept again to mend it",
        ));
    }
    let id = match register_unlocked(root)? {
        Registration::Registered(id) => id,
        Registration::NotKept => return Err(cannot_seal()),
    };
    let found = read_place(&place, None);
    let mut draft = Draft {
        entries: here.entries,
        imported_nevers: manifest_of(&found).is_some_and(|m| m.imported_nevers),
        ..Draft::default()
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

/// A counter no store of this app reaches: a person's presses are not that many. An unsealed
/// claim above it was written by hand.
const PLAUSIBLE_COUNTER: u64 = 1 << 32;

/// **The highest counter `place` has reached**, for a store that starts again past it.
///
/// - **Where the anchor reads and holds its seal, it is the floor**, and a manifest that does not
///   hold its seal is ignored: its claim, above the anchor's or not, is an unsealed claim, and
///   a hand-edited `u64::MAX` would freeze the counter. A sealed manifest above the anchor counts
///   (the anchor was taken away and made again).
/// - **Where the anchor is missing or does not hold its seal**, the unsealed counters still say
///   how far the store got, so a store restarted below them would let an older copy pass; but a
///   claim beyond [`PLAUSIBLE_COUNTER`] is refused, saying why.
/// - **Where the keyring did not answer, refused**: that is not a nought.
fn highest_counter(place: &Place) -> io::Result<u64> {
    let claimed = |text: &str| {
        serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .and_then(|value| value.get("counter").and_then(serde_json::Value::as_u64))
            .unwrap_or(0)
    };
    let sealed_manifest = read_manifest(place)
        .ok()
        .flatten()
        .map_or(0, |manifest| manifest.counter);
    let unsealed_manifest = std::fs::read_to_string(place.folder.join("manifest.json"))
        .map_or(0, |text| claimed(&text));
    let unsealed_anchor = match read_anchor(&place.account, &place.anchor_bind) {
        Read::Value(anchor) => return Ok(anchor.counter.max(sealed_manifest)),
        Read::Unreadable => {
            return Err(io::Error::other(
                "purlis could not read the answers' anchor from its keyring, so nothing was changed",
            ));
        }
        Read::Missing => 0,
        Read::Broken => match seal::item_read(&place.account) {
            Item::Value(text) => claimed(&text),
            _ => {
                return Err(io::Error::other(
                    "purlis could not read the answers' anchor from its keyring, so nothing was changed",
                ));
            }
        },
    };
    let unsealed = unsealed_manifest.max(unsealed_anchor);
    if unsealed > PLAUSIBLE_COUNTER {
        return Err(io::Error::other(
            "the answers' counter was edited to a value purlis never reached and its anchor does \
             not hold to check it, so nothing was changed: take the store's manifest away to start \
             again",
        ));
    }
    Ok(unsealed.max(sealed_manifest))
}

/// **The person's press that mends the deny state**: the plane's store starts again from
/// `entries`, at a counter past every one seen (see [`highest_counter`]), so no older copy ever
/// replays. A registry that does not hold its seal, or is missing where something is kept, is
/// replaced first ([`mend_registry`]). Needs the person's [`Press`], an empty `entries` included.
pub fn recommit(root: &Path, press: &Press, entries: &[Entry]) -> io::Result<u64> {
    let place = plane_place(root).ok_or_else(no_home)?;
    let _turn = store_lock()?;
    mend_registry(press, Some(root))?;
    let id = match register_unlocked(root)? {
        Registration::Registered(id) => id,
        Registration::NotKept => return Err(cannot_seal()),
    };
    let counter = highest_counter(&place)?;
    // **The nevers are kept** (Q320): the ones the last readable state still holds, and the ones
    // the old files hold (which an earlier import may have left only there), and the imported mark
    // with them. The press mends the deny state and never takes a never away, and a plane with
    // its mark lost must not dead-end in [`Deny::NeversNotImported`] because the old file was
    // deleted, as doctor said it could be: pressing is the person's decision to start again.
    let last = read_place(&place, None);
    let mut draft = Draft::default();
    for entry in last.entries.iter().filter(|e| e.kind == NEVER) {
        draft.add(press, NEVER, &entry.text);
    }
    if let crate::dispatchnever::Left::Pairs(pairs) = crate::dispatchnever::left_behind(root) {
        for (asking, target) in &pairs {
            draft.add(press, NEVER, &import::never_text(asking, target));
        }
    }
    draft.mark_nevers_imported();
    for entry in entries {
        draft.add(press, &entry.kind, &entry.text);
    }
    write(&place, Some((root, &id)), draft, None, counter)
}

/// **Commits a change to the Everywhere entry** (the nevers every plane shares), as [`commit`].
pub fn commit_everywhere(change: impl FnOnce(&mut Draft)) -> io::Result<u64> {
    let place = everywhere_place().ok_or_else(no_home)?;
    let _turn = store_lock()?;
    let here = everywhere_standing();
    if here.is_deny() {
        return Err(io::Error::other(
            "the Everywhere nevers are in the deny state, so nothing was added to them",
        ));
    }
    if !registry_reads() {
        return Err(io::Error::other(
            "purlis could not read its registry of planes, so nothing was added to the nevers: accept again to mend it",
        ));
    }
    let found = read_place(&place, None);
    let mut draft = Draft {
        entries: here.entries,
        imported_nevers: manifest_of(&found).is_some_and(|m| m.imported_nevers),
        ..Draft::default()
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

/// **The person's press that mends the Everywhere entry**, as [`recommit`] mends a plane: the
/// entry starts again from `entries`, past every counter seen, and a registry that does not hold
/// its seal is replaced first. Needs the person's [`Press`]. Never a chat's.
pub fn recommit_everywhere(press: &Press, entries: &[Entry]) -> io::Result<u64> {
    let place = everywhere_place().ok_or_else(no_home)?;
    let _turn = store_lock()?;
    mend_registry(press, None)?;
    if !registry_reads() {
        return Err(io::Error::other(
            "purlis could not read its registry of planes, so nothing was changed",
        ));
    }
    let mut draft = Draft::default();
    for entry in entries {
        draft.add(press, &entry.kind, &entry.text);
    }
    write(&place, None, draft, None, highest_counter(&place)?)
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
    if let Some(why) = draft.refused {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, why));
    }
    // The marker follows the nevers, so a list taken away is never "no nevers".
    let mut entries = draft.entries;
    if let Some(entry) = entries.iter().find(|e| !KINDS.contains(&e.kind.as_str())) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{:?} is not a kind of answer the store keeps", entry.kind),
        ));
    }
    entries.retain(|e| e.kind != NEVER_MARKER);
    if entries.iter().any(|e| e.kind == NEVER) {
        entries.push(Entry::new(NEVER_MARKER, "present"));
    }
    // A counter that cannot rise is a store that cannot be told from an older copy of itself.
    let counter = anchor
        .map_or(0, |a| a.counter)
        .max(manifest_counter)
        .checked_add(1)
        .ok_or_else(|| {
            io::Error::other("the answers' counter cannot rise any further, so nothing was changed")
        })?;
    // Everything is sealed before anything is written.
    let mut listed = Vec::new();
    let mut files = Vec::new();
    for entry in &entries {
        let digest = entry_digest(&entry.kind, &entry.text);
        let mac = seal::sealed(&entry_kind(&entry.kind), &place.bind, &entry.text)
            .ok_or_else(cannot_seal)?;
        files.push((
            place
                .folder
                .join(&entry.kind)
                .join(format!("{digest}.json")),
            serde_json::to_string(&EntryFile {
                kind: entry.kind.clone(),
                text: entry.text.clone(),
                mac: mac.clone(),
            })
            .map_err(io::Error::other)?,
        ));
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
    let manifest_json = serde_json::to_string_pretty(&manifest).map_err(io::Error::other)?;
    // The anchor's counter first, before a single file: a keyring that fails here changes
    // nothing at all, and a state folder put back from before cannot replay past it.
    write_anchor(
        &place.account,
        &place.anchor_bind,
        Anchor {
            counter,
            nevers: count(&manifest, NEVER),
            floors: count(&manifest, FLOOR),
        },
    )?;
    match plane {
        Some((root, id)) => {
            if let Some(marker) = sealed_here_marker(root) {
                write_file(&marker, "sealed")?;
            }
            mark_sealed(root, id, &place.bind)?;
        }
        None => mark_everywhere_sealed()?,
    }
    for (file, text) in &files {
        write_file(file, text)?;
    }
    write_file(&place.folder.join("manifest.json"), &manifest_json)?;
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
    let Some(plane) = registry.planes.iter_mut().find(|plane| plane.id == id) else {
        return Err(io::Error::other("the plane is not in the registry"));
    };
    if plane.sealed && plane.clone == clone && plane.anchor == anchor {
        return Ok(());
    }
    // A plane the app moved ([`follow_move`]) is the same id at its new path: the registry
    // follows it, and drops a placeholder an earlier open made for the new path.
    plane.sealed = true;
    plane.clone = clone.to_owned();
    if plane.anchor != anchor {
        plane.anchor = anchor.clone();
        plane.real = crate::sandbox::real(root);
        plane.opened = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
        plane.git = git_identity(root);
    }
    registry
        .planes
        .retain(|other| other.id == id || other.anchor != anchor || other.sealed);
    write_registry(&mut registry)
}

/// The registry notes that something was sealed in the Everywhere entry: the backstop where its
/// anchor and folder are both taken away.
fn mark_everywhere_sealed() -> io::Result<()> {
    let _one = locked();
    let mut registry = match read_registry() {
        Read::Value(registry) => registry,
        Read::Missing => Registry::default(),
        Read::Broken | Read::Unreadable => {
            return Err(io::Error::other(
                "purlis could not read its registry of planes",
            ));
        }
    };
    if registry.everywhere_sealed {
        return Ok(());
    }
    registry.everywhere_sealed = true;
    seed_device(&mut registry);
    write_registry(&mut registry)
}

/// A folder only the person reads, made with its parents.
fn make_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)
}

/// A file written whole or not at all, in a folder only the person reads.
fn write_file(path: &Path, text: &str) -> io::Result<()> {
    use std::io::Write;
    let dir = path.parent().ok_or_else(no_home)?;
    make_dir(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.write_all(text.as_bytes())?;
    file.persist(path).map_err(|why| why.error)?;
    Ok(())
}

#[path = "answers_import.rs"]
mod import;
pub use import::{Import, Imported, import_nevers, imported_nevers, old_files};
pub(crate) use import::{Step, nevers_in_step};
#[path = "answers_move.rs"]
mod moved;
pub use moved::{Moved, follow_move};

#[cfg(test)]
#[path = "answers_tests.rs"]
mod tests;
