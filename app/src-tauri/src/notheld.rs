// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The projects a runner would not hold again after a re-link** (#2846, item 3 of ADR 0078 as
//! amended 2026-10-20 for #2645).
//!
//! The desktop's host tells the window, in the runner's words, that a runner it relays a
//! project's link to no longer holds the project, and the window holds it again at once. Where
//! the runner refuses that hold, the project is kept here with the runner's words, as plain
//! text and cut short (`crate::devices::MOST_SHOWN`), until a hold of it is taken, and the Pools group draws a
//! standing mark for it on the row of the pool its chats run on (`app/src/notheld.ts`). Only a
//! refusal of the hold is kept: what purlis itself could not do is said where it happened.
//!
//! The mark goes once the project is held again, and also once the project is forgotten, its
//! pool is removed, or none of its workspaces runs its chats on a pool any more ([`recheck`]).
//!
//! Kept in memory only, for this run of the window: a project not held is one this window tried
//! and was refused, and the next window tries again as it holds its projects.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

/// The event the window is sent each time the projects not held change.
pub const EVENT: &str = "projects-not-held";

/// The most of the runner's words a mark says, its `…` included: what the one sanitiser keeps
/// ([`crate::devices::shown_safely`]).
#[cfg(test)]
pub const MOST_SAID: usize = crate::devices::MOST_SHOWN + 1;

/// One project the runner its chats run on would not hold, as the Pools group draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ProjectNotHeld {
    /// Where it is on this machine.
    pub root: String,
    /// What the window calls it: its folder's name.
    pub name: String,
    /// What the runner said, as plain text, at most what the one sanitiser keeps (`crate::devices::MOST_SHOWN`, then `…`).
    pub why: String,
    /// The pools its workspaces run their chats on, as this machine keeps them: the row the
    /// mark is drawn on where there is one.
    pub pools: Vec<String>,
}

/// Told each time the projects not held change.
type Teller = Box<dyn Fn() + Send + Sync + 'static>;

fn kept() -> std::sync::MutexGuard<'static, BTreeMap<PathBuf, String>> {
    static KEPT: Mutex<BTreeMap<PathBuf, String>> = Mutex::new(BTreeMap::new());
    KEPT.lock().unwrap_or_else(PoisonError::into_inner)
}

static TELL: OnceLock<Teller> = OnceLock::new();

/// Whom to tell each time the projects not held change: the window, once it is there.
pub fn telling(tell: impl Fn() + Send + Sync + 'static) {
    let _ = TELL.set(Box::new(tell));
}

fn told() {
    if let Some(tell) = TELL.get() {
        tell();
    }
}

/// **The project at `root` is not held where its chats run**, for `why`, the runner's words:
/// kept as the window may show it ([`crate::devices::shown_safely`]): one line, nothing a reader
/// would not see, no address's user or password, and capped.
pub fn not_held(root: &Path, why: &str) {
    let why = crate::devices::shown_safely(why);
    let changed = kept().insert(root.to_path_buf(), why.clone()).as_deref() != Some(&why);
    if changed {
        told();
    }
}

/// **The project at `root` is held again**, or forgotten: its mark goes.
pub fn held(root: &Path) {
    let changed = kept().remove(root).is_some();
    if changed {
        told();
    }
}

/// **The marks of projects that no longer run their chats on a pool here go** (#2846): one
/// forgotten on this machine, or none of whose workspaces runs its chats on a pool this
/// machine keeps. What `config` keeps is read; nothing goes where it cannot be.
pub fn recheck(config: &Path) {
    let Ok(pools) = purlis_core::runners::pools(config) else {
        return;
    };
    let store = purlis_core::machine::read(config).store;
    let gone: Vec<PathBuf> = kept()
        .keys()
        .filter(|root| pools_of(&store, &pools, root).is_empty())
        .cloned()
        .collect();
    for root in gone {
        held(&root);
    }
}

/// The pools this machine keeps that `root`'s workspaces run their chats on.
fn pools_of(
    store: &purlis_core::machine::Store,
    pools: &[purlis_core::runners::Pool],
    root: &Path,
) -> Vec<String> {
    let mut named: Vec<String> = store
        .recent(root)
        .map(|recent| recent.chats_run_on.values().cloned().collect())
        .unwrap_or_default();
    named.sort();
    named.dedup();
    named.retain(|pool| pools.iter().any(|kept| kept.name() == pool));
    named
}

/// The projects not held now, in the order of their paths, each with the pools its chats run
/// on in `config`.
pub fn now(config: Option<&Path>) -> Vec<ProjectNotHeld> {
    let (store, pools) = config.map_or((None, Vec::new()), |config| {
        (
            Some(purlis_core::machine::read(config).store),
            purlis_core::runners::pools(config).unwrap_or_default(),
        )
    });
    kept()
        .iter()
        .map(|(root, why)| ProjectNotHeld {
            root: root.display().to_string(),
            name: root.file_name().map_or_else(
                || root.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            ),
            why: why.clone(),
            pools: store
                .as_ref()
                .map(|store| pools_of(store, &pools, root))
                .unwrap_or_default(),
        })
        .collect()
}

/// **The projects a runner would not hold again after a re-link**, each with the runner's words
/// and the pools its chats run on.
#[tauri::command]
#[specta::specta]
pub async fn projects_not_held() -> Result<Vec<ProjectNotHeld>, String> {
    crate::off_the_window("reading the projects not held", || {
        let config = purlis_core::machine::config_root();
        if let Some(config) = &config {
            recheck(config);
        }
        Ok(now(config.as_deref()))
    })
    .await
}

#[cfg(test)]
mod tests;
