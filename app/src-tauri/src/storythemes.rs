// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The built-in themes a component story is drawn in** (#2945, `docs/component-stories.md`).
//!
//! A story runs in a browser, which the core cannot hand anything to, and every story is drawn in
//! each appearance of each theme purlis ships: Purlis Classic and Purlis. So what the core hands
//! a window for them, [`WindowTheme`] by [`WindowTheme`], is kept beside the stories in
//! [`FIXTURE`], and the test below holds it to what the core resolves now. One engine draws the
//! window and the stories; the stories never resolve a theme themselves.
//!
//! **What a window is handed at creation is kept by reference beside it** ([`AT_CREATION`],
//! #3034): the default's appearance, which a story is drawn in when nothing names another cell,
//! and the appearance each built-in pick names ([`crate::windowprefs::handed_built_ins`]), each
//! as a theme's id and an appearance's id in [`FIXTURE`]. So the stories' default is the core's,
//! and no story file spells one.
//!
//! When a built-in theme changes, the test fails and says how to write the files again, from the
//! repository's root: [`WRITE_AGAIN`].

use std::collections::BTreeMap;

use purlis_core::theme::{self, Theme};

use crate::extensions::WindowTheme;

/// The command that writes [`FIXTURE`] and [`AT_CREATION`] again, from the repository's root:
/// the app crate builds only with a frontend folder, and only its library holds the test.
const WRITE_AGAIN: &str =
    "mkdir -p app/dist && PURLIS_WRITE_STORY_THEMES=1 cargo test -p purlis-app --lib storythemes";

/// Where the stories read the built-in themes from (`app/.storybook/handed.plugin.ts`).
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../.storybook/built-in-themes.json"
);

/// Where the stories read what a window is handed at creation from, by reference into
/// [`FIXTURE`] (`app/.storybook/handed.plugin.ts`, #3034).
const AT_CREATION: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../.storybook/at-creation.json"
);

/// One appearance of a built-in theme, by the theme's id and the appearance's id: a key into
/// [`FIXTURE`].
#[derive(Debug, PartialEq, serde::Serialize)]
pub(crate) struct Cell {
    pub theme: &'static str,
    pub appearance: String,
}

/// The built-in theme `one` is an appearance of, by its id, and the appearance's id.
fn cell_of(one: &theme::ResolvedTheme) -> Cell {
    let of = |theme: &Theme| theme.appearances.iter().any(|it| std::ptr::eq(it, one));
    let theme = if of(theme::classic()) {
        theme::CLASSIC
    } else {
        assert!(of(theme::purlis()), "{} is a built-in appearance", one.name);
        theme::PURLIS
    };
    Cell {
        theme,
        appearance: one.id.clone(),
    }
}

/// **What a window is handed at creation, by reference** (#3034): the default's appearance
/// (`default`), and each built-in pick's (`built_in`).
#[derive(Debug, serde::Serialize)]
pub(crate) struct AtCreation {
    pub default: Cell,
    pub built_in: BTreeMap<&'static str, Cell>,
}

pub(crate) fn at_creation() -> AtCreation {
    AtCreation {
        default: cell_of(theme::default_theme()),
        // The picks the window is handed, and no other (`windowprefs::handed_built_ins`).
        built_in: crate::windowprefs::HANDED_PICKS
            .into_iter()
            .filter_map(|pick| theme::built_in(pick).map(|one| (pick, cell_of(one))))
            .collect(),
    }
}

/// Every built-in theme by its id, and each of its appearances by the appearance's id, as the
/// window is handed it.
pub(crate) fn built_in_themes() -> BTreeMap<&'static str, BTreeMap<String, WindowTheme>> {
    let by_appearance = |one: &Theme| {
        one.appearances
            .iter()
            .map(|appearance| (appearance.id.clone(), WindowTheme::from(appearance)))
            .collect()
    };
    BTreeMap::from([
        (theme::CLASSIC, by_appearance(theme::classic())),
        (theme::PURLIS, by_appearance(theme::purlis())),
    ])
}

/// [`built_in_themes`] as the fixture holds it: JSON, two spaces in, a newline at the end.
pub(crate) fn written() -> String {
    pretty(&built_in_themes())
}

/// `value` as a fixture holds it: JSON, two spaces in, a newline at the end.
fn pretty(value: &impl serde::Serialize) -> String {
    let mut text = serde_json::to_string_pretty(value)
        .expect("a resolved theme is plain data serde can always write");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stories_draw_the_built_in_themes_the_core_resolves() {
        for (file, now) in [(FIXTURE, written()), (AT_CREATION, pretty(&at_creation()))] {
            if std::env::var_os("PURLIS_WRITE_STORY_THEMES").is_some() {
                std::fs::write(file, &now).expect("the stories' themes are written");
            }
            let kept = std::fs::read_to_string(file).unwrap_or_default();
            assert!(
                kept == now,
                "{file} is not what the core resolves the built-in themes to; write it again \
                 with `{WRITE_AGAIN}` and commit it"
            );
        }
    }

    #[test]
    fn a_story_is_drawn_in_the_default_and_each_pick_in_what_the_window_is_handed() {
        // #3034: the default is Purlis's dark, and the stories read it from here.
        let at = at_creation();
        assert_eq!(
            at.default,
            Cell {
                theme: theme::PURLIS,
                appearance: "dark".to_owned()
            }
        );
        let themes = built_in_themes();
        let layer = theme::Layer::default();
        for (pick, handed) in crate::windowprefs::handed_built_ins(&layer) {
            let cell = &at.built_in[pick.as_str()];
            assert_eq!(themes[cell.theme][&cell.appearance], handed, "{pick}");
        }
        assert_eq!(
            themes[at.default.theme][&at.default.appearance],
            WindowTheme::from(theme::default_theme())
        );
    }

    #[test]
    fn every_appearance_of_both_built_in_themes_is_kept() {
        let themes = built_in_themes();
        let ids = |id: &str| themes[id].keys().cloned().collect::<Vec<_>>();
        assert_eq!(ids(theme::CLASSIC), ["dark", "light"]);
        assert_eq!(ids(theme::PURLIS), ["dark", "dark-high-contrast", "light"]);
    }
}
