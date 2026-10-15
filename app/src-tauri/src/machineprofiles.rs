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
//!
//! **Sign in is the harness's own** (#2035, ADR 0095 §3): each row says how its profile signs
//! in and whether its harness says it is signed in on this device, and Sign in opens a shell
//! tab into which [`type_profile_sign_in`] types the harness's own sign-in with the profile's
//! folder set. The person drives it there; purlis reads none of what it prints.

use purlis_core::profilestore::{self, Homes, MachineSettings, sign_in};

use crate::planes::{PlaneId, Planes};

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
    /// How it signs in, and where: *through Claude Code's own sign-in, on this Mac*.
    pub signs_in: String,
    /// Whether its harness says it is signed in on this device: `signed-in`, `signed-out` or
    /// `cannot-tell`. Null for an adopted profile, signed in by the person's own sign-in and
    /// never asked.
    pub standing: Option<String>,
}

impl From<&profilestore::MachineProfile> for MachineProfileRow {
    fn from(record: &profilestore::MachineProfile) -> Self {
        Self {
            name: record.name.clone(),
            kind: record.harness.clone(),
            harness: purlis_core::harness_declaration::builtin(&record.harness)
                .map_or_else(|| record.harness.clone(), |d| d.title.clone()),
            origin: if record.adopted.is_some() {
                "adopted"
            } else {
                "made"
            }
            .to_owned(),
            folder: record.folders.config.display().to_string(),
            imported_at: record
                .imported
                .as_ref()
                .map(|i| u32::try_from(i.at).unwrap_or(u32::MAX)),
            imported_from: record
                .imported
                .as_ref()
                .map(|i| i.from.display().to_string()),
            signs_in: signs_in(record),
            standing: None,
        }
    }
}

/// How `record` signs in, and where, as its row says it.
fn signs_in(record: &profilestore::MachineProfile) -> String {
    let harness = purlis_core::harness_declaration::builtin(&record.harness)
        .map_or_else(|| record.harness.clone(), |d| d.title.clone());
    let here = profilestore::this_device();
    if record.adopted.is_some() {
        format!("{harness}'s own sign-in on {here}, as it was before purlis")
    } else {
        format!("through {harness}'s own sign-in, on {here}")
    }
}

/// Each of `records` as a row, with what its harness says of its sign-in, each asked at once.
fn rows(records: &[profilestore::MachineProfile]) -> Vec<MachineProfileRow> {
    std::thread::scope(|scope| {
        let asked: Vec<_> = records
            .iter()
            .map(|record| scope.spawn(move || sign_in::standing_of(record, &sign_in::ThisMachine)))
            .collect();
        records
            .iter()
            .zip(asked)
            .map(|(record, asked)| MachineProfileRow {
                standing: asked.join().ok().flatten().map(|s| s.word().to_owned()),
                ..MachineProfileRow::from(record)
            })
            .collect()
    })
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

fn store() -> Result<Homes, String> {
    Homes::here().ok_or_else(|| {
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
        let records = profilestore::list(store()?.config_root())?;
        Ok(rows(&records))
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

/// Types the profile `name`'s sign-in into shell session `session`, and runs it (#2035, ADR
/// 0095 §3): its harness's own sign-in command, with the profile's folder variables set, which
/// the person then drives in that tab.
///
/// **The window names the profile, never the command.** The line is the core's, made from this
/// machine's own record of the profile ([`sign_in::login_of`]), so no text from a project, a
/// plane or the window reaches the shell through here. It is typed only into a shell at the
/// project root, the one the window opened for it, never into a chat's pane. An adopted
/// profile is refused: it is signed in by the person's own sign-in.
#[tauri::command]
#[specta::specta]
pub fn type_profile_sign_in(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    name: String,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    if !held.chats().is_shell_at(session, held.root()) {
        return Err(
            "that tab is not a shell at the project root, so no sign-in was typed.".to_owned(),
        );
    }
    let record = profilestore::list(store()?.config_root())?
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| {
            format!(
                "this machine has no profile named '{}', so no sign-in was typed.",
                purlis_core::shown::short(&name)
            )
        })?;
    let line = sign_in::login_of(&record)?.line()?;
    held.operator_input(session, line.as_bytes())
}
