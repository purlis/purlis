// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Skill studio's reads** (SK-2, #2765; SK-4, #2767): every skill a chat in a project
//! gets, at its root or in one workspace, with its level, and every one left out with why, as
//! `purlis_core::skills::in_the_studio` works them out; and Share, which makes a local skill
//! shared. Behind the `skill-studio` flag until its finish line (#2786).

use serde::Serialize;
use specta::Type;

/// The level a skill comes from, as the studio draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    BuiltIn,
    Project,
    Workspace,
}

impl From<purlis_core::skills::Origin> for Level {
    fn from(origin: purlis_core::skills::Origin) -> Level {
        match origin {
            purlis_core::skills::Origin::BuiltIn => Level::BuiltIn,
            purlis_core::skills::Origin::Project => Level::Project,
            purlis_core::skills::Origin::Workspace => Level::Workspace,
            // A level a later slice adds is drawn as the project's until the studio names it.
            _ => Level::Project,
        }
    }
}

/// One skill the studio lists: [`purlis_core::skills::StudioRow`], for the window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StudioSkill {
    /// Its name, or its folder's where it names none.
    pub name: String,
    pub level: Level,
    /// Its level as a person reads it: `project`, `local workspace`, the words the command line
    /// shows (`purlis_core::skills::level_shown`).
    pub label: String,
    /// Whether it is a local skill, kept on this machine in its level's `skills.local/`, which
    /// Share makes shared.
    pub local: bool,
    /// Where a project skill reaches: `root`, `all`, or the workspaces' names. `None` for a
    /// skill of another level.
    pub grant: Option<String>,
    /// Whether the chat the studio shows gets it.
    pub on: bool,
    /// Why it is left out, in one line, where it is.
    pub left_out: Option<String>,
    /// What it does and when to use it: its frontmatter's `description`.
    pub description: String,
    /// Its folder, relative to the project where it is inside it.
    pub folder: String,
}

/// What the studio shows: whether it is on, whose chat it shows, and its skills by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct Studio {
    pub on: bool,
    /// The workspace whose chats it shows, or `None` for a chat at the project's root.
    pub workspace: Option<String>,
    /// Every workspace of the project, which it can be shown for.
    pub workspaces: Vec<String>,
    pub skills: Vec<StudioSkill>,
}

/// The studio of the project at `root`, for a chat at its root (`workspace` `None`) or in
/// `workspace`, with the built-in skills in `built_in`: nothing while the flag is off. Its rows
/// are the command line's (`purlis_core::skills::studio_rows`).
pub(crate) fn studio(
    root: &std::path::Path,
    built_in: Option<&std::path::Path>,
    on: bool,
    workspace: Option<&str>,
) -> Studio {
    let workspace = workspace.map(str::to_owned);
    if !on {
        return Studio {
            on,
            workspace,
            workspaces: Vec::new(),
            skills: Vec::new(),
        };
    }
    let set = purlis_core::skills::in_the_studio(built_in, root, workspace.as_deref());
    let skills = purlis_core::skills::studio_rows(&set, root)
        .into_iter()
        .map(|row| StudioSkill {
            name: row.name,
            level: row.origin.into(),
            label: purlis_core::skills::level_shown(row.origin, row.local),
            local: row.local,
            grant: row.grant,
            on: row.left_out.is_none(),
            left_out: row.left_out,
            description: row.description,
            folder: row.folder,
        })
        .collect();
    let workspaces = purlis_core::workspaces::Plane::open(root)
        .workspaces()
        .unwrap_or_default();
    Studio {
        on,
        workspace,
        workspaces,
        skills,
    }
}

/// One skill run, as a skill's Runs panel lists it (SK-3): what started it and its signals,
/// never a pass or a fail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StudioRun {
    /// Its id, or null for a use recorded before runs were.
    pub id: Option<String>,
    pub skill: String,
    pub persona: Option<String>,
    /// `skill-tool` or `read`, or null for a use recorded before runs were.
    pub started_by: Option<String>,
    /// When it started.
    pub started: String,
    /// How it ended (`turn`, `interrupted` or `session`), or null while no end is recorded.
    pub ended: Option<String>,
    pub ms: Option<u32>,
    pub tool_calls: Option<u32>,
    pub tool_errors: Option<u32>,
    pub refusals: Option<u32>,
    pub asks: Option<u32>,
    /// Tokens spent inside it, where the harness reported them.
    pub tokens: Option<u32>,
}

/// Every run of `skill` the project at `root` has recorded, newest first, at most `limit`.
pub(crate) fn runs(root: &std::path::Path, skill: &str, limit: usize) -> Vec<StudioRun> {
    let small = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    purlis_core::skilluse::runs(root)
        .into_iter()
        .filter(|run| run.is_of(skill))
        .take(limit)
        .map(|run| {
            let end = run.end.as_ref();
            StudioRun {
                id: run.id.clone(),
                skill: run.skill.clone(),
                persona: run.persona.clone(),
                started_by: run.started_by.clone(),
                started: run.started.clone(),
                ended: end.map(|end| end.ended.clone()),
                ms: end.map(|end| small(end.ms)),
                tool_calls: end.map(|end| small(end.tool_calls)),
                tool_errors: end.map(|end| small(end.tool_errors)),
                refusals: end.map(|end| small(end.refusals)),
                asks: end.map(|end| small(end.asks)),
                tokens: end.and_then(|end| end.tokens).map(small),
            }
        })
        .collect()
}

/// How many runs a skill's Runs panel lists at most.
const RUNS_SHOWN: usize = 200;

/// The runs of skill `skill` in the project, newest first, for its Runs panel: none while the
/// flag is off. On a blocking thread, as it reads the project's skill log.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_runs(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    skill: String,
) -> Result<Vec<StudioRun>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the skill's runs", move || {
        if !purlis_core::skills::studio_on_here() {
            return Ok(Vec::new());
        }
        Ok(runs(held.root(), &skill, RUNS_SHOWN))
    })
    .await
}

/// Whether the Skill studio is on in this window: this machine's flag, or the variable over it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_studio_on() -> Result<bool, String> {
    crate::off_the_window("reading this machine's flags", || {
        Ok(purlis_core::skills::studio_on_here())
    })
    .await
}

/// **Turns the Skill studio on or off on this machine** (Settings › You › Previews), then says
/// whether it is on, which `PURLIS_SKILL_STUDIO` still decides where it is set.
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_skill_studio_on(on: bool) -> Result<bool, String> {
    crate::off_the_window("turning the Skill studio preview on or off", move || {
        let root = purlis_core::machine::config_root()
            .ok_or_else(|| "There is no config home on this machine to keep it in.".to_owned())?;
        purlis_core::skills::set_studio_on(&root, on)?;
        Ok(purlis_core::skills::studio_on_here())
    })
    .await
}

/// The Skill studio of the project, for a chat at its root or in `workspace`: every skill with
/// its level, and why each left out is left out. On a blocking thread, as it reads the project's
/// skill folders.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_studio(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
) -> Result<Studio, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the project's skills", move || {
        let built_in =
            crate::bundled_plugin_here().and_then(|plugin| purlis_core::skills::in_bundle(&plugin));
        Ok(studio(
            held.root(),
            built_in.as_deref(),
            purlis_core::skills::studio_on_here(),
            workspace.as_deref(),
        ))
    })
    .await
}

/// **Share** (#2767): moves the local skill `name` of the project's root (`workspace` `None`) or
/// of `workspace` into that level's `skills/`, so the project's save commits it, or says why not.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_share(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
    name: String,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("sharing the skill", move || {
        if !purlis_core::skills::studio_on_here() {
            return Err("The Skill studio is not turned on on this machine.".to_owned());
        }
        purlis_core::skills::share_here(held.root(), workspace.as_deref(), &name).map(|_| ())
    })
    .await
}

#[cfg(test)]
mod tests;
