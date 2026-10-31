// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Settings › You › Profiles** (#2034, ADR 0095): this machine's own profiles, listed, added,
//! imported into again, pinned to a harness version (#2038) and removed. A profile here is available in every project on this
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
    /// The plans Sign in offers, where purlis names the provider (opencode's ChatGPT and GitHub
    /// Copilot, never a Claude plan); empty where the harness signs in to its own.
    pub plans: Vec<PlanRow>,
    /// The harness version its chats run on (#2038); null until its first chat pins it.
    pub pinned: Option<String>,
    /// The version of its harness installed on this machine, as its program answers
    /// `--version`; null where purlis could not read one.
    pub installed: Option<String>,
    /// The version **Update the pin** offers: the one installed, where the profile's next chat
    /// would not start on it (another major, or an older one). Null where its chats run on the
    /// version installed, or follow it.
    pub offered: Option<String>,
    /// Whether it is for sandboxed chats only (#2037): only a sandboxed chat starts on it.
    pub sandbox_only: bool,
    /// Where the commands its chats run can read its sign-in (#2037, ADR 0095 §5): the profile
    /// page's warning, in the core's words; null where its harness keeps the sign-in from them.
    pub commands_can_read_sign_in: Option<String>,
    /// What it is drawn with (#2922): its harness's glyph where it stands for the built-in, else
    /// its kind's monogram.
    pub icon: crate::harnessicon::HarnessIconRow,
}

/// A plan a profile is signed in to, by the provider id its harness takes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PlanRow {
    pub id: String,
    pub title: String,
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
            plans: sign_in::providers(&record.harness)
                .iter()
                .map(|plan| PlanRow {
                    id: plan.provider.clone(),
                    title: plan.title.clone(),
                })
                .collect(),
            pinned: record.pinned.as_ref().map(|p| p.version.clone()),
            installed: None,
            offered: None,
            sandbox_only: record.sandbox_only,
            commands_can_read_sign_in: purlis_core::sandbox::signin::commands_can_read(
                &record.harness,
            ),
            icon: crate::harnessicon::of_kind(&record.name, &record.harness),
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
        // Each harness asked its version once (#2038), beside the sign-ins.
        let mut harnesses: Vec<&str> = records.iter().map(|r| r.harness.as_str()).collect();
        harnesses.sort_unstable();
        harnesses.dedup();
        let versions: Vec<_> = harnesses
            .iter()
            .map(|&harness| {
                (
                    harness,
                    scope.spawn(move || profilestore::pin::installed_here(harness)),
                )
            })
            .collect();
        let installed: std::collections::BTreeMap<&str, profilestore::pin::Version> = versions
            .into_iter()
            .filter_map(|(harness, asked)| Some((harness, asked.join().ok().flatten()?)))
            .collect();
        records
            .iter()
            .zip(asked)
            .map(|(record, asked)| MachineProfileRow {
                standing: asked.join().ok().flatten().map(|s| s.word().to_owned()),
                ..pinned_row(record, installed.get(record.harness.as_str()))
            })
            .collect()
    })
}

/// `record`'s row with the version of its harness installed here, `here`, beside its pin, and
/// the version Update the pin offers where its next chat would not start on `here` (#2038).
fn pinned_row(
    record: &profilestore::MachineProfile,
    here: Option<&profilestore::pin::Version>,
) -> MachineProfileRow {
    MachineProfileRow {
        offered: here
            .and_then(|here| profilestore::pin::offered(record, here))
            .map(|v| v.to_string()),
        installed: here.map(ToString::to_string),
        ..MachineProfileRow::from(record)
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

/// This machine's profiles, in the order they were made, each with the version of its harness
/// installed here beside its pin (#2038): each harness asked `--version` once.
#[tauri::command]
#[specta::specta]
pub async fn machine_profiles() -> Result<Vec<MachineProfileRow>, String> {
    blocking(|| {
        let records = profilestore::list(store()?.config_root())?;
        Ok(rows(&records))
    })
    .await
}

/// **Update the pin** (#2038): pins `name` to `version`, the one the page showed installed, so
/// its chats run, and its conversations resume, on that version from now on.
#[tauri::command]
#[specta::specta]
pub async fn machine_profile_pin(
    name: String,
    version: String,
) -> Result<MachineProfileRow, String> {
    blocking(move || {
        profilestore::pin::update_pin(
            store()?.config_root(),
            &name,
            &version,
            purlis_core::halt::now(),
        )
        .map(|record| MachineProfileRow::from(&record))
    })
    .await
}

/// Makes the profile `name` of the harness `kind`, its settings imported from this machine's;
/// or, `sandbox_only`, one for sandboxed chats only, with nothing imported (#2037).
#[tauri::command]
#[specta::specta]
pub async fn machine_profile_create(
    planes: tauri::State<'_, Planes>,
    name: String,
    kind: String,
    sandbox_only: bool,
) -> Result<MachineProfileImported, String> {
    let made = blocking(move || {
        let store = store()?;
        let now = purlis_core::halt::now();
        if sandbox_only {
            profilestore::create_sandbox_only(&store, &name, &kind, now)
        } else {
            settings_here(|machine| profilestore::create(&store, &name, &kind, machine, now))
        }
        .map(MachineProfileImported::from)
    })
    .await?;
    // A sandbox-only Codex profile clears every project's wait for one (#2037).
    if sandbox_only {
        crate::asking::every_projects_asks_may_have_moved(&planes);
    }
    Ok(made)
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

/// Removes `name`, only if `shown` is what removing it takes away now, and has each pool's
/// runner that kept it forget it, its sign-in there with it (#3849). `Some` note, in words, for
/// a runner of a purlis before 1.52, which keeps it.
#[tauri::command]
#[specta::specta]
pub async fn machine_profile_remove(
    name: String,
    shown: MachineProfileRemoval,
) -> Result<Option<String>, String> {
    blocking(move || {
        profilestore::remove(&store()?, &name, &shown.into())?;
        Ok(crate::poolprofiles::forget_off_the_pools(&name))
    })
    .await
}

/// How often, and for how long, a profile's harness is asked again after its Sign in, so its
/// Ask leaves the Inbox once it is signed in.
const WATCHED_EVERY: std::time::Duration = std::time::Duration::from_secs(5);
const WATCHED_FOR: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// Types the profile `name`'s sign-in into shell session `session`, and runs it (#2035, ADR
/// 0095 §3): its harness's own sign-in command, with the profile's folder variables set, to the
/// plan `provider` names where purlis names one, which the person then drives in that tab. What
/// its harness said of it is forgotten, and it is asked again every few seconds until it says
/// the profile is signed in, when its Ask leaves the Inbox.
///
/// **The window names the profile, never the command.** The line is the core's, made from this
/// machine's own record of the profile ([`sign_in::login_of`]), so no text from a project, a
/// plane or the window reaches the shell through here. It is typed only into a shell at the
/// project root, the one the window opened for it, never into a chat's pane. An adopted
/// profile is refused: it is signed in by the person's own sign-in.
///
/// **With `pool`, it is signed in on that pool's runner** (#2049): the line is the runner's own
/// `purlis profile sign-in`, run there through this machine's `docker`, in the runner's folder
/// for the profile ([`crate::poolprofiles::sign_in_line`]); nothing of this machine's sign-in
/// goes there. The runner is asked again until it says the profile is signed in there or the
/// person closes the shell (#4009). With `device`, by the harness's device-code flow there
/// (#4044).
#[tauri::command]
#[specta::specta]
pub fn type_profile_sign_in(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    name: String,
    provider: Option<String>,
    pool: Option<String>,
    device: bool,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    if !held.chats().is_shell_at(session, held.root()) {
        return Err(
            "that tab is not a shell at the project root, so no sign-in was typed.".to_owned(),
        );
    }
    if let Some(pool) = pool {
        let line = crate::poolprofiles::sign_in_line(&pool, &name, provider.as_deref(), device)?;
        held.operator_input(session, line.as_bytes())?;
        let (shell, open) = (
            std::sync::Arc::downgrade(&held),
            std::sync::Arc::downgrade(&held),
        );
        let root = held.root().to_path_buf();
        let shell_root = root.clone();
        crate::poolprofiles::watch(
            pool,
            name,
            session,
            crate::poolprofiles::Watcher {
                project: std::sync::Arc::as_ptr(&held) as usize,
                root,
                shell_open: Box::new(move |tab| {
                    shell
                        .upgrade()
                        .is_some_and(|held| held.chats().is_shell_at(tab, &shell_root))
                }),
                project_open: Box::new(move || {
                    open.upgrade().is_some_and(|held| !held.chats().ending())
                }),
                told: Box::new(held.hooks().asks_teller()),
            },
        );
        return Ok(());
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
    // This machine's own profile: its harness is the one on `PATH`. Only a runner fetches the
    // version a profile pins (#4007), in its own `purlis profile sign-in`.
    let line = sign_in::login_of(&record, provider.as_deref(), None)?.line()?;
    held.operator_input(session, line.as_bytes())?;
    sign_in::forget_of(&record);
    if let Some(profile) = purlis_core::profiles::of_record(&record) {
        sign_in::watch(
            profile,
            sign_in::ThisMachine,
            WATCHED_EVERY,
            WATCHED_FOR,
            held.hooks().asks_teller(),
        );
    }
    Ok(())
}

/// Puts away project `plane`'s Ask to sign the profile `profile` in (#2035): Dismiss. A chat's
/// next start on it asks again while its harness says it is signed out.
#[tauri::command]
#[specta::specta]
pub fn dismiss_sign_in(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    profile: String,
    pool: Option<String>,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    match pool {
        Some(pool) => profilestore::onrunner::let_go(Some(held.root()), &pool, &profile),
        None => sign_in::dismiss(held.root(), &profile),
    }
    held.hooks().asks_teller()();
    Ok(())
}

/// **Answers project `plane`'s Ask *Codex needs a sandbox-only profile here*** (#2037, the
/// operator's ruling of 2026-10-15): makes a sandbox-only Codex profile, with nothing imported,
/// and puts the Ask away. The window then opens Codex's own sign-in for it in a shell tab
/// ([`type_profile_sign_in`]). The profile's name.
#[tauri::command]
#[specta::specta]
pub async fn make_sandbox_only_profile(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let teller = held.hooks().asks_teller();
    let made = blocking(move || {
        let made = profilestore::sandbox_only::make(&store()?, purlis_core::halt::now())?;
        profilestore::sandbox_only::dismiss(&root);
        Ok(made)
    })
    .await?;
    teller();
    Ok(made.record.name)
}

/// Puts away project `plane`'s Ask *Codex needs a sandbox-only profile here* (#2037): Dismiss. A
/// sandboxed Codex chat's next start asks again while this machine has no such profile.
#[tauri::command]
#[specta::specta]
pub fn dismiss_sandbox_only(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    profilestore::sandbox_only::dismiss(held.root());
    held.hooks().asks_teller()();
    Ok(())
}
