// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which theme the window draws** (charter-app#273, ADR 0048), from every source there is.
//!
//! The pick a project or a workspace makes is read and ordered by
//! [`crate::extension::project::theme::resolve`]; this answers what that pick, and everything
//! else this machine has, draw:
//!
//! 1. **What the project picked**: an appearance of Purlis Classic by the id a pick names it by;
//!    `system`, the person's own theme in the appearance the operating system has when it has a
//!    dark and a light one, else Purlis Classic's; or the extension theme it names, which is drawn only while the project has that
//!    extension on and this machine approved it. A pick that cannot be drawn draws the default.
//! 2. With no pick, **the person's own theme file**, the one theme they wrote for this machine.
//!    A project's pick is more specific than a machine's file, which is why it comes first.
//! 3. With neither, the first theme from an extension the project has on, or the default.
//!
//! **Following the system works with any theme that has a dark and a light appearance**: the
//! window is told it follows, and asks again when the system turns. [`follow`] of a theme with
//! only one appearance draws that one, and [`Drawn::note`] says that following the system stays
//! there, which Settings › Theme says on a theme's card (#2331). The old `system` id with a
//! person's `theme.json` of one appearance follows with Purlis Classic, as it did before format
//! 2: that file is no card, so nothing would say why it stopped following. With no
//! `system` pick a theme draws its first appearance, as Purlis Classic draws its dark. With no
//! pick, the first extension theme in force that is not refused is drawn.
//!
//! **More contrast picks an appearance tagged high contrast** (#2328): when the operating system
//! asks for more contrast ([`Inputs::contrast`]), each answer that draws an appearance of a tag
//! draws the theme's appearance of that tag tagged high contrast instead, when it has one. A
//! built-in pick names its appearance, and Purlis Classic has none tagged high contrast.
//!
//! **The consent floors hold for every source** (ADR 0097 §6): whatever is drawn is drawn by
//! [`draw`], so a pick, the person's own theme, an extension's and a built-in all clear them.
//!
//! **One rule for every source**: a theme that is not one — text that is not JSON, a document
//! that is not an object, a value outside its type or bound — is refused whole, and Purlis is
//! drawn in its place with the complaints that say why (ADR 0097 §2).

use std::sync::Arc;

use super::{
    Appearance, Complaint, Contrast, Held, Resolved, ResolvedTheme, Theme, built_in, classic,
    default_theme,
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
    /// Text past the one cap every theme file is read under ([`crate::windowprefs::MAX_BYTES`])
    /// is refused unread, as a theme folder's and the person's own are.
    pub fn of(extension: String, name: String, text: &str) -> Self {
        let most = crate::windowprefs::MAX_BYTES;
        let theme = if text.len() as u64 > most {
            Resolved::refused_for(format!(
                "{name} is {} bytes, and a theme is never larger than {most}",
                text.len()
            ))
        } else {
            super::load_text(text)
        };
        Self {
            extension,
            name,
            theme,
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
    pub(super) fn has(&self, extension: &str) -> bool {
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
    /// The contrast the operating system asks for right now: more draws a theme's appearance
    /// tagged high contrast, when it has one of the tag that is drawn (#2328).
    pub contrast: Contrast,
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
    /// The consent floors the theme breaks and the tokens drawn as purlis's own for them, for
    /// the one Notice that names them; `None` for a theme that clears them.
    pub held: Option<Held>,
    /// Which level of the precedence chose it ([`super::resolve_levels`], #2323); `None` from
    /// [`resolve`], which knows no levels.
    pub level: Option<super::Level>,
}

impl Drawn {
    /// `theme` as it is drawn: with the consent floors held ([`super::floors`]).
    pub(super) fn of(theme: &ResolvedTheme) -> Self {
        let (theme, held) = super::floors::hold(theme);
        Self {
            theme: Some(theme),
            complaints: Vec::new(),
            follows_system: false,
            note: None,
            held,
            level: None,
        }
    }

    /// [`follow`].
    fn following(theme: &Theme, system: Appearance, contrast: Contrast) -> Self {
        let (Some(dark), Some(light)) = (
            theme.of_for(Appearance::Dark, contrast),
            theme.of_for(Appearance::Light, contrast),
        ) else {
            let only = theme.first_for(contrast);
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

/// **One appearance as the window draws it**: the consent floors held, and what holding them
/// put right in [`Drawn::held`]. What the window is handed as it is created is drawn by this, as
/// every answer of [`resolve`] is.
pub fn draw(theme: &ResolvedTheme) -> Drawn {
    Drawn::of(theme)
}

/// **`theme` following the operating system**, whose appearance is `system` now: the theme's
/// appearance of that tag, and the window told it follows, when it has a dark and a light one;
/// its one appearance otherwise, with a note that following the system stays there. With more
/// `contrast` asked for, the appearance of that tag tagged high contrast, when it has one.
pub fn follow(theme: &Theme, system: Appearance, contrast: Contrast) -> Drawn {
    Drawn::following(theme, system, contrast)
}

/// **Resolves the theme the window draws** from `inputs`. `offered` is every theme approved
/// extensions contribute, in the order the survey found them; it is asked for only when the
/// answer needs it, because a survey reads and fingerprints every extension's files.
pub fn resolve(inputs: &Inputs<'_>, offered: impl FnOnce() -> Arc<[Offered]>) -> Drawn {
    match inputs.pick {
        // `system` is the spec's old id for "the picked theme, following the OS": the person's
        // own theme, the machine's level, else Purlis Classic.
        Some(Pick::System) => {
            let two = |theme: &&Theme| {
                theme.of(Appearance::Dark).is_some() && theme.of(Appearance::Light).is_some()
            };
            let theme = inputs.theirs.filter(two).unwrap_or_else(|| classic());
            return Drawn::following(theme, inputs.system, inputs.contrast);
        }
        Some(Pick::BuiltIn(id)) => {
            return Drawn::of(built_in(id).unwrap_or_else(default_theme));
        }
        Some(Pick::Extension { .. }) => {}
        // A theme folder is a level's: [`super::resolve_levels`] reads the levels' folders, and
        // this, which has none, draws Purlis and says so.
        Some(Pick::Folder(name)) => {
            return Drawn {
                complaints: vec![Complaint::said(format!(
                    "\"{name}\" is not a theme purlis has"
                ))],
                ..Drawn::of(default_theme())
            };
        }
        None => {
            if let Some(theirs) = inputs.theirs {
                return Drawn::of(theirs.first_for(inputs.contrast));
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
        // With no pick, a refused theme is passed over: it would draw Purlis in the place of
        // the next one in force.
        _ => offered.iter().filter(may).find(|one| !one.theme.refused),
    };
    match chosen.map(|one| &one.theme) {
        None => Drawn::of(default_theme()),
        Some(resolved) => Drawn {
            complaints: resolved.complaints.clone(),
            ..Drawn::of(resolved.theme.first_for(inputs.contrast))
        },
    }
}
