// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's pins** (SK-15, #2778; ADR 0102 §4): the skills its workspace and its persona keep
//! always listed, which no router hides, read where its set is routed
//! ([`super::for_chat`] and every other ask that routes) and written by `purlis skill pin`.

use std::path::Path;

use super::{built_in_names_here, read, verbs};

/// **The skills the workspace `workspace` pins** (SK-15, ADR 0102 §4): `settings.skills.pinned`,
/// as the Workspace settings layer reads its `workspace.json`. Empty where it names none, or
/// holds something other than a list of names, which that layer's save refuses to write.
pub fn pinned_in(plane: &Path, workspace: &str) -> Vec<String> {
    super::names_in_settings(plane, workspace, crate::settings::workspace::SKILLS_PINNED)
}

/// **A chat's pins** (ADR 0102 §4): the skills its workspace pins (`settings.skills.pinned`)
/// and those its persona pins (`pinned-skills:`), each name once. A pin only keeps a skill of the
/// chat's set listed: it adds none.
pub fn pins_of(plane: &Path, workspace: Option<&str>, persona: Option<&str>) -> Vec<String> {
    let mut pins = workspace.map(|ws| pinned_in(plane, ws)).unwrap_or_default();
    for name in persona
        .map(|persona| crate::personaverbs::pinned_skills(plane, persona))
        .unwrap_or_default()
    {
        if !pins.contains(&name) {
            pins.push(name);
        }
    }
    pins
}

/// **Whose pins a pin changes** (SK-15, ADR 0102 §4): a workspace's or a persona's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinnedBy {
    Workspace(String),
    Persona(String),
}

/// **Pins `name` (`on`) or unpins it** for `by`'s chats (SK-15, ADR 0102 §4): the workspace's
/// `settings.skills.pinned`, through the Workspace level's own save, or the persona's
/// `pinned-skills:` line. `Ok(false)` where it already was so, and nothing is written. A pin only
/// keeps a skill of a chat's set listed, so a name no skill has is refused, as is a workspace or
/// persona the project does not have, and a project purlis opens read-only.
pub fn pin(plane: &Path, by: &PinnedBy, name: &str, on: bool) -> Result<bool, String> {
    crate::settings::writable(plane)?;
    if on && !any_skill_named(plane, name) {
        return Err(format!(
            "no skill is named {name:?}, so nothing was pinned: `purlis skill list` lists them"
        ));
    }
    let mut pins = match by {
        PinnedBy::Workspace(ws) => {
            let held = crate::workspaces::Plane::open(plane);
            if !held.workspace(ws).is_ok_and(|one| one.dir().is_dir()) {
                return Err(format!(
                    "this project has no workspace {ws:?}: `purlis workspace list` lists them"
                ));
            }
            pinned_in(plane, ws)
        }
        PinnedBy::Persona(persona) => crate::personaverbs::pinned_skills(plane, persona),
    };
    if pins.iter().any(|one| one == name) == on {
        return Ok(false);
    }
    if on {
        pins.push(name.to_owned());
    } else {
        pins.retain(|one| one != name);
    }
    match by {
        PinnedBy::Workspace(ws) => super::set_names_in_settings(
            plane,
            ws,
            crate::settings::workspace::SKILLS_PINNED,
            pins,
        )?,
        PinnedBy::Persona(persona) => super::persona::write_pins(plane, persona, &pins)?,
    }
    Ok(true)
}

/// Whether any skill a chat could get is named `name`: a built-in skill, or one at any level of
/// the plane.
fn any_skill_named(plane: &Path, name: &str) -> bool {
    built_in_names_here().iter().any(|one| one == name)
        || verbs::Level::every(plane)
            .iter()
            .filter_map(|level| level.dir(plane))
            .any(|dir| read(&dir).iter().any(|skill| skill.name == name))
}

#[cfg(test)]
mod tests;
