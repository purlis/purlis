// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Format 2 at the engine's interface: theme folders, `extends`, the palette, appearances, the
//! schema, Purlis Classic as a folder, and following the system with any theme (#2322).

use std::sync::Arc;

use serde_json::{Value, json};

use super::vocabulary::TOKENS;
use super::*;
use crate::extension::project::theme::Pick;

/// The schema as the repo publishes it.
const PUBLISHED_SCHEMA: &str = include_str!("../../../../docs/schema/theme.v2.schema.json");

fn said(resolved: &Resolved) -> Vec<String> {
    resolved
        .complaints
        .iter()
        .map(|one| one.said.clone())
        .collect()
}

/// A format 2 theme with one dark appearance that sets `tokens`.
fn dark_only(name: &str, tokens: Value) -> Value {
    json!({
        "$schema": SCHEMA_URL,
        "name": name,
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }]
    })
}

/// A format 2 theme with a dark and a light appearance.
fn both(name: &str) -> Value {
    json!({
        "name": name,
        "palette": { "ink": "#101010", "paper": "#fafafa" },
        "appearances": [
            { "id": "night", "appearance": "dark", "tokens": { "surface.base": "ink" } },
            { "id": "day", "appearance": "light", "tokens": { "surface.base": "paper" } }
        ]
    })
}

fn classic_dark() -> &'static ResolvedTheme {
    classic().first()
}

fn classic_light() -> &'static ResolvedTheme {
    classic()
        .of(Appearance::Light)
        .expect("Classic's light appearance")
}

/// The tokens #2327 added after the digests below were taken: what Classic gives each is held
/// to what the window drew before in `typed_tests.rs`. The quiet and faint tiers (#2902) are
/// Classic's muted, held in `tiers_tests.rs`.
fn added_later(token: &str) -> bool {
    [
        "syntax.",
        "markdown.",
        "selection.",
        "scrollbar.",
        "chart.",
        "window.",
        "terminal.selection-foreground",
        "terminal.selection-inactive",
        "terminal.scrollbar",
    ]
    .iter()
    .any(|prefix| token.starts_with(prefix))
        || ["caret", "placeholder", "text.quiet", "text.faint"].contains(&token)
}

/// Each value of `theme` in token order, as one string to digest: the tokens there were when
/// the digests were taken.
fn digest(theme: &ResolvedTheme) -> String {
    use sha2::Digest;
    let then: std::collections::BTreeMap<&str, &String> = theme
        .values
        .iter()
        .filter(|(token, _)| !added_later(token))
        .map(|(token, value)| (*token, value))
        .collect();
    let sorted = serde_json::to_string(&then).expect("values serialise");
    sha2::Sha256::digest(sorted.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

// ---------------------------------------------------------------------------------------
// Purlis Classic is a theme folder with a dark and a light appearance
// ---------------------------------------------------------------------------------------

#[test]
fn purlis_classic_is_one_theme_with_a_dark_and_a_light_appearance() {
    let classic = classic();
    assert_eq!(classic.name, "Purlis Classic");
    let tags: Vec<(&str, Appearance)> = classic
        .appearances
        .iter()
        .map(|one| (one.id.as_str(), one.appearance))
        .collect();
    assert_eq!(
        tags,
        [("dark", Appearance::Dark), ("light", Appearance::Light)]
    );
    assert!(classic.appearances.iter().all(|one| !one.high_contrast));
    // The appearances are what the old ids draw; the window comes up in Purlis (#3034).
    assert_eq!(&classic.appearances[0], classic_dark());
    assert_eq!(&classic.appearances[1], classic_light());
    assert_eq!(default_theme(), purlis().first());
}

#[test]
fn purlis_classic_draws_exactly_today_s_values() {
    // The sha-256 of today's `purlis-dark.json` and `purlis-light.json` tokens, taken before
    // they moved into the Classic folder (sorted keys, compact JSON). A changed value anywhere
    // in Classic changes one of these. Dark's `needs-you.base` moved from `#b85050` to
    // `#bd5654` for the mark on a hovered control (#2504). Light's `state.waiting` (and its
    // glow) moved from `#906000` to `#805500`, so the words of a warning-soft button hold 4.5:1
    // on a control and a hovered one (#2945). Both gained `wash.warning`, the amber wash a
    // waiting pill stands on (#2913), and `border.control` and the accent's wash, words and fill
    // (#3177), each the value of the token that drew it before, so nothing Classic draws moved.
    // Light's `state.waiting` (and its glow) moved again, to `#7a5100`, so a title-bar chip's
    // waiting count holds 4.5:1 on its amber wash over the title bar (#3041).
    assert_eq!(
        digest(classic_dark()),
        "6361923b65d2626ed8df14b02b7c3ebd79b43ef146bdf6e5f8157cf630be3195"
    );
    assert_eq!(
        digest(classic_light()),
        "1c03d7f31bf5ebe14ec5d511a567fcbefa079a2e3165a250d2d1f6c16fed7c61"
    );
    for theme in [classic_dark(), classic_light()] {
        assert_eq!(theme.motion.durations["duration.quick"], 120);
        assert_eq!(theme.motion.durations["duration.enter"], 160);
        assert_eq!(theme.motion.durations["duration.settle"], 280);
        assert_eq!(theme.motion.durations["duration.spin"], 900);
        assert_eq!(theme.motion.durations["duration.breathe"], 1600);
        assert_eq!(
            theme.motion.easings["easing.standard"],
            [0.2, 0.0, 0.0, 1.0]
        );
        assert_eq!(theme.motion.easings["easing.enter"], [0.0, 0.0, 0.2, 1.0]);
        assert_eq!(theme.motion.easings["easing.steady"], [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(theme.motion.easings["easing.breathe"], [0.4, 0.0, 0.6, 1.0]);
    }
}

#[test]
fn the_classic_folder_loads_as_itself_without_a_word() {
    let loaded = load_text(CLASSIC_FILE);
    assert_eq!(said(&loaded), Vec::<String>::new());
    assert!(!loaded.refused);
    assert_eq!(loaded.theme.name, "Purlis Classic");
    for (got, shipped) in loaded.theme.appearances.iter().zip(&classic().appearances) {
        assert_eq!(got.values, shipped.values);
        assert_eq!(got.motion, shipped.motion);
    }
}

#[test]
fn the_classic_folder_names_the_published_schema() {
    let file: Value = serde_json::from_str(CLASSIC_FILE).unwrap();
    assert_eq!(
        file["$schema"], SCHEMA_URL,
        "Purlis Classic's folder names the schema of the purlis it ships in: after a version \
         bump, run PURLIS_THEME_SCHEMA_WRITE=1 cargo test -p purlis-core the_published_schema"
    );
}

// ---------------------------------------------------------------------------------------
// Format 1 keeps loading, as a one-appearance theme
// ---------------------------------------------------------------------------------------

#[test]
fn a_format_1_file_loads_as_a_theme_with_one_appearance() {
    let loaded = load(&json!({
        "name": "Old",
        "appearance": "light",
        "tokens": { "accent.base": "#123456" }
    }));
    assert!(!loaded.refused);
    assert_eq!(loaded.theme.name, "Old");
    assert_eq!(loaded.theme.appearances.len(), 1);
    let only = loaded.theme.first();
    assert_eq!(only.id, "light");
    assert_eq!(only.appearance, Appearance::Light);
    assert_eq!(only.value("accent.base"), "#123456");
    assert_eq!(
        only.value("surface.base"),
        classic_light().value("surface.base")
    );
}

#[test]
fn purlis_classic_by_its_own_id_is_its_first_appearance_for_a_pick_and_a_lookup() {
    assert_eq!(built_in(CLASSIC), Some(classic_dark()));
    assert_eq!(Pick::parse(CLASSIC), Some(Pick::BuiltIn(CLASSIC)));
    let it = resolve(
        &Inputs {
            pick: Some(&Pick::BuiltIn(CLASSIC)),
            theirs: None,
            on: On::Every,
            system: Appearance::Light,
            contrast: Contrast::Standard,
        },
        || unreachable!("no survey for a built-in"),
    );
    assert_eq!(it.theme.as_ref(), Some(classic_dark()));
}

#[test]
fn the_old_ids_draw_purlis_s_dark_and_light() {
    // They were written as the default's dark or light, so they draw Purlis's (#3034).
    for (old, appearance) in [
        ("purlis-dark", Appearance::Dark),
        ("charter-dark", Appearance::Dark),
        ("purlis-light", Appearance::Light),
        ("charter-light", Appearance::Light),
    ] {
        let drawn = built_in(old).expect(old);
        assert_eq!(drawn.appearance, appearance, "{old}");
        assert_eq!(drawn, purlis().of(appearance).unwrap(), "{old}");
    }
}

// ---------------------------------------------------------------------------------------
// The palette
// ---------------------------------------------------------------------------------------

#[test]
fn a_token_may_name_a_colour_of_the_theme_s_palette() {
    let loaded = load(&json!({
        "name": "Inked",
        "palette": { "lavender": "#b4a7f5" },
        "appearances": [{
            "id": "dark", "appearance": "dark",
            "tokens": { "accent.base": "lavender", "focus.ring": "lavender", "text.primary": "#eeeeee" }
        }]
    }));
    assert_eq!(said(&loaded), Vec::<String>::new());
    let dark = loaded.theme.first();
    assert_eq!(dark.value("accent.base"), "#b4a7f5");
    assert_eq!(dark.value("focus.ring"), "#b4a7f5");
    assert_eq!(dark.value("text.primary"), "#eeeeee");
    // A token the theme does not name is Purlis Classic's of the same appearance.
    assert_eq!(
        dark.value("surface.base"),
        classic_dark().value("surface.base")
    );
}

#[test]
fn a_name_that_is_not_in_the_palette_refuses_the_theme_and_says_so() {
    let loaded = load(&dark_only("Typo", json!({ "accent.base": "lavendr" })));
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        [
            "accent.base in appearance dark is \"lavendr\", which is neither a hex colour nor a \
          name in this theme's palette"
        ]
    );
    // A refused theme draws the default, Purlis, whole.
    assert_eq!(&loaded.theme, purlis());
}

#[test]
fn css_is_never_a_colour_even_spelled_as_a_palette_name() {
    for value in ["red", "rgb(0,0,0)", "var(--x)", "#12345", "Lavender"] {
        let loaded = load(&json!({
            "name": "Css",
            "palette": { "lavender": "#b4a7f5" },
            "appearances": [{ "id": "dark", "appearance": "dark", "tokens": { "accent.base": value } }]
        }));
        assert!(loaded.refused, "{value}");
    }
}

#[test]
fn a_palette_holds_hex_under_plain_names() {
    let loaded = load(&json!({
        "name": "P",
        "palette": { "ok": "#000", "Bad Name": "#111111", "soft": "lavender" },
        "appearances": [{ "id": "dark", "appearance": "dark" }]
    }));
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        [
            "palette name \"Bad Name\" is not lower-case letters, digits and dashes starting \
             with a letter, at most 32 long",
            "palette colour soft is \"lavender\", which is not #rgb, #rgba, #rrggbb or #rrggbbaa",
        ]
    );
}

#[test]
fn a_palette_name_is_local_to_the_theme_that_declares_it() {
    let library = Library::new().with(
        "parent",
        json!({
            "name": "Parent",
            "palette": { "brand": "#00aa88" },
            "appearances": [{ "id": "dark", "appearance": "dark", "tokens": { "accent.base": "brand" } }]
        }),
    );
    let child = library.load(
        Some("child"),
        &json!({
            "name": "Child",
            "extends": "parent",
            "appearances": [{ "id": "dark", "tokens": { "focus.ring": "brand" } }]
        }),
    );
    assert!(child.refused);
    assert_eq!(
        said(&child),
        [
            "focus.ring in appearance dark is \"brand\", which is neither a hex colour nor a name \
          in this theme's palette"
        ]
    );
}

// ---------------------------------------------------------------------------------------
// extends
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_that_extends_another_holds_only_its_differences() {
    let library = Library::new().with("parent", both("Parent"));
    let child = library.load(
        Some("child"),
        &json!({
            "name": "Child",
            "extends": "parent",
            "palette": { "teal": "#00aa88" },
            "appearances": [
                { "id": "day", "tokens": { "accent.base": "teal" } },
                { "id": "dim", "appearance": "dark", "high-contrast": true }
            ]
        }),
    );
    assert_eq!(said(&child), Vec::<String>::new());
    let theme = &child.theme;
    assert_eq!(theme.name, "Child");
    let ids: Vec<&str> = theme
        .appearances
        .iter()
        .map(|one| one.id.as_str())
        .collect();
    assert_eq!(ids, ["night", "day", "dim"]);
    let day = &theme.appearances[1];
    assert_eq!(day.appearance, Appearance::Light);
    // The parent's value stands where the child says nothing, and the child's wins where it does.
    assert_eq!(day.value("surface.base"), "#fafafa");
    assert_eq!(day.value("accent.base"), "#00aa88");
    assert_eq!(theme.appearances[0].value("surface.base"), "#101010");
    // A new appearance starts from Purlis Classic's of its tag.
    let dim = &theme.appearances[2];
    assert!(dim.high_contrast);
    assert_eq!(dim.values, classic_dark().values);
    assert!(theme.appearances.iter().all(|one| one.name == "Child"));
}

#[test]
fn a_theme_may_extend_purlis_classic_by_its_id_or_an_old_one() {
    for parent in ["purlis-classic", "purlis-dark", "charter-light"] {
        let loaded = load(&json!({
            "name": "Mine",
            "extends": parent,
            "appearances": [{ "id": "light", "tokens": { "accent.base": "#aa0000" } }]
        }));
        assert_eq!(said(&loaded), Vec::<String>::new(), "{parent}");
        assert_eq!(loaded.theme.appearances.len(), 2);
        assert_eq!(loaded.theme.appearances[1].value("accent.base"), "#aa0000");
        assert_eq!(loaded.theme.appearances[0].values, classic_dark().values);
    }
}

#[test]
fn a_parent_that_is_not_there_refuses_the_theme_with_its_name() {
    let loaded = load(&json!({ "name": "Orphan", "extends": "nord" }));
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        ["Orphan extends \"nord\", which is not a theme purlis has"]
    );
}

#[test]
fn a_cycle_of_extends_refuses_the_theme_and_names_the_cycle() {
    let library = Library::new()
        .with("a", json!({ "name": "A", "extends": "b" }))
        .with("b", json!({ "name": "B", "extends": "c" }))
        .with("c", json!({ "name": "C", "extends": "a" }));
    let loaded = library.load_id("a");
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        ["the themes extend one another in a circle: a extends b extends c extends a"]
    );
    let own = library.load(Some("self"), &json!({ "name": "S", "extends": "self" }));
    assert!(own.refused);
    assert_eq!(
        said(&own),
        ["the themes extend one another in a circle: self extends self"]
    );
}

#[test]
fn a_refused_parent_refuses_its_child_and_says_why() {
    let library = Library::new().with("bad", dark_only("Bad", json!({ "accent.base": "nope" })));
    let loaded = library.load(Some("kid"), &json!({ "name": "Kid", "extends": "bad" }));
    assert!(loaded.refused);
    // The parent's complaint keeps its source.
    assert_eq!(
        said(&loaded),
        [
            "Kid extends \"bad\", which purlis cannot draw",
            "in \"bad\": accent.base in appearance dark is \"nope\", which is neither a hex \
             colour nor a name in this theme's palette",
        ]
    );
}

// ---------------------------------------------------------------------------------------
// Appearances
// ---------------------------------------------------------------------------------------

#[test]
fn every_appearance_has_an_id_and_a_tag_of_dark_or_light() {
    let loaded = load(&json!({
        "name": "Tags",
        "appearances": [
            { "appearance": "dark" },
            { "id": "dusk", "appearance": "sepia" },
            { "id": "Two Words", "appearance": "light" },
            { "id": "dawn" },
            { "id": "a", "appearance": "light", "high-contrast": "yes" },
            { "id": "a", "appearance": "light" }
        ]
    }));
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        [
            "appearance 1 has no id",
            "appearance dusk is \"sepia\"; an appearance is \"dark\" or \"light\"",
            "appearance id \"Two Words\" is not lower-case letters, digits and dashes starting \
             with a letter, at most 32 long",
            "appearance dawn says neither \"dark\" nor \"light\"",
            "high-contrast in appearance a is \"yes\", which is not true or false",
            "two appearances are called a",
        ]
    );
}

#[test]
fn a_theme_has_an_appearance_or_a_parent_to_take_them_from() {
    let loaded = load(&json!({ "name": "Empty", "appearances": [] }));
    assert!(loaded.refused);
    assert_eq!(said(&loaded), ["Empty has no appearance"]);
}

#[test]
fn an_appearance_s_motion_keeps_the_closed_grammar() {
    let loaded = load(&json!({
        "name": "Slow",
        "appearances": [{ "id": "dark", "appearance": "dark", "motion": { "duration.quick": 0 } }]
    }));
    assert_eq!(said(&loaded), Vec::<String>::new());
    assert_eq!(loaded.theme.first().motion.durations["duration.quick"], 0);
    let bad = load(&json!({
        "name": "Slow",
        "appearances": [{ "id": "dark", "appearance": "dark", "motion": { "duration.quick": "1s" } }]
    }));
    assert!(bad.refused);
    assert_eq!(bad.complaints.len(), 1);
}

#[test]
fn a_key_format_2_does_not_have_refuses_the_theme() {
    let loaded = load(&json!({
        "name": "Extra",
        "appearances": [{ "id": "dark", "appearance": "dark", "colours": {} }],
        "colors": {}
    }));
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        [
            "colors is not a key of theme.json",
            "colours is not a key of an appearance",
        ]
    );
}

#[test]
fn a_token_or_motion_name_purlis_does_not_have_is_left_out_and_said() {
    let loaded = load(&json!({
        "name": "Ahead",
        "appearances": [{
            "id": "dark", "appearance": "dark",
            "tokens": { "gutter.marker": "#ff00ff", "accent.base": "#123456" },
            "motion": { "duration.linger": 400 }
        }]
    }));
    assert!(!loaded.refused);
    assert_eq!(loaded.theme.first().value("accent.base"), "#123456");
    assert_eq!(
        said(&loaded),
        [
            "gutter.marker in appearance dark is not a purlis token, so it is left out",
            "duration.linger is not a purlis motion token, so it is left out",
        ]
    );
}

#[test]
fn a_theme_with_more_appearances_than_the_cap_is_refused_with_its_count() {
    let many: Vec<Value> = (0..=MOST_APPEARANCES)
        .map(|n| json!({ "id": format!("a{n}"), "appearance": "dark" }))
        .collect();
    let loaded = load(&json!({ "name": "Many", "appearances": many }));
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        [format!(
            "Many has {} appearances, and a theme has at most {MOST_APPEARANCES}",
            MOST_APPEARANCES + 1
        )]
    );
}

#[test]
fn a_format_2_theme_has_a_name() {
    let loaded = load(&json!({ "appearances": [{ "id": "dark", "appearance": "dark" }] }));
    assert!(loaded.refused);
    assert_eq!(said(&loaded), ["a theme has a name"]);
}

// ---------------------------------------------------------------------------------------
// A theme folder
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_folder_is_its_theme_json_and_may_carry_more() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("dusk");
    std::fs::create_dir_all(folder.join("fonts")).unwrap();
    std::fs::create_dir_all(folder.join("icons")).unwrap();
    std::fs::write(folder.join("LICENSE"), "MIT").unwrap();
    std::fs::write(folder.join("preview.png"), [0u8; 4]).unwrap();
    std::fs::write(folder.join("theme.json"), both("Dusk").to_string()).unwrap();
    let loaded = Library::new().load_folder(dir.path(), "dusk");
    assert_eq!(said(&loaded), Vec::<String>::new());
    assert_eq!(loaded.theme.name, "Dusk");
    assert_eq!(loaded.theme.appearances.len(), 2);
}

#[test]
fn a_lone_colours_only_theme_json_is_still_a_theme_folder() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("lone")).unwrap();
    std::fs::write(
        dir.path().join("lone/theme.json"),
        json!({ "name": "Lone", "appearance": "dark", "tokens": { "accent.base": "#abcdef" } })
            .to_string(),
    )
    .unwrap();
    let loaded = Library::new().load_folder(dir.path(), "lone");
    assert_eq!(said(&loaded), Vec::<String>::new());
    assert_eq!(loaded.theme.first().value("accent.base"), "#abcdef");
}

#[test]
fn a_folder_without_a_readable_theme_json_is_refused_and_said() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("empty")).unwrap();
    let missing = Library::new().load_folder(dir.path(), "empty");
    assert!(missing.refused);
    assert_eq!(missing.complaints.len(), 1);
    assert!(
        missing.complaints[0].said.contains("has no theme.json"),
        "{:?}",
        said(&missing)
    );

    std::fs::write(dir.path().join("empty/theme.json"), "{ nope").unwrap();
    let broken = Library::new().load_folder(dir.path(), "empty");
    assert!(broken.refused);
    // The sentence every source says about text that is not JSON.
    let one = &broken.complaints[0].said;
    assert_eq!(
        one.rsplit_once("theme.json: ").map(|(_, it)| it),
        Some(load_text("{ nope").complaints[0].said.as_str())
    );
}

#[test]
fn a_theme_json_past_the_cap_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("big")).unwrap();
    let padding = " ".repeat(usize::try_from(crate::windowprefs::MAX_BYTES).unwrap());
    std::fs::write(
        dir.path().join("big/theme.json"),
        format!("{}{padding}", both("Big")),
    )
    .unwrap();
    let loaded = Library::new().load_folder(dir.path(), "big");
    assert!(loaded.refused);
    assert!(
        loaded.complaints[0]
            .said
            .contains("never larger than 65536"),
        "{:?}",
        said(&loaded)
    );
}

#[test]
fn a_folder_name_is_one_plain_name_inside_the_level_s_themes_folder() {
    let dir = tempfile::tempdir().unwrap();
    let themes = dir.path().join("themes");
    std::fs::create_dir(&themes).unwrap();
    std::fs::create_dir(dir.path().join("outside")).unwrap();
    std::fs::write(
        dir.path().join("outside/theme.json"),
        both("Out").to_string(),
    )
    .unwrap();
    // `C:` is a Windows drive-relative name: refused before any path is made of it.
    for name in ["..", ".", "", "../outside", "a/b", "/abs", "C:", "c:dusk"] {
        let loaded = Library::new().load_folder(&themes, name);
        assert!(loaded.refused, "{name}");
        assert!(
            loaded.complaints[0]
                .said
                .contains("is not the name of a theme folder"),
            "{name}: {:?}",
            said(&loaded)
        );
    }
}

#[cfg(unix)]
#[test]
fn a_theme_folder_that_is_a_link_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let themes = dir.path().join("themes");
    std::fs::create_dir(&themes).unwrap();
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    std::fs::write(elsewhere.join("theme.json"), both("Far").to_string()).unwrap();
    std::os::unix::fs::symlink(&elsewhere, themes.join("far")).unwrap();
    let loaded = Library::new().load_folder(&themes, "far");
    assert!(loaded.refused);
    assert_eq!(&loaded.theme, purlis());
}

#[test]
fn a_parent_folder_that_cannot_be_read_says_why() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("broken")).unwrap();
    std::fs::write(dir.path().join("broken/theme.json"), "{ nope").unwrap();
    let library = Library::new().with_folder(dir.path(), "broken");
    let child = library.load(None, &json!({ "name": "Kid", "extends": "broken" }));
    assert!(child.refused);
    let said = said(&child);
    assert_eq!(said[0], "Kid extends \"broken\", which purlis cannot draw");
    assert!(said[1].starts_with("in \"broken\": "), "{said:?}");
    assert!(said[1].contains("a theme is JSON: "), "{said:?}");
}

#[cfg(unix)]
#[test]
fn a_theme_json_that_is_a_link_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = dir.path().join("elsewhere.json");
    std::fs::write(&elsewhere, both("Linked").to_string()).unwrap();
    let folder = dir.path().join("linked");
    std::fs::create_dir(&folder).unwrap();
    std::os::unix::fs::symlink(&elsewhere, folder.join("theme.json")).unwrap();
    let loaded = Library::new().load_folder(dir.path(), "linked");
    assert!(loaded.refused);
    assert_eq!(&loaded.theme, purlis());
}

#[test]
fn a_folder_in_the_library_is_extended_by_its_folder_name() {
    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().join("dusk");
    std::fs::create_dir(&parent).unwrap();
    std::fs::write(parent.join("theme.json"), both("Dusk").to_string()).unwrap();
    let library = Library::new().with_folder(dir.path(), "dusk");
    let child = library.load(None, &json!({ "name": "Kid", "extends": "dusk" }));
    assert_eq!(said(&child), Vec::<String>::new());
    assert_eq!(child.theme.appearances[1].value("surface.base"), "#fafafa");
}

// ---------------------------------------------------------------------------------------
// The published schema
// ---------------------------------------------------------------------------------------

#[test]
fn the_published_schema_is_the_one_the_engine_generates() {
    let published: Value =
        serde_json::from_str(PUBLISHED_SCHEMA).expect("the published schema is JSON");
    // Before anything is written: a format only grows (`theme::published`).
    let narrowed = published::narrowings(&published, &schema());
    assert!(
        narrowed.is_empty(),
        "format 2's schema only grows, and the engine's takes away or narrows what \
         {SCHEMA_FILE} has; a theme valid against an earlier release would stop being valid. \
         Keep it, or make the change a new format:\n{}",
        narrowed.join("\n")
    );
    if std::env::var_os("PURLIS_THEME_SCHEMA_WRITE").is_some_and(|v| v == "1") {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/schema/theme.v2.schema.json"
        );
        std::fs::write(path, schema_text()).expect("the schema is written");
        published_tests::name_this_release_in_the_classic_folder();
        return;
    }
    assert_eq!(
        published,
        schema(),
        "{SCHEMA_FILE} is not what the engine generates: run \
         PURLIS_THEME_SCHEMA_WRITE=1 cargo test -p purlis-core the_published_schema"
    );
    assert_eq!(PUBLISHED_SCHEMA, schema_text());
}

#[test]
fn the_schema_completes_every_token_and_motion_name_and_nothing_else() {
    let schema = schema();
    let tokens = schema["$defs"]["tokens"]["properties"]
        .as_object()
        .expect("token names");
    let named: Vec<&str> = tokens.keys().map(String::as_str).collect();
    let expected: Vec<&str> = TOKENS
        .iter()
        .copied()
        .chain(typed::all())
        .chain(geometry::STEPS.iter().map(|step| step.token))
        .collect();
    assert_eq!(named, expected);
    assert_eq!(schema["$defs"]["tokens"]["additionalProperties"], false);
    let motion = schema["$defs"]["motion"]["properties"].as_object().unwrap();
    let names: Vec<&str> = motion.keys().map(String::as_str).collect();
    let expected: Vec<&str> = vocabulary::DURATIONS
        .iter()
        .chain(vocabulary::EASINGS)
        .copied()
        .collect();
    assert_eq!(names, expected);
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["$defs"]["appearance"]["additionalProperties"], false);
}

#[test]
fn the_schema_s_hex_pattern_is_the_engine_s_colour_grammar() {
    let schema = schema();
    let pattern = schema["$defs"]["hex"]["pattern"].as_str().unwrap();
    assert_eq!(
        pattern,
        "^#([0-9a-fA-F]{3}|[0-9a-fA-F]{4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$"
    );
    let name = schema["$defs"]["name"]["pattern"].as_str().unwrap();
    assert_eq!(name, "^[a-z][a-z0-9-]{0,31}$");
}

// ---------------------------------------------------------------------------------------
// Following the system, with any theme
// ---------------------------------------------------------------------------------------

fn no_survey() -> Arc<[Offered]> {
    unreachable!("no survey when the person has a theme")
}

fn follow(theirs: &Theme, system: Appearance) -> Drawn {
    resolve(
        &Inputs {
            pick: Some(&Pick::System),
            theirs: Some(theirs),
            on: On::Every,
            system,
            contrast: Contrast::Standard,
        },
        no_survey,
    )
}

#[test]
fn following_the_system_with_a_two_appearance_theme_draws_the_one_the_system_has() {
    let mine = load(&both("Mine")).theme;
    let light = follow(&mine, Appearance::Light);
    assert!(light.follows_system);
    assert_eq!(light.note, None);
    let drawn = light.theme.expect("drawn");
    assert_eq!((drawn.name.as_str(), drawn.id.as_str()), ("Mine", "day"));
    let dark = follow(&mine, Appearance::Dark).theme.expect("drawn");
    assert_eq!(dark.id, "night");
}

#[test]
fn a_dark_only_theme_says_that_following_the_system_stays_dark() {
    let mine = load(&dark_only("Night Owl", json!({}))).theme;
    for system in [Appearance::Light, Appearance::Dark] {
        let drawn = super::follow(&mine, system, Contrast::Standard);
        assert_eq!(
            drawn.theme.as_ref().map(|it| it.appearance),
            Some(Appearance::Dark)
        );
        assert!(!drawn.follows_system);
        assert_eq!(
            drawn.note.as_deref(),
            Some("Night Owl has only a dark appearance, so following the system stays dark")
        );
    }
}

#[test]
fn a_light_only_theme_says_that_following_the_system_stays_light() {
    let mine = load(&json!({ "name": "Paper", "appearance": "light", "tokens": {} })).theme;
    let drawn = super::follow(&mine, Appearance::Dark, Contrast::Standard);
    assert_eq!(
        drawn.note.as_deref(),
        Some("Paper has only a light appearance, so following the system stays light")
    );
}

#[test]
fn a_system_pick_with_a_one_appearance_theme_of_the_person_s_stays_in_it_and_says_so() {
    // The old id is no special case (#2615): it names the person's own theme, followed as any
    // theme is, and Settings › Theme says that it is theirs and what following does with it.
    let mine = load(&dark_only("Night Owl", json!({}))).theme;
    for system in [Appearance::Light, Appearance::Dark] {
        let drawn = follow(&mine, system);
        assert_eq!(
            drawn,
            Drawn {
                theirs: true,
                ..super::follow(&mine, system, Contrast::Standard)
            }
        );
        assert!(drawn.theirs);
        assert!(!drawn.follows_system);
        assert_eq!(
            drawn.note.as_deref(),
            Some("Night Owl has only a dark appearance, so following the system stays dark")
        );
    }
}

#[test]
fn a_system_pick_follows_with_two_appearances_of_the_person_s_or_with_purlis() {
    // The person's own theme of two appearances follows the system with it (#2615), and with
    // none the default, Purlis, follows (#3034).
    let mine = load(&both("Mine")).theme;
    for system in [Appearance::Light, Appearance::Dark] {
        let drawn = follow(&mine, system);
        assert_eq!(
            drawn,
            Drawn {
                theirs: true,
                ..super::follow(&mine, system, Contrast::Standard)
            }
        );
        assert!(drawn.follows_system && drawn.theirs);
        let none = resolve(
            &Inputs {
                pick: Some(&Pick::System),
                theirs: None,
                on: On::Every,
                system,
                contrast: Contrast::Standard,
            },
            || unreachable!("no survey for following the system"),
        );
        assert_eq!(none, super::follow(purlis(), system, Contrast::Standard));
        assert!(none.follows_system && !none.theirs);
    }
}

#[test]
fn following_the_system_with_no_theme_of_the_person_s_follows_with_purlis() {
    let purlis = purlis();
    for (system, drawn) in [
        (Appearance::Light, purlis.of(Appearance::Light).unwrap()),
        (Appearance::Dark, purlis.first()),
    ] {
        let it = resolve(
            &Inputs {
                pick: Some(&Pick::System),
                theirs: None,
                on: On::Every,
                system,
                contrast: Contrast::Standard,
            },
            || unreachable!("no survey for following the system"),
        );
        assert_eq!(it.theme.as_ref(), Some(drawn));
        assert!(it.follows_system);
    }
}

#[test]
fn with_no_pick_a_two_appearance_theme_draws_its_first_appearance() {
    let mine = load(&both("Mine")).theme;
    let it = resolve(
        &Inputs {
            pick: None,
            theirs: Some(&mine),
            on: On::Every,
            system: Appearance::Light,
            contrast: Contrast::Standard,
        },
        no_survey,
    );
    assert_eq!(it.theme.map(|it| it.id), Some("night".to_owned()));
    assert!(!it.follows_system);
}

#[test]
fn a_high_contrast_appearance_is_not_what_following_the_system_picks() {
    let mine = load(&json!({
        "name": "Sharp",
        "appearances": [
            { "id": "hc", "appearance": "dark", "high-contrast": true },
            { "id": "dark", "appearance": "dark" },
            { "id": "light", "appearance": "light" }
        ]
    }))
    .theme;
    assert_eq!(follow(&mine, Appearance::Dark).theme.unwrap().id, "dark");
}

#[test]
fn an_extension_s_format_2_theme_is_read_like_any_other() {
    let text = both("Packed").to_string();
    let it = resolve(
        &Inputs {
            pick: None,
            theirs: None,
            on: On::Every,
            system: Appearance::Dark,
            contrast: Contrast::Standard,
        },
        || Arc::from(vec![Offered::of("acme".into(), "Packed".into(), &text)]),
    );
    assert_eq!(it.theme.unwrap().value("surface.base"), "#101010");
}

#[test]
fn the_person_s_format_2_theme_that_is_refused_is_trouble_and_draws_purlis() {
    let home = tempfile::tempdir().unwrap();
    let dir = crate::machine::dir(home.path());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("theme.json"),
        json!({ "name": "Mine", "extends": "nowhere" }).to_string(),
    )
    .unwrap();
    let mine = read_theirs(Some(home.path()));
    assert!(mine.found);
    assert!(mine.theme.is_none());
    // What refused it is kept as found, beside the sentence the window shows.
    assert_eq!(
        mine.complaints
            .iter()
            .map(|one| one.said.as_str())
            .collect::<Vec<_>>(),
        ["Mine extends \"nowhere\", which is not a theme purlis has"]
    );
    let trouble = mine.trouble.expect("trouble");
    assert!(
        trouble.contains("Mine extends \"nowhere\", which is not a theme purlis has"),
        "{trouble}"
    );

    std::fs::write(dir.join("theme.json"), both("Two").to_string()).unwrap();
    let two = read_theirs(Some(home.path()));
    assert_eq!(two.theme.map(|it| it.appearances.len()), Some(2));
}
