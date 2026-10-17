// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which theme the window draws** (charter-app#273, ADR 0048), from every source there is.
//!
//! The pick a project or a workspace makes is read and ordered by
//! [`crate::extension::project::theme::resolve`]; this answers what that pick, and everything
//! else this machine has, draw:
//!
//! 1. **What the project picked**: a built-in, the built-in matching the operating system's
//!    appearance, or the extension theme it names, which is drawn only while the project has that
//!    extension on and this machine approved it. A pick that cannot be drawn draws the default.
//! 2. With no pick, **the person's own theme file**, the one theme they wrote for this machine.
//!    A project's pick is more specific than a machine's file, which is why it comes first.
//! 3. With neither, the first theme from an extension the project has on, or the default.
//!
//! An extension theme whose text is not JSON at all is dropped: with no pick the window keeps
//! what it draws, and a pick of it draws the default (keeping it would leave the last project's
//! theme on this one).

use std::sync::Arc;

use super::{Appearance, Complaint, Resolved, ResolvedTheme, built_in, default_theme};
use crate::extension::project::theme::Pick;

/// One theme an approved extension contributes, resolved from the text its fingerprint was taken
/// over. `theme` is `None` when that text is not JSON at all.
#[derive(Debug, Clone, PartialEq)]
pub struct Offered {
    /// The extension it comes from.
    pub extension: String,
    /// The theme's name in that extension's manifest.
    pub name: String,
    pub theme: Option<Resolved>,
}

impl Offered {
    /// An extension's theme, resolved from its text.
    pub fn of(extension: String, name: String, text: &str) -> Self {
        Self {
            extension,
            name,
            theme: super::load_text(text),
        }
    }
}

/// Whose extension themes may be drawn: every approved one (no project in front), or only those
/// of the extensions the project in front has on (charter-app#253).
#[derive(Debug, Clone, Copy)]
pub enum On<'a> {
    Every,
    Only(&'a [String]),
}

impl On<'_> {
    fn has(&self, extension: &str) -> bool {
        match self {
            Self::Every => true,
            Self::Only(ids) => ids.iter().any(|id| id == extension),
        }
    }
}

/// What [`resolve`] decides from.
#[derive(Debug, Clone, Copy)]
pub struct Inputs<'a> {
    /// What the project or workspace in front picked, in force; `None` when nothing picked.
    pub pick: Option<&'a Pick>,
    /// The person's own theme, when there is one in force.
    pub theirs: Option<&'a ResolvedTheme>,
    /// Whose extension themes may be drawn.
    pub on: On<'a>,
    /// The operating system's appearance right now.
    pub system: Appearance,
}

/// What the window draws.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Drawn {
    /// The theme to draw, or `None` to keep the one the window draws.
    pub theme: Option<ResolvedTheme>,
    /// What resolving it had to put right.
    pub complaints: Vec<Complaint>,
    /// Whether it follows the operating system's appearance: the window asks again when that
    /// changes.
    pub follows_system: bool,
}

impl Drawn {
    fn of(theme: &ResolvedTheme) -> Self {
        Self {
            theme: Some(theme.clone()),
            complaints: Vec::new(),
            follows_system: false,
        }
    }
}

/// **Resolves the theme the window draws** from `inputs`. `offered` is every theme approved
/// extensions contribute, in the order the survey found them; it is asked for only when the
/// answer needs it, because a survey reads and fingerprints every extension's files.
pub fn resolve(inputs: &Inputs<'_>, offered: impl FnOnce() -> Arc<[Offered]>) -> Drawn {
    match inputs.pick {
        Some(Pick::System) => {
            let theme = super::built_in_for(inputs.system);
            return Drawn {
                follows_system: true,
                ..Drawn::of(theme)
            };
        }
        Some(Pick::BuiltIn(id)) => {
            return Drawn::of(built_in(id).unwrap_or_else(default_theme));
        }
        Some(Pick::Extension { .. }) => {}
        None => {
            if let Some(theirs) = inputs.theirs {
                return Drawn::of(theirs);
            }
        }
    }
    let offered = offered();
    let may = |one: &&Offered| inputs.on.has(&one.extension);
    let chosen = match inputs.pick {
        // Choosing among several is the project's pick; with none, the first in force.
        Some(Pick::Extension { id, name }) => offered
            .iter()
            .filter(may)
            .find(|one| &one.extension == id && &one.name == name),
        _ => offered.iter().find(may),
    };
    match chosen.map(|one| one.theme.as_ref()) {
        None => Drawn::of(default_theme()),
        Some(Some(resolved)) => Drawn {
            theme: Some(resolved.theme.clone()),
            complaints: resolved.complaints.clone(),
            follows_system: false,
        },
        Some(None) if inputs.pick.is_some() => Drawn::of(default_theme()),
        Some(None) => Drawn {
            theme: None,
            complaints: Vec::new(),
            follows_system: false,
        },
    }
}
