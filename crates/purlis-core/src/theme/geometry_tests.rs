// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Geometry, fonts, density and the font override at the engine's interface (#2326).

use serde_json::{Value, json};

use super::*;

/// The window's stylesheet of core geometry: what Purlis Classic draws today.
const GEOMETRY_CSS: &str = include_str!("../../../../app/src/theme/geometry.css");

fn said(resolved: &Resolved) -> Vec<String> {
    resolved
        .complaints
        .iter()
        .map(|one| one.said.clone())
        .collect()
}

/// Each custom property `geometry.css` declares, and its value as written.
fn declared() -> std::collections::BTreeMap<String, String> {
    GEOMETRY_CSS
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("--"))
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| {
            (
                format!("--{name}"),
                value.trim().trim_end_matches(';').to_owned(),
            )
        })
        .collect()
}

/// A format 2 theme with one dark appearance that sets `tokens`, and `more` beside it.
fn dark(tokens: Value, more: Value) -> Value {
    let mut theme = json!({
        "name": "Dense",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }]
    });
    for (key, value) in more.as_object().cloned().unwrap_or_default() {
        theme[key] = value;
    }
    theme
}

fn classic_dark() -> &'static ResolvedTheme {
    built_in("purlis-dark").expect("Classic's dark appearance")
}

// ---------------------------------------------------------------------------------------
// Core's geometry is Purlis Classic's, exactly
// ---------------------------------------------------------------------------------------

#[test]
fn core_s_geometry_writes_exactly_what_the_stylesheet_holds_for_purlis_classic() {
    let css = declared();
    for theme in built_ins() {
        let written = geometry::properties(&theme.geometry);
        assert!(!written.is_empty());
        for (property, value) in &written {
            assert_eq!(
                css.get(*property).map(String::as_str),
                Some(value.as_str()),
                "{property} as Purlis Classic draws it"
            );
        }
    }
    // Every geometry property the stylesheet declares is one a theme reaches, but the fonts.
    let reached: Vec<&str> = geometry::properties(&geometry::classic())
        .keys()
        .copied()
        .chain(["--font-ui", "--font-mono"])
        .collect();
    let mut declared: Vec<&str> = css.keys().map(String::as_str).collect();
    declared.sort_unstable();
    let mut reached = reached;
    reached.sort_unstable();
    assert_eq!(declared, reached);
}

#[test]
fn core_s_fonts_are_purlis_classic_s_and_the_terminal_s_own_family() {
    let css = declared();
    let fonts = &classic_dark().fonts;
    assert_eq!(css["--font-ui"], fonts.ui.stack);
    assert_eq!(css["--font-mono"], fonts.mono.stack);
    // What the terminal was handed as a literal before a theme could set it.
    assert_eq!(
        fonts.terminal.stack,
        "ui-monospace, SFMono-Regular, Menlo, monospace"
    );
    assert!(fonts.ui.faces.is_empty() && fonts.terminal.faces.is_empty());
    assert_eq!(fonts.ui.family, None);
}

#[test]
fn every_geometry_token_s_bound_holds_purlis_classic_s_values() {
    for step in geometry::STEPS {
        for value in [step.classic, step.compact] {
            assert!(
                (step.least..=step.most).contains(&value),
                "{} {value} is inside {}..{}",
                step.token,
                step.least,
                step.most
            );
        }
    }
    let properties: Vec<&str> = geometry::STEPS
        .iter()
        .map(|one| one.property)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut core: Vec<&str> = vocabulary::GEOMETRY
        .iter()
        .copied()
        .filter(|name| !name.starts_with("--font-ui") && *name != "--font-mono")
        .collect();
    core.sort_unstable();
    assert_eq!(properties, core);
}

// ---------------------------------------------------------------------------------------
// A theme sets geometry within bounds
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_may_set_geometry_inside_its_bounds_and_names_only_what_it_changes() {
    let loaded = load(&dark(
        json!({ "radius.md": 6, "space.4": 10, "line-height.body": 1.5, "accent.base": "#123456" }),
        json!({}),
    ));
    assert_eq!(said(&loaded), Vec::<String>::new());
    let theme = loaded.theme.first();
    assert_eq!(theme.geometry["radius.md"], 6.0);
    assert_eq!(theme.geometry["space.4"], 10.0);
    assert_eq!(theme.geometry["line-height.body"], 1.5);
    assert_eq!(theme.geometry["radius.sm"], 3.0);
    assert_eq!(theme.value("accent.base"), "#123456");
    let written = geometry::properties(&theme.geometry);
    assert_eq!(written["--radius-md"], "6px");
    assert_eq!(written["--space-4"], "calc(10rem / 14)");
    assert_eq!(written["--line-height-body"], "1.5");
}

#[test]
fn a_geometry_value_outside_its_bound_refuses_the_theme_and_names_the_bound() {
    let loaded = load(&dark(
        json!({ "radius.md": 40, "size.control": 12, "focus-ring.width": 0 }),
        json!({}),
    ));
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        [
            "radius.md in appearance dark is 40, outside its bound: it is a whole number from 0 \
             to 10 px",
            "size.control in appearance dark is 12, outside its bound: it is a whole number from \
             20 to 40 px at the default text size",
            "focus-ring.width in appearance dark is 0, outside its bound: it is a whole number \
             from 1 to 4 px",
        ]
    );
    assert_eq!(loaded.theme.name, "Purlis");
}

#[test]
fn a_geometry_value_is_a_number_never_css() {
    let loaded = load(&dark(
        json!({ "space.2": "4px; color: red", "radius.sm": 2.5, "line-height.tight": "1.2" }),
        json!({}),
    ));
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        [
            "space.2 in appearance dark is \"4px; color: red\", which is not a number; it is a \
             whole number from 1 to 8 px at the default text size",
            "radius.sm in appearance dark is 2.5, which is not a whole number; it is a whole \
             number from 0 to 8 px",
            "line-height.tight in appearance dark is \"1.2\", which is not a number; it is a \
             number from 1 to 1.4 times the text's size",
        ]
    );
}

#[test]
fn a_bound_that_is_one_value_says_so() {
    let loaded = load(&dark(json!({ "space.0": 2 }), json!({})));
    assert_eq!(
        said(&loaded),
        ["space.0 in appearance dark is 2, outside its bound: it is 0 in every theme"]
    );
}

#[test]
fn a_format_1_file_still_names_no_geometry() {
    let loaded = load(&json!({ "name": "Old", "tokens": { "radius.md": "#000" } }));
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        [
            "radius.md is geometry, which a theme sets in a theme folder's theme.json (format \
             2), not here"
        ]
    );
}

#[test]
fn a_theme_that_extends_another_keeps_its_parent_s_geometry() {
    let library = Library::new().with(
        "soft",
        dark(
            json!({ "radius.md": 8 }),
            json!({ "compact": { "radius.md": 5 } }),
        ),
    );
    let child = library.load(
        None,
        &json!({
            "name": "Softer", "extends": "soft",
            "appearances": [{ "id": "dark", "tokens": { "radius.lg": 12 } }]
        }),
    );
    assert_eq!(said(&child), Vec::<String>::new());
    let theme = child.theme.first();
    assert_eq!(theme.geometry["radius.md"], 8.0);
    assert_eq!(theme.geometry["radius.lg"], 12.0);
    assert_eq!(theme.compact["radius.md"], 5.0);
    assert_eq!(theme.compact["radius.lg"], 12.0);
}

// ---------------------------------------------------------------------------------------
// Density: every theme has a comfortable and a compact geometry
// ---------------------------------------------------------------------------------------

#[test]
fn purlis_classic_s_compact_is_defined_and_tighter_than_its_comfortable() {
    let theme = classic_dark();
    assert_eq!(theme.compact["space.4"], 6.0);
    assert_eq!(theme.compact["size.control"], 24.0);
    assert_eq!(theme.compact["size.chrome"], 18.0);
    // Text size is the person's own setting, and compact leaves it.
    assert_eq!(theme.compact["font-size.base"], 14.0);
    for step in geometry::STEPS {
        assert!(theme.compact[step.token] <= theme.geometry[step.token] || step.least < 0.0);
    }
    let written = geometry::properties(&theme.compact);
    assert_eq!(written["--size-control"], "calc(24rem / 14)");
    assert_eq!(written["--space-4"], "calc(6rem / 14)");
}

#[test]
fn every_theme_has_a_compact_geometry_even_one_that_names_none() {
    let loaded = load(&dark(json!({ "radius.md": 7 }), json!({})));
    let theme = loaded.theme.first();
    // Core's compact moves the spacing; a corner core's compact leaves follows the theme.
    assert_eq!(theme.compact["space.4"], 6.0);
    assert_eq!(theme.compact["radius.md"], 7.0);
    assert_eq!(theme.geometry["space.4"], 8.0);
}

#[test]
fn a_theme_tunes_its_compact_values_in_a_compact_layer() {
    let loaded = load(&dark(
        json!({}),
        json!({ "compact": { "space.4": 5, "size.control": 22 } }),
    ));
    assert_eq!(said(&loaded), Vec::<String>::new());
    let theme = loaded.theme.first();
    assert_eq!(theme.compact["space.4"], 5.0);
    assert_eq!(theme.compact["size.control"], 22.0);
    assert_eq!(theme.compact["size.chrome"], 18.0);
    assert_eq!(theme.geometry["space.4"], 8.0);
}

#[test]
fn a_compact_layer_is_held_to_the_same_bounds() {
    let loaded = load(&dark(
        json!({}),
        json!({ "compact": { "size.control": 8, "accent.base": "#000" } }),
    ));
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        [
            "size.control in compact is 8, outside its bound: it is a whole number from 20 to \
             40 px at the default text size",
            "accent.base in compact is not a purlis geometry token",
        ]
    );
}

// ---------------------------------------------------------------------------------------
// Fonts: woff2 files inside the theme's folder, served by purlis
// ---------------------------------------------------------------------------------------

/// A woff2 file's bytes: the signature, then `body`.
fn woff2(body: &str) -> Vec<u8> {
    let mut bytes = b"wOF2".to_vec();
    bytes.extend_from_slice(body.as_bytes());
    bytes
}

/// A theme folder `name` in `themes` holding `theme` and each of `files` under `fonts/`.
fn folder(themes: &std::path::Path, name: &str, theme: &Value, files: &[(&str, Vec<u8>)]) {
    let at = themes.join(name);
    std::fs::create_dir_all(at.join("fonts")).unwrap();
    std::fs::write(at.join(THEME_JSON), theme.to_string()).unwrap();
    for (file, bytes) in files {
        std::fs::write(at.join("fonts").join(file), bytes).unwrap();
    }
}

fn with_fonts(fonts: Value) -> Value {
    dark(json!({}), json!({ "fonts": fonts }))
}

fn plex() -> Value {
    json!({
        "ui": { "family": "IBM Plex Sans", "faces": [
            { "file": "fonts/Plex-Regular.woff2", "weight": 400 },
            { "file": "fonts/Plex-SemiBold.woff2", "weight": 600 }
        ] },
        "terminal": { "family": "JetBrains Mono", "faces": [
            { "file": "fonts/Mono-Regular.woff2" }
        ] }
    })
}

fn plex_files() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("Plex-Regular.woff2", woff2("plex regular")),
        ("Plex-SemiBold.woff2", woff2("plex semibold")),
        ("Mono-Regular.woff2", woff2("mono regular")),
    ]
}

#[test]
fn a_theme_folder_names_its_fonts_from_woff2_files_inside_it() {
    let dir = tempfile::tempdir().unwrap();
    folder(dir.path(), "plex", &with_fonts(plex()), &plex_files());
    let loaded = Library::new()
        .reading_named_fonts()
        .load_folder(dir.path(), "plex");
    assert_eq!(said(&loaded), Vec::<String>::new());
    let fonts = &loaded.theme.first().fonts;
    assert_eq!(fonts.ui.family.as_deref(), Some("IBM Plex Sans"));
    let weights: Vec<u16> = fonts.ui.faces.iter().map(|one| one.weight).collect();
    assert_eq!(weights, [400, 600]);
    assert_eq!(fonts.terminal.faces[0].weight, 400);
    assert_eq!(fonts.terminal.faces[0].style, Style::Normal);
    // The family the window draws is purlis's own name for the faces, then core's family.
    assert!(
        fonts.ui.stack.starts_with("\"purlis-ui-"),
        "{}",
        fonts.ui.stack
    );
    assert!(fonts.ui.stack.ends_with(&format!("\", {CLASSIC_UI}")));
    assert!(!fonts.ui.stack.contains("Plex"));
    // A font the theme does not name is core's.
    assert_eq!(fonts.mono.stack, CLASSIC_MONO);
}

#[test]
fn purlis_serves_a_font_s_bytes_by_their_digest_and_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    // Bytes of this test's own, so no other test's folder holds the same face.
    let mut files = plex_files();
    files[0].1 = woff2("served by its digest");
    folder(dir.path(), "plex", &with_fonts(plex()), &files);
    let loaded = Library::new()
        .reading_named_fonts()
        .load_folder(dir.path(), "plex");
    let asset = loaded.theme.first().fonts.ui.faces[0].asset.clone();
    assert_eq!(asset.len(), 64);
    assert_eq!(served(&asset).unwrap(), woff2("served by its digest"));
    assert!(served(&"0".repeat(64)).is_err());
    assert!(served("../../machine.json").is_err());
    // A file that changed after the theme was read is not served as what it was.
    std::fs::write(
        dir.path().join("plex/fonts/Plex-Regular.woff2"),
        woff2("swapped"),
    )
    .unwrap();
    let refused = served(&asset).unwrap_err();
    assert!(
        refused.contains("changed since purlis read the theme"),
        "{refused}"
    );
}

#[test]
fn a_font_is_never_a_url_or_a_path_out_of_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    for (name, file) in [
        ("url", "https://fonts.example/Plex.woff2"),
        ("up", "fonts/../../other/Plex.woff2"),
        ("abs", "/Library/Fonts/Plex.woff2"),
        ("aside", "Plex.woff2"),
        ("ttf", "fonts/Plex.ttf"),
        ("deep", "fonts/sub/Plex.woff2"),
    ] {
        let theme = with_fonts(json!({ "ui": { "family": "Plex", "faces": [{ "file": file }] } }));
        folder(dir.path(), name, &theme, &[]);
        let loaded = Library::new()
            .reading_named_fonts()
            .load_folder(dir.path(), name);
        assert!(loaded.refused, "{file}");
        assert_eq!(
            said(&loaded),
            [format!(
                "a face of fonts.ui names \"{file}\", which is not fonts/<name>.woff2 inside the \
                 theme's folder"
            )]
        );
    }
}

#[test]
fn a_font_file_is_a_woff2_file_under_the_cap() {
    let dir = tempfile::tempdir().unwrap();
    let theme = with_fonts(json!({ "ui": { "family": "Plex", "faces": [
        { "file": "fonts/Fake.woff2" }
    ] } }));
    folder(
        dir.path(),
        "fake",
        &theme,
        &[("Fake.woff2", b"OTTO not woff2".to_vec())],
    );
    assert_eq!(
        said(
            &Library::new()
                .reading_named_fonts()
                .load_folder(dir.path(), "fake")
        ),
        ["fonts/Fake.woff2 is not a woff2 file"]
    );
    let big = vec![b'w'; usize::try_from(MOST_FONT_BYTES).unwrap() + 1];
    folder(dir.path(), "big", &theme, &[("Fake.woff2", big)]);
    assert_eq!(
        said(
            &Library::new()
                .reading_named_fonts()
                .load_folder(dir.path(), "big")
        ),
        [format!(
            "fonts/Fake.woff2 is {} bytes, and a font is never larger than {MOST_FONT_BYTES}",
            MOST_FONT_BYTES + 1
        )]
    );
    folder(dir.path(), "gone", &theme, &[]);
    assert_eq!(
        said(
            &Library::new()
                .reading_named_fonts()
                .load_folder(dir.path(), "gone")
        ),
        ["fonts/Fake.woff2 is not in the theme's folder"]
    );
}

#[cfg(unix)]
#[test]
fn a_font_reached_through_a_link_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(elsewhere.path().join("Plex.woff2"), woff2("x")).unwrap();
    let theme = with_fonts(json!({ "ui": { "family": "Plex", "faces": [
        { "file": "fonts/Plex.woff2" }
    ] } }));
    folder(dir.path(), "linked", &theme, &[]);
    std::os::unix::fs::symlink(
        elsewhere.path().join("Plex.woff2"),
        dir.path().join("linked/fonts/Plex.woff2"),
    )
    .unwrap();
    let loaded = Library::new()
        .reading_named_fonts()
        .load_folder(dir.path(), "linked");
    assert!(loaded.refused);
    // The folder itself as a link is refused too.
    std::fs::remove_dir_all(dir.path().join("linked/fonts")).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), dir.path().join("linked/fonts")).unwrap();
    assert!(
        Library::new()
            .reading_named_fonts()
            .load_folder(dir.path(), "linked")
            .refused
    );
}

#[test]
fn a_theme_not_read_from_a_folder_names_no_font() {
    let loaded = load(&with_fonts(plex()));
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        [
            "fonts are read from a theme folder's fonts/, and this theme is not read from a \
             folder"
        ]
    );
}

#[test]
fn a_font_s_grammar_is_closed() {
    let dir = tempfile::tempdir().unwrap();
    let theme = with_fonts(json!({
        "ui": { "family": "Plex\"; color: red", "faces": [{ "file": "fonts/A.woff2", "weight": 950, "style": "oblique" }] },
        "headline": { "family": "X", "faces": [] },
        "mono": { "family": "Mono" }
    }));
    folder(dir.path(), "odd", &theme, &[("A.woff2", woff2("a"))]);
    let loaded = Library::new()
        .reading_named_fonts()
        .load_folder(dir.path(), "odd");
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        [
            "fonts.ui.family \"Plex\\\"; color: red\" is not letters, digits, spaces, dashes, \
             underscores and dots starting with a letter or a digit, at most 64 long",
            "a face's weight in fonts.ui is 950, which is not a whole number from 100 to 900",
            "a face's style in fonts.ui is \"oblique\", which is \"normal\" or \"italic\"",
            "headline is not a font of a theme: ui, mono or terminal",
            "fonts.mono.faces is a list of one to 8 woff2 files in the folder's fonts/",
        ]
    );
}

#[test]
fn a_theme_that_extends_another_keeps_its_parent_s_fonts() {
    let dir = tempfile::tempdir().unwrap();
    folder(dir.path(), "plex", &with_fonts(plex()), &plex_files());
    folder(
        dir.path(),
        "warm",
        &json!({ "name": "Warm", "extends": "plex",
                 "appearances": [{ "id": "dark", "tokens": { "accent.base": "#aa5500" } }] }),
        &[],
    );
    let library = Library::new()
        .reading_named_fonts()
        .with_folder(dir.path(), "plex")
        .with_folder(dir.path(), "warm");
    let loaded = library.load_id("warm");
    assert_eq!(said(&loaded), Vec::<String>::new());
    let fonts = &loaded.theme.first().fonts;
    assert_eq!(fonts.ui.family.as_deref(), Some("IBM Plex Sans"));
    assert_eq!(fonts.ui.faces.len(), 2);
}

#[test]
fn the_person_s_own_theme_reads_its_fonts_from_beside_it() {
    let config = tempfile::tempdir().unwrap();
    let home = crate::machine::dir(config.path());
    std::fs::create_dir_all(home.join("fonts")).unwrap();
    std::fs::write(home.join("fonts/Mono-Regular.woff2"), woff2("mine")).unwrap();
    std::fs::write(
        home.join("theme.json"),
        with_fonts(json!({ "terminal": { "family": "Mine", "faces": [
            { "file": "fonts/Mono-Regular.woff2" }
        ] } }))
        .to_string(),
    )
    .unwrap();
    let theirs = read_theirs(Some(config.path()));
    assert_eq!(theirs.trouble, None);
    let theme = theirs.theme.expect("in force");
    let face = &theme.first().fonts.terminal.faces[0];
    assert_eq!(served(&face.asset).unwrap(), woff2("mine"));
}

// ---------------------------------------------------------------------------------------
// The person's layer: density and the font override, over every theme
// ---------------------------------------------------------------------------------------

#[test]
fn the_person_s_density_and_fonts_are_read_from_the_layout_file() {
    let (person, complaints) = Person::read(Some(&json!({
        "density": "compact",
        "fonts": { "terminal": "Iosevka Term", "ui": "" }
    })));
    assert!(complaints.is_empty());
    assert_eq!(person.density, Density::Compact);
    let layer = person.layer();
    assert_eq!(layer.density, Density::Compact);
    assert_eq!(
        layer.fonts.get("terminal").map(String::as_str),
        Some("\"Iosevka Term\", ui-monospace, SFMono-Regular, Menlo, monospace")
    );
    // An empty field is the theme's font.
    assert!(!layer.fonts.contains_key("ui"));
}

#[test]
fn nothing_set_is_comfortable_and_the_theme_s_fonts() {
    let (person, complaints) = Person::read(None);
    assert!(complaints.is_empty());
    assert_eq!(person.layer(), Layer::default());
    assert_eq!(person.layer().density, Density::Comfortable);
}

#[test]
fn a_setting_the_engine_cannot_use_is_the_default_and_said() {
    let (person, complaints) = Person::read(Some(&json!({
        "density": "cosy",
        "fonts": { "ui": "Evil\"; } body { color: red", "headline": "X" },
        "zoom": 2
    })));
    assert_eq!(person, Person::default());
    let said: Vec<&str> = complaints.iter().map(|one| one.said.as_str()).collect();
    assert_eq!(
        said,
        [
            "zoom is not a setting of appearance, so it is left out",
            "density is \"cosy\", which is \"comfortable\" or \"compact\", so it is comfortable",
            "the ui font \"Evil\\\"; } body { color: red\" is not letters, digits, spaces, dashes, \
             underscores and dots starting with a letter or a digit, at most 64 long, so it is \
             the theme's",
            "headline is not a font you can set: ui, mono or terminal",
        ]
    );
}

#[test]
fn the_person_s_font_beats_the_theme_s_in_every_theme() {
    let dir = tempfile::tempdir().unwrap();
    folder(dir.path(), "plex", &with_fonts(plex()), &plex_files());
    let theme = Library::new()
        .reading_named_fonts()
        .load_folder(dir.path(), "plex")
        .theme;
    let (person, _) = Person::read(Some(&json!({ "fonts": { "ui": "Atkinson Hyperlegible" } })));
    let layer = person.layer();
    for drawn in [theme.first(), classic_dark()] {
        let ui = layer.drawn_font("ui", drawn);
        assert_eq!(ui, format!("\"Atkinson Hyperlegible\", {CLASSIC_UI}"));
        // The slot the person left is the theme's.
        assert_eq!(
            layer.drawn_font("terminal", drawn),
            drawn.fonts.terminal.stack
        );
    }
}

#[test]
fn the_person_s_density_picks_which_of_a_theme_s_geometries_is_drawn() {
    let (compact, _) = Person::read(Some(&json!({ "density": "compact" })));
    let comfortable = Layer::default();
    let loaded = load(&dark(json!({}), json!({ "compact": { "space.4": 5 } })));
    for theme in [loaded.theme.first(), classic_dark()] {
        assert_eq!(compact.layer().drawn_geometry(theme), &theme.compact);
        assert_eq!(comfortable.drawn_geometry(theme), &theme.geometry);
    }
    assert_eq!(
        compact.layer().drawn_geometry(loaded.theme.first())["space.4"],
        5.0
    );
}

// ---------------------------------------------------------------------------------------
// The schema
// ---------------------------------------------------------------------------------------

#[test]
fn the_schema_bounds_every_geometry_token_and_describes_fonts_and_compact() {
    let schema = schema();
    let tokens = &schema["$defs"]["tokens"]["properties"];
    for step in geometry::STEPS {
        let one = &tokens[step.token];
        assert_eq!(one["minimum"].as_f64(), Some(step.least), "{}", step.token);
        assert_eq!(one["maximum"].as_f64(), Some(step.most), "{}", step.token);
    }
    assert_eq!(tokens["radius.md"]["type"], "integer");
    assert_eq!(tokens["line-height.body"]["type"], "number");
    assert!(schema["properties"]["compact"].is_object());
    assert!(schema["properties"]["fonts"].is_object());
    assert_eq!(
        schema["$defs"]["face"]["properties"]["file"]["pattern"],
        "^fonts/[A-Za-z0-9_-][A-Za-z0-9._-]*\\.woff2$"
    );
}

// ---------------------------------------------------------------------------------------
// Review round: compact is never looser, faces of a refused theme, the person's icons path
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_s_compact_is_never_looser_than_its_comfortable() {
    let loaded = load(&dark(
        json!({ "size.control": 22, "size.chrome": 16, "space.4": 5, "space.6": 20 }),
        json!({}),
    ));
    assert_eq!(said(&loaded), Vec::<String>::new());
    let theme = loaded.theme.first();
    // Tighter than core's compact already: compact keeps the theme's own.
    assert_eq!(theme.compact["size.control"], 22.0);
    assert_eq!(theme.compact["size.chrome"], 16.0);
    assert_eq!(theme.compact["space.4"], 5.0);
    // Looser than core's compact: core's compact.
    assert_eq!(theme.compact["space.6"], 12.0);
    for step in geometry::STEPS {
        if step.compact != step.classic {
            assert!(
                theme.compact[step.token] <= theme.geometry[step.token],
                "{}",
                step.token
            );
        }
    }
}

/// The sha-256 of `bytes`, as the engine names a face.
fn sha(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn a_refused_theme_s_faces_are_never_served() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = woff2("a face of a refused theme");
    // The fonts are fine; a colour elsewhere is not, so the theme is refused.
    let theme = dark(
        json!({ "accent.base": "red" }),
        json!({ "fonts": { "ui": { "family": "Refused", "faces": [
            { "file": "fonts/Refused.woff2" }
        ] } } }),
    );
    folder(
        dir.path(),
        "refused",
        &theme,
        &[("Refused.woff2", bytes.clone())],
    );
    let loaded = Library::new()
        .reading_named_fonts()
        .load_folder(dir.path(), "refused");
    assert!(loaded.refused);
    assert!(served(&sha(&bytes)).is_err());
    // The same folder, put right, is served.
    let fixed = dark(
        json!({}),
        json!({ "fonts": { "ui": { "family": "Refused", "faces": [
            { "file": "fonts/Refused.woff2" }
        ] } } }),
    );
    folder(dir.path(), "refused", &fixed, &[]);
    assert!(
        !Library::new()
            .reading_named_fonts()
            .load_folder(dir.path(), "refused")
            .refused
    );
    assert_eq!(served(&sha(&bytes)).unwrap(), bytes);
}

#[test]
fn the_person_s_icon_set_is_a_file_beside_their_theme_or_in_its_icons_folder() {
    let config = tempfile::tempdir().unwrap();
    let home = crate::machine::dir(config.path());
    std::fs::create_dir_all(&home).unwrap();
    let theme = |icons: &str| {
        std::fs::write(
            home.join("theme.json"),
            dark(json!({}), json!({ "icons": icons })).to_string(),
        )
        .unwrap();
        read_theirs(Some(config.path()))
    };
    for deeper in ["vaults/icons.json", "icons/sub/set.json", "../icons.json"] {
        let theirs = theme(deeper);
        let trouble = theirs.trouble.unwrap_or_default();
        assert!(
            trouble.contains("which is not a file beside theme.json or in its icons/ folder"),
            "{deeper}: {trouble}"
        );
    }
    // A plain file beside it, and one in its icons/, are read (here: not there, and said so).
    for allowed in ["icons.json", "icons/set.json"] {
        let trouble = theme(allowed).trouble.unwrap_or_default();
        assert!(
            trouble.contains(&format!("has no {allowed}")),
            "{allowed}: {trouble}"
        );
    }
}

/// What `purlis docs show themes` prints.
const THEMES_MD: &str = include_str!("../../docs/themes.md");

/// One row of the docs' geometry table: token, unit, least, most, Classic, Compact.
type DocsRow = (String, String, f64, f64, f64, f64);

/// How the docs' table writes a unit.
fn unit_said(unit: geometry::Unit) -> &'static str {
    match unit {
        geometry::Unit::Text => "px at 14px",
        geometry::Unit::Px => "px",
        geometry::Unit::Weight => "weight",
        geometry::Unit::Ratio => "× text",
    }
}

/// Each row of the docs' geometry table, in its order: one token a row.
fn docs_geometry_rows(page: &str) -> Vec<DocsRow> {
    let number = |text: &str| -> f64 {
        let text = text.trim().replace('−', "-");
        text.parse()
            .unwrap_or_else(|_| panic!("not a number: {text:?}"))
    };
    page.lines()
        .skip_while(|line| {
            !line.starts_with("| Token | Unit | Bounds | Classic | Compact | What it reaches |")
        })
        .skip(2)
        .take_while(|line| line.starts_with('|'))
        .map(|line| {
            let cells: Vec<&str> = line.trim_matches('|').split(" | ").collect();
            assert!(cells.len() == 6, "six cells in {line:?}");
            let token = cells[0].trim().trim_matches('`');
            let (least, most) = cells[2]
                .split_once('–')
                .unwrap_or_else(|| panic!("a range: {:?}", cells[2]));
            (
                token.to_owned(),
                cells[1].trim().to_owned(),
                number(least),
                number(most),
                number(cells[3]),
                number(cells[4]),
            )
        })
        .collect()
}

#[test]
fn the_docs_geometry_table_is_the_engine_s_steps_row_for_row() {
    let steps: Vec<DocsRow> = geometry::STEPS
        .iter()
        .map(|one| {
            (
                one.token.to_owned(),
                unit_said(one.unit).to_owned(),
                one.least,
                one.most,
                one.classic,
                one.compact,
            )
        })
        .collect();
    assert_eq!(
        docs_geometry_rows(THEMES_MD),
        steps,
        "docs/themes.md's geometry table is STEPS: every token once, in its order, with its unit, \
         its bounds, Classic's value and Classic's compact value"
    );
}

#[test]
fn the_docs_geometry_scan_reads_one_token_a_row_and_stops_at_the_table_s_end() {
    let page = "x\n\n| Token | Unit | Bounds | Classic | Compact | What it reaches |\n|---|---|---|---|---|---|\n| `a.b` | px | −4–0 | −2 | −1 | x |\n| `a.c` | × text | 1.2–1.8 | 1.4 | 1.3 | y |\n\n| `not.read` | px | 1–2 | 1 | 1 | z |\n";
    assert_eq!(
        docs_geometry_rows(page),
        vec![
            ("a.b".to_owned(), "px".to_owned(), -4.0, 0.0, -2.0, -1.0),
            ("a.c".to_owned(), "× text".to_owned(), 1.2, 1.8, 1.4, 1.3),
        ]
    );
}

/// `text` with every run of whitespace one space, so a sentence is found however it wraps.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A number as the docs write it: to two places, no trailing zeros.
fn written(number: f64) -> String {
    format!("{}", (number * 100.0).round() / 100.0)
}

/// The tokens Classic's compact moves, as the docs' compact rule names them: the run of
/// `space.*` as `first` to `last`, then each other token.
fn moved_said() -> String {
    let moved: Vec<&str> = geometry::STEPS
        .iter()
        .filter(|one| one.compact != one.classic)
        .map(|one| one.token)
        .collect();
    let spaces: Vec<&str> = moved
        .iter()
        .copied()
        .filter(|token| token.starts_with("space."))
        .collect();
    let mut parts = Vec::new();
    if let (Some(first), Some(last)) = (spaces.first(), spaces.last()) {
        parts.push(format!("`{first}` to `{last}`"));
    }
    parts.extend(
        moved
            .iter()
            .filter(|token| !token.starts_with("space."))
            .map(|token| format!("`{token}`")),
    );
    parts.join(", ")
}

#[test]
fn the_docs_hold_each_line_the_check_fits_with_classic_s_numbers_in_both_densities() {
    let docs = one_line(THEMES_MD);
    let section = docs
        .split_once("### A line fits its box")
        .and_then(|(_, after)| after.split(" ## ").next())
        .expect("the docs' line-fits section");
    assert_eq!(
        section.matches("` within `").count(),
        FITS.len(),
        "the docs list one line for each line the check fits"
    );
    for fit in FITS {
        let named = format!(
            "`{}` × `{}` within `{}`",
            fit.size, fit.line_height, fit.height
        );
        assert!(section.contains(&named), "the docs say {named}");
        for (density, value) in [
            ("comfortable", geometry::classic()),
            ("compact", geometry::classic_compact()),
        ] {
            let (size, line_height, room) = (
                value[&fit.size],
                value[&fit.line_height],
                value[&fit.height],
            );
            let sum = format!(
                "{} × {} = {} within {}",
                written(size),
                written(line_height),
                written(size * line_height),
                written(room)
            );
            assert!(
                section.contains(&sum),
                "the docs say Classic's {density} line: {sum}"
            );
        }
    }
}

#[test]
fn the_docs_compact_rule_names_the_tokens_classic_s_compact_moves() {
    // `first` to `last` holds only while the spacing steps compact moves are one run.
    let at: Vec<usize> = geometry::STEPS
        .iter()
        .enumerate()
        .filter(|(_, one)| one.token.starts_with("space.") && one.compact != one.classic)
        .map(|(at, _)| at)
        .collect();
    assert!(
        at.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "the spacing steps compact moves are one run: {at:?}"
    );
    let named = moved_said();
    assert!(one_line(THEMES_MD).contains(&named), "the docs say {named}");
}
