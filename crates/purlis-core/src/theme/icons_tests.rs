// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A theme's icon set at the engine's interface (#2337): read from the theme's folder under the
//! folder's containment and cap, held to the path grammar, inherited through `extends`, and
//! carried by every appearance the window is handed.

use std::path::Path;

use serde_json::{Value, json};

use super::vocabulary::{GLYPHS, RAIL_VIEWS};
use super::*;

/// Purlis's own icon set, as the Purlis theme folder ships it.
const PURLIS_ICONS: &str = include_str!("built-in/purlis/icons/icons.json");

fn said(resolved: &Resolved) -> Vec<String> {
    resolved
        .complaints
        .iter()
        .map(|one| one.said.clone())
        .collect()
}

/// A theme that names the icon set `icons`, with one dark appearance.
fn naming(name: &str, icons: Value) -> Value {
    json!({
        "name": name,
        "icons": icons,
        "appearances": [{ "id": "dark", "appearance": "dark" }]
    })
}

/// The folder `name` in `themes` holding `theme`, and each of `files` beside it.
fn folder(themes: &Path, name: &str, theme: &Value, files: &[(&str, &str)]) {
    let at = themes.join(name);
    std::fs::create_dir_all(&at).unwrap();
    std::fs::write(at.join("theme.json"), theme.to_string()).unwrap();
    for (file, text) in files {
        let path = at.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

/// An icon set drawing `views`, each as one path.
fn set(views: &[(&str, &str)]) -> String {
    let views: serde_json::Map<String, Value> = views
        .iter()
        .map(|(view, d)| ((*view).to_owned(), json!({ "paths": [d] })))
        .collect();
    json!({ "views": views }).to_string()
}

/// The theme folder `dusk` naming `icons/icons.json`, holding `icons`, loaded.
fn dusk_with(icons: &str) -> Resolved {
    let dir = tempfile::tempdir().unwrap();
    folder(
        dir.path(),
        "dusk",
        &naming("Dusk", json!("icons/icons.json")),
        &[("icons/icons.json", icons)],
    );
    Library::new().load_folder(dir.path(), "dusk")
}

#[test]
fn a_theme_folder_s_icon_set_draws_its_views_in_every_appearance() {
    let dir = tempfile::tempdir().unwrap();
    folder(
        dir.path(),
        "dusk",
        &json!({
            "name": "Dusk",
            "icons": "icons/icons.json",
            "appearances": [
                { "id": "dark", "appearance": "dark" },
                { "id": "light", "appearance": "light" }
            ]
        }),
        &[("icons/icons.json", &set(&[("chats", "M3 3h18v18H3z")]))],
    );
    let loaded = Library::new().load_folder(dir.path(), "dusk");
    assert_eq!(said(&loaded), Vec::<String>::new());
    for one in &loaded.theme.appearances {
        assert_eq!(
            one.icons.views["chats"].paths,
            ["M3 3h18v18H3z"],
            "{}",
            one.id
        );
        assert_eq!(one.icons.views.len(), 1);
    }
}

#[test]
fn purlis_classic_draws_the_window_s_own_marks() {
    for one in &classic().appearances {
        assert!(one.icons.is_empty(), "{}", one.id);
    }
    // And says nothing of icons to whoever reads a resolved theme.
    let shown = serde_json::to_value(classic().first()).unwrap();
    assert!(shown.get("icons").is_none());
}

#[test]
fn purlis_s_icon_set_is_every_rail_view_in_the_grammar() {
    let dir = tempfile::tempdir().unwrap();
    folder(
        dir.path(),
        "purlis",
        &naming("Purlis", json!("icons/icons.json")),
        &[("icons/icons.json", PURLIS_ICONS)],
    );
    let loaded = Library::new().load_folder(dir.path(), "purlis");
    assert_eq!(said(&loaded), Vec::<String>::new());
    let views: Vec<&str> = loaded.theme.first().icons.views.keys().copied().collect();
    let mut every: Vec<&str> = RAIL_VIEWS.to_vec();
    every.sort_unstable();
    assert_eq!(views, every);
}

#[test]
fn a_view_purlis_does_not_have_is_left_out_and_said() {
    let loaded = dusk_with(&set(&[("chats", "M1 1h2"), ("weather", "M1 1h2")]));
    assert!(!loaded.refused);
    assert_eq!(
        said(&loaded),
        ["weather in icons/icons.json is not a purlis view, so it is left out"]
    );
    assert_eq!(loaded.theme.first().icons.views.len(), 1);
}

#[test]
fn a_path_outside_the_grammar_refuses_the_theme() {
    for (d, why) in [
        (
            "L1 1",
            "path 1 of chats in icons/icons.json is not path data",
        ),
        (
            "M1 1 url(#x)",
            "path 1 of chats in icons/icons.json is not path data",
        ),
        (
            "M1 1\"/><script",
            "path 1 of chats in icons/icons.json is not path data",
        ),
        ("", "path 1 of chats in icons/icons.json is not path data"),
    ] {
        let loaded = dusk_with(&set(&[("chats", d)]));
        assert!(loaded.refused, "{d}");
        assert!(
            said(&loaded)[0].starts_with(why),
            "{d}: {:?}",
            said(&loaded)
        );
        assert_eq!(&loaded.theme, purlis());
    }
}

#[test]
fn a_path_past_the_cap_refuses_the_theme_with_its_length() {
    let long = format!("M{}", "1".repeat(MOST_PATH));
    let loaded = dusk_with(&set(&[("chats", &long)]));
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        [format!(
            "path 1 of chats in icons/icons.json is {} long, and a path is at most {MOST_PATH}",
            MOST_PATH + 1
        )]
    );
}

#[test]
fn a_symbol_is_one_to_the_cap_of_paths_and_nothing_else() {
    let many: Vec<&str> = vec!["M1 1"; MOST_PATHS + 1];
    for (icons, why) in [
        (
            json!({ "views": { "chats": { "paths": [] } } }),
            format!("paths of chats in icons/icons.json is a list of 1 to {MOST_PATHS} paths"),
        ),
        (
            json!({ "views": { "chats": { "paths": many } } }),
            format!("paths of chats in icons/icons.json is a list of 1 to {MOST_PATHS} paths"),
        ),
        (
            json!({ "views": { "chats": { "paths": ["M1 1"], "fill": "red" } } }),
            "fill is not a key of an icon (chats in icons/icons.json)".to_owned(),
        ),
        (
            json!({ "views": { "chats": "M1 1" } }),
            "chats in icons/icons.json is not an icon: an icon is an object with its paths"
                .to_owned(),
        ),
        (
            json!({ "views": {}, "style": "x" }),
            "style is not a key of an icon set (icons/icons.json)".to_owned(),
        ),
        (
            json!({ "views": [] }),
            "views in icons/icons.json is an object of view names to icons".to_owned(),
        ),
        (
            json!([]),
            "icons/icons.json is not an icon set: a set is a JSON object".to_owned(),
        ),
    ] {
        let loaded = dusk_with(&icons.to_string());
        assert!(loaded.refused, "{icons}");
        assert_eq!(said(&loaded), [why], "{icons}");
    }
}

#[test]
fn a_set_with_more_views_than_the_cap_is_refused_with_its_count() {
    let views: serde_json::Map<String, Value> = (0..65)
        .map(|at| (format!("view-{at}"), json!({ "paths": ["M1 1"] })))
        .collect();
    let loaded = dusk_with(&json!({ "views": views }).to_string());
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        ["icons/icons.json names 65 views, and a set names at most 64"]
    );
}

#[test]
fn an_icon_set_is_a_path_inside_the_theme_s_folder() {
    let dir = tempfile::tempdir().unwrap();
    let themes = dir.path().join("themes");
    std::fs::create_dir(&themes).unwrap();
    std::fs::write(dir.path().join("outside.json"), set(&[("chats", "M1 1")])).unwrap();
    for named in [
        "../../outside.json",
        "/etc/hosts",
        "icons/../../../outside.json",
        "./icons.json",
        "",
        "C:icons.json",
        "icons\\icons.json",
    ] {
        folder(&themes, "dusk", &naming("Dusk", json!(named)), &[]);
        let loaded = Library::new().load_folder(&themes, "dusk");
        assert!(loaded.refused, "{named}");
        assert_eq!(
            said(&loaded),
            [format!(
                "icons is {}, which is not a path inside the theme's folder",
                serde_json::to_string(named).unwrap()
            )],
            "{named}"
        );
    }
}

#[test]
fn an_icon_set_that_is_not_there_or_not_json_is_said() {
    let dir = tempfile::tempdir().unwrap();
    folder(
        dir.path(),
        "dusk",
        &naming("Dusk", json!("icons/icons.json")),
        &[],
    );
    let missing = Library::new().load_folder(dir.path(), "dusk");
    assert!(missing.refused);
    assert!(
        said(&missing)[0].ends_with("has no icons/icons.json"),
        "{:?}",
        said(&missing)
    );

    let broken = dusk_with("{ nope");
    assert!(broken.refused);
    assert!(
        said(&broken)[0].contains("an icon set is JSON: "),
        "{:?}",
        said(&broken)
    );
}

#[test]
fn an_icon_set_past_the_cap_is_not_read() {
    let padding = " ".repeat(usize::try_from(crate::windowprefs::MAX_BYTES).unwrap());
    let loaded = dusk_with(&format!("{}{padding}", set(&[("chats", "M1 1")])));
    assert!(loaded.refused);
    assert!(
        said(&loaded)[0].contains("never larger than 65536"),
        "{:?}",
        said(&loaded)
    );
}

#[cfg(unix)]
#[test]
fn an_icon_set_that_is_a_link_or_under_a_link_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    std::fs::write(elsewhere.join("icons.json"), set(&[("chats", "M1 1")])).unwrap();
    let themes = dir.path().join("themes");
    folder(&themes, "file", &naming("File", json!("icons.json")), &[]);
    std::os::unix::fs::symlink(elsewhere.join("icons.json"), themes.join("file/icons.json"))
        .unwrap();
    folder(
        &themes,
        "dir",
        &naming("Dir", json!("icons/icons.json")),
        &[],
    );
    std::os::unix::fs::symlink(&elsewhere, themes.join("dir/icons")).unwrap();
    for name in ["file", "dir"] {
        let loaded = Library::new().load_folder(&themes, name);
        assert!(loaded.refused, "{name}");
        assert_eq!(&loaded.theme, purlis(), "{name}");
    }
}

#[test]
fn an_icon_set_that_is_not_a_plain_file_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    folder(dir.path(), "dusk", &naming("Dusk", json!("icons")), &[]);
    std::fs::create_dir(dir.path().join("dusk/icons")).unwrap();
    let loaded = Library::new().load_folder(dir.path(), "dusk");
    assert!(loaded.refused);
    assert!(
        said(&loaded)[0].ends_with("is not a plain file"),
        "{:?}",
        said(&loaded)
    );
}

#[test]
fn icons_is_a_path_and_is_read_only_from_a_theme_folder() {
    let inline = load(&naming("Inline", json!({ "views": {} })));
    assert!(inline.refused);
    assert_eq!(
        said(&inline),
        ["icons is the path of an icon set inside the theme's folder"]
    );

    let alone = load(&naming("Alone", json!("icons/icons.json")));
    assert!(alone.refused);
    assert_eq!(
        said(&alone),
        ["icons names \"icons/icons.json\", and an icon set is read only from a theme folder"]
    );
}

#[test]
fn a_child_s_set_draws_over_its_parent_s_view_by_view() {
    let dir = tempfile::tempdir().unwrap();
    folder(
        dir.path(),
        "base",
        &naming("Base", json!("icons.json")),
        &[("icons.json", &set(&[("chats", "M1 1"), ("search", "M2 2")]))],
    );
    folder(
        dir.path(),
        "kid",
        &json!({ "name": "Kid", "extends": "base", "icons": "icons.json",
                 "appearances": [{ "id": "light", "appearance": "light" }] }),
        &[(
            "icons.json",
            &set(&[("search", "M3 3"), ("vaults", "M4 4")]),
        )],
    );
    folder(
        dir.path(),
        "plain",
        &json!({ "name": "Plain", "extends": "base" }),
        &[],
    );
    let library = Library::new()
        .with_folder(dir.path(), "base")
        .with_folder(dir.path(), "kid")
        .with_folder(dir.path(), "plain");

    let kid = library.load_id("kid");
    assert_eq!(said(&kid), Vec::<String>::new());
    // The parent's appearance and the child's new one carry the same, merged set.
    assert_eq!(kid.theme.appearances.len(), 2);
    for one in &kid.theme.appearances {
        let paths = |view: &str| one.icons.views[view].paths.clone();
        assert_eq!(paths("chats"), ["M1 1"]);
        assert_eq!(paths("search"), ["M3 3"]);
        assert_eq!(paths("vaults"), ["M4 4"]);
    }

    let plain = library.load_id("plain");
    assert_eq!(said(&plain), Vec::<String>::new());
    assert_eq!(plain.theme.first().icons.views["search"].paths, ["M2 2"]);
}

#[test]
fn a_parent_whose_icon_set_is_refused_refuses_its_child() {
    let dir = tempfile::tempdir().unwrap();
    folder(
        dir.path(),
        "base",
        &naming("Base", json!("icons.json")),
        &[("icons.json", &set(&[("chats", "javascript:1")]))],
    );
    let library = Library::new().with_folder(dir.path(), "base");
    let child = library.load(None, &json!({ "name": "Kid", "extends": "base" }));
    assert!(child.refused);
    let said = said(&child);
    assert_eq!(said[0], "Kid extends \"base\", which purlis cannot draw");
    assert!(
        said[1].starts_with("in \"base\": path 1 of chats"),
        "{said:?}"
    );
}

#[test]
fn the_window_is_handed_the_set_with_the_appearance_it_draws() {
    let loaded = dusk_with(&set(&[("inbox", "M1 1")]));
    let drawn = follow(&loaded.theme, Appearance::Dark, Contrast::Standard);
    let theme = drawn.theme.expect("an appearance is drawn");
    assert_eq!(theme.icons.views["inbox"].paths, ["M1 1"]);
}

#[test]
fn the_path_grammar_is_the_icon_theme_s() {
    for good in ["M1 1", "m0 0L1 1z", "M1.5-2e3 3,4 A1 1 0 0 1 2 2"] {
        assert!(is_path(good), "{good}");
    }
    for bad in ["", "L1 1", "M1 1 #", "M1 1;", "M1 1 x", "M1\u{a0}1"] {
        assert!(!is_path(bad), "{bad}");
    }
}

#[test]
fn the_schema_names_icons_as_a_path() {
    let icons = &schema()["properties"]["icons"];
    assert_eq!(icons["type"], "string");
    assert_eq!(icons["minLength"], 1);
    assert_eq!(icons["maxLength"], 256);
}

/// **Harness glyphs are part of the icon set** (#2922, ruling #2866 ·4): Purlis draws every one,
/// and an icon pack redraws them, held to the same checks as a view.
#[test]
fn purlis_s_icon_set_draws_every_harness_glyph_in_the_grammar() {
    let dir = tempfile::tempdir().unwrap();
    folder(
        dir.path(),
        "purlis",
        &naming("Purlis", json!("icons/icons.json")),
        &[("icons/icons.json", PURLIS_ICONS)],
    );
    let loaded = Library::new().load_folder(dir.path(), "purlis");
    assert_eq!(said(&loaded), Vec::<String>::new());
    let glyphs: Vec<&str> = loaded.theme.first().icons.glyphs.keys().copied().collect();
    let mut every: Vec<&str> = GLYPHS.to_vec();
    every.sort_unstable();
    assert_eq!(glyphs, every);
}

#[test]
fn an_icon_pack_may_draw_only_glyphs() {
    let loaded = dusk_with(&json!({ "glyphs": { "spark": { "paths": ["M1 1h2"] } } }).to_string());
    assert_eq!(said(&loaded), Vec::<String>::new());
    let icons = &loaded.theme.first().icons;
    assert!(icons.views.is_empty());
    assert_eq!(icons.glyphs["spark"].paths, ["M1 1h2"]);
}

#[test]
fn a_glyph_purlis_does_not_have_is_left_out_and_said() {
    let loaded = dusk_with(
        &json!({ "glyphs": { "spark": { "paths": ["M1 1"] }, "comet": { "paths": ["M1 1"] } } })
            .to_string(),
    );
    assert!(!loaded.refused);
    assert_eq!(
        said(&loaded),
        ["comet in icons/icons.json is not a purlis glyph, so it is left out"]
    );
    assert_eq!(loaded.theme.first().icons.glyphs.len(), 1);
}

#[test]
fn a_glyph_is_checked_like_any_icon() {
    for (icons, why) in [
        (
            json!({ "glyphs": { "braces": { "paths": ["url(#x)"] } } }),
            "path 1 of braces in icons/icons.json is not path data: a move, then path commands \
             and numbers",
        ),
        (
            json!({ "glyphs": { "braces": { "paths": ["M1 1"], "fill": "red" } } }),
            "fill is not a key of an icon (braces in icons/icons.json)",
        ),
        (
            json!({ "glyphs": [] }),
            "glyphs in icons/icons.json is an object of glyph names to icons",
        ),
        (json!({}), "icons/icons.json names no views or glyphs"),
    ] {
        let loaded = dusk_with(&icons.to_string());
        assert!(loaded.refused, "{icons}");
        assert_eq!(said(&loaded), [why], "{icons}");
    }
}

#[test]
fn a_child_s_set_draws_over_its_parent_s_glyph_by_glyph() {
    let dir = tempfile::tempdir().unwrap();
    folder(
        dir.path(),
        "base",
        &naming("Base", json!("icons.json")),
        &[(
            "icons.json",
            &json!({ "glyphs": { "spark": { "paths": ["M1 1"] }, "braces": { "paths": ["M2 2"] } } })
                .to_string(),
        )],
    );
    folder(
        dir.path(),
        "kid",
        &json!({ "name": "Kid", "extends": "base", "icons": "icons.json" }),
        &[(
            "icons.json",
            &json!({ "glyphs": { "braces": { "paths": ["M3 3"] } } }).to_string(),
        )],
    );
    let library = Library::new()
        .with_folder(dir.path(), "base")
        .with_folder(dir.path(), "kid");
    let kid = library.load_id("kid");
    assert_eq!(said(&kid), Vec::<String>::new());
    let glyphs = &kid.theme.first().icons.glyphs;
    assert_eq!(glyphs["spark"].paths, ["M1 1"]);
    assert_eq!(glyphs["braces"].paths, ["M3 3"]);
}
