// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The keyring provider — the operating system's own credential store (ADR 0047): the macOS
//! Keychain, the Secret Service on Linux, and the Windows Credential Manager, through the
//! standard `keyring` crate.
//!
//! **One item per secret**, and one index file beside the registry that names them:
//!
//! ```text
//! charter vault 'ops', key 'API_TOKEN'
//!     → keyring item   service  charter/ops/3f9a2c1b   (made once per vault, random)
//!                      account  API_TOKEN
//!     → keys index     .charter/vaults/ops.keys.json:
//!                      {"service": "charter/ops/3f9a2c1b",
//!                       "keys": {"API_TOKEN": {"size": "16–31 bytes",
//!                                              "updated": "2026-09-24T11:32:17Z"}}}
//! ```
//!
//! **The index is how a keyring vault is listed**, because a keyring cannot be asked what it
//! holds — the `keyring` crate reads, writes and deletes one named entry and nothing else. It
//! holds each key's name, its size BAND and when it was last written, and never a value. It is
//! written after every write that succeeded, 0600, under the state directory — which is
//! gitignored, so nothing here lands in a committed path.
//!
//! **The service is random per vault** (`charter/<vault>/<8 hex>`), made at the vault's first
//! write and kept in its index. A vault name is only unique within one plane, and the keyring is
//! the machine's: two planes each with a vault called `ops` would otherwise read, overwrite and
//! delete each other's items while each index claimed its own. A service the index names is
//! refused unless it is one of charter's (`charter/…`): the index is a file on disk, and one
//! pointing at another program's item would make charter a deputy for reading it.
//!
//! **Which store a build talks to** is decided once, here ([`store`]): a fenced build — every
//! test build, and the app's `e2e` build (`crate::fence`) — keeps values in
//! `<state>/keyring-stub.json` (the state directory: `.charter/`, or `$CHARTER_HOME`) and never
//! reaches the operating system's store. So no test in this repository can read or write the
//! operator's login keychain, however it is written.
//!
//! **On macOS every item is held to charter's app** (ruling V90a, [`super::keyhold`]): written
//! by the app's own binary, so the Keychain's default rule lets that binary alone read it
//! without asking, and any other program, the `charter` command and every program a chat runs
//! included, is refused or makes the Keychain ask the person. The index marks each key whose
//! item was written that way (`held`). An item written before is written again the same way
//! the next time charter reads it (one the `charter` command made, the next time the command
//! does), and the vault's next read through the command says so once (ruling V90d).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Map, Value};

use super::registry::Vault;
use super::{Ctx, VaultError, fingerprint};
use crate::names::KEYCHAIN_PREFIX;

/// Every service charter makes now starts with this (`purlis/`, #1261).
pub const SERVICE_PREFIX: &str = KEYCHAIN_PREFIX.write;

/// The prefixes a service charter reads or holds may start with: [`SERVICE_PREFIX`], and the
/// old `charter/` while the rename window lasts (V93l). Exactly these, a security boundary: a
/// service under any other is not charter's, and charter is never a deputy for it. Written out,
/// not derived from every spelling the name module knows, so a name recognised only for
/// history can never widen it.
pub const OWN_PREFIXES: [&str; 2] = [KEYCHAIN_PREFIX.write, KEYCHAIN_PREFIX.reads[0]];

/// What follows the product's own prefix in `service`, or `None` when it has neither.
fn own_tail(service: &str) -> Option<&str> {
    OWN_PREFIXES.iter().find_map(|p| service.strip_prefix(p))
}

/// Whether `id` can be the last segment of a service: not empty, one segment, no control
/// character.
fn id_ok(id: &str) -> bool {
    !id.is_empty() && !id.contains('/') && !id.chars().any(char::is_control)
}

/// Whether `service` names an item charter makes: `<own prefix><vault>/<id>` for a vault
/// [`registry::name_ok`] accepts, or `<own prefix>@identity/<id>` for a moved identity
/// ([`super::identity`]); under one of [`OWN_PREFIXES`] and nothing else. What the app's
/// one-item writer ([`super::keyhold`]) checks before it writes anything.
pub fn own_service(service: &str) -> bool {
    let Some((owner, id)) = own_tail(service).and_then(|rest| rest.split_once('/')) else {
        return false;
    };
    (owner == super::identity::OWNER || super::registry::name_ok(owner)) && id_ok(id)
}

/// The file a fenced build keeps its keyring in, under the state directory. Plaintext, and only
/// ever written by a build that a test or a scenario run made.
pub const STUB_FILE: &str = "keyring-stub.json";

/// How the command line names the store, the same words on every platform: what charter prints
/// is recorded, and the recording is replayed on macOS and Linux alike.
pub const STORE_NAME: &str = "the system keyring";

/// A value read out of a store. Its `Debug` never prints it.
pub struct Secret(String);

impl Secret {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

/// What a write left an item under (ruling V90a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// Held to charter's app: any other program is refused or makes the store ask the person.
    ToTheApp,
    /// This store keeps no rule charter sets: the test stub, the Secret Service and the
    /// Credential Manager.
    NoRule,
    /// This store has a rule and the write could not set it, so the item has the access the
    /// writing program gave it. The next read tries again.
    NotYet,
}

/// Whether a store holds an entry, as [`Store::exists`] answers without reading its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// The store holds it.
    Present,
    /// The store provably holds none.
    Absent,
    /// The store cannot say without reading the value, or did not say.
    Unknown,
}

/// A credential store: one entry per `(service, account)`, read, written and deleted by name.
///
/// **An error never carries a value**, and never an entry the store returned: an
/// implementation says which operation failed and the store's own reason, which names a
/// failure and not a credential.
pub trait Store: Send + Sync {
    /// The entry, or `None` when the store PROVABLY has none.
    fn get(&self, service: &str, account: &str) -> Result<Option<Secret>, VaultError>;
    /// Create or replace the entry, held to charter's app where the store can hold it.
    fn set(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError>;
    /// Delete the entry. `false` when there was none.
    fn delete(&self, service: &str, account: &str) -> Result<bool, VaultError>;
    /// Whether the entry is there, asked of its attributes and **never of its value** (#1660),
    /// so asking it never makes the store ask the person. [`Presence::Unknown`] where this
    /// store cannot say without reading the value, or did not say.
    fn exists(&self, _service: &str, _account: &str) -> Presence {
        Presence::Unknown
    }
    /// Whether this store holds an item to charter's app, so an item written before it did is
    /// written again when read (ruling V90d).
    fn holds(&self) -> bool {
        false
    }
    /// The entry's value, unchanged, written again held to charter's app (ruling V90d). Where it
    /// cannot be held, a store may leave the item as it is rather than write it in place.
    fn rehold(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.set(service, account, value)
    }
    /// The entry made as a fresh item, the way the keychain copy writes (RN-6): where this
    /// store holds items to charter's app, an item another program owns is an error and nothing
    /// is written into it, never a write in place under that program's access.
    fn make_fresh(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.set(service, account, value)
    }
    /// The entry made as a fresh item by THIS process, never through the app's writer (#1306):
    /// what a terminal's keychain copy writes, so that it reads its copy back without anyone
    /// being asked. Where this store holds items, an item there that is not this program's is
    /// an error and nothing is written into it; the answer is [`Held::ToTheApp`] in the app and
    /// [`Held::NotYet`] anywhere else.
    fn make_own(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.make_fresh(service, account, value)
    }
    /// Forget what this process remembers of the entry, so the next read asks the store again
    /// (#2656). A store that remembers nothing has nothing to forget.
    fn forget(&self, _service: &str, _account: &str) {}
}

/// The store this build talks to. See the module header: a fenced build never reaches the
/// operating system's.
pub fn store(ctx: &Ctx) -> Box<dyn Store> {
    if crate::fence::FENCED {
        Box::new(FileStore::at(ctx.state.join(STUB_FILE)))
    } else {
        Box::new(OsStore)
    }
}

// ---------------------------------------------------------------------------------------
// The operating system's store.

/// The platform's own store, through `keyring`'s one-entry API: the login keychain on macOS,
/// the Secret Service on Linux, the Credential Manager on Windows.
///
/// **On macOS an item is held to charter's app** (ruling V90a): it is written by the app's own
/// binary ([`super::keyhold::set`]), and the Keychain's default rule lets the program that
/// created an item read it without asking. Any other program — `security
/// find-generic-password -w`, the `charter` command, a script a chat runs — is refused or
/// makes the Keychain ask the person first. ADR 0047 records what was measured and what that
/// does and does not protect.
pub struct OsStore;

impl std::fmt::Debug for OsStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OsStore")
    }
}

impl OsStore {
    fn entry(service: &str, account: &str) -> Result<::keyring::Entry, VaultError> {
        ::keyring::Entry::new(service, account).map_err(|e| failure("reach", service, &e))
    }
}

impl Store for OsStore {
    fn get(&self, service: &str, account: &str) -> Result<Option<Secret>, VaultError> {
        match Self::entry(service, account)?.get_password() {
            Ok(value) => Ok(Some(Secret(value))),
            Err(::keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(failure("read", service, &e)),
        }
    }

    fn set(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        if cfg!(target_os = "macos") {
            return super::keyhold::set(service, account, value, super::keyhold::Why::Write);
        }
        set_here(service, account, value)?;
        Ok(Held::NoRule)
    }

    fn delete(&self, service: &str, account: &str) -> Result<bool, VaultError> {
        // A spare a cut-short write left holds the same value (#1309): it goes with the item.
        // Best effort, as there is usually none.
        if cfg!(target_os = "macos") {
            for spare in super::keyhold::spares_of(service) {
                if let Ok(entry) = ::keyring::Entry::new(&spare, account) {
                    let _ = entry.delete_credential();
                }
            }
        }
        match Self::entry(service, account)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(::keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(failure("delete", service, &e)),
        }
    }

    fn exists(&self, service: &str, account: &str) -> Presence {
        exists_here(service, account)
    }

    fn holds(&self) -> bool {
        cfg!(target_os = "macos")
    }

    fn rehold(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        if cfg!(target_os = "macos") {
            return super::keyhold::set(service, account, value, super::keyhold::Why::Move);
        }
        Ok(Held::NoRule)
    }

    fn make_fresh(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        if cfg!(target_os = "macos") {
            return super::keyhold::set(service, account, value, super::keyhold::Why::Copy);
        }
        self.set(service, account, value)
    }

    fn make_own(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        if cfg!(target_os = "macos") {
            return super::keyhold::make_own(service, account, value);
        }
        self.set(service, account, value)
    }
}

/// Create or replace the entry from this process, with whatever access the store gives an
/// item this program writes (on macOS: this program alone reads a new one without asking, and
/// a replaced one keeps the access it had).
pub(super) fn set_here(service: &str, account: &str, value: &str) -> Result<(), VaultError> {
    OsStore::entry(service, account)?
        .set_password(value)
        .map_err(|e| failure("write", service, &e))
}

/// Whether the login keychain holds a generic password under `service` and `account`, from a
/// search that returns the item's attributes and not its data (#1660). The Keychain decrypts
/// nothing for it, so it has nothing to ask the person about: the same question as `security
/// find-generic-password` without `-w`. Any answer but found or not found is unknown.
#[cfg(target_os = "macos")]
fn exists_here(service: &str, account: &str) -> Presence {
    use security_framework::item::{ItemClass, ItemSearchOptions, Limit};
    let found = ItemSearchOptions::new()
        .class(ItemClass::generic_password())
        .service(service)
        .account(account)
        .load_attributes(true)
        .limit(Limit::Max(1))
        .search();
    match found {
        Ok(items) if items.is_empty() => Presence::Absent,
        Ok(_) => Presence::Present,
        Err(e) if e.code() == ITEM_NOT_FOUND => Presence::Absent,
        Err(_) => Presence::Unknown,
    }
}

/// Elsewhere purlis asks nothing it does not need: the store reads as it did before #1660.
#[cfg(not(target_os = "macos"))]
fn exists_here(_service: &str, _account: &str) -> Presence {
    Presence::Unknown
}

/// `errSecItemNotFound`: a search found no item.
#[cfg(target_os = "macos")]
const ITEM_NOT_FOUND: i32 = -25300;

/// A keyring failure as charter says it: the operation, the service, and the store's reason.
///
/// **Never `e`'s `Display` wholesale.** `BadEncoding` carries the bytes the store returned —
/// the credential itself — and prints nothing of them only by the crate's grace; it is named
/// here and not formatted. The other kinds carry the platform's own error, which names a
/// failure ("User interaction is not allowed.") and not a value.
pub(super) fn failure(what: &str, service: &str, e: &::keyring::Error) -> VaultError {
    let why = match e {
        ::keyring::Error::BadEncoding(_) => "the stored entry is not UTF-8 text".to_string(),
        ::keyring::Error::NoStorageAccess(inner) | ::keyring::Error::PlatformFailure(inner) => {
            inner.to_string()
        }
        ::keyring::Error::NoDefaultStore => "this platform has no keyring purlis can use".into(),
        ::keyring::Error::TooLong(name, _) => format!("the {name} is longer than the store allows"),
        ::keyring::Error::Invalid(name, reason) => format!("the {name} {reason}"),
        _ => "the store refused it".into(),
    };
    let said = format!("purlis could not {what} '{service}' in {STORE_NAME}: {why}");
    if would_have_asked(e) {
        return VaultError::would_ask(said);
    }
    VaultError::new(said)
}

/// `errSecInteractionNotAllowed`: the Keychain would have asked the person, and its dialogs are
/// off ([`DialogsOff`]).
#[cfg(target_os = "macos")]
const INTERACTION_NOT_ALLOWED: i32 = -25308;

/// Whether the store refused only because it would have asked the person.
#[cfg(target_os = "macos")]
fn would_have_asked(e: &::keyring::Error) -> bool {
    match e {
        ::keyring::Error::NoStorageAccess(inner) | ::keyring::Error::PlatformFailure(inner) => {
            inner
                .downcast_ref::<security_framework::base::Error>()
                .is_some_and(|e| e.code() == INTERACTION_NOT_ALLOWED)
        }
        _ => false,
    }
}

#[cfg(not(target_os = "macos"))]
fn would_have_asked(_e: &::keyring::Error) -> bool {
    false
}

/// The Keychain's dialogs, off for this process while this lives, and as they were after
/// (#1306): a read or write that would ask the person fails with [`Kind::WouldAsk`] instead.
/// Elsewhere a store has no dialogs, and this does nothing.
///
/// [`Kind::WouldAsk`]: super::Kind::WouldAsk
pub struct DialogsOff {
    #[cfg(target_os = "macos")]
    _off: Option<security_framework::os::macos::keychain::KeychainUserInteractionLock>,
}

impl DialogsOff {
    /// Turn them off, or `Err` when the Keychain would not say it did.
    pub fn now() -> Result<Self, VaultError> {
        #[cfg(target_os = "macos")]
        {
            use security_framework::os::macos::keychain::SecKeychain;
            let failed = |e: security_framework::base::Error| {
                VaultError::new(format!(
                    "purlis could not turn off {STORE_NAME}'s dialogs: {e}"
                ))
            };
            // Already off (by whoever turned them off): left so, and so when this ends.
            if !SecKeychain::user_interaction_allowed().map_err(failed)? {
                return Ok(Self { _off: None });
            }
            let off = SecKeychain::disable_user_interaction().map_err(failed)?;
            Ok(Self { _off: Some(off) })
        }
        #[cfg(not(target_os = "macos"))]
        {
            Ok(Self {})
        }
    }
}

// ---------------------------------------------------------------------------------------
// The stub a fenced build keeps.

/// A keyring in one 0600 JSON file, for fenced builds only: `{"<service>\n<account>": value}`.
pub struct FileStore {
    path: PathBuf,
}

impl std::fmt::Debug for FileStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileStore")
            .field("path", &self.path)
            .finish()
    }
}

impl FileStore {
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    fn slot(service: &str, account: &str) -> String {
        format!("{service}\n{account}")
    }

    fn load(&self) -> Result<Map<String, Value>, VaultError> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(Value::Object(map)) => Ok(map),
                // Named by path and never quoted: the file holds values.
                _ => Err(VaultError::new(format!(
                    "the test keyring {} is not a JSON object",
                    self.path.display()
                ))),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
            Err(e) => Err(VaultError::new(format!(
                "the test keyring {} cannot be read: {e}",
                self.path.display()
            ))),
        }
    }

    fn save(&self, map: Map<String, Value>) -> Result<(), VaultError> {
        super::plain_file::write_private(&self.path, &Value::Object(map))
    }
}

/// Whether `vault`'s values come from the keyring: a `keyring` vault's own, or a vault whose
/// token is kept there ([`super::identity::in_keyring`]). From the registry alone: the keyring
/// itself is not read, so asking never makes it ask the person (#1638).
pub fn holds_values_of(ctx: &Ctx, vault: &Vault) -> bool {
    vault.provider == "keyring" || super::identity::in_keyring(ctx, vault)
}

/// How many times a fenced build's stub at each path was read for an item: what a test counts
/// to say how often a run would make the Keychain ask the person (#1638). Per path, so
/// tests running at once in one process never count each other's reads.
#[cfg(feature = "fenced")]
static STUB_READS: std::sync::Mutex<BTreeMap<PathBuf, usize>> =
    std::sync::Mutex::new(BTreeMap::new());

/// The items read so far from the stub at `path` ([`STUB_READS`]).
#[cfg(feature = "fenced")]
pub fn stub_reads(path: &std::path::Path) -> usize {
    STUB_READS
        .lock()
        .map_or(0, |reads| reads.get(path).copied().unwrap_or(0))
}

/// How many times a fenced build's stub at each path was asked whether an item is there
/// ([`Store::exists`]): on macOS that is a question of its own, so a test counts it beside the
/// reads (#2656).
#[cfg(feature = "fenced")]
static STUB_ASKS: std::sync::Mutex<BTreeMap<PathBuf, usize>> =
    std::sync::Mutex::new(BTreeMap::new());

/// The questions asked so far of the stub at `path` ([`STUB_ASKS`]).
#[cfg(feature = "fenced")]
pub fn stub_asks(path: &std::path::Path) -> usize {
    STUB_ASKS
        .lock()
        .map_or(0, |asks| asks.get(path).copied().unwrap_or(0))
}

impl Store for FileStore {
    fn get(&self, service: &str, account: &str) -> Result<Option<Secret>, VaultError> {
        #[cfg(feature = "fenced")]
        if let Ok(mut reads) = STUB_READS.lock() {
            *reads.entry(self.path.clone()).or_default() += 1;
        }
        Ok(self
            .load()?
            .get(&Self::slot(service, account))
            .and_then(Value::as_str)
            .map(|v| Secret(v.to_owned())))
    }

    fn set(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        let mut map = self.load()?;
        map.insert(
            Self::slot(service, account),
            Value::String(value.to_owned()),
        );
        self.save(map)?;
        Ok(Held::NoRule)
    }

    /// From the file's slots, and not counted as a read ([`STUB_READS`]): the question reads no
    /// value. It is counted as a question ([`STUB_ASKS`]).
    fn exists(&self, service: &str, account: &str) -> Presence {
        #[cfg(feature = "fenced")]
        if let Ok(mut asks) = STUB_ASKS.lock() {
            *asks.entry(self.path.clone()).or_default() += 1;
        }
        match self.load() {
            Ok(map) if map.contains_key(&Self::slot(service, account)) => Presence::Present,
            Ok(_) => Presence::Absent,
            Err(_) => Presence::Unknown,
        }
    }

    fn delete(&self, service: &str, account: &str) -> Result<bool, VaultError> {
        let mut map = self.load()?;
        if map.shift_remove(&Self::slot(service, account)).is_none() {
            return Ok(false);
        }
        self.save(map)?;
        Ok(true)
    }
}

// ---------------------------------------------------------------------------------------
// The keys index.

/// What the index says about one key. Never its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub key: String,
    /// The value's size band (`fingerprint::size_band`), never its length.
    pub size: String,
    /// When it was last written, as RFC 3339 UTC to the second.
    pub updated: String,
}

/// What the index records about one key: its size band, when it was last written, and whether
/// its item was written held to charter's app (ruling V90a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub size: String,
    pub updated: String,
    pub held: bool,
}

/// A vault's index: the service its items live under, what each key is, and where the vault's
/// one note that its items were moved under charter's access rule stands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Index {
    pub service: Option<String>,
    pub keys: BTreeMap<String, Entry>,
    pub note: Note,
}

/// The one note a vault gets when charter moves its items under its access rule (ruling V90d).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Note {
    /// No item was moved yet.
    #[default]
    None,
    /// An item was moved, and the note is still to be said.
    Due,
    /// It was said, and never is again.
    Said,
}

impl Note {
    fn word(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Due => Some("due"),
            Self::Said => Some("said"),
        }
    }

    fn of(word: Option<&str>) -> Self {
        match word {
            Some("due") => Self::Due,
            Some("said") => Self::Said,
            _ => Self::None,
        }
    }
}

/// The index's field saying where the vault's one note about its items' access rule stands.
const NOTE: &str = "held_note";

/// Where a keyring vault's index lives: beside the local registry, in the state directory.
pub fn index_path(ctx: &Ctx, vault: &Vault) -> PathBuf {
    ctx.vaults_dir().join(format!("{}.keys.json", vault.name))
}

/// The file whose lock is a vault's keys' where a folder cannot be locked (#1180): beside its
/// index. Not on unix, where the vaults' own folder is locked and nothing is left beside the
/// index ([`lock_keys`]).
#[cfg(not(unix))]
fn lock_path(ctx: &Ctx, vault: &str) -> PathBuf {
    ctx.vaults_dir().join(format!("{vault}.keys.lock"))
}

/// How long a write of a vault's keys waits for another writer of them: long enough for a
/// write that is waiting on the system's own question to the person, and an end all the same.
pub(super) const A_WRITE_WAITS: std::time::Duration = std::time::Duration::from_secs(10);

/// How long the move a read makes waits ([`get_with`]): the read is what was asked for, so a
/// vault busy with a write is read without moving, and the next read moves it.
const A_MOVE_WAITS: std::time::Duration = std::time::Duration::from_millis(250);

/// The vault's keys, held by this writer until the answer is dropped, or an error once `wait`
/// has passed with another writer still holding them (#1180).
///
/// **One writer of a vault's keys at a time.** Every change to its index, and every item
/// written under it, is made under this lock: `set`, `delete`, the move of an item under the
/// access rule (ruling V90d), the switch of its service and its one note. So a write is never
/// lost to another that read the index before it, and a move never writes the value it read over
/// a newer one. The lock is an advisory `flock` on the folder the vaults' indexes are kept in,
/// reached as the index is, never through a link: never the index itself, which is replaced and
/// not written in place, and no file of its own, which would be one more file in the project's
/// state for every vault. So the keyring vaults of one project take turns, which costs a write
/// nothing it would notice.
///
/// The identity writers take the same lock ([`lock_keys_named`]): a token stored, replaced or
/// deleted, and the move of a record's items, each under it from the record read to the last
/// item written.
///
/// **It never deadlocks.** Nothing that holds it takes it again, and every wait for it ends
/// ([`crate::filelock::lock_within`]). A writer refused this way writes nothing.
fn lock_keys(
    ctx: &Ctx,
    vault: &Vault,
    wait: std::time::Duration,
) -> Result<crate::filelock::Held, VaultError> {
    lock_keys_named(ctx, &vault.name, wait)
}

/// [`lock_keys`] for the vault named `vault`, for a writer that has a name and no [`Vault`].
/// On unix it is the one lock every vault of the project shares, so a holder never calls a
/// writer that takes it again.
pub(super) fn lock_keys_named(
    ctx: &Ctx,
    vault: &str,
    wait: std::time::Duration,
) -> Result<crate::filelock::Held, VaultError> {
    let refused = |e: std::io::Error| {
        VaultError::new(format!(
            "purlis could not take the keys of vault '{vault}': {e}"
        ))
    };
    let dir = ctx.vaults_dir();
    crate::contain::no_link_on_the_way(ctx.trust(), &dir).map_err(refused)?;
    super::make_private_dir(&dir).map_err(refused)?;
    #[cfg(unix)]
    let file = crate::contain::nofollow(std::fs::OpenOptions::new().read(true))
        .open(&dir)
        .map_err(refused)?;
    #[cfg(not(unix))]
    let file =
        crate::contain::create_no_link(ctx.trust(), &lock_path(ctx, vault)).map_err(refused)?;
    crate::filelock::lock_within(file, wait, &format!("the keys of vault '{vault}'"))
        .map_err(refused)
}

/// The vault's index. A missing file is a vault nothing has been written to.
pub fn load_index(ctx: &Ctx, vault: &Vault) -> Result<Index, VaultError> {
    let p = index_path(ctx, vault);
    // Never through a link, on the way from the plane or at the index (#440).
    let text = match crate::contain::read_text_no_link(ctx.trust(), &p) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Index::default()),
        Err(e) => {
            return Err(VaultError::new(format!(
                "the keys index {} cannot be read: {e}",
                p.display()
            )));
        }
    };
    let corrupt = |why: &str| {
        VaultError::new(format!(
            "the keys index {} is corrupt: {why}",
            super::short_path(&ctx.root, &p)
        ))
    };
    let doc: Value =
        serde_json::from_str(&text).map_err(|e| corrupt(&super::registry::py_json_error(&e)))?;
    let service = match doc.get("service") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if service_ok_for(s, &vault.name) => Some(s.clone()),
        Some(_) => {
            return Err(corrupt(&format!(
                "its service is not one purlis writes for vault '{}' (they are \
                 '{SERVICE_PREFIX}{}/<id>'), and purlis will not read a keyring item this vault \
                 does not own",
                vault.name, vault.name
            )));
        }
    };
    let mut keys = BTreeMap::new();
    for (k, v) in doc
        .get("keys")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        let field = |name: &str| {
            v.get(name)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        keys.insert(
            k.clone(),
            Entry {
                size: field("size"),
                updated: field("updated"),
                held: v.get("held").and_then(Value::as_bool).unwrap_or(false),
            },
        );
    }
    let note = Note::of(doc.get(NOTE).and_then(Value::as_str));
    Ok(Index {
        service,
        keys,
        note,
    })
}

pub(super) fn save_index(ctx: &Ctx, vault: &Vault, index: &Index) -> Result<(), VaultError> {
    let mut keys = Map::new();
    for (k, e) in &index.keys {
        let mut entry = Map::new();
        entry.insert("size".into(), Value::String(e.size.clone()));
        entry.insert("updated".into(), Value::String(e.updated.clone()));
        // Written only when true, so a vault in a store with no rule reads as it always has.
        if e.held {
            entry.insert("held".into(), Value::Bool(true));
        }
        keys.insert(k.clone(), Value::Object(entry));
    }
    let mut doc = Map::new();
    doc.insert(
        "service".into(),
        index.service.clone().map_or(Value::Null, Value::String),
    );
    doc.insert("keys".into(), Value::Object(keys));
    if let Some(word) = index.note.word() {
        doc.insert(NOTE.into(), Value::String(word.into()));
    }
    let p = index_path(ctx, vault);
    // The index lives in the state directory, so it is gated from the plane (#440): a
    // `.charter/` that is itself a link is refused, not written through.
    crate::contain::no_link_on_the_way(ctx.trust(), &p)
        .map_err(|e| VaultError::new(format!("cannot write {}: {e}", p.display())))?;
    super::plain_file::write_private(&p, &Value::Object(doc))
}

/// Whether `service` is one charter wrote for THIS vault: `purlis/<vault>/<id>`, and nothing
/// else. Scoped to the vault on purpose — a prefix-only check let an index name
/// `charter/identity` or another vault's `purlis/<other>/<hex>`, so a keyring vault could be
/// pointed at an item it must never read as a secret (#271 review, U4). The `<id>` is the
/// random tail [`new_service`] makes: one path segment, no control character.
///
/// During the rename window (#1261) `<vault>` may follow either of [`OWN_PREFIXES`]: an index
/// written before the rename names a `charter/` service, and its items are still found there.
pub fn service_ok_for(service: &str, vault: &str) -> bool {
    own_tail(service)
        .and_then(|rest| rest.strip_prefix(vault))
        .and_then(|rest| rest.strip_prefix('/'))
        .is_some_and(id_ok)
}

/// The purlis spelling of `service` when it is one of charter's own under the old prefix:
/// `charter/<owner>/<id>` → `purlis/<owner>/<id>`. `None` for anything else, a purlis service
/// included. What the keychain copy (RN-6, V93h) copies an item to.
pub fn renamed(service: &str) -> Option<String> {
    let tail = service.strip_prefix(KEYCHAIN_PREFIX.reads[0])?;
    let new = format!("{SERVICE_PREFIX}{tail}");
    (own_service(service) && own_service(&new)).then_some(new)
}

/// Point the vault's index at `to` instead of `from`, and nothing else in it: `false`, with the
/// index untouched, when it does not name `from` now, or when its keys are not exactly `keys`
/// (each key's `updated`, as they were when its items were copied): a secret written since is
/// under `from` alone, and must never be left behind. Both services must be this vault's own
/// ([`service_ok_for`]). The keychain copy's switch (RN-6), and its undo's.
pub fn switch_service(
    ctx: &Ctx,
    vault: &Vault,
    from: &str,
    to: &str,
    keys: &BTreeMap<String, String>,
) -> Result<bool, VaultError> {
    if !service_ok_for(from, &vault.name) || !service_ok_for(to, &vault.name) {
        return Err(VaultError::new(format!(
            "vault '{}' cannot be pointed from '{from}' to '{to}': each must be its own service",
            vault.name
        )));
    }
    let _keys = lock_keys(ctx, vault, A_WRITE_WAITS)?;
    let mut index = load_index(ctx, vault)?;
    let unchanged = index.keys.len() == keys.len()
        && index
            .keys
            .iter()
            .all(|(key, entry)| keys.get(key) == Some(&entry.updated));
    if index.service.as_deref() != Some(from) || !unchanged {
        return Ok(false);
    }
    index.service = Some(to.to_owned());
    save_index(ctx, vault, &index)?;
    Ok(true)
}

/// Whether `key` can name a keyring item: not empty, and no control character.
fn key_ok(key: &str) -> bool {
    !key.is_empty() && !key.chars().any(char::is_control)
}

pub(super) fn check_key(key: &str) -> Result<(), VaultError> {
    if key_ok(key) {
        Ok(())
    } else {
        Err(VaultError::new(
            "a keyring secret needs a key name that is not empty and holds no control character",
        ))
    }
}

/// A new service for `vault`: `purlis/<vault>/<8 hex>`.
pub(super) fn new_service(vault: &Vault) -> Result<String, VaultError> {
    let mut bytes = [0u8; 4];
    getrandom::fill(&mut bytes)
        .map_err(|e| VaultError::new(format!("purlis could not make a random name: {e}")))?;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("{SERVICE_PREFIX}{}/{hex}", vault.name))
}

// ---------------------------------------------------------------------------------------
// The provider.

/// `keys`: the names the index holds, sorted.
pub fn keys(ctx: &Ctx, vault: &Vault) -> Result<Vec<String>, VaultError> {
    Ok(load_index(ctx, vault)?.keys.into_keys().collect())
}

/// Every key with its size band and when it was written — what a list of the vault shows.
/// Read from the index alone: nothing here touches the store, so nothing here can prompt.
pub fn listed(ctx: &Ctx, vault: &Vault) -> Result<Vec<Listed>, VaultError> {
    Ok(load_index(ctx, vault)?
        .keys
        .into_iter()
        .map(|(key, e)| Listed {
            key,
            size: e.size,
            updated: e.updated,
        })
        .collect())
}

fn not_found(vault: &Vault, key: &str) -> VaultError {
    VaultError::not_found(format!(
        "secret '{key}' not found in vault '{}'",
        vault.name
    ))
}

/// `get`: the value of `key`, read from the store under the vault's service.
pub fn get(ctx: &Ctx, vault: &Vault, key: &str) -> Result<String, VaultError> {
    if let Some(why) =
        super::brokered::refused_in_this_chat(ctx, vault, super::brokered::Reader::Other)
    {
        return Err(VaultError::new(why));
    }
    get_with(&*store(ctx), ctx, vault, key)
}

/// [`get`] against a store the caller chose.
pub fn get_with(
    store: &dyn Store,
    ctx: &Ctx,
    vault: &Vault,
    key: &str,
) -> Result<String, VaultError> {
    check_key(key)?;
    let index = load_index(ctx, vault)?;
    let Some(service) = index.service.clone() else {
        return Err(not_found(vault, key));
    };
    let unheld = |index: &Index| {
        index
            .keys
            .get(key)
            .filter(|entry| index.service.as_deref() == Some(service.as_str()) && !entry.held)
            .map(|entry| entry.updated.clone())
    };
    // An item to move is read under the vault's lock, and its index entry read again under it:
    // the value moved is then the newest there is, and no write lands between the read and the
    // move (#1180). A vault busy with a write is read as it is, and moved at a later read.
    let moving = (store.holds() && unheld(&index).is_some())
        .then(|| lock_keys(ctx, vault, A_MOVE_WAITS).ok())
        .flatten();
    let read_at = moving
        .as_ref()
        .and_then(|_| load_index(ctx, vault).ok())
        .and_then(|now| unheld(&now));
    let value = store
        .get(&service, key)?
        .map(Secret::into_inner)
        .ok_or_else(|| not_found(vault, key))?;
    if let Some(read_at) = read_at {
        rehold(store, ctx, vault, &service, key, &value, &read_at);
    }
    drop(moving);
    Ok(value)
}

/// Ruling V90d: an item written before the store held it is written again, held, on its first
/// read. Best-effort: the read already succeeded, and a failure tries again next time.
///
/// **Never over a newer write.** The caller holds the vault's lock ([`lock_keys`]) from before
/// its read to after this, so no `secret set` of this build lands in between (#1180). The
/// checks stay, for a writer that does not take the lock (an older build's): the index is read
/// again first and the move is made only while the key's `updated` is still `read_at`;
/// afterwards only `held` and the note change, and only while it still is.
fn rehold(
    store: &dyn Store,
    ctx: &Ctx,
    vault: &Vault,
    service: &str,
    key: &str,
    value: &str,
    read_at: &str,
) {
    let unchanged = |index: &Index| {
        index.service.as_deref() == Some(service)
            && index
                .keys
                .get(key)
                .is_some_and(|entry| !entry.held && entry.updated == read_at)
    };
    if !load_index(ctx, vault).is_ok_and(|index| unchanged(&index)) {
        return;
    }
    if store.rehold(service, key, value) != Ok(Held::ToTheApp) {
        return;
    }
    // A newer write that landed during the move is its own: it is marked as it was written,
    // never as held by this move.
    let Ok(mut index) = load_index(ctx, vault) else {
        return;
    };
    match index.keys.get_mut(key) {
        Some(entry) if entry.updated == read_at => entry.held = true,
        _ => return,
    }
    if index.note == Note::None {
        index.note = Note::Due;
    }
    let _ = save_index(ctx, vault, &index);
}

/// Whether `key`'s item was just written where its store holds items to charter's app and
/// could not be held this time ([`Held::NotYet`]), as its index says.
pub fn left_unheld(ctx: &Ctx, vault: &Vault, key: &str) -> bool {
    left_unheld_in(&*store(ctx), ctx, vault, key)
}

/// [`left_unheld`] against a store the caller chose.
pub fn left_unheld_in(store: &dyn Store, ctx: &Ctx, vault: &Vault, key: &str) -> bool {
    store.holds()
        && load_index(ctx, vault)
            .ok()
            .and_then(|index| index.keys.get(key).map(|entry| !entry.held))
            .unwrap_or(false)
}

/// What holding a vault's items again came to ([`hold_again_with`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeldAgain {
    /// How many keys the vault has.
    pub keys: usize,
    /// How many of them are held to this app now (or have no item to hold).
    pub held: usize,
    /// How many could not be read or written without the system asking the person: left
    /// exactly as they were, for the person to finish.
    pub would_ask: usize,
    /// Why each of the others was not held, said. Each is left as it was.
    pub failed: Vec<String>,
}

/// Every key of `vault` held again by this app (RN-9): what the app does at its first launch
/// under a new identity, whose code signature the items' access does not name.
///
/// **One key at a time, and only after its value was read.** A key is read, then written again
/// through [`Store::rehold`] (the ruling V90d move: deleted and made again by the app, add-only).
/// Its index entry changes only for that key and only by what the write answered: held to the
/// app, or not held, so the next read through the command tries again quietly. A key that
/// could not be read, or whose write would have asked the person ([`Kind::WouldAsk`]), is left
/// exactly as it was, and the next key is tried. A newer write of a key since it was read is
/// never undone.
///
/// [`Kind::WouldAsk`]: super::Kind::WouldAsk
pub fn hold_again_with(
    store: &dyn Store,
    ctx: &Ctx,
    vault: &Vault,
) -> Result<HeldAgain, VaultError> {
    let index = load_index(ctx, vault)?;
    let mut out = HeldAgain {
        keys: index.keys.len(),
        ..HeldAgain::default()
    };
    let Some(service) = index.service.clone() else {
        out.held = out.keys;
        return Ok(out);
    };
    if !store.holds() {
        out.held = out.keys;
        return Ok(out);
    }
    let would_ask = |e: &VaultError| e.kind == super::Kind::WouldAsk;
    for (key, entry) in &index.keys {
        // Read, checked, moved and marked under the vault's lock (#1180), one key at a time.
        let _keys = match lock_keys(ctx, vault, A_WRITE_WAITS) {
            Ok(held) => held,
            Err(e) => {
                out.failed.push(format!("'{key}': {}", e.message));
                continue;
            }
        };
        let value = match store.get(&service, key) {
            Ok(Some(value)) => value.into_inner(),
            Ok(None) => {
                out.held += 1;
                continue;
            }
            Err(e) if would_ask(&e) => {
                out.would_ask += 1;
                continue;
            }
            Err(e) => {
                out.failed.push(format!("'{key}': {}", e.message));
                continue;
            }
        };
        let still = |index: &Index| {
            index.service.as_deref() == Some(service.as_str())
                && index
                    .keys
                    .get(key)
                    .is_some_and(|now| now.updated == entry.updated)
        };
        // Written since it was read: that write is the newer one, made by this app's writer.
        if !load_index(ctx, vault).is_ok_and(|index| still(&index)) {
            out.held += 1;
            continue;
        }
        let held = match store.rehold(&service, key, &value) {
            Ok(held) => held == Held::ToTheApp,
            Err(e) if would_ask(&e) => {
                out.would_ask += 1;
                continue;
            }
            Err(e) => {
                out.failed.push(format!("'{key}': {}", e.message));
                continue;
            }
        };
        if held {
            out.held += 1;
        }
        let mut now = load_index(ctx, vault)?;
        if still(&now)
            && let Some(marked) = now.keys.get_mut(key)
            && marked.held != held
        {
            marked.held = held;
            save_index(ctx, vault, &now)?;
        }
    }
    Ok(out)
}

/// What `secret set` says when the item it wrote could not be held to charter's app.
pub fn unheld_sentence(key: &str) -> String {
    format!(
        "purlis could not hold '{key}' to its app, so the program that wrote it reads it \
         without asking you. purlis tries again the next time the purlis command reads it."
    )
}

/// The one note that this vault's items were moved under charter's access rule (ruling V90d),
/// once: `None` after it was taken, and until an item was moved.
pub fn take_note(ctx: &Ctx, vault: &Vault) -> Option<String> {
    if load_index(ctx, vault).ok()?.note != Note::Due {
        return None;
    }
    // Said once: taken under the vault's lock, or left for the next read to say.
    let _keys = lock_keys(ctx, vault, A_MOVE_WAITS).ok()?;
    let mut index = load_index(ctx, vault).ok()?;
    if index.note != Note::Due {
        return None;
    }
    index.note = Note::Said;
    save_index(ctx, vault, &index).ok()?;
    Some(format!(
        "purlis moved the secrets of vault '{}' under its access rule in {STORE_NAME}: from \
         now on only purlis's app reads them without asking you. Any other program, this \
         command included, makes the system ask you first.",
        vault.name
    ))
}

/// `set`: the value into the store, then its size band and time into the index.
pub fn set(ctx: &Ctx, vault: &Vault, key: &str, value: &str) -> Result<(), VaultError> {
    set_with(&*store(ctx), ctx, vault, key, value, &now())
}

/// [`set`] against a store the caller chose, at a time the caller chose.
pub fn set_with(
    store: &dyn Store,
    ctx: &Ctx,
    vault: &Vault,
    key: &str,
    value: &str,
    at: &str,
) -> Result<(), VaultError> {
    check_key(key)?;
    let _keys = lock_keys(ctx, vault, A_WRITE_WAITS)?;
    let mut index = load_index(ctx, vault)?;
    let service = match &index.service {
        Some(s) => s.clone(),
        None => {
            // Kept BEFORE the first item is written: an item under a service no index records
            // is one nothing will ever find again.
            let made = new_service(vault)?;
            index.service = Some(made.clone());
            save_index(ctx, vault, &index)?;
            made
        }
    };
    let held = store.set(&service, key, value)?;
    index.keys.insert(
        key.to_owned(),
        Entry {
            size: fingerprint::size_band(value),
            updated: at.to_owned(),
            held: held == Held::ToTheApp,
        },
    );
    save_index(ctx, vault, &index)
}

/// `delete`: the item, then its line in the index. A key the index does not hold is not
/// found, whatever the store holds under that name.
pub fn delete(ctx: &Ctx, vault: &Vault, key: &str) -> Result<(), VaultError> {
    delete_with(&*store(ctx), ctx, vault, key)
}

/// [`delete`] against a store the caller chose.
pub fn delete_with(
    store: &dyn Store,
    ctx: &Ctx,
    vault: &Vault,
    key: &str,
) -> Result<(), VaultError> {
    check_key(key)?;
    let _keys = lock_keys(ctx, vault, A_WRITE_WAITS)?;
    let mut index = load_index(ctx, vault)?;
    let (Some(service), true) = (index.service.clone(), index.keys.contains_key(key)) else {
        return Err(not_found(vault, key));
    };
    store.delete(&service, key)?;
    index.keys.remove(key);
    save_index(ctx, vault, &index)
}

/// `ages`: key → days since it was last written, `None` where the index has no readable time.
pub fn ages(
    ctx: &Ctx,
    vault: &Vault,
    today: chrono::NaiveDate,
) -> Result<Vec<(String, Option<i64>)>, VaultError> {
    Ok(listed(ctx, vault)?
        .into_iter()
        .map(|l| {
            let age = chrono::DateTime::parse_from_rfc3339(&l.updated)
                .ok()
                .map(|t| (today - t.date_naive()).num_days());
            (l.key, age)
        })
        .collect())
}

/// `health`: the count and where the items are, from the index alone — never the store, so
/// `vault list` never makes the Keychain ask the operator anything.
pub fn health(ctx: &Ctx, vault: &Vault) -> (bool, String) {
    match load_index(ctx, vault) {
        Err(e) => (false, e.message.to_string()),
        Ok(Index { service: None, .. }) => (true, format!("no secrets yet in {STORE_NAME}")),
        Ok(Index {
            service: Some(service),
            keys,
            ..
        }) => (
            true,
            format!(
                "{} in {STORE_NAME}, service '{service}'",
                crate::counts::counted(keys.len(), "secret", "secrets")
            ),
        ),
    }
}

/// Now, as the index writes it.
pub(super) fn now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[cfg(test)]
mod tests;
