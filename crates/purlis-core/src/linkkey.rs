// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **This desktop's link key** (ADR 0078 §3 and §8, #2679): the static X25519 key it links to
//! its runners with, minted with its first runner and kept in the Keyring tier, one item,
//! `purlis/@link-key/<device>` ([`SERVICE`]), its private half as 64 lower-case hex.
//!
//! - **Made once, under a lock, and only as a pool is added** ([`desktop`]): the first add on a
//!   device that keeps none makes the key from the operating system's generator and keeps it,
//!   under `link-key.lock` beside the machine store, so a window and its session host asking at
//!   once make one key between them. A link reads it and never makes one ([`kept`]).
//! - **What every runner pins**: a pairing hands a runner this key's public half with the
//!   device's id ([`Desktop::pinned`]), and the desktop's host presents the key on every link.
//!   Losing it unpairs every runner, which is made and paired again (ADR 0066, amended).
//! - **Chats never reach it**: the keyring item is written by purlis's own binary, so on macOS
//!   another program, a chat's included, makes the Keychain ask the person first (ADR 0047).

use std::path::Path;

use purlis_session_protocol::hostlink::{Identity, LinkKey, Pinned, Zeroizing};

use crate::secrets::keyring;

/// The keyring's service for this device's link key: `purlis/@link-key/<device>`.
pub const SERVICE: &str = "purlis/@link-key";

/// The keyring's account the key is kept under.
pub const ACCOUNT: &str = "key";

/// The lock a key is made under, beside the machine store.
const LOCK: &str = "link-key.lock";

/// This desktop on a link: its device id and its link key.
pub struct Desktop(Identity);

impl Desktop {
    /// Who it is on a link.
    pub fn identity(&self) -> &Identity {
        &self.0
    }

    /// What a runner pins of it at its pairing.
    pub fn pinned(&self) -> Pinned {
        Pinned {
            device: self.0.device.clone(),
            key: self.0.key.public(),
        }
    }

    pub fn into_identity(self) -> Identity {
        self.0
    }
}

/// **This desktop's device id and link key**, the key read from `store` or, where it keeps
/// none, made and kept there first. Refused where the device has no id, or the item cannot be
/// read or kept, or holds something that is not one key: a key is never made over one that
/// does not read.
pub fn desktop(config_root: &Path, store: &dyn keyring::Store) -> Result<Desktop, String> {
    let device = crate::machine::device_id(config_root)
        .map_err(|why| format!("this device has no id to link with: {why}"))?;
    let service = format!("{SERVICE}/{device}");
    // Across the keyring, which may be asking the person. Past this, a second key made alongside
    // does not read back as kept, and is refused below.
    let _held =
        crate::machine::Lock::named_within(config_root, LOCK, std::time::Duration::from_secs(300));
    let key = match read(store, &service)? {
        Some(key) => key,
        None => {
            let key = LinkKey::generate().map_err(|why| why.to_string())?;
            let private = key
                .private_half()
                .ok_or_else(|| "a link key with no private half cannot be kept".to_owned())?;
            let hex = Zeroizing::new(crate::runnerlink::hex_of(private.as_ref()));
            store
                .set(&service, ACCOUNT, &hex)
                .map_err(|why| format!("this desktop's link key could not be kept ({why})"))?;
            // Read back, so the key every runner pins is the one the keyring gives the host.
            read(store, &service)?
                .filter(|kept| kept.public() == key.public())
                .ok_or_else(|| {
                    "this desktop's link key did not read back as it was kept".to_owned()
                })?
        }
    };
    Ok(Desktop(Identity { device, key }))
}

/// **This desktop's device id and link key, as they are kept**, for a link to a runner it
/// paired: read and never made, so a key the keyring does not give is said, and no new key is
/// made in its place, which no runner pinned. Only adding a pool makes one ([`desktop`]).
pub fn kept(config_root: &Path, store: &dyn keyring::Store) -> Result<Desktop, String> {
    let device = crate::machine::known_device_id(config_root).ok_or_else(|| {
        "this machine has no device id, so it has paired no runner to link to".to_owned()
    })?;
    let key = read(store, &format!("{SERVICE}/{device}"))?.ok_or_else(|| {
        "this machine's link key is not in your keychain, so purlis links to no runner it \
         paired; remove the pool and add it again"
            .to_owned()
    })?;
    Ok(Desktop(Identity { device, key }))
}

/// The keyring this build keeps the link key in: the operating system's, or, in a fenced
/// build, a stub file in the machine store, so a test never reaches the person's keyring.
pub fn this_builds_keyring(config_root: &Path) -> Box<dyn keyring::Store> {
    if crate::fence::FENCED {
        Box::new(keyring::FileStore::at(
            crate::machine::dir(config_root).join(keyring::STUB_FILE),
        ))
    } else {
        Box::new(keyring::OsStore)
    }
}

/// The key in `service`, or `None` when the keyring provably holds none.
fn read(store: &dyn keyring::Store, service: &str) -> Result<Option<LinkKey>, String> {
    let Some(kept) = store
        .get(service, ACCOUNT)
        .map_err(|why| format!("this desktop's link key could not be read ({why})"))?
    else {
        return Ok(None);
    };
    let hex = Zeroizing::new(kept.into_inner());
    let private = crate::runnerlink::bytes_of(hex.trim())
        .map(Zeroizing::new)
        .ok_or_else(|| "this desktop's link key does not read as one key".to_owned())?;
    LinkKey::from_private(private)
        .map(Some)
        .map_err(|why| format!("this desktop's link key does not read as one key: {why}"))
}

#[cfg(test)]
mod tests;
