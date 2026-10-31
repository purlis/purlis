// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! This machine's own profiles: `<config>/profiles.json` (ADR 0095, spec #2030).
//!
//! **A profile here belongs to this machine and is available in every project on it**, where a
//! profile in a clone's `charter.local.toml` belongs to that clone ([`crate::profiles`]). This
//! module is the one place the file is read and written, under one lock: every change is an
//! [`update`] of the whole [`Store`]. The profiles the window makes (#2034) are records in it
//! too ([`made`]). A kind of record is a field of [`MachineProfile`] (`adopted`, and `imported`
//! for a profile purlis made, and `pinned` once a chat ran on it, [`pin`]); a field a
//! build does not know is kept in [`MachineProfile::rest`] and written back as it was, and a
//! record a build cannot read at all is kept as written.
//!
//! **The first records are adopted, never made** (ADR 0095 §2). On upgrade, each harness folder
//! this machine already uses becomes a profile in place: the same folder, so the same sign-in,
//! the same settings and the same conversations. Nothing is copied, moved or written in the
//! harness's folder, and no file in it is read: whether the folder is there, and whose it is, is
//! all adoption asks of it. The folder is the one a chat started on the harness's built-in
//! profile reaches: the one the harness's own folder variable names (`[folders]` of its
//! declaration, ADR 0073), where the environment sets it, and otherwise the harness's own
//! default. A folder is adopted only where it is the person's own: named by an absolute path,
//! owned by this user, and outside every project and git work tree.
//!
//! **An adopted profile keeps its built-in's name** (`claude`, `codex`, `opencode`), which is
//! what every chat record, persona and dispatch list already names, and is shown by its title,
//! *Claude Code (this Mac)*. So a chat's next run is on the adopted profile with nothing
//! rewritten, and it resumes its own conversation from the folder it was always in.
//!
//! **Adoption is idempotent.** A folder a record already points at, a harness whose built-in
//! name a record already has, and a folder the person removed as a profile ([`Store::forget`])
//! are left alone, and a run that adopts nothing writes nothing.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The file, in purlis's directory of the config home.
pub const FILE: &str = "profiles.json";

/// The lock every [`update`] of [`FILE`] holds, beside it.
const LOCK: &str = "profiles.json.lock";

/// The version [`FILE`] is written at. A store written at a newer one is never written over.
const VERSION: u64 = 1;

/// The most [`FILE`] may be. A machine has a handful of profiles.
const MOST_BYTES: u64 = 1 << 20;

/// Where [`FILE`] is under the config home `config_root`.
pub fn path(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join(FILE)
}

/// One profile of this machine's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineProfile {
    /// The name chats, personas and dispatch lists use. An adopted profile keeps its
    /// built-in's (`claude`).
    pub name: String,
    /// The harness, as a profile's `kind` names it.
    pub harness: String,
    /// How the window and the listing name it: *Claude Code (this Mac)*.
    pub title: String,
    /// The folders its harness's folder variables reach.
    pub folders: Folders,
    /// When and how it was adopted, for a profile that was; `None` for one purlis made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adopted: Option<Adopted>,
    /// When its settings were imported and from where, for a profile purlis made (#2034).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported: Option<Imported>,
    /// How it signs in, where that is recorded (#2036): its kind, the label it is billed to,
    /// and for a key or a cloud the vault reference of each variable its harness is handed,
    /// never a value ([`crate::profilesignin`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_in: Option<crate::profilesignin::SignIn>,
    /// The harness version its chats run on, from its first chat on (#2038, [`pin`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<pin::Pinned>,
    /// **A profile purlis made for sandboxed chats only** (#2037, ruled 2026-10-15): only a
    /// sandboxed chat starts on it, and a sandboxed Codex chat starts on no other. Signed in
    /// through its harness's own sign-in, never from a copy; its settings are purlis's, so
    /// nothing is imported into it. Written only where true.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sandbox_only: bool,
    /// **On a runner, the desktop it is kept for** (#2049), by its device id: a profile the
    /// desktop paired with this runner had it keep (`keep_profile`, [`onrunner`]). Only such a
    /// record of a folder adopted in place is signed in by purlis here. Written only where set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept_for: Option<String>,
    /// Every field of the record this build does not know, written back as it was read.
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

/// The folders a profile's harness keeps its files in: what its declaration's `config_env` and
/// `data_env` name (ADR 0073, as amended by #2031).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folders {
    pub config: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<PathBuf>,
}

/// How a folder was adopted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adopted {
    /// Seconds since the epoch.
    pub at: u64,
    /// The folder variables the environment set when the folder was found there: the only
    /// ones that reach it again. Empty for a folder found where the harness looks by default,
    /// which is reached with none set: Claude Code given `CLAUDE_CONFIG_DIR`, even naming its
    /// default folder, looks for its sign-in somewhere else. A chat on an adopted profile
    /// starts as it did on the built-in, so purlis sets none of them today.
    #[serde(default)]
    pub variables: Vec<String>,
}

/// When a made profile's settings were imported, and from where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Imported {
    /// Seconds since the epoch.
    pub at: u64,
    /// The folder they were read from.
    pub from: PathBuf,
}

/// The whole store, as one [`update`] changes it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Store {
    /// The profiles this build can read, in the order they were added.
    pub profiles: Vec<MachineProfile>,
    /// The folders the person removed as profiles, resolved: never adopted again.
    pub forgotten: Vec<PathBuf>,
    /// The records this build cannot read, kept as written.
    unread: Vec<Value>,
    /// Every top-level field this build does not know.
    rest: Map<String, Value>,
}

impl Store {
    /// Whether a profile of `harness` already points at `folder`, however either is spelled.
    pub fn points_at(&self, harness: &str, folder: &Path) -> bool {
        self.profiles
            .iter()
            .any(|p| p.harness == harness && same_folder(&p.folders.config, folder))
    }

    /// Never adopt `folder` again: what removing an adopted profile records (#2034).
    pub fn forget(&mut self, folder: &Path) {
        if !self.is_forgotten(folder) {
            self.forgotten.push(resolved(folder));
        }
    }

    /// Whether `folder` was removed as a profile.
    pub fn is_forgotten(&self, folder: &Path) -> bool {
        self.forgotten.iter().any(|gone| same_folder(gone, folder))
    }
}

/// The file as written.
#[derive(Default, Serialize, Deserialize)]
struct OnDisk {
    #[serde(default)]
    version: u64,
    #[serde(default)]
    profiles: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    forgotten: Vec<PathBuf>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

/// The file, or an empty one when there is none yet, with the version it was written at. One
/// that cannot be read is an error, never an empty list: writing over it would lose every
/// profile in it.
fn read(config_root: &Path) -> Result<(u64, Store), String> {
    let at = path(config_root);
    let file = match std::fs::File::open(&at) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((0, Store::default())),
        Err(e) => return Err(format!("purlis cannot read {}: {e}", at.display())),
    };
    let mut text = String::new();
    std::io::Read::read_to_string(&mut std::io::Read::take(file, MOST_BYTES + 1), &mut text)
        .map_err(|e| format!("purlis cannot read {}: {e}", at.display()))?;
    if text.len() as u64 > MOST_BYTES {
        return Err(format!(
            "{} is larger than a list of profiles can be",
            at.display()
        ));
    }
    let on_disk: OnDisk = serde_json::from_str(&text)
        .map_err(|e| format!("{} is not a list of profiles: {e}", at.display()))?;
    let mut store = Store {
        forgotten: on_disk.forgotten,
        rest: on_disk.rest,
        ..Store::default()
    };
    for value in on_disk.profiles {
        match serde_json::from_value::<MachineProfile>(value.clone()) {
            Ok(profile) => store.profiles.push(profile),
            Err(_) => store.unread.push(value),
        }
    }
    Ok((on_disk.version, store))
}

fn write(config_root: &Path, store: &Store) -> Result<(), String> {
    let at = path(config_root);
    let mut profiles = Vec::with_capacity(store.profiles.len() + store.unread.len());
    for profile in &store.profiles {
        profiles.push(
            serde_json::to_value(profile)
                .map_err(|e| format!("purlis cannot write {}: {e}", at.display()))?,
        );
    }
    profiles.extend(store.unread.iter().cloned());
    let file = OnDisk {
        version: VERSION,
        profiles,
        forgotten: store.forgotten.clone(),
        rest: store.rest.clone(),
    };
    let text = serde_json::to_string_pretty(&file)
        .map_err(|e| format!("purlis cannot write {}: {e}", at.display()))?
        + "\n";
    let dir = crate::machine::private_dir(config_root)
        .map_err(|e| format!("purlis cannot write {}: {e}", at.display()))?;
    crate::machine::write_beside(config_root, &dir.join(FILE), text.as_bytes())
        .map_err(|e| format!("purlis cannot write {}: {e}", at.display()))
}

/// This machine's profiles, in the order they were added. A record this purlis cannot read is
/// left out, and kept in the file.
pub fn list(config_root: &Path) -> Result<Vec<MachineProfile>, String> {
    Ok(read(config_root)?.1.profiles)
}

/// **The one way the store changes**: under its lock, the store as read is handed to `change`,
/// and written back only when `change` changed it. A store that cannot be read, or that a newer
/// purlis wrote, is refused and never written over.
pub fn update<T>(config_root: &Path, change: impl FnOnce(&mut Store) -> T) -> Result<T, String> {
    let _held = crate::machine::Lock::named(config_root, LOCK);
    let (version, mut store) = read(config_root)?;
    if version > VERSION {
        return Err(format!(
            "{} was written by a newer purlis (version {version}; this one writes version \
             {VERSION}), so this one leaves it as it is",
            path(config_root).display()
        ));
    }
    let before = store.clone();
    let answer = change(&mut store);
    if store != before {
        write(config_root, &store)?;
    }
    Ok(answer)
}

/// **Keeps how the profile `name` signs in**, or forgets it (`None`), in the store under
/// `config_root` (#2036). A sign-in its harness cannot take is refused and the one it had is
/// kept ([`crate::profilesignin::SignIn::check`]); so is a name no profile has.
pub fn sign_in_with(
    config_root: &Path,
    name: &str,
    sign_in: Option<crate::profilesignin::SignIn>,
) -> Result<(), String> {
    update(config_root, |store| {
        let Some(profile) = store.profiles.iter_mut().find(|p| p.name == name) else {
            return Err(format!(
                "this machine has no profile named '{}'",
                crate::shown::short(name)
            ));
        };
        if let Some(sign_in) = &sign_in {
            sign_in.check(&profile.harness)?;
        }
        profile.sign_in = sign_in;
        Ok(())
    })?
}

/// [`list`] of this machine's own config home, for a reader that only shows what is there: empty
/// where there is no store, it cannot be read, or a fenced test build has no business reading
/// it.
pub fn listed_here() -> Vec<MachineProfile> {
    let Some(config) = crate::machine::config_root_unheld() else {
        return Vec::new();
    };
    if crate::fence::FENCED && !crate::fence::inside(&config, &crate::fence::fence()) {
        return Vec::new();
    }
    list(&config).unwrap_or_default()
}

/// The record `profiles` holds for a built-in profile named `name` that was adopted.
pub fn adopted_as<'a>(profiles: &'a [MachineProfile], name: &str) -> Option<&'a MachineProfile> {
    profiles
        .iter()
        .find(|p| p.name == name && p.adopted.is_some())
}

/// Where a harness purlis ships keeps its files when its folder variables are not set: the
/// folder each variable would name, from `home`, and the folder of its own name it keeps under
/// them, if any. Facts of each harness, read from its own documentation and code: Claude Code's
/// `~/.claude`, Codex's `~/.codex`, and opencode's `opencode` folder under the XDG folders
/// (`~/.config`, `~/.local/share`), as its `xdg-basedir` answers on every platform.
pub(crate) fn defaults(
    harness: &str,
    home: &Path,
) -> Option<(PathBuf, Option<PathBuf>, &'static str)> {
    match harness {
        "claude" => Some((home.join(".claude"), None, "")),
        "codex" => Some((home.join(".codex"), None, "")),
        "opencode" => Some((
            home.join(".config"),
            Some(home.join(".local/share")),
            "opencode",
        )),
        _ => None,
    }
}

/// **The sign-in files a made profile's harness keeps under its folder** (#4011): Claude Code's
/// `.credentials.json` and Codex's `auth.json` in the folder its config variable names, and
/// opencode's `opencode/auth.json` in its data folder. Facts of each harness, as
/// [`sign_in`]'s statuses read them. Nothing else of a profile is a sign-in.
pub const SIGN_IN_FILES: [&str; 3] = [
    "config/.credentials.json",
    "config/auth.json",
    "data/opencode/auth.json",
];

/// **The sign-in file of `harness`** (#4011), one of [`SIGN_IN_FILES`] under its profile's
/// folders: the only file of a profile a drained runner's carry takes in. `None` for a harness
/// purlis does not ship.
pub fn sign_in_file_of(harness: &str) -> Option<&'static str> {
    match harness {
        "claude" => Some(SIGN_IN_FILES[0]),
        "codex" => Some(SIGN_IN_FILES[1]),
        "opencode" => Some(SIGN_IN_FILES[2]),
        _ => None,
    }
}

/// **The folder a runner keeps the sign-ins carried to it in** (#4011, #4055), under its profiles'
/// folder ([`made::DIR`]), which a chat neither reads nor writes:
/// `<data>/profiles/.carried/<batch>/<profile>/`, a batch of each drained runner's, until it keeps
/// that profile for its desktop and takes them in. A profile's name starts with no dot.
pub const CARRIED: &str = ".carried";

/// Where the carry put them before: only ever cleared, never taken in ([`made::take_carried`]).
pub(crate) const CARRIED_BEFORE: &str = "carried";

/// [`CARRIED`] under the data home `data`.
pub fn carried_in(data: &Path) -> PathBuf {
    data.join(made::DIR).join(CARRIED)
}

/// **Where under a runner's home the carry puts the sign-ins it carries** (#4055): [`carried_in`]
/// its data home, walked to through no link.
pub fn carried_under_a_runner_home() -> String {
    Path::new(".local/share")
        .join(
            crate::names::DATA_HOME
                .spellings()
                .next()
                .unwrap_or("purlis"),
        )
        .join(made::DIR)
        .join(CARRIED)
        .display()
        .to_string()
}

/// [`carried_under_a_runner_home`], in a home whose data home has its old spelling, or none yet:
/// a data home is read under its old spelling until a migration moves it (`crate::names`), so
/// the carry goes there rather than make the new spelling under a runner already running.
pub fn carried_under_an_old_runner_home() -> String {
    Path::new(".local/share")
        .join(crate::names::DATA_HOME.newest_old())
        .join(made::DIR)
        .join(CARRIED)
        .display()
        .to_string()
}

/// **What a Docker pool's new runner takes of a runner drained beside it** (#4011, #4055, ruled
/// 2026-10-30): the sign-ins of the profiles purlis made there, and nothing else, each as
/// `<from>=<to>`, `<from>` under the old home and `<to>` under the carry's batch ([`CARRIED`]). A
/// made profile's sign-in files ([`SIGN_IN_FILES`], `*` its name, under either spelling of the
/// data home) go under its name. A harness's own default folder is not carried for now, so a
/// built-in default profile signs in again on the new runner. No setting, hook, plugin,
/// instruction or transcript of a harness, and nothing of purlis's own: not the profiles' records,
/// the runner's device id, link key or peers, its harness installs, its projects or its browsers.
pub fn carried_in_a_runner_home() -> Vec<String> {
    let mut pairs = Vec::new();
    for data in crate::names::DATA_HOME.spellings() {
        let old_data = Path::new(".local/share").join(data);
        for file in SIGN_IN_FILES {
            pairs.push(format!(
                "{}={}",
                old_data.join(made::DIR).join("*").join(file).display(),
                Path::new("*").join(file).display()
            ));
        }
    }
    pairs
}

/// What this device is called in a profile's title.
pub fn this_device() -> &'static str {
    if cfg!(target_os = "macos") {
        "this Mac"
    } else {
        "this computer"
    }
}

/// What adoption found: the profiles it adopted, and each folder it passed over and why.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Adoption {
    pub adopted: Vec<MachineProfile>,
    /// One sentence per folder that was there and was not adopted.
    pub passed_over: Vec<String>,
}

/// The folders a profile's harness keeps its own files in, as a chat on the harness's built-in
/// profile reaches them in the environment `env`, and the variables that were set to reach them.
/// `Ok(None)` when the harness has no folder there, or declares no folder variable; `Err` with
/// the reason when the folder is there and is not the person's own to adopt.
fn machine_folders(
    declaration: &crate::harness_declaration::Declaration,
    home: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<(Folders, Vec<String>)>, String> {
    let Some(config_env) = declaration.folders.config_env.as_deref() else {
        return Ok(None);
    };
    let Some((config_default, data_default, own)) = defaults(&declaration.name, home) else {
        return Ok(None);
    };
    let mut variables = Vec::new();
    let mut reached = |name: Option<&str>, default: PathBuf| -> Result<PathBuf, String> {
        match name.and_then(|name| Some((name, env(name).filter(|v| !v.is_empty())?))) {
            Some((name, value)) => {
                let value = PathBuf::from(value);
                if !value.is_absolute() {
                    return Err(format!(
                        "{} was not adopted: `{name}` names {}, which is not an absolute path",
                        declaration.title,
                        value.display()
                    ));
                }
                variables.push(name.to_owned());
                Ok(value)
            }
            None => Ok(default),
        }
    };
    let config = reached(Some(config_env), config_default)?;
    let data = match data_default {
        Some(default) => Some(reached(declaration.folders.data_env.as_deref(), default)?),
        None => None,
    };
    let own_folder = |folder: &Path| {
        if own.is_empty() {
            folder.to_path_buf()
        } else {
            folder.join(own)
        }
    };
    let found: Vec<PathBuf> = std::iter::once(&config)
        .chain(data.as_ref())
        .map(|folder| own_folder(folder))
        .filter(|folder| folder.is_dir())
        .collect();
    if found.is_empty() {
        return Ok(None);
    }
    for folder in &found {
        if let Some(why) = not_the_persons_own(folder) {
            return Err(format!("{} was not adopted: {why}", declaration.title));
        }
    }
    Ok(Some((Folders { config, data }, variables)))
}

/// Why `folder` is not the person's own harness folder, or `None` when it is: it is not this
/// user's, or it is inside a project or a git work tree, where what is in it is not only theirs.
fn not_the_persons_own(folder: &Path) -> Option<String> {
    let real = resolved(folder);
    for above in real.ancestors() {
        if above.join(".git").exists() {
            return Some(format!(
                "{} is inside the git work tree at {}",
                folder.display(),
                above.display()
            ));
        }
        if crate::names::has_manifest(above) {
            return Some(format!(
                "{} is inside the project at {}",
                folder.display(),
                above.display()
            ));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let me = rustix::process::getuid().as_raw();
        match std::fs::metadata(&real) {
            Ok(meta) if meta.uid() == me => {}
            Ok(_) => return Some(format!("{} belongs to another user", folder.display())),
            Err(e) => return Some(format!("{} could not be read: {e}", folder.display())),
        }
    }
    None
}

/// `folder` as the operating system reaches it.
fn resolved(folder: &Path) -> PathBuf {
    crate::datahome::resolved(folder)
}

/// Whether `a` and `b` are one folder, however each is spelled.
fn same_folder(a: &Path, b: &Path) -> bool {
    resolved(a) == resolved(b)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// **Adopts each harness folder this machine uses** as a profile, in the store under
/// `config_root`, and answers what it adopted and what it passed over: the upgrade of ADR 0095
/// §2. `home` is the person's home and `env` the environment chats are started from, which is
/// where a harness's folder variable is read. Runs no program, reads no file in a harness's
/// folder, and writes nothing there; writes the store only when it adopts something.
pub fn adopt(
    config_root: &Path,
    home: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Adoption, String> {
    update(config_root, |store| {
        let mut adoption = Adoption::default();
        for kind in crate::profiles::KINDS {
            let Some(declaration) = crate::harness_declaration::builtin(kind.word) else {
                continue;
            };
            let (folders, variables) = match machine_folders(declaration, home, env) {
                Ok(Some(found)) => found,
                Ok(None) => continue,
                Err(why) => {
                    adoption.passed_over.push(why);
                    continue;
                }
            };
            let taken = store.profiles.iter().any(|p| p.name == kind.word)
                || store.points_at(kind.word, &folders.config)
                || store.is_forgotten(&folders.config);
            if taken {
                continue;
            }
            let profile = MachineProfile {
                name: kind.word.to_owned(),
                harness: kind.word.to_owned(),
                title: format!("{} ({})", declaration.title, this_device()),
                folders,
                adopted: Some(Adopted {
                    at: now(),
                    variables,
                }),
                imported: None,
                sign_in: None,
                pinned: None,
                sandbox_only: false,
                kept_for: None,
                rest: Map::new(),
            };
            store.profiles.push(profile.clone());
            adoption.adopted.push(profile);
        }
        adoption
    })
}

/// [`adopt`] on this machine, as the app runs it at launch: its config home, the person's home
/// and the app's own environment, which is the one its chats are started from
/// ([`crate::chatenv`]). Nothing where there is no home.
pub fn adopt_at_launch() -> Result<Adoption, String> {
    let (Some(config), Some(home)) = (crate::machine::config_root(), dirs::home_dir()) else {
        return Ok(Adoption::default());
    };
    adopt(&config, &home, &crate::envvar::var)
}

pub mod installs;
mod made;
pub use made::*;

pub mod onrunner;
pub mod pin;
pub mod sandbox_only;
pub mod sent;
pub mod sign_in;

#[cfg(test)]
mod tests;
