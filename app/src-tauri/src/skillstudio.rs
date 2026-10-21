// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Skill studio's reads** (SK-2, #2765; SK-4, #2767; SK-5, #2768): every skill a chat in a
//! project gets, at its root, in one workspace or as one persona, with its level, and every one
//! left out with why, as `purlis_core::skills::in_the_studio` works them out; Share, which makes
//! a local skill shared; and a persona's `skills:` list, which its page adds to and takes from.
//! Behind the `skill-studio` flag until its finish line (#2786).

use serde::Serialize;
use specta::Type;

/// The level a skill comes from, as the studio draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    BuiltIn,
    Project,
    Workspace,
    Persona,
    /// A harness skill: the harness's own, which purlis never hands over.
    Installed,
}

impl From<purlis_core::skills::Origin> for Level {
    fn from(origin: purlis_core::skills::Origin) -> Level {
        match origin {
            purlis_core::skills::Origin::BuiltIn => Level::BuiltIn,
            purlis_core::skills::Origin::Project => Level::Project,
            purlis_core::skills::Origin::Workspace => Level::Workspace,
            purlis_core::skills::Origin::Persona => Level::Persona,
            purlis_core::skills::Origin::Harness => Level::Installed,
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
    /// Its folder, relative to the project where it is inside it; empty for an installed skill.
    pub folder: String,
    /// How a persona's skills name it: `purlis:<name>` for a built-in skill, `<plugin>:<name>`
    /// for a plugin's, else its name (#2768). What Add and Remove send.
    pub written: String,
    /// Whether the persona the studio shows names it, so Remove can take it out.
    pub named: bool,
    /// What is said of an installed skill the persona names that purlis cannot check this
    /// machine for, or `None`.
    pub unchecked: Option<String>,
}

/// What the studio shows: whether it is on, whose chat it shows, and its skills by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct Studio {
    pub on: bool,
    /// The workspace whose chats it shows, or `None` for a chat at the project's root.
    pub workspace: Option<String>,
    /// The persona whose chats it shows, which brings its skills into every workspace, or
    /// `None` for a chat on no persona (SK-5).
    pub persona: Option<String>,
    /// Every workspace of the project, which it can be shown for.
    pub workspaces: Vec<String>,
    pub skills: Vec<StudioSkill>,
}

/// The studio of the project at `root`, for a chat at its root (`workspace` `None`) or in
/// `workspace`, as `persona` or on none, with the built-in skills in `built_in`: nothing while
/// the flag is off. Its rows are the command line's (`purlis_core::skills::studio_rows`).
pub(crate) fn studio(
    root: &std::path::Path,
    built_in: Option<&std::path::Path>,
    on: bool,
    workspace: Option<&str>,
    persona: Option<&str>,
) -> Studio {
    let workspace = workspace.map(str::to_owned);
    let persona = persona.map(str::to_owned);
    if !on {
        return Studio {
            on,
            workspace,
            persona,
            workspaces: Vec::new(),
            skills: Vec::new(),
        };
    }
    let set = purlis_core::skills::in_the_studio(
        built_in,
        root,
        workspace.as_deref(),
        persona.as_deref(),
    );
    let names = persona
        .as_deref()
        .map(|persona| purlis_core::skills::names_of(root, persona))
        .unwrap_or_default();
    let skills = purlis_core::skills::studio_rows(&set, root, &names)
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
            written: row.written,
            named: row.named,
            unchecked: row
                .unchecked
                .then(|| purlis_core::skills::UNCHECKED.to_owned()),
        })
        .collect();
    let workspaces = purlis_core::workspaces::Plane::open(root)
        .workspaces()
        .unwrap_or_default();
    Studio {
        on,
        workspace,
        persona,
        workspaces,
        skills,
    }
}

/// What started a skill run, as the Runs panel draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum RunStart {
    /// The chat's own Skill tool.
    SkillTool,
    /// The chat read the skill's `SKILL.md`.
    Read,
    /// A start a newer purlis wrote.
    Unknown,
}

impl From<purlis_core::skills::runs::StartedBy> for RunStart {
    fn from(started_by: purlis_core::skills::runs::StartedBy) -> RunStart {
        use purlis_core::skills::runs::StartedBy;
        match started_by {
            StartedBy::SkillTool => RunStart::SkillTool,
            StartedBy::Read => RunStart::Read,
            _ => RunStart::Unknown,
        }
    }
}

/// How a skill run ended, as the Runs panel draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum RunEnd {
    /// With its turn.
    Turn,
    /// A new prompt came first.
    Interrupted,
    /// The chat ended under it.
    Session,
    /// An end a newer purlis wrote.
    Unknown,
}

impl From<purlis_core::skills::runs::Ended> for RunEnd {
    fn from(ended: purlis_core::skills::runs::Ended) -> RunEnd {
        use purlis_core::skills::runs::Ended;
        match ended {
            Ended::Turn => RunEnd::Turn,
            Ended::Interrupted => RunEnd::Interrupted,
            Ended::Session => RunEnd::Session,
            _ => RunEnd::Unknown,
        }
    }
}

/// One skill run, as a skill's Runs panel lists it (SK-3): what started it and its signals,
/// never a pass or a fail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StudioRun {
    /// Its id, or null for a use from before runs were recorded.
    pub id: Option<String>,
    pub skill: String,
    pub persona: Option<String>,
    /// What started it, or null for a use from before runs were recorded.
    pub started_by: Option<RunStart>,
    /// When it started.
    pub started: String,
    /// How it ended, or null while no end is recorded.
    pub ended: Option<RunEnd>,
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
    purlis_core::skills::studio_runs(root, Some(skill), limit)
        .into_iter()
        .map(|run| {
            let end = run.end.as_ref();
            StudioRun {
                id: run.id.clone(),
                skill: run.skill.clone(),
                persona: run.persona.clone(),
                started_by: run.started_by.map(RunStart::from),
                started: run.started.clone(),
                ended: end.map(|end| RunEnd::from(end.ended)),
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

/// The Skill studio of the project, for a chat at its root or in `workspace`, as `persona` or on
/// none: every skill with its level, and why each left out is left out. On a blocking thread, as
/// it reads the project's skill folders.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_studio(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
    persona: Option<String>,
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
            persona.as_deref(),
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

/// What a change to a persona's skills does, as the studio asks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum PersonaSkillChange {
    Add,
    Remove,
}

impl From<PersonaSkillChange> for purlis_core::skills::persona::Change {
    fn from(change: PersonaSkillChange) -> Self {
        match change {
            PersonaSkillChange::Add => Self::Add,
            PersonaSkillChange::Remove => Self::Remove,
        }
    }
}

/// **Adds a skill to a persona's skills, or takes one out** (SK-5, #2768): Add and Remove in the
/// Skill studio filtered to the persona. `written` is the skill as the studio's row writes it.
/// Answers whether the persona's definition changed. Refused while the studio's flag is off.
#[tauri::command]
#[specta::specta]
pub(crate) async fn persona_skill(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    persona: String,
    written: String,
    change: PersonaSkillChange,
) -> Result<bool, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("changing the persona's skills", move || {
        if !purlis_core::skills::studio_on_here() {
            return Err("The Skill studio is off on this machine.".to_owned());
        }
        let changed =
            purlis_core::skills::persona::change(held.root(), &persona, &written, change.into())?;
        if changed {
            held.wrote(&["personas".to_owned()]);
        }
        Ok(changed)
    })
    .await
}

/// **The skills a chat's persona names that it started without** (SK-5, #2768): what its tab's
/// Notice says, until it is put away. `None` for a chat that has every one.
#[tauri::command]
#[specta::specta]
pub(crate) async fn chat_skills_missing(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
) -> Result<Option<crate::chats::SkillsMissing>, String> {
    let held = planes.held(&plane)?;
    Ok(held.chats().skills_missing(session))
}

/// Puts a chat's missing-skills Notice away.
#[tauri::command]
#[specta::specta]
pub(crate) async fn put_away_chat_skills_missing(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    held.chats().put_away_skills_missing(session);
    Ok(())
}

#[cfg(test)]
mod tests;
