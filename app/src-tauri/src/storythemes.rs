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
//! When a built-in theme changes, the test fails and says how to write the file again:
//! `PURLIS_WRITE_STORY_THEMES=1 cargo test -p purlis-app storythemes`.

use std::collections::BTreeMap;

use purlis_core::theme::{self, Theme};

use crate::extensions::WindowTheme;

/// Where the stories read the built-in themes from (`app/.storybook/handed.plugin.ts`).
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../.storybook/built-in-themes.json"
);

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
    let mut text = serde_json::to_string_pretty(&built_in_themes())
        .expect("a resolved theme is plain data serde can always write");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stories_draw_the_built_in_themes_the_core_resolves() {
        let now = written();
        if std::env::var_os("PURLIS_WRITE_STORY_THEMES").is_some() {
            std::fs::write(FIXTURE, &now).expect("the stories' themes are written");
        }
        let kept = std::fs::read_to_string(FIXTURE).unwrap_or_default();
        assert!(
            kept == now,
            "app/.storybook/built-in-themes.json is not what the core resolves the built-in \
             themes to; write it again with \
             `PURLIS_WRITE_STORY_THEMES=1 cargo test -p purlis-app storythemes` and commit it"
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
