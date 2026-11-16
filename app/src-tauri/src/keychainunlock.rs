// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Your keys locked at a start, unlocked from the window** (T1a-61, R-510, R-592): the
//! command behind W10's *Unlock*, which runs the OS's own unlock of the keychain
//! (`purlis_core::yourkeys::start::OsUnlock`) off the window's thread, and says how it went: the
//! window keeps W10 as it was on a cancel, and puts W84 under it on a failure. Neither makes a
//! run.

use purlis_core::yourkeys::start::{self, OsUnlock, Unlocks as _};

/// What *Unlock* did, as the window draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum Unlock {
    /// The keychain unlocked: the start goes on, with no second press.
    Unlocked,
    /// The person cancelled the system's prompt: W10 stays as it was.
    Cancelled,
    /// The unlock failed: W84 under W10.
    Failed,
}

impl From<start::Unlock> for Unlock {
    fn from(unlock: start::Unlock) -> Self {
        match unlock {
            start::Unlock::Unlocked => Self::Unlocked,
            start::Unlock::Cancelled => Self::Cancelled,
            start::Unlock::Failed => Self::Failed,
        }
    }
}

/// **W10's *Unlock***: the OS's unlock of the keychain. The window's start that waits on the
/// Ask reads its entry again where it unlocked, so the start goes on with no second press.
#[tauri::command]
#[specta::specta]
pub async fn unlock_your_keys() -> Result<Unlock, String> {
    crate::off_the_window("unlocking your keychain", move || {
        Ok(Unlock::from(OsUnlock.unlock()))
    })
    .await
}
