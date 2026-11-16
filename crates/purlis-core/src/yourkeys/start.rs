// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The start's one read of the key** (T1a-61; R-078, R-510, R-592).
//!
//! - **Once, at the start** (R-078): the window reads the entry from your keys with
//!   [`read_once`], through [`super::mark::read_model_key`], when the run starts, and hands the
//!   value to the host, which hands it to the key proxy in memory. No request reads it again: the
//!   key proxy's route holds the value. Nothing of the chat, its project or its persona reaches
//!   the read, so two chats in two projects under two personas start on one entry, each with its
//!   own read, and no grant is asked (R-172).
//! - **A locked keychain** (R-510, R-592): where the read finds the keychain locked
//!   ([`super::ReadError::Locked`]: the store would have asked the person, its dialogs off), the
//!   start makes no run and raises an Ask, W10 with *Unlock*, as `ask.raised` in
//!   [`AskGroup::ModelKey`], keyed by the entry ([`Locked`]). *Unlock* runs the OS's unlock
//!   ([`Unlocks`]) and, where it unlocks, the start reads again and goes on, with no second press.
//!   A cancelled unlock leaves the Ask as it was; a failed one leaves it with W84 under it. Neither
//!   makes a run. Where the unlock succeeds and the read is still refused (an item whose access
//!   list doesn't trust this build looks locked), the start is refused, and *Unlock* is not
//!   offered again.

use std::path::Path;

use ulid::Ulid;

use super::ReadError;
use super::mark::{Binding, ModelKeyError, read_model_key};
use crate::eventlog::stops::{AskGroup, AskRaised, Stop};
use crate::seam::{Credentials, Files};
use crate::secrets::keyring::Secret;

/// What a start's one read found.
#[derive(Debug)]
pub enum Read {
    /// The key and its mark's binding, for the host to hand the key proxy.
    Key(Secret, Binding),
    /// The keychain is locked: the start waits on the Ask.
    Locked(Locked),
    /// The start is refused, with why (not in your keys, unreadable, unmarked).
    Refused(ModelKeyError),
}

/// **A start the keychain's lock stopped**: the Ask W10 with *Unlock*, keyed by the entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locked {
    entry: Ulid,
}

impl Locked {
    /// The entry the Ask is keyed by.
    pub fn entry(&self) -> Ulid {
        self.entry
    }

    /// What the Ask says: W10.
    pub fn says(&self) -> &'static str {
        crate::window_copy::line::W10
    }

    /// What the Ask is keyed by, as the stops mapping table words it.
    pub fn keyed_by(&self) -> &'static str {
        "entry"
    }

    /// The stop the Ask is raised as: `ask.raised` in the model key's group.
    pub fn raised(&self) -> Stop {
        Stop::AskRaised(AskRaised {
            ask: AskGroup::ModelKey,
        })
    }

    /// **The Ask answered *Unlock***: the OS's unlock, run once, and where it unlocked, the
    /// start's read again, so the start goes on with no second press.
    pub fn unlock(
        self,
        unlocks: &dyn Unlocks,
        config_root: &Path,
        files: &dyn Files,
        credentials: &dyn Credentials,
    ) -> Unlocking {
        match unlocks.unlock() {
            Unlock::Cancelled => Unlocking::Cancelled(self),
            Unlock::Failed => Unlocking::Failed(self),
            Unlock::Unlocked => match read_once(config_root, files, credentials, self.entry) {
                Read::Key(value, binding) => Unlocking::Read(value, binding),
                // The keychain unlocked and still refuses the read. macOS refuses with the same
                // code where the item's access list doesn't trust this build (after a re-sign, or
                // a dev build), and no safe call in the crates in use tells a locked keychain
                // from that. Another *Unlock* would succeed and be refused again, so the start is
                // refused instead of asking again.
                Read::Locked(_) => Unlocking::Refused(ModelKeyError::Read(ReadError::Locked)),
                Read::Refused(why) => Unlocking::Refused(why),
            },
        }
    }
}

/// What the OS's unlock did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unlock {
    Unlocked,
    /// The person cancelled the OS's prompt.
    Cancelled,
    Failed,
}

/// **The OS's unlock of the keychain**: the system's own prompt. [`OsUnlock`] on this machine.
pub trait Unlocks {
    fn unlock(&self) -> Unlock;
}

/// What a start did after *Unlock*.
#[derive(Debug)]
pub enum Unlocking {
    /// The keychain unlocked and the start read the key: it goes on.
    Read(Secret, Binding),
    /// The person cancelled the unlock: the Ask stands as it was, and no run is made.
    Cancelled(Locked),
    /// The unlock failed: the Ask stands with W84 under it, and no run is made.
    Failed(Locked),
    /// The keychain unlocked and the start is refused: for another reason, or because the read
    /// is still refused as locked ([`ReadError::Locked`]), which another *Unlock* can't lift.
    Refused(ModelKeyError),
}

/// **The start's one read** of entry `entry` from your keys, as a model key.
pub fn read_once(
    config_root: &Path,
    files: &dyn Files,
    credentials: &dyn Credentials,
    entry: Ulid,
) -> Read {
    match read_model_key(config_root, files, credentials, entry) {
        Ok((value, binding)) => Read::Key(value, binding),
        Err(ModelKeyError::Read(ReadError::Locked)) => Read::Locked(Locked { entry }),
        Err(why) => Read::Refused(why),
    }
}

/// **The login keychain's own unlock prompt** on macOS; elsewhere a store has no lock purlis
/// can ask the system to lift, and the unlock fails.
#[derive(Debug, Default, Clone, Copy)]
pub struct OsUnlock;

/// `errSecUserCanceled`: the person cancelled the system's prompt.
#[cfg(target_os = "macos")]
const USER_CANCELED: i32 = -128;

impl Unlocks for OsUnlock {
    fn unlock(&self) -> Unlock {
        #[cfg(target_os = "macos")]
        {
            use security_framework::os::macos::keychain::SecKeychain;
            match SecKeychain::default().and_then(|mut keychain| keychain.unlock(None)) {
                Ok(()) => Unlock::Unlocked,
                Err(e) if e.code() == USER_CANCELED => Unlock::Cancelled,
                Err(_) => Unlock::Failed,
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            Unlock::Failed
        }
    }
}
