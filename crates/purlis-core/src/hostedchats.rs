// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a session host composed for each chat it started in a sandbox** (#2047): its persona,
//! its folder, the harness, and what its sandbox was compiled to here, with the tools its start
//! armed here ([`crate::sandbox::Confines`]).
//!
//! A runner composes its harness chats itself (ADR 0078, amended 2026-10-17), so the runner's
//! host, not the desktop's window, holds the only record of what a runner chat's sandbox armed.
//! When the desktop sends the values of one `secret exec` for such a chat, the runner's host
//! runs the declared tool held to this record, exactly as the window runs one for a chat of its
//! own ([`crate::secrets::brokered::serve_handed`]). The record is this process's memory alone:
//! made as [`crate::hoststart`] composes the start, gone when the chat's hook channel forgets
//! it. It holds no value.
//!
//! **A run is held to the chat's sandbox as it is now** (#3407): what it was compiled to as the
//! chat started, with the hosts its confinement here allows live since, read from that
//! confinement's own board as each run asks ([`Composed::live`]). An Allow on a block's Notice
//! for the chat reaches its tool's next run here, and taking it back narrows that run again, as
//! a run for a chat of the desktop's own is held (`confines_now` in the app).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use purlis_session_protocol::session::ReachNow;

use crate::harness::Harness;
use crate::sandbox::Confines;
use crate::sandbox::reach::by_of;
use crate::secrets::Env;
use crate::secrets::brokered::Asker;

/// One chat a host composed in a sandbox.
#[derive(Debug, Clone)]
pub struct Composed {
    pub persona: Option<String>,
    /// The folder it runs in.
    pub folder: PathBuf,
    pub harness: Option<Harness>,
    /// What its sandbox was compiled to here, with the tools its start armed here.
    pub confines: Confines,
    /// What its confinement here allows live now (#3407): `None` where this host keeps no
    /// confinement it can read, and the chat's runs are held to what it was compiled to.
    pub live: Option<Reached>,
}

/// What a chat's confinement holds now, read from its board as it is asked: the hosts allowed
/// live since the chat started, none once the confinement has ended. The same type as the host
/// crate's `purlis_host::ReachOf`, which hands it over.
pub type Live = Arc<dyn Fn() -> ReachNow + Send + Sync + 'static>;

/// [`Live`], kept with a chat's record.
#[derive(Clone)]
pub struct Reached(pub Live);

impl std::fmt::Debug for Reached {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reached(..)")
    }
}

/// Each chat composed here, by its project's path and its number there.
type Held = HashMap<(PathBuf, u32), Composed>;

fn held() -> MutexGuard<'static, Held> {
    static HELD: OnceLock<Mutex<Held>> = OnceLock::new();
    HELD.get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Chat `number` of the project at `root` was composed here as `composed`.
pub fn record(root: &Path, number: u32, composed: Composed) {
    held().insert((root.to_path_buf(), number), composed);
}

/// Chat `number` of the project at `root` is gone: nothing of it is kept.
pub fn forget(root: &Path, number: u32) {
    held().remove(&(root.to_path_buf(), number));
}

/// **Chat `number` of the project at `root` as it asks for a run here**, where this host
/// composed it in a sandbox: the asker a brokered run is held to, from this host's own record
/// and never from a line, with the hosts its confinement allows live now (#3407). Values are
/// read in nothing here, so the environment is this process's.
pub fn asker(root: &Path, number: u32) -> Option<Asker> {
    let mut composed = held().get(&(root.to_path_buf(), number)).cloned()?;
    // Read outside the hold: the board takes locks of its own.
    if let Some(Reached(live)) = &composed.live {
        for listed in live().allowed_live {
            composed
                .confines
                .allow_live(&listed.host, by_of(listed.layer));
        }
    }
    Some(Asker {
        root: root.to_path_buf(),
        env: Env::from_process(),
        chat: number,
        persona: composed.persona,
        folder: Some(composed.folder),
        confines: Some(composed.confines),
        harness: composed.harness,
    })
}

#[cfg(test)]
mod tests;
