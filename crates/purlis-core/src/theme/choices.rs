// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The person's theme choices** (spec #2313, #2323): which theme, and which of its
//! appearances, they chose for this machine, for each project and for each workspace in one.
//!
//! **Machine config, keyed by project id, never committed**: a file beside `machine.json`
//! ([`FILE`]), for the reason [`crate::windowprefs`] gives for the layout and the theme, which
//! are files beside it too: how one person likes their window is not a fact about the plane, so
//! it is not committed, and it is not one of the machine store's few things. A project is keyed by
//! its stable id ([`super::levels::project_key`]), so a re-clone of the same project finds the
//! choices again.
//!
//! ```json
//! {
//!   "version": 1,
//!   "machine": { "theme": "dusk", "appearance": "system" },
//!   "projects": {
//!     "01J…": {
//!       "theme": "purlis-classic", "appearance": "light",
//!       "workspaces": { "alpha": { "theme": "brand" } }
//!     }
//!   }
//! }
//! ```
//!
//! **Bounded, least recently used out first**: each project's entry keeps when it was last
//! changed (`used`, seconds since the epoch), and a change that would take the file past its cap
//! drops the projects changed longest ago until it fits, never the one being changed.
//!
//! **Everything read back is untrusted**, as every file beside `machine.json` is: read through
//! the same bounded, link-refusing read ([`crate::machine::read_beside`]), every choice held to
//! the grammar a pick has, and an entry that is not one is left out. A file purlis cannot read
//! is never written over.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde_json::{Map, Value, json};

use super::levels::Choice;

/// The file, inside purlis's own folder in the config home.
pub const FILE: &str = "theme-choices.json";

/// The version of the file purlis writes and reads. A file of another version holds no choice
/// for this purlis, and is not written over.
pub const VERSION: u64 = 1;

/// The most the file may be: the one cap of every file beside `machine.json`.
const MOST_BYTES: u64 = crate::windowprefs::MAX_BYTES;

/// The lock a change to the file holds, beside it.
const LOCK: &str = "theme-choices.json.lock";

/// The person's choices for one project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectChoices {
    /// For the project (level 3), and at its plane root.
    pub project: Option<Choice>,
    /// For each workspace in it (level 1), by the workspace's name.
    pub workspaces: BTreeMap<String, Choice>,
    /// The LIVE workspaces whose committed default the person was told now comes before their
    /// choice for the project (#2323): each told once.
    pub told: Vec<String>,
    /// When the entry was last changed, in seconds since the epoch: what the least recently
    /// used entries are dropped by at the cap.
    pub used: u64,
}

/// Every choice the person made on this machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choices {
    /// The machine default (level 5).
    pub machine: Option<Choice>,
    /// Each project's, by its id.
    pub projects: BTreeMap<String, ProjectChoices>,
}

/// Where a choice is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot<'a> {
    /// The machine default.
    Machine,
    /// The project of this id.
    Project(&'a str),
    /// The workspace of this name in the project of this id.
    Workspace(&'a str, &'a str),
}

/// **The person's choices under `config_root`.** No file, a file purlis cannot read and a file
/// of another version all hold none; an entry that is not a choice is left out.
pub fn read(config_root: &Path) -> Choices {
    read_held(config_root).ok().flatten().unwrap_or_default()
}

/// The choices, `None` for no file or one of another version, or why the file cannot be read.
fn read_held(config_root: &Path) -> Result<Option<Choices>, String> {
    let text = crate::machine::read_beside(config_root, FILE, MOST_BYTES, "the theme choices")
        .map_err(|why| why.to_string())?;
    let Some(text) = text else {
        return Ok(None);
    };
    let document: Value =
        serde_json::from_str(&text).map_err(|why| format!("{FILE} is not JSON: {why}"))?;
    let Some(top) = document.as_object() else {
        return Err(format!("{FILE} is not a JSON object"));
    };
    if top.get("version").and_then(Value::as_u64) != Some(VERSION) {
        return Err(format!("{FILE} is not version {VERSION}"));
    }
    let mut choices = Choices {
        machine: top.get("machine").and_then(choice_of),
        ..Choices::default()
    };
    for (id, project) in top
        .get("projects")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        if crate::reopen::a_ulid(id).as_deref() != Some(id.as_str()) {
            continue;
        }
        let workspaces = project
            .get("workspaces")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter(|(name, _)| crate::contain::workspace_name_ok(name))
            .filter_map(|(name, one)| choice_of(one).map(|choice| (name.clone(), choice)))
            .collect();
        let told = project
            .get("told")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|name| crate::contain::workspace_name_ok(name))
            .map(str::to_owned)
            .collect();
        choices.projects.insert(
            id.clone(),
            ProjectChoices {
                project: choice_of(project),
                workspaces,
                told,
                used: project.get("used").and_then(Value::as_u64).unwrap_or(0),
            },
        );
    }
    Ok(Some(choices))
}

/// The choice an entry holds, or `None` when it holds none or not one.
fn choice_of(entry: &Value) -> Option<Choice> {
    let theme = entry.get("theme")?.as_str()?;
    let appearance = match entry.get("appearance") {
        None => None,
        Some(Value::String(it)) => Some(it.as_str()),
        Some(_) => return None,
    };
    Choice::new(theme, appearance)
}

/// The entry a choice is written as.
fn entry_of(choice: &Choice) -> Map<String, Value> {
    let mut entry = Map::new();
    entry.insert("theme".into(), Value::String(choice.theme.clone()));
    if let Some(appearance) = &choice.appearance {
        entry.insert("appearance".into(), Value::String(appearance.clone()));
    }
    entry
}

/// The file's document for `choices`.
fn document_of(choices: &Choices) -> Value {
    let mut projects = Map::new();
    for (id, project) in &choices.projects {
        let mut entry = project.project.as_ref().map(entry_of).unwrap_or_default();
        if !project.workspaces.is_empty() {
            let workspaces: Map<String, Value> = project
                .workspaces
                .iter()
                .map(|(name, choice)| (name.clone(), Value::Object(entry_of(choice))))
                .collect();
            entry.insert("workspaces".into(), Value::Object(workspaces));
        }
        if !project.told.is_empty() {
            entry.insert("told".into(), json!(project.told));
        }
        if !entry.is_empty() {
            entry.insert("used".into(), json!(project.used));
            projects.insert(id.clone(), Value::Object(entry));
        }
    }
    let mut top = json!({ "version": VERSION, "projects": projects });
    if let Some(machine) = &choices.machine {
        top["machine"] = Value::Object(entry_of(machine));
    }
    top
}

/// **Changes the person's choices** with `change`, under the file's lock: read, changed and
/// written as one act, so two windows choosing at once both keep theirs. A file purlis cannot
/// read is refused and left as it is; a change that would take the file past its cap is refused.
pub fn edit(config_root: &Path, change: impl FnOnce(&mut Choices)) -> io::Result<()> {
    let _held = crate::machine::Lock::named(config_root, LOCK);
    let mut choices = match read_held(config_root) {
        Ok(held) => held.unwrap_or_default(),
        Err(why) => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("purlis will not write into theme choices it could not read: {why}"),
            ));
        }
    };
    let before = choices.clone();
    change(&mut choices);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let mut changed = Vec::new();
    for (id, project) in &mut choices.projects {
        if before.projects.get(id) != Some(project) {
            project.used = now;
            changed.push(id.clone());
        }
    }
    let text = within_cap(&mut choices, &changed)?;
    let dir = crate::machine::private_dir(config_root)?;
    crate::machine::write_beside(config_root, &dir.join(FILE), text.as_bytes())
}

/// The file's text for `choices`, under the cap: the projects changed longest ago, other than
/// `changed`, dropped first until it fits.
fn within_cap(choices: &mut Choices, changed: &[String]) -> io::Result<String> {
    loop {
        let text = serde_json::to_string_pretty(&document_of(choices))
            .expect("a document of strings can always be written")
            + "\n";
        if text.len() as u64 <= MOST_BYTES {
            return Ok(text);
        }
        let oldest = choices
            .projects
            .iter()
            .filter(|(id, _)| !changed.contains(id))
            .min_by_key(|(id, project)| (project.used, (*id).clone()))
            .map(|(id, _)| id.clone());
        let Some(oldest) = oldest else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "the theme choices would be past {MOST_BYTES} bytes, so nothing was changed"
                ),
            ));
        };
        choices.projects.remove(&oldest);
    }
}

/// **Keeps `choice` at `slot`**, or takes the choice there away with `None`. A project id that is
/// not a ULID, or a workspace name that is not one, is refused.
pub fn set(config_root: &Path, slot: Slot<'_>, choice: Option<Choice>) -> io::Result<()> {
    let refused = |why: String| Err(io::Error::new(io::ErrorKind::InvalidInput, why));
    let project = match slot {
        Slot::Machine => None,
        Slot::Project(id) | Slot::Workspace(id, _) => Some(id),
    };
    if let Some(id) = project
        && crate::reopen::a_ulid(id).as_deref() != Some(id)
    {
        return refused(format!("{id:?} is not a project's id"));
    }
    if let Slot::Workspace(_, name) = slot
        && !crate::contain::workspace_name_ok(name)
    {
        return refused(format!("{name:?} is not a workspace's name"));
    }
    edit(config_root, |held| match slot {
        Slot::Machine => held.machine = choice,
        Slot::Project(id) => {
            let mine = held.projects.entry(id.to_owned()).or_default();
            mine.project = choice;
        }
        Slot::Workspace(id, name) => {
            let mine = held.projects.entry(id.to_owned()).or_default();
            match choice {
                Some(choice) => {
                    mine.workspaces.insert(name.to_owned(), choice);
                }
                None => {
                    mine.workspaces.remove(name);
                }
            }
        }
    })
}
