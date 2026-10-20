// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A path, said relative to the level it came from** (#2763).
//!
//! Every sentence the engine says about a file or a folder it reads from a level's theme
//! folders names it as that level's words and the path inside the level's own folder: "the
//! project's themes/dusk/theme.json", "your purlis config folder's themes/dusk", "the workspace
//! alpha's themes/dusk/fonts/Plex.woff2", "the chat's theme draft dusk/theme.json". A
//! [`super::Complaint`] carries the same place as data ([`Placed`]), so a surface that shows it
//! never has to find where a path ends in a sentence: no absolute path is in the sentence to
//! find.
//!
//! A theme an extension contributes and a theme built into purlis are read from text, never from
//! a folder on disk, so no sentence about them names a path.

use std::fmt;

/// **The level a path is relative to**: whose folder it starts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlacedIn {
    /// The person's purlis config folder on this machine.
    Machine,
    /// The project: the plane's root.
    Project,
    /// A workspace's folder in the project, [`Placed::name`] naming it.
    Workspace,
    /// One chat's theme drafts folder.
    Draft,
}

/// **A file or a folder, as a level and a path inside it**: what a sentence names it as
/// ([`fmt::Display`]), and what a complaint carries ([`super::Complaint::at`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub struct Placed {
    pub level: PlacedIn,
    /// The workspace's name, for a path in a workspace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The path inside the level's folder, its parts joined by `/`; empty for the folder
    /// itself.
    pub path: String,
}

impl Placed {
    /// The folder of `level` itself.
    pub(crate) fn level(level: PlacedIn) -> Self {
        Self {
            level,
            name: None,
            path: String::new(),
        }
    }

    /// The folder of the workspace `name`.
    pub(crate) fn workspace(name: &str) -> Self {
        Self {
            name: Some(name.to_owned()),
            ..Self::level(PlacedIn::Workspace)
        }
    }

    /// `below` inside this place: each of its parts, joined by `/`.
    #[must_use]
    pub(crate) fn join(&self, below: impl AsRef<std::path::Path>) -> Self {
        let mut path = self.path.clone();
        for part in below.as_ref().components() {
            let part = part.as_os_str().to_string_lossy();
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(&part);
        }
        Self {
            path,
            ..self.clone()
        }
    }

    /// The level's folder in words.
    fn folder(&self) -> String {
        match self.level {
            PlacedIn::Machine => "your purlis config folder".to_owned(),
            PlacedIn::Project => "the project".to_owned(),
            PlacedIn::Workspace => {
                format!("the workspace {}", self.name.as_deref().unwrap_or_default())
            }
            PlacedIn::Draft => "the chat's theme drafts".to_owned(),
        }
    }
}

/// **The place in words**, as every sentence names it: the level's folder, and the path inside
/// it after it.
impl fmt::Display for Placed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.level, self.path.is_empty()) {
            (_, true) => out.write_str(&self.folder()),
            (PlacedIn::Draft, false) => write!(out, "the chat's theme draft {}", self.path),
            (_, false) => write!(out, "{}'s {}", self.folder(), self.path),
        }
    }
}
