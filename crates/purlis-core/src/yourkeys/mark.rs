// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The model-key mark** (R-174, R-175, R-176, R-177, R-179, R-182, R-620, R-649; spec C-23,
//! T1a-10): an entry in your keys is a model key, for one declaration and its origins, only
//! while one sealed record on this machine says so.
//!
//! - **Where** (R-174): `<config>/model-keys/marks/<entry id>.seal` ([`path`]), in the
//!   `model-keys` folder every chat's sandbox denies both ways (`crate::sandbox`).
//! - **Sealed under a device key** (R-649): a keychain item, service
//!   `purlis/@model-key-marks/<device>` ([`SERVICE`]), account `key` ([`ACCOUNT`]), 32 random
//!   bytes made on the first mark and never backed up, so a restored mark is inert and asks to be
//!   made again ([`Unmarked::NoDeviceKey`]).
//! - **What the seal covers** (R-175, U7): the entry's id and its kind (what it resolves
//!   through; a 1b reference's target joins with that kind), the declaration's `{ "from", "id" }`,
//!   the origin set, and a **fingerprint** of the value: HMAC-SHA256 of the value under the device
//!   key, behind a prefix of its own so it never equals a seal. Never the value itself. The record
//!   is one line of JSON and one line of the seal, HMAC-SHA256 of that line's exact bytes under
//!   the device key, so a changed byte anywhere unmarks the entry.
//! - **How it changes** (R-176): [`mark`] is the one writer (R-620), called by the window's
//!   command for the key form (and later *Mark again* and the re-bind Ask) and by
//!   [`super::replace`], which seals again over the new value's fingerprint so a rotated key keeps
//!   its mark. A value written by any other path leaves the old fingerprint in the seal, so the
//!   entry reads [`Unmarked::ChangedOutside`] and the next start asks to mark it again (W13,
//!   T1a-27). A line of a kind this build doesn't read is refused in words; your keys hold only
//!   keychain entries in 1a.
//! - **How it is used** (R-177, R-179, R-182): [`read_model_key`] is the only way an entry is
//!   read as a model key; an unmarked entry is refused and no other source is tried. The binding
//!   it returns is compared at every connect by [`route_allowed`]: the same declaration (a key
//!   marked for a built-in goes through that built-in only) and [`origins_allowed`], a pure
//!   function of the route's origins and the marked ones.
//!
//! Only two things are taken from the no-2FA-seed mark (`crate::secrets::no_seed`): one window
//! command writes the record, and its folder is denied to every chat. That mark's seal of the
//! value under the plane's key is not taken (design §4.8).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use hmac::{KeyInit as _, Mac as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use subtle::ConstantTimeEq as _;
use ulid::Ulid;
use zeroize::Zeroizing;

use super::{Entry, Kind, Product, ReadError};
use crate::model_provider::Declaration;
use crate::seam::{Credentials, Files};
use crate::secrets::keyring::{Held, Secret};

/// The keychain service of the mark's device key, before `/<device>`.
pub const SERVICE: &str = "purlis/@model-key-marks";

/// The keychain account the device key is kept under.
pub const ACCOUNT: &str = "key";

/// The marks' folder, in your keys' folder.
pub const DIR: &str = "marks";

/// The lock the device key is made under, beside the machine store's.
const LOCK: &str = "model-key-marks.lock";

/// The version a record is written at; any other is not a mark this build reads.
const VERSION: u64 = 1;

/// The most a record may be.
const MOST_BYTES: u64 = 16 * 1024;

/// What the seal's input starts with, and what a value's fingerprint input starts with: two
/// domains, so a seal over a record is never the fingerprint of some value, and the reverse.
const SEALED_AS: &[u8] = b"purlis/model-key-mark/1\n";
const FINGERPRINTED_AS: &[u8] = b"purlis/model-key-value/1\n";

/// Where entry `id`'s mark is under the config home `config_root`.
pub fn path(config_root: &Path, id: Ulid) -> PathBuf {
    crate::machine::dir(config_root)
        .join(super::DIR)
        .join(DIR)
        .join(format!("{id}.seal"))
}

/// What a key is bound to: its declaration and that declaration's concrete origins, each
/// `scheme://host[:port]` as the declaration reader writes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub declaration: Product,
    pub origins: BTreeSet<String>,
}

impl Binding {
    /// What a key marked for `declaration` is bound to: its reference and its hosts.
    pub fn of(declaration: &Declaration) -> Self {
        Self {
            declaration: Product {
                from: declaration.from.word().to_owned(),
                id: declaration.id.clone(),
            },
            origins: declaration.hosts(),
        }
    }
}

/// Why an entry is not marked as a model key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unmarked {
    /// No record is kept for the entry.
    NoMark,
    /// The device key isn't in the keychain, as after a restore: every mark is inert.
    NoDeviceKey,
    /// The record doesn't verify as this entry's under the device key, or doesn't read.
    Broken(String),
    /// The record verifies and the value isn't the one marked: written by another path than the
    /// window's (W13, W60).
    ChangedOutside,
}

impl std::fmt::Display for Unmarked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoMark => f.write_str("the key isn't marked as a model key"),
            Self::NoDeviceKey => {
                f.write_str("the key's mark was made on another install, so it isn't marked here")
            }
            Self::Broken(why) => write!(f, "the key's mark doesn't verify: {why}"),
            Self::ChangedOutside => f.write_str("the key was changed outside purlis"),
        }
    }
}

/// Why [`read_model_key`] has no model key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelKeyError {
    /// The entry or its value couldn't be read.
    Read(ReadError),
    /// The entry isn't marked as a model key.
    Unmarked(Unmarked),
}

impl std::fmt::Display for ModelKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(why) => why.fmt(f),
            Self::Unmarked(why) => why.fmt(f),
        }
    }
}

/// The record's body: what the seal covers, in this order. Never the value.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    version: u64,
    entry: Ulid,
    kind: Kind,
    declaration: Product,
    origins: BTreeSet<String>,
    fingerprint: String,
}

/// HMAC-SHA256 of `parts`, one after another, under `key`, as lower-case hex.
fn hmac_hex(key: &[u8], parts: &[&[u8]]) -> String {
    let mut mac =
        hmac::Hmac::<sha2::Sha256>::new_from_slice(key).expect("HMAC takes a key of any length");
    for part in parts {
        mac.update(part);
    }
    crate::extension::hex(&mac.finalize().into_bytes())
}

/// The value's keyed fingerprint: its HMAC under the device key, behind its own prefix.
fn fingerprint(key: &[u8], value: &str) -> String {
    hmac_hex(key, &[FINGERPRINTED_AS, value.as_bytes()])
}

/// The device key's keychain service on this device.
fn service_on(device: &str) -> String {
    format!("{SERVICE}/{device}")
}

/// The device key kept in `service`, or `None` where the keychain provably holds none.
fn device_key(
    credentials: &dyn Credentials,
    service: &str,
) -> Result<Option<Zeroizing<[u8; 32]>>, String> {
    let Some(kept) = credentials
        .get(service, ACCOUNT)
        .map_err(|why| format!("the marks' device key couldn't be read ({why})"))?
    else {
        return Ok(None);
    };
    let hex = Zeroizing::new(kept.into_inner());
    crate::runnerlink::bytes_of(hex.trim())
        .map(|bytes| Some(Zeroizing::new(bytes)))
        .ok_or_else(|| "the marks' device key doesn't read as one key".to_owned())
}

/// The device key, made and kept first where the keychain holds none (R-649): under its lock,
/// so two windows marking at once make one key between them. A key held outside the app on
/// macOS is taken out again and refused, as your keys' own items are.
fn device_key_made(
    config_root: &Path,
    credentials: &dyn Credentials,
) -> Result<Zeroizing<[u8; 32]>, String> {
    let device = crate::machine::device_id(config_root)
        .map_err(|why| format!("this device has no id to keep the marks' key under: {why}"))?;
    let service = service_on(&device);
    let _held = crate::machine::Lock::named(config_root, LOCK);
    if let Some(key) = device_key(credentials, &service)? {
        return Ok(key);
    }
    let mut made = Zeroizing::new([0u8; 32]);
    getrandom::fill(made.as_mut()).map_err(|why| why.to_string())?;
    let hex = Zeroizing::new(crate::runnerlink::hex_of(made.as_ref()));
    match credentials.set(&service, ACCOUNT, &hex) {
        Ok(Held::ToTheApp | Held::NoRule) => {}
        Ok(Held::NotYet) => {
            let _ = credentials.delete(&service, ACCOUNT);
            return Err(
                "the marks' device key couldn't be held to the purlis app; mark the \
                        key in the purlis window"
                    .to_owned(),
            );
        }
        Err(why) => return Err(format!("the marks' device key couldn't be kept ({why})")),
    }
    // Read back, so the key sealed under is the one the keychain gives the next read.
    device_key(credentials, &service)?
        .filter(|kept| kept.as_ref() == made.as_ref())
        .ok_or_else(|| "the marks' device key didn't read back as it was kept".to_owned())
}

/// **Marks entry `id` as a model key for `binding`** (R-174, R-175, R-620): the one writer of
/// the record, called by the window's command alone (the key form, *Mark again*, the re-bind
/// Ask) and by [`super::replace`]. The entry must be one of your keys holding its own value,
/// for the declaration it was added for, and the binding must name an origin. The device key is
/// made on the first mark.
pub fn mark(
    config_root: &Path,
    credentials: &dyn Credentials,
    id: Ulid,
    binding: &Binding,
) -> Result<(), String> {
    let index = super::read(config_root)?;
    let Some(entry) = index.entries.iter().find(|e| e.id == id) else {
        return Err(refusal_for_unread(&index.unread, id));
    };
    if entry.product != binding.declaration {
        return Err(format!(
            "this key was added for {} {}, so purlis didn't mark it for {} {}",
            entry.product.from, entry.product.id, binding.declaration.from, binding.declaration.id
        ));
    }
    if binding.origins.is_empty() {
        return Err(format!(
            "{} {} names no origin, so purlis didn't mark a key for it",
            binding.declaration.from, binding.declaration.id
        ));
    }
    let value = match credentials.get(super::SERVICE, &id.to_string()) {
        Ok(Some(value)) => value,
        Ok(None) => return Err(ReadError::Unreadable.to_string()),
        Err(why) => return Err(why.to_string()),
    };
    let key = device_key_made(config_root, credentials)?;
    seal_into(config_root, key.as_ref(), entry, binding, &value)
}

/// Writes entry's record, sealed under `key`: the one place a mark is written, reached only
/// through [`mark`].
fn seal_into(
    config_root: &Path,
    key: &[u8],
    entry: &Entry,
    binding: &Binding,
    value: &Secret,
) -> Result<(), String> {
    let at = path(config_root, entry.id);
    let fail = |e: &dyn std::fmt::Display| format!("purlis cannot write {}: {e}", at.display());
    let body = serde_json::to_string(&Body {
        version: VERSION,
        entry: entry.id,
        kind: entry.kind,
        declaration: binding.declaration.clone(),
        origins: binding.origins.clone(),
        fingerprint: fingerprint(key, value.expose()),
    })
    .map_err(|e| fail(&e))?;
    let seal = hmac_hex(key, &[SEALED_AS, body.as_bytes()]);
    let dir = crate::machine::private_dir(config_root).map_err(|e| fail(&e))?;
    crate::profiletrust::private_dir(&dir.join(super::DIR)).map_err(|e| fail(&e))?;
    crate::profiletrust::private_dir(&dir.join(super::DIR).join(DIR)).map_err(|e| fail(&e))?;
    crate::machine::write_beside(config_root, &at, format!("{body}\n{seal}\n").as_bytes())
        .map_err(|e| fail(&e))
}

/// **Deletes entry `id`'s mark** (*Remove*'s second step, C-23): nothing to delete is done.
pub(super) fn forget(config_root: &Path, id: Ulid) -> Result<(), String> {
    let at = path(config_root, id);
    match std::fs::remove_file(&at) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("purlis cannot delete {}: {e}", at.display())),
    }
}

/// The refusal for marking `id`, which no line this build reads holds: a line of a kind it
/// doesn't read is not one purlis marks.
fn refusal_for_unread(unread: &[Value], id: Ulid) -> String {
    let wanted = id.to_string();
    if unread
        .iter()
        .any(|line| line.get("id").and_then(Value::as_str) == Some(wanted.as_str()))
    {
        "this purlis can't read this key's line in your keys, so it didn't mark it".to_owned()
    } else {
        ReadError::NotInYourKeys.to_string()
    }
}

/// The record in `bytes` as entry `id`'s under `key`, or why it isn't one.
fn verified(bytes: &[u8], key: &[u8], id: Ulid) -> Result<Body, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "it isn't text".to_owned())?;
    let (body, seal) = text
        .strip_suffix('\n')
        .and_then(|lines| lines.split_once('\n'))
        .ok_or_else(|| "it isn't a record and its seal".to_owned())?;
    let expected = hmac_hex(key, &[SEALED_AS, body.as_bytes()]);
    if !bool::from(expected.as_bytes().ct_eq(seal.as_bytes())) {
        return Err("its seal isn't this device's".to_owned());
    }
    let body: Body =
        serde_json::from_str(body).map_err(|e| format!("its record doesn't read: {e}"))?;
    if body.version != VERSION {
        return Err(format!("its record is version {}", body.version));
    }
    if body.entry != id {
        return Err("it is another entry's".to_owned());
    }
    Ok(body)
}

/// Entry's mark, verified against the value it holds now: its binding, or why it is unmarked.
/// Read through `files` and `credentials` only, the device key never made here.
fn verify(
    config_root: &Path,
    files: &dyn Files,
    credentials: &dyn Credentials,
    entry: &Entry,
    value: &Secret,
) -> Result<Binding, Unmarked> {
    let at = path(config_root, entry.id);
    let bytes = match files.read(&at) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Unmarked::NoMark),
        Err(e) => {
            return Err(Unmarked::Broken(format!(
                "purlis cannot read {}: {e}",
                at.display()
            )));
        }
    };
    if bytes.len() as u64 > MOST_BYTES {
        return Err(Unmarked::Broken(
            "it is larger than a mark can be".to_owned(),
        ));
    }
    let device = crate::machine::known_device_id(config_root).ok_or(Unmarked::NoDeviceKey)?;
    let key = device_key(credentials, &service_on(&device))
        .map_err(Unmarked::Broken)?
        .ok_or(Unmarked::NoDeviceKey)?;
    let body = verified(&bytes, key.as_ref(), entry.id).map_err(Unmarked::Broken)?;
    if body.kind != entry.kind {
        return Err(Unmarked::Broken(
            "it was made for another kind of entry".to_owned(),
        ));
    }
    let now = fingerprint(key.as_ref(), value.expose());
    if !bool::from(now.as_bytes().ct_eq(body.fingerprint.as_bytes())) {
        return Err(Unmarked::ChangedOutside);
    }
    Ok(Binding {
        declaration: body.declaration,
        origins: body.origins,
    })
}

/// **The one way an entry is read as a model key** (R-177): its line and its value, as
/// [`super::read_key`] reads them, and its mark, verified under the device key. An entry that
/// isn't marked is refused with why, and nothing else is tried: no other entry, vault or
/// variable.
pub fn read_model_key(
    config_root: &Path,
    files: &dyn Files,
    credentials: &dyn Credentials,
    id: Ulid,
) -> Result<(Secret, Binding), ModelKeyError> {
    let entry = super::read_entry(config_root, files, id).map_err(ModelKeyError::Read)?;
    let value = super::read_item(credentials, &entry).map_err(ModelKeyError::Read)?;
    let binding =
        verify(config_root, files, credentials, &entry, &value).map_err(ModelKeyError::Unmarked)?;
    Ok((value, binding))
}

/// Entry's binding where its mark verifies against `value`, for [`super::replace`].
pub(super) fn binding_for(
    config_root: &Path,
    credentials: &dyn Credentials,
    entry: &Entry,
    value: &Secret,
) -> Result<Binding, Unmarked> {
    verify(
        config_root,
        &crate::seam::RealFiles,
        credentials,
        entry,
        value,
    )
}

/// **The origin compare** (R-179): a route may use a key only where every origin it reaches is
/// one the key is marked for, and it reaches at least one. Pure: the key proxy calls it at every
/// connect. The refusal names the first origin that isn't marked, and the marked ones.
pub fn origins_allowed(route: &BTreeSet<String>, marked: &BTreeSet<String>) -> Result<(), String> {
    let marked_words = || marked.iter().cloned().collect::<Vec<_>>().join(", ");
    if route.is_empty() {
        return Err(format!(
            "the route names no origin, and the key is marked for {}",
            marked_words()
        ));
    }
    match route.iter().find(|origin| !marked.contains(*origin)) {
        Some(origin) => Err(format!(
            "the route reaches {origin}, and the key is marked for {} only",
            marked_words()
        )),
        None => Ok(()),
    }
}

/// **Whether a key bound to `marked` may be used on `route`** (R-179, R-182): the same
/// declaration, by where it comes from and its id, so a key marked for a built-in goes through
/// that built-in only; then [`origins_allowed`].
pub fn route_allowed(marked: &Binding, route: &Binding) -> Result<(), String> {
    if route.declaration != marked.declaration {
        return Err(format!(
            "the key is marked for {} {}, and the route is {} {}'s",
            marked.declaration.from,
            marked.declaration.id,
            route.declaration.from,
            route.declaration.id
        ));
    }
    origins_allowed(&route.origins, &marked.origins)
}
