// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The native window follows the theme it draws** (spec #2313, #2328): the operating system's
//! own parts of the window (the traffic lights, the menus, the dialogs) are drawn in the resolved
//! appearance's tag, and the window's colour before the page's first frame is the appearance's
//! `window.background`. The tray and the app icon stay as they are: they are brand, not theme.
//!
//! [`ResolvedTheme::native`] answers both from a resolved theme, and [`Native::parse`] reads the
//! same two values back from the page, which hands them over as it draws a theme: one grammar,
//! the engine's, whichever side gives them. [`first_frame`] is what the window draws as it is
//! created, the levels resolved once (#2602), so the window can be made in that colour and that
//! appearance, with no flash of another and no repaint when the window asks what is in force.

use std::sync::Arc;

use super::vocabulary::is_colour;
use super::{
    Appearance, Drawn, LevelInputs, ResolvedTheme, default_for, default_theme, draw, resolve_levels,
};

/// **What the operating system draws of the window**: the appearance its own parts take, and
/// the colour the window is before the page paints, opaque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Native {
    /// The tag of the appearance drawn: the native parts are drawn dark or light with it.
    pub appearance: Appearance,
    /// `window.background` as red, green and blue. Always opaque: a window is never drawn
    /// through, so an alpha a theme wrote is not the window's.
    pub background: [u8; 3],
}

impl ResolvedTheme {
    /// What the operating system draws of a window that draws this appearance.
    pub fn native(&self) -> Native {
        Native {
            appearance: self.appearance,
            // A resolved theme's `window.background` is a colour: the grammar held it as it was
            // read, and Purlis Classic's is checked in.
            background: rgb(self.value("window.background")).unwrap_or_else(|| {
                rgb(default_theme().value("window.background")).unwrap_or([0, 0, 0])
            }),
        }
    }
}

impl Native {
    /// The native window for the tag `appearance` (`dark` or `light`) and the colour
    /// `background`, as the page hands a drawn theme's over: refused, with why, for any other
    /// word or a value that is not a colour.
    pub fn parse(appearance: &str, background: &str) -> Result<Self, String> {
        let appearance = match appearance {
            "dark" => Appearance::Dark,
            "light" => Appearance::Light,
            other => return Err(format!("{other:?} is not an appearance: dark or light")),
        };
        let background = rgb(background).ok_or_else(|| {
            format!("{background:?} is not a colour: #rgb, #rrggbb or with alpha")
        })?;
        Ok(Self {
            appearance,
            background,
        })
    }
}

/// **What the window draws as it is created** (#2602): the theme the levels choose
/// ([`resolve_levels`], resolved once, for the project and workspace the window is created on),
/// for the system's appearance and `contrast` in `inputs`, the consent floors held. The page draws
/// this in its first frame and the window is created in its [`Native`], so neither is drawn again
/// once the window asks what is in force.
///
/// **No extension's theme is drawn here**: the themes extensions contribute come from a survey,
/// which reads and fingerprints every extension's files, and the first frame does not wait for
/// one. Where the levels' answer needs it, the first frame is the person's own theme in its
/// first appearance for `contrast`, else Purlis, and the window draws the answer once it has it.
pub fn first_frame(inputs: &LevelInputs<'_>) -> Drawn {
    let mut surveyed = false;
    let drawn = resolve_levels(inputs, || {
        surveyed = true;
        Arc::from(Vec::new())
    });
    if !surveyed {
        return drawn;
    }
    inputs.theirs.map_or_else(
        || draw(default_for(inputs.contrast)),
        |theme| draw(theme.first_for(inputs.contrast)),
    )
}

/// A colour of the theme grammar ([`is_colour`]) as red, green and blue; its alpha left out.
fn rgb(hex: &str) -> Option<[u8; 3]> {
    if !is_colour(hex) {
        return None;
    }
    let digits = &hex[1..];
    let channel = |at: usize, wide: bool| {
        let text = if wide {
            digits.get(at * 2..at * 2 + 2)?.to_owned()
        } else {
            digits.get(at..=at)?.repeat(2)
        };
        u8::from_str_radix(&text, 16).ok()
    };
    let wide = digits.len() > 4;
    Some([channel(0, wide)?, channel(1, wide)?, channel(2, wide)?])
}
