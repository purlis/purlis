// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Skill studio's reads** (SK-2, #2765): every skill a chat in a project gets, with its
//! level, and every one left out with why, as `purlis_core::skills::in_the_studio` works them
//! out. Read only, and behind the `skill-studio` flag until its finish line (#2786).

use serde::Serialize;
use specta::Type;

/// One skill the studio lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StudioSkill {
    /// Its name, or its folder's where it names none.
    pub name: String,
    /// `built-in` or `project`.
    pub level: String,
    /// Whether a chat at the plane root gets it.
    pub on: bool,
    /// Why it is left out, in one line, where it is.
    pub left_out: Option<String>,
    /// What it does and when to use it: its frontmatter's `description`.
    pub description: String,
    /// Its folder, relative to the plane where it is inside it.
    pub folder: String,
}

/// What the studio shows: whether it is on, and its skills by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct Studio {
    pub on: bool,
    pub skills: Vec<StudioSkill>,
}

/// The studio of the project at `root`, with the built-in skills in `built_in`: nothing while
/// the flag is off.
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
    let folder = |path: &std::path::Path| {
        path.strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned()
    };
    let mut skills: Vec<StudioSkill> = set
        .effective
        .iter()
        .map(|e| StudioSkill {
            name: e.skill.name.clone(),
            level: e.origin.level().to_owned(),
            on: true,
            left_out: None,
            description: e.skill.description.clone(),
            folder: folder(e.skill.path.parent().unwrap_or(&e.skill.path)),
        })
        .chain(set.left_out.iter().map(|l| StudioSkill {
            name: l.name.clone(),
            level: l.origin.level().to_owned(),
            on: false,
            left_out: Some(l.reason.to_string()),
            description: String::new(),
            folder: folder(&l.folder),
        }))
        .collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name).then(a.level.cmp(&b.level)));
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
