// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Skill studio's reads** (SK-2, #2765): every skill a chat in a project gets, with its
//! level, and every one left out with why, as `purlis_core::skills::in_the_studio` works them
//! out. Read only, and behind the `skill-studio` flag until its finish line (#2786).

use serde::Serialize;
use specta::Type;

/// The level a skill comes from, as the studio draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    BuiltIn,
    Project,
}

impl From<purlis_core::skills::Origin> for Level {
    fn from(origin: purlis_core::skills::Origin) -> Level {
        match origin {
            purlis_core::skills::Origin::BuiltIn => Level::BuiltIn,
            purlis_core::skills::Origin::Project => Level::Project,
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
    /// Whether a chat at the project's root gets it.
    pub on: bool,
    /// Why it is left out, in one line, where it is.
    pub left_out: Option<String>,
    /// What it does and when to use it: its frontmatter's `description`.
    pub description: String,
    /// Its folder, relative to the project where it is inside it.
    pub folder: String,
}

/// What the studio shows: whether it is on, and its skills by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct Studio {
    pub on: bool,
    pub skills: Vec<StudioSkill>,
}

/// The studio of the project at `root`, with the built-in skills in `built_in`: nothing while
/// the flag is off. Its rows are the command line's (`purlis_core::skills::studio_rows`).
pub(crate) fn studio(
    root: &std::path::Path,
    built_in: Option<&std::path::Path>,
    on: bool,
) -> Studio {
    if !on {
        return Studio {
            on,
            skills: Vec::new(),
        };
    }
    let set = purlis_core::skills::in_the_studio(built_in, root, None);
    let skills = purlis_core::skills::studio_rows(&set, root)
        .into_iter()
        .map(|row| StudioSkill {
            name: row.name,
            level: row.origin.into(),
            on: row.left_out.is_none(),
            left_out: row.left_out,
            description: row.description,
            folder: row.folder,
        })
        .collect();
    Studio { on, skills }
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

/// The Skill studio of the project: every built-in and project skill, with its level, and why
/// each left out is left out. On a blocking thread, as it reads the project's skill folders.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_studio(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<Studio, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the project's skills", move || {
        let built_in =
            crate::bundled_plugin_here().and_then(|plugin| purlis_core::skills::in_bundle(&plugin));
        Ok(studio(
            held.root(),
            built_in.as_deref(),
            purlis_core::skills::studio_on_here(),
        ))
    })
    .await
}

#[cfg(test)]
mod tests;
