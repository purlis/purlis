// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! This machine's own profiles: `<config>/profiles.json` (ADR 0095, spec #2030).
//!
//! **A profile here belongs to this machine and is available in every project on it**, where a
//! profile in a clone's `charter.local.toml` belongs to that clone ([`crate::profiles`]). This
//! module is the one place the file is read and written; the profiles the window makes (#2034)
//! are records in it too.
//!
//! **The first records are adopted, never made** (ADR 0095 §2). On upgrade, each harness folder
//! this machine already uses becomes a profile in place: the same folder, so the same sign-in,
//! the same settings and the same conversations. Nothing is copied, moved or written in the
//! harness's folder, and no file in it is read: whether the folder is there is all adoption
//! asks of it. The folder is the one a chat started on the harness's built-in profile reaches:
//! the one the harness's own folder variable names (`[folders]` of its declaration, ADR 0073),
//! where the environment sets it, and otherwise the harness's own default.
//!
//! **An adopted profile keeps its built-in's name** (`claude`, `codex`, `opencode`), which is
//! what every chat record, persona and dispatch list already names, and is shown by its title,
//! *Claude Code (this Mac)*. So a chat's next run is on the adopted profile with nothing
//! rewritten, and it resumes its own conversation from the folder it was always in.
//!
//! **Adoption is idempotent.** A folder a record already points at, or a harness whose
//! built-in name a record already has, is left alone, and a run that adopts nothing writes
//! nothing.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The file, in purlis's directory of the config home.
pub const FILE: &str = "profiles.json";

/// The lock a read-modify-write of [`FILE`] holds, beside it.
const LOCK: &str = "profiles.json.lock";

/// The version [`FILE`] is written at.
const VERSION: u64 = 1;

/// The most [`FILE`] may be. A machine has a handful of profiles.
const MOST_BYTES: u64 = 1 << 20;

/// Where [`FILE`] is under the config home `config_root`.
pub fn path(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join(FILE)
}

/// One profile of this machine's.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub adopted: Option<Adopted>,
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

/// A record as its JSON object has it.
#[derive(Serialize, Deserialize)]
struct Written {
    name: String,
    harness: String,
    title: String,
    folders: Folders,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    adopted: Option<Adopted>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

/// The file as read: every record as it was written, so a field a later purlis adds, and a
/// record this one cannot read, survive a write by this one.
#[derive(Default, Serialize, Deserialize)]
struct OnDisk {
    #[serde(default)]
    version: u64,
    #[serde(default)]
    profiles: Vec<Value>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

fn profile_of(value: &Value) -> Option<MachineProfile> {
    let written: Written = serde_json::from_value(value.clone()).ok()?;
    Some(MachineProfile {
        name: written.name,
        harness: written.harness,
        title: written.title,
        folders: written.folders,
        adopted: written.adopted,
    })
}

fn written(profile: &MachineProfile) -> Value {
    serde_json::to_value(Written {
        name: profile.name.clone(),
        harness: profile.harness.clone(),
        title: profile.title.clone(),
        folders: profile.folders.clone(),
        adopted: profile.adopted.clone(),
        rest: Map::new(),
    })
    .unwrap_or(Value::Null)
}

/// The file, or an empty one when there is none yet. One that cannot be read is an error, never
/// an empty list: writing over it would lose every profile in it.
fn read(config_root: &Path) -> Result<OnDisk, String> {
    let at = path(config_root);
    let file = match std::fs::File::open(&at) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(OnDisk::default()),
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
    serde_json::from_str(&text)
        .map_err(|e| format!("{} is not a list of profiles: {e}", at.display()))
}

fn write(config_root: &Path, mut file: OnDisk) -> Result<(), String> {
    file.version = VERSION;
    let at = path(config_root);
    let dir = crate::machine::private_dir(config_root)
        .map_err(|e| format!("purlis cannot write {}: {e}", at.display()))?;
    let text = serde_json::to_string_pretty(&file).unwrap_or_default() + "\n";
    crate::machine::write_beside(config_root, &dir.join(FILE), text.as_bytes())
        .map_err(|e| format!("purlis cannot write {}: {e}", at.display()))
}

/// This machine's profiles, in the order they were added. A record this purlis cannot read is
/// left out, and kept in the file.
pub fn list(config_root: &Path) -> Result<Vec<MachineProfile>, String> {
    Ok(read(config_root)?
        .profiles
        .iter()
        .filter_map(profile_of)
        .collect())
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
fn defaults(harness: &str, home: &Path) -> Option<(PathBuf, Option<PathBuf>, &'static str)> {
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

/// What this device is called in a profile's title.
pub fn this_device() -> &'static str {
    if cfg!(target_os = "macos") {
        "this Mac"
    } else {
        "this computer"
    }
}

/// The folder a profile's harness keeps its own files in, as a chat on the harness's built-in
/// profile reaches it in the environment `env`, and the variables that were set to reach it.
/// `None` when the harness has no folder there, or declares no folder variable.
fn machine_folders(
    declaration: &crate::harness_declaration::Declaration,
    home: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<(Folders, Vec<String>)> {
    let config_env = declaration.folders.config_env.as_deref()?;
    let (config_default, data_default, own) = defaults(&declaration.name, home)?;
    let mut variables = Vec::new();
    let mut reached = |name: Option<&str>, default: PathBuf| match name
        .and_then(|name| Some((name, env(name).filter(|v| !v.is_empty())?)))
    {
        Some((name, value)) => {
            variables.push(name.to_owned());
            PathBuf::from(value)
        }
        None => default,
    };
    let config = reached(Some(config_env), config_default);
    let data =
        data_default.map(|default| reached(declaration.folders.data_env.as_deref(), default));
    let own_folder = |folder: &Path| {
        if own.is_empty() {
            folder.to_path_buf()
        } else {
            folder.join(own)
        }
    };
    let there =
        own_folder(&config).is_dir() || data.as_deref().is_some_and(|d| own_folder(d).is_dir());
    there.then_some((Folders { config, data }, variables))
}

/// Whether `a` and `b` are one folder, however each is spelled.
fn same_folder(a: &Path, b: &Path) -> bool {
    let resolved = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    resolved(a) == resolved(b)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// **Adopts each harness folder this machine uses** as a profile, in the store under
/// `config_root`, and answers what it adopted: the upgrade of ADR 0095 §2. `home` is the
/// person's home and `env` the environment chats are started from, which is where a harness's
/// folder variable is read. Runs no program, reads no file in a harness's folder, and writes
/// nothing there; writes the store only when it adopts something.
pub fn adopt(
    config_root: &Path,
    home: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Vec<MachineProfile>, String> {
    let _held = crate::machine::Lock::named(config_root, LOCK);
    let mut file = read(config_root)?;
    let mut known: Vec<MachineProfile> = file.profiles.iter().filter_map(profile_of).collect();
    let mut adopted = Vec::new();
    for kind in crate::profiles::KINDS {
        let Some(declaration) = crate::harness_declaration::builtin(kind.word) else {
            continue;
        };
        let Some((folders, variables)) = machine_folders(declaration, home, env) else {
            continue;
        };
        let taken = known.iter().any(|p| {
            p.name == kind.word
                || (p.harness == kind.word && same_folder(&p.folders.config, &folders.config))
        });
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
        };
        file.profiles.push(written(&profile));
        known.push(profile.clone());
        adopted.push(profile);
    }
    if !adopted.is_empty() {
        write(config_root, file)?;
    }
    Ok(adopted)
}

/// [`adopt`] on this machine, as the app runs it at launch: its config home, the person's home
/// and the app's own environment, which is the one its chats are started from
/// ([`crate::chatenv`]). Nothing where there is no home.
pub fn adopt_at_launch() -> Result<Vec<MachineProfile>, String> {
    let (Some(config), Some(home)) = (crate::machine::config_root(), dirs::home_dir()) else {
        return Ok(Vec::new());
    };
    adopt(&config, &home, &crate::envvar::var)
}

#[cfg(test)]
mod tests;
