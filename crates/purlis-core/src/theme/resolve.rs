// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which theme the window draws** (charter-app#273, ADR 0048), from every source there is.
//!
//! The pick a project or a workspace makes is read and ordered by
//! [`crate::extension::project::theme::resolve`]; this answers what that pick, and everything
//! else this machine has, draw:
//!
//! 1. **What the project picked**: an appearance of Purlis Classic by the id a pick names it by;
//!    `system`, the person's own theme (else Purlis Classic) in the appearance the operating
//!    system has; or the extension theme it names, which is drawn only while the project has that
//!    extension on and this machine approved it. A pick that cannot be drawn draws the default.
//! 2. With no pick, **the person's own theme file**, the one theme they wrote for this machine.
//!    A project's pick is more specific than a machine's file, which is why it comes first.
//! 3. With neither, the first theme from an extension the project has on, or the default.
//!
//! **Following the system works with any theme that has a dark and a light appearance**: the
//! window is told it follows, and asks again when the system turns. A theme with only one
//! appearance draws that one, and [`Drawn::note`] says that following the system stays there.
//! With no `system` pick a theme draws its first appearance, as Purlis Classic draws its dark.
//!
//! **One rule for every source**: a theme that is not one — text that is not JSON, a document
//! that is not an object, a value outside its type or bound — is refused whole, and Purlis is
//! drawn in its place with the complaints that say why (ADR 0097 §2).

use std::sync::Arc;

use super::{
    Appearance, Complaint, Resolved, ResolvedTheme, Theme, built_in, classic, default_theme,
};
use crate::extension::project::theme::Pick;

/// One theme an approved extension contributes, resolved from the text its fingerprint was taken
/// over: refused, as any theme is, when that text is not one.
#[derive(Debug, Clone, PartialEq)]
pub struct Offered {
    /// The extension it comes from.
    pub extension: String,
    /// The theme's name in that extension's manifest.
    pub name: String,
    pub theme: Resolved,
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
    pub theirs: Option<&'a Theme>,
    /// Whose extension themes may be drawn.
    pub on: On<'a>,
    /// The operating system's appearance right now.
    pub system: Appearance,
}

/// What the window draws.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Drawn {
    /// The theme to draw. Always one: `None`, which keeps what the window draws, is no longer
    /// answered, and the field stays an option so the window's wire type is unchanged.
    pub theme: Option<ResolvedTheme>,
    /// What resolving it had to put right.
    pub complaints: Vec<Complaint>,
    /// Whether it follows the operating system's appearance: the window asks again when that
    /// changes.
    pub follows_system: bool,
    /// What to tell the person about what they chose: that following the system stays in the
    /// one appearance a theme has.
    pub note: Option<String>,
}

impl Drawn {
    fn of(theme: &ResolvedTheme) -> Self {
        Self {
            theme: Some(theme.clone()),
            complaints: Vec::new(),
            follows_system: false,
            note: None,
        }
    }

    /// `theme` following the operating system, which is `system` now.
    fn following(theme: &Theme, system: Appearance) -> Self {
        let (Some(dark), Some(light)) = (theme.of(Appearance::Dark), theme.of(Appearance::Light))
        else {
            let only = theme.first();
            let word = match only.appearance {
                Appearance::Dark => "dark",
                Appearance::Light => "light",
            };
            return Self {
                note: Some(format!(
                    "{} has only a {word} appearance, so following the system stays {word}",
                    theme.name
                )),
                ..Self::of(only)
            };
        };
        Self {
            follows_system: true,
            ..Self::of(match system {
                Appearance::Dark => dark,
                Appearance::Light => light,
            })
        }
    }
}

/// **Resolves the theme the window draws** from `inputs`. `offered` is every theme approved
/// extensions contribute, in the order the survey found them; it is asked for only when the
/// answer needs it, because a survey reads and fingerprints every extension's files.
pub fn resolve(inputs: &Inputs<'_>, offered: impl FnOnce() -> Arc<[Offered]>) -> Drawn {
    match inputs.pick {
        // `system` is the spec's old id for "the picked theme, following the OS": the person's
        // own theme, the machine's level, else Purlis Classic.
        Some(Pick::System) => {
            return Drawn::following(inputs.theirs.unwrap_or_else(|| classic()), inputs.system);
        }
        Some(Pick::BuiltIn(id)) => {
            return Drawn::of(built_in(id).unwrap_or_else(default_theme));
        }
        Some(Pick::Extension { .. }) => {}
        None => {
            if let Some(theirs) = inputs.theirs {
                return Drawn::of(theirs.first());
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
    match chosen.map(|one| &one.theme) {
        None => Drawn::of(default_theme()),
        Some(resolved) => Drawn {
            complaints: resolved.complaints.clone(),
            ..Drawn::of(resolved.theme.first())
        },
    }
}
