// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Settings › You › Profiles** (#2034, ADR 0095): this machine's own profiles, listed, added,
//! imported into again and removed. A profile here is available in every project on this
//! machine; it lives in purlis's data home, never in a project.
//!
//! All of it is the core's ([`purlis_core::profilestore`]); this file only carries it to the
//! window. Every command here is the window's alone (`WINDOW_ONLY`): each is the person's act on
//! what their chats run as, and no chat makes, imports or removes a profile.
//!
//! **Remove is asked first**: [`machine_profile_removal`] says what goes, the window shows it,
//! and [`machine_profile_remove`] removes only what was shown, refused when it changed since.

use purlis_core::profilestore::{self, MachineSettings, Store};

/// One of this machine's profiles, as Settings lists it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct MachineProfileRow {
    pub name: String,
    /// The harness, as a profile's kind names it: `claude`, `codex` or `opencode`.
    pub kind: String,
    /// The harness as people call it: `Claude Code`.
    pub harness: String,
    /// `made` (its folder is purlis's) or `adopted` (the harness's own folder).
    pub origin: String,
    /// The folder its harness is pointed at.
    pub folder: String,
    /// When its settings were imported, in seconds since the epoch, for a made profile.
    pub imported_at: Option<u32>,
    /// The folder they were imported from.
    pub imported_from: Option<String>,
}

impl From<&profilestore::Record> for MachineProfileRow {
    fn from(record: &profilestore::Record) -> Self {
        Self {
            name: record.name.clone(),
            kind: record.kind.clone(),
            harness: purlis_core::harness_declaration::builtin(&record.kind)
                .map_or_else(|| record.kind.clone(), |d| d.title.clone()),
            origin: match record.origin {
                profilestore::Origin::Made => "made",
                profilestore::Origin::Adopted => "adopted",
            }
            .to_owned(),
            folder: record.config.display().to_string(),
            imported_at: record
                .imported
                .as_ref()
                .map(|i| u32::try_from(i.at).unwrap_or(u32::MAX)),
            imported_from: record
                .imported
                .as_ref()
                .map(|i| i.from.display().to_string()),
        }
    }
}

/// A profile made or imported into, and what its import did.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct MachineProfileImported {
    pub profile: MachineProfileRow,
    /// The settings copied, as paths under the folder they were read from.
    pub copied: Vec<String>,
    /// What was left out, each with why.
    pub skipped: Vec<String>,
}

impl From<profilestore::Made> for MachineProfileImported {
    fn from(made: profilestore::Made) -> Self {
        Self {
            profile: MachineProfileRow::from(&made.record),
            copied: made.copied.copied,
            skipped: made.copied.skipped,
        }
    }
}

/// What removing a profile takes away, as the window asks about it. Sent back as it was shown,
/// so the core removes only that.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct MachineProfileRemoval {
    pub name: String,
    /// Adopted: only the record goes, and the folder stays.
    pub adopted: bool,
    pub folder: String,
    /// The conversations that go, as paths under the folder; null where they cannot be listed.
    pub conversations: Option<Vec<String>>,
    /// The confirmation's sentence, the core's.
    pub says: String,
}

impl From<profilestore::Removal> for MachineProfileRemoval {
    fn from(removal: profilestore::Removal) -> Self {
        Self {
            says: removal.says(),
            name: removal.name,
            adopted: removal.adopted,
            folder: removal.folder.display().to_string(),
            conversations: removal.conversations,
        }
    }
}

impl From<MachineProfileRemoval> for profilestore::Removal {
    fn from(shown: MachineProfileRemoval) -> Self {
        Self {
            name: shown.name,
            adopted: shown.adopted,
            folder: shown.folder.into(),
            conversations: shown.conversations,
        }
    }
}

fn store() -> Result<Store, String> {
    Store::here().ok_or_else(|| {
        "purlis cannot tell where its data home is, or it is inside a project or a git work \
         tree, so it keeps no profile there."
            .to_owned()
    })
}

/// This machine's harness settings, as an import reads them: the home and this process's
/// environment.
fn settings_here<T>(work: impl FnOnce(&MachineSettings) -> T) -> T {
    let home = purlis_core::profiles::home().unwrap_or_default();
    let env = |name: &str| std::env::var(name).ok();
    work(&MachineSettings {
        home: &home,
        env: &env,
    })
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|err| format!("this machine's profiles did not answer: {err}"))?
}

/// This machine's profiles, in the order they were made.
#[tauri::command]
#[specta::specta]
pub async fn machine_profiles() -> Result<Vec<MachineProfileRow>, String> {
    blocking(|| {
        let store = store()?;
        let records = store.records().map_err(|e| {
            format!(
                "purlis could not read this machine's profiles in {} ({}).",
                store.dir().display(),
                purlis_core::rewrite::os_words(&e)
            )
        })?;
        Ok(records.iter().map(MachineProfileRow::from).collect())
    })
    .await
}

/// Makes the profile `name` of the harness `kind`, its settings imported from this machine's.
#[tauri::command]
#[specta::specta]
pub async fn machine_profile_create(
    name: String,
    kind: String,
) -> Result<MachineProfileImported, String> {
    blocking(move || {
        let store = store()?;
        settings_here(|machine| {
            profilestore::create(&store, &name, &kind, machine, purlis_core::halt::now())
        })
        .map(MachineProfileImported::from)
    })
    .await
}

/// Imports `name`'s settings again from this machine's.
#[tauri::command]
#[specta::specta]
pub async fn machine_profile_import(name: String) -> Result<MachineProfileImported, String> {
    blocking(move || {
        let store = store()?;
        settings_here(|machine| {
            profilestore::import(&store, &name, machine, purlis_core::halt::now())
        })
        .map(MachineProfileImported::from)
    })
    .await
}

/// What removing `name` takes away, for the window to ask about.
#[tauri::command]
#[specta::specta]
pub async fn machine_profile_removal(name: String) -> Result<MachineProfileRemoval, String> {
    blocking(move || profilestore::removal(&store()?, &name).map(MachineProfileRemoval::from)).await
}

/// Removes `name`, only if `shown` is what removing it takes away now.
#[tauri::command]
#[specta::specta]
pub async fn machine_profile_remove(
    name: String,
    shown: MachineProfileRemoval,
) -> Result<(), String> {
    blocking(move || profilestore::remove(&store()?, &name, &shown.into()).map(|_| ())).await
}
