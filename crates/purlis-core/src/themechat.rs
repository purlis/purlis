// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat that makes or edits a theme, from Settings › Theme** (spec #2313, #2333).
//!
//! The gallery's **New theme** and **Edit with a chat** open a chat with a prompt typed into it
//! and never sent, as a curation action does (ADR 0061): the person reads it and presses Enter.
//! Both name purlis's `purlis-theme` skill, which holds the procedure. New theme is purlis's own
//! "Create a theme" action ([`crate::curation::CREATE_THEME`]) on the level the gallery is at,
//! the same prompt the Curate menu types; Edit with a chat is that action with one more thing
//! named: the theme, by the id `purlis theme list` gives it, so the chat starts on the theme the
//! person pointed at.
//!
//! **The level is where the chat works**: a workspace's gallery opens it in that workspace, so
//! its theme draft is drawn there, and the project's at the plane root. The machine's level has
//! no project for a chat to work in, so nothing is opened from it.

use std::path::Path;

use crate::curation::{self, CREATE_THEME, Kind, Resolved, Subject};
use crate::theme::gallery::{Card, From};

/// What the gallery asked for.
#[derive(Debug, Clone, Copy)]
pub enum Asked<'a> {
    /// **New theme**: a theme made at this level.
    New,
    /// **Edit with a chat**: the theme of this card, as this level's gallery shows it.
    Edit(&'a Card),
}

/// What an Edit with a chat's tab says.
pub const EDIT_LABEL: &str = "Edit a theme";

/// The most characters a theme's id may have to be typed into a prompt.
const MOST_ID: usize = 96;

/// **The chat Settings › Theme opens** at the level of `workspace` (the project's with none):
/// who runs it, where, and the prompt typed into it. A refusal is a sentence.
pub fn chat(root: &Path, workspace: Option<&str>, asked: Asked<'_>) -> Result<Resolved, String> {
    let subject = Subject {
        kind: if workspace.is_some() {
            Kind::Workspace
        } else {
            Kind::Plane
        },
        name: workspace.map(str::to_owned),
    };
    let create = curation::resolve(root, &subject)?
        .actions
        .into_iter()
        .find(|offered| offered.id == crate::names::own_id(CREATE_THEME))
        .ok_or_else(|| "purlis offers no Create a theme here, so no chat was opened".to_owned())?;
    let Asked::Edit(card) = asked else {
        return Ok(create);
    };
    let theme = listed_id(card, workspace);
    if !typable(&theme) {
        return Err(format!(
            "{} has an id purlis does not type into a prompt, so no chat was opened: open a \
             chat and ask for the theme by its name",
            crate::shown::short(&card.name)
        ));
    }
    let name = workspace.map_or_else(
        || {
            root.file_name().map_or_else(
                || root.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            )
        },
        str::to_owned,
    );
    Ok(Resolved {
        label: EDIT_LABEL.to_owned(),
        prompt: format!(
            "Use the purlis-theme skill to edit the theme \"{theme}\" for the {} {name}.",
            subject.kind.as_str()
        ),
        ..create
    })
}

/// **The id `purlis theme list` gives the card's theme** at the level of `workspace`: a theme
/// folder's names its level (`project:dusk`, `workspace:web:dusk`, `machine:dusk`), a built-in
/// or an extension's theme is its own id.
pub fn listed_id(card: &Card, workspace: Option<&str>) -> String {
    match (&card.from, workspace) {
        (From::Machine, _) => format!("machine:{}", card.id),
        (From::Project, _) => format!("project:{}", card.id),
        (From::Workspace, Some(workspace)) => format!("workspace:{workspace}:{}", card.id),
        (From::Workspace, None) | (From::BuiltIn | From::Extension { .. }, _) => card.id.clone(),
    }
}

/// Whether `id` is typed into a prompt as it is: short, one line, and only the letters, digits
/// and marks an id is spelled with. A theme's name comes from its file, so nothing else of it is
/// typed.
fn typable(id: &str) -> bool {
    !id.is_empty()
        && id.chars().count() <= MOST_ID
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " -_./:()+".contains(c))
}

#[cfg(test)]
#[path = "themechat_tests.rs"]
mod tests;
