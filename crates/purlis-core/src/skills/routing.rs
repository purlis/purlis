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
pub(crate) fn pinned_in(plane: &Path, workspace: &str) -> Vec<String> {
    super::names_in_settings(plane, workspace, crate::settings::workspace::SKILLS_PINNED)
}

/// The most pins one chat reads, its workspace's and its persona's together.
pub(crate) const MOST_PINS: usize = 64;

/// **A chat's pins** (ADR 0102 §4): the skills its workspace pins (`settings.skills.pinned`)
/// and those its persona pins (`pinned-skills:`), each name once. A pin only keeps a skill of the
/// chat's set listed: it adds none.
///
/// Read from files a chat's project holds, so held to limits before anything is done with them:
/// at most [`MOST_PINS`], each a skill's name by the open format's grammar (64 characters at
/// most, [`purlis_skills::change::name_problem`]). Past either the chat has no pins at all, and
/// the sentence that says why: a pin only ever keeps a skill listed, so reading none fails
/// closed.
pub(crate) fn pins_of(
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
) -> Result<Vec<String>, String> {
    let written = workspace
        .map(|ws| pinned_in(plane, ws))
        .unwrap_or_default()
        .into_iter()
        .chain(
            persona
                .map(|persona| crate::personaverbs::pinned_skills(plane, persona))
                .unwrap_or_default(),
        );
    read_pins(written).map_err(|refused| match refused.bad {
        Some(_) => format!(
            "a pin of this chat's {}, so none of its pins are read",
            refused.why
        ),
        None => format!("this chat has more than {MOST_PINS} pins, so none of them are read"),
    })
}

/// **Why a list of pins is not read** (SK-15d, #3232): the entry that is not a skill's name, or
/// that it holds more than [`MOST_PINS`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinsRefused {
    /// What is wrong, in a clause: `"Not A Name" is not a skill's name (…)`, `the list holds more
    /// than 64 pins`.
    pub why: String,
    /// The entry that is not a skill's name, which unpinning takes out; `None` for too many.
    pub bad: Option<String>,
    /// The file the list is written in, relative to the project.
    pub file: String,
}

/// `written`, each name once, or why a chat reads none of it ([`pins_of`]).
fn read_pins(written: impl Iterator<Item = String>) -> Result<Vec<String>, PinsRefused> {
    let mut seen = std::collections::HashSet::new();
    let mut pins = Vec::new();
    for name in written {
        if let Some(problem) = purlis_skills::change::name_problem(&name) {
            return Err(PinsRefused {
                why: format!("{name:?} is not a skill's name ({problem})"),
                bad: Some(name),
                file: String::new(),
            });
        }
        if seen.insert(name.clone()) {
            pins.push(name);
        }
        if pins.len() > MOST_PINS {
            return Err(PinsRefused {
                why: format!("the list holds more than {MOST_PINS} pins"),
                bad: None,
                file: String::new(),
            });
        }
    }
    Ok(pins)
}

/// **Why a chat would read none of `by`'s pins**, or `None` where it reads them (SK-15d): the
/// pinner's own list held to the limits [`pins_of`] holds a chat's to, so a pin that has no
/// effect never looks as if it works.
pub fn pins_refused(plane: &Path, by: &PinnedBy) -> Option<PinsRefused> {
    read_pins(pins_written(plane, by).into_iter())
        .err()
        .map(|refused| PinsRefused {
            file: pins_file(plane, by),
            ..refused
        })
}

/// The file `by`'s pins are written in, relative to the project.
pub fn pins_file(plane: &Path, by: &PinnedBy) -> String {
    match by {
        PinnedBy::Workspace(ws) => crate::settings::workspace::named(ws),
        PinnedBy::Persona(persona) => {
            let path = crate::personas::def_path(plane, persona);
            path.strip_prefix(plane)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned()
        }
    }
}

/// **A chat's pins, or none where they are refused** ([`pins_of`]), the refusal said in the log:
/// what every set routed for a chat reads, so a refused list fails closed the same way everywhere.
pub(crate) fn pins_or_none(
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
) -> Vec<String> {
    pins_of(plane, workspace, persona).unwrap_or_else(|why| {
        tracing::warn!("purlis: {why}");
        Vec::new()
    })
}

/// **Whose pins a pin changes** (SK-15, ADR 0102 §4): a workspace's or a persona's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinnedBy {
    Workspace(String),
    Persona(String),
}

/// **The pins `by` has written** (SK-15d, #3232): the workspace's `settings.skills.pinned` or the
/// persona's `pinned-skills:`, as written, held to no limit: what the studio's Pin switch shows
/// and changes, where [`pins_of`] is what a chat reads.
pub fn pins_written(plane: &Path, by: &PinnedBy) -> Vec<String> {
    match by {
        PinnedBy::Workspace(ws) => pinned_in(plane, ws),
        PinnedBy::Persona(persona) => crate::personaverbs::pinned_skills(plane, persona),
    }
}

/// **Every workspace and persona of the project whose pins name `name`** (SK-15d, #3232): the
/// workspaces first, then the personas, each by name in order. What a skill's tab lists, so a
/// pin is always seen and can be undone.
pub fn pinners_of(plane: &Path, name: &str) -> Vec<PinnedBy> {
    let workspaces = crate::workspaces::Plane::open(plane)
        .workspaces()
        .unwrap_or_default()
        .into_iter()
        .map(PinnedBy::Workspace);
    let personas = crate::personaverbs::names(plane)
        .into_iter()
        .map(PinnedBy::Persona);
    workspaces
        .chain(personas)
        .filter(|by| pins_written(plane, by).iter().any(|one| one == name))
        .collect()
}

/// **Which of a chat's workspace and persona pin anything** (SK-15d, #3232): where its pins are
/// past the budget, where unpinning one is done.
pub(crate) fn pinners_of_a_chat(
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
) -> Vec<PinnedBy> {
    workspace
        .map(|ws| PinnedBy::Workspace(ws.to_owned()))
        .into_iter()
        .chain(persona.map(|p| PinnedBy::Persona(p.to_owned())))
        .filter(|by| !pins_written(plane, by).is_empty())
        .collect()
}

/// **Pins `name` (`on`) or unpins it** for `by`'s chats (SK-15, ADR 0102 §4): the workspace's
/// `settings.skills.pinned`, through the Workspace level's own save, or the persona's
/// `pinned-skills:` line. `Ok(false)` where it already was so, and nothing is written. A pin only
/// keeps a skill of a chat's set listed, so a name no skill has is refused, as is a workspace or
/// persona the project does not have, and a project purlis opens read-only.
pub(crate) fn pin(plane: &Path, by: &PinnedBy, name: &str, on: bool) -> Result<bool, String> {
    crate::settings::writable(plane)?;
    if on && !any_skill_named(plane, name) {
        return Err(format!(
            "no skill is named {name:?}, so nothing was pinned: `purlis skill list` lists them"
        ));
    }
    if let PinnedBy::Workspace(ws) = by {
        let held = crate::workspaces::Plane::open(plane);
        if !held.workspace(ws).is_ok_and(|one| one.dir().is_dir()) {
            return Err(format!(
                "this project has no workspace {ws:?}: `purlis workspace list` lists them"
            ));
        }
    }
    let mut pins = pins_written(plane, by);
    if pins.iter().any(|one| one == name) == on {
        return Ok(false);
    }
    // A chat reads none of a list past the most it reads, so a pin that would take it there is
    // refused rather than written to no effect (SK-15d).
    if on && pins.len() >= MOST_PINS {
        let whose = match by {
            PinnedBy::Workspace(ws) => ws,
            PinnedBy::Persona(persona) => persona,
        };
        return Err(format!(
            "{whose} already pins {MOST_PINS} skills, the most a chat reads, so {name} was not \
             pinned: unpin one first"
        ));
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
