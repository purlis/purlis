// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The theme engine, at its interface: a document, a pick or a theme in; a resolved theme,
//! complaints, a drawn answer or a report out.

use std::sync::Arc;

use serde_json::{Value, json};

use super::vocabulary::{DURATIONS, EASINGS, LONGEST, TINTED, TOKENS, is_colour};
use super::*;
use crate::extension::project::theme::Pick;

fn dark() -> &'static ResolvedTheme {
    classic().first()
}

fn light() -> &'static ResolvedTheme {
    classic().of(Appearance::Light).unwrap()
}

/// A theme file as one arrives from disk: parsed JSON and nothing more.
fn file(over: Value) -> Value {
    let mut base = json!({ "name": "probe", "appearance": "dark", "tokens": {} });
    for (key, value) in over.as_object().expect("an object").clone() {
        base[key] = value;
    }
    base
}

fn said(resolved: &Resolved) -> Vec<String> {
    resolved
        .complaints
        .iter()
        .map(|one| one.said.clone())
        .collect()
}

// ---------------------------------------------------------------------------------------
// The built-ins
// ---------------------------------------------------------------------------------------

#[test]
fn the_registry_s_ids_name_purlis_s_dark_and_light_and_the_window_comes_up_in_the_dark_one() {
    // `purlis-dark` and `purlis-light` name the default's dark and light (#3034).
    let named: Vec<(&str, &str)> = crate::extension::BUILT_IN_THEMES
        .iter()
        .map(|id| {
            let one = built_in(id).expect("a built-in");
            (one.name.as_str(), one.id.as_str())
        })
        .collect();
    assert_eq!(named, [("Purlis", "dark"), ("Purlis", "light")]);
    assert!(built_ins().iter().all(|it| it.name == "Purlis Classic"));
    // The window comes up in the default, Purlis's dark (#3034).
    assert_eq!(default_theme(), purlis().first());
    assert_eq!(dark().appearance, Appearance::Dark);
    assert_eq!(light().appearance, Appearance::Light);
}

#[test]
fn every_built_in_gives_every_token_and_every_motion_value() {
    for theme in built_ins() {
        for token in TOKENS {
            assert!(
                is_colour(theme.value(token)) || vocabulary::is_platform(token, theme.value(token)),
                "{} has no {token}",
                theme.name
            );
        }
        assert_eq!(theme.values.len(), TOKENS.len());
        assert_eq!(theme.motion.durations.len(), DURATIONS.len());
        assert_eq!(theme.motion.easings.len(), EASINGS.len());
    }
}

#[test]
fn every_built_in_appearance_holds_an_id_a_tag_colours_and_motion_and_no_geometry() {
    let file: Value = serde_json::from_str(CLASSIC_FILE).unwrap();
    let mut keys: Vec<&String> = file.as_object().unwrap().keys().collect();
    keys.sort();
    // Its parts are the looks its tabs (#2914) and its activity items (#2917) keep: the family
    // defaults draw Purlis.
    assert_eq!(keys, ["$schema", "appearances", "name", "parts"]);
    for one in file["appearances"].as_array().unwrap() {
        let mut keys: Vec<&String> = one.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["appearance", "id", "motion", "tokens"]);
        let tokens = one["tokens"].as_object().unwrap();
        assert!(tokens.keys().all(|key| !vocabulary::names_geometry(key)));
        assert_eq!(tokens.len(), TOKENS.len() + typed::all().count());
    }
}

#[test]
fn the_two_built_ins_differ_everywhere_a_theme_is_allowed_to() {
    let same: Vec<&str> = TOKENS
        .iter()
        .copied()
        .filter(|token| dark().value(token) == light().value(token))
        .collect();
    // And what both leave to the platform, clear in each (#2327).
    assert_eq!(
        same,
        [
            "needs-you.text",
            "terminal.selection-foreground",
            "selection.background",
            "selection.text",
            "caret",
            "placeholder",
            "scrollbar.thumb",
            "scrollbar.track",
        ]
    );
}

#[test]
fn every_built_in_washes_in_its_own_danger_colour_and_holds_lower_case_hex() {
    for theme in built_ins() {
        let wash = theme.value("danger.wash");
        assert_eq!(&wash[..7], theme.value("danger.base"));
        assert_eq!(wash.len(), 9);
        for value in theme.values.values() {
            assert!(
                value.bytes().skip(1).all(|b| !b.is_ascii_uppercase()),
                "{}: {value}",
                theme.name
            );
        }
    }
}

#[test]
fn an_old_id_is_read_as_the_renamed_built_in_keeping_its_appearance() {
    for (old, now) in [
        ("charter-dark", "purlis-dark"),
        ("charter-light", "purlis-light"),
    ] {
        assert_eq!(built_in(old), built_in(now), "{old}");
    }
    for id in ["system", "solarized/Solarized Dark", "", "toString"] {
        assert!(built_in(id).is_none(), "{id}");
    }
}

// ---------------------------------------------------------------------------------------
// What a value may be
// ---------------------------------------------------------------------------------------

#[test]
fn a_colour_is_hex_and_only_hex() {
    for value in ["#abc", "#abcd", "#aabbcc", "#aabbccdd", "#ABC", "#AABBCC"] {
        assert!(is_colour(value), "{value}");
    }
    for value in [
        "rgb(1 2 3)",
        "red",
        "var(--x)",
        "#ab",
        "#abcde",
        "#abcdefg",
        "#aabbcc;}",
        "#aabbcc; } body { display: none",
        " #aabbcc",
        "#aabbcc ",
        "",
    ] {
        assert!(!is_colour(value), "{value}");
    }
}

#[test]
fn a_value_that_is_not_a_string_is_never_a_colour() {
    for value in [json!(null), json!(16_711_680), json!({}), json!([])] {
        let resolved = load(&file(json!({ "tokens": { "surface.base": value } })));
        assert!(resolved.refused);
        assert_eq!(&resolved.theme, purlis());
        assert_eq!(resolved.complaints.len(), 1);
    }
}

// ---------------------------------------------------------------------------------------
// Loading a theme that is wrong never leaves the window without one
// ---------------------------------------------------------------------------------------

#[test]
fn a_document_that_is_not_an_object_is_the_default_with_one_complaint() {
    for raw in [
        json!(null),
        json!(3),
        json!("purlis-dark"),
        json!(["purlis-dark"]),
    ] {
        let resolved = load(&raw);
        assert_eq!(&resolved.theme, purlis());
        assert!(resolved.refused);
        assert_eq!(said(&resolved), ["a theme is a JSON object"]);
    }
}

#[test]
fn a_value_outside_its_type_refuses_the_whole_theme_and_names_the_value() {
    let resolved = load(&file(json!({
        "tokens": { "surface.base": "#123456", "text.primary": "rgb(1 2 3)" }
    })));
    // Purlis is drawn whole: not the file's one good colour beside Purlis's.
    assert!(resolved.refused);
    assert_eq!(&resolved.theme, purlis());
    assert_eq!(resolved.complaints.len(), 1);
    assert_eq!(
        resolved.complaints[0].token.as_deref(),
        Some("text.primary")
    );
    assert_eq!(
        resolved.complaints[0].said,
        "text.primary is \"rgb(1 2 3)\", which is not #rgb, #rgba, #rrggbb or #rrggbbaa"
    );
}

#[test]
fn a_token_the_file_did_not_mention_is_the_built_in_s_silently() {
    let resolved = load(&file(json!({ "tokens": { "surface.base": "#123456" } })));
    assert!(resolved.complaints.is_empty());
    assert_eq!(
        resolved.theme.first().value("state.failed"),
        dark().value("state.failed")
    );
}

#[test]
fn a_token_purlis_does_not_have_is_named_and_left_out_and_the_rest_is_drawn() {
    // A name reaches nothing, and a teammate's purlis may have a token this one does not.
    let resolved = load(&file(json!({ "tokens": {
        "gray.800": "#123456", "accent.base": "#654321"
    } })));
    assert!(!resolved.refused);
    assert_eq!(resolved.theme.first().value("accent.base"), "#654321");
    assert!(!resolved.theme.first().values.contains_key("gray.800"));
    assert_eq!(
        resolved.complaints,
        [Complaint {
            said: "gray.800 is not a purlis token, so it is left out".to_owned(),
            token: Some("gray.800".to_owned()),
            awaits_approval: None,
            at: None,
        }]
    );
}

#[test]
fn a_key_format_1_does_not_have_is_said_and_left_out() {
    // Format 1 is frozen, and people's files and extensions' themes carry keys of their own.
    let resolved = load(&json!({
        "name": "Signed", "appearance": "dark", "author": "Ana", "description": "warm",
        "$comment": "made by hand", "tokens": { "accent.base": "#123456" }
    }));
    assert!(!resolved.refused);
    assert_eq!(resolved.theme.first().value("accent.base"), "#123456");
    assert_eq!(
        said(&resolved),
        [
            "author is not a key of a theme file, so it is left out",
            "description is not a key of a theme file, so it is left out",
            "$comment is not a key of a theme file, so it is left out",
        ]
    );
    // Geometry still refuses it, by name.
    let sized = load(&json!({ "name": "Sized", "appearance": "dark", "radius": { "md": 9 } }));
    assert!(sized.refused);
}

#[test]
fn an_extension_theme_past_the_one_cap_is_refused_unread() {
    let most = usize::try_from(crate::windowprefs::MAX_BYTES).unwrap();
    let text = format!(
        "{}{}",
        json!({ "name": "Big", "appearance": "dark" }),
        " ".repeat(most)
    );
    let offered = Offered::of("acme".into(), "Big".into(), &text);
    assert!(offered.theme.refused);
    assert_eq!(
        offered.theme.complaints[0].said,
        format!(
            "Big is {} bytes, and a theme is never larger than {most}",
            text.len()
        )
    );
    let small = Offered::of(
        "acme".into(),
        "Small".into(),
        &json!({ "name": "Small", "appearance": "dark" }).to_string(),
    );
    assert!(!small.theme.refused);
}

#[test]
fn geometry_in_format_1_is_refused_by_key_and_never_read() {
    let resolved = load(&file(json!({
        "tokens": { "space.4": "#123456", "radius.md": "#123456" },
        "geometry": { "space.4": 99 },
    })));
    let said = said(&resolved);
    assert!(said.contains(
        &"space.4 is geometry, which a theme sets in a theme folder's theme.json (format 2), not \
          here"
            .to_owned()
    ));
    assert!(
        said.iter()
            .any(|one| one.starts_with("radius.md is geometry"))
    );
    assert!(
        said.iter()
            .any(|one| one.starts_with("geometry is geometry"))
    );
    assert!(!resolved.theme.first().values.contains_key("space.4"));
}

#[test]
fn a_theme_that_tries_every_spelling_of_geometry_is_refused_and_told_each_is_core() {
    // The property itself, the token's dotted form, and whole groups.
    let mut tokens = serde_json::Map::new();
    for name in vocabulary::GEOMETRY {
        tokens.insert((*name).to_owned(), json!("#ffffff"));
        tokens.insert(name[2..].replacen('-', ".", 1), json!("#ffffff"));
    }
    let keys: Vec<String> = tokens.keys().cloned().collect();
    tokens.insert("accent.base".to_owned(), json!("#123456"));
    let resolved = load(&json!({
        "name": "tries", "appearance": "dark", "tokens": tokens,
        "geometry": { "space.4": "100px" }, "radius": { "md": "20px" },
        "sizes": { "control": "60px" }, "motion": { "duration.quick": 100 },
    }));
    assert!(resolved.refused);
    assert_eq!(&resolved.theme, purlis());
    let said = said(&resolved);
    for key in keys
        .iter()
        .map(String::as_str)
        .chain(["geometry", "radius", "sizes"])
    {
        assert!(
            said.iter()
                .any(|line| line.starts_with(&format!("{key} is geometry"))),
            "nothing said about {key}"
        );
    }
    assert!(
        TOKENS
            .iter()
            .all(|token| !vocabulary::names_geometry(token))
    );
}

#[test]
fn a_light_theme_is_filled_from_the_light_built_in() {
    let resolved = load(&file(json!({ "appearance": "light" })));
    assert_eq!(
        resolved.theme.first().value("text.primary"),
        light().value("text.primary")
    );
    assert_eq!(resolved.theme.first().appearance, Appearance::Light);
    assert_eq!(resolved.theme.first().motion, light().motion);
}

#[test]
fn an_unknown_appearance_refuses_the_theme_and_is_said() {
    let resolved = load(&file(json!({ "appearance": "solarized" })));
    assert!(resolved.refused);
    assert_eq!(
        said(&resolved),
        ["appearance is \"solarized\"; a theme's appearance is \"dark\" or \"light\""]
    );
}

#[test]
fn a_nameless_theme_is_refused() {
    for name in [json!(null), json!(""), json!("   "), json!(7)] {
        let mut raw = file(json!({}));
        raw["name"] = name;
        let resolved = load(&raw);
        assert!(resolved.refused);
        assert_eq!(said(&resolved), ["a theme has a name"]);
    }
    let resolved = load(&file(json!({ "name": "  Mine  " })));
    assert_eq!(resolved.theme.first().name, "Mine");
}

#[test]
fn tokens_that_are_not_an_object_refuse_the_theme_and_one_without_tokens_is_classic_s() {
    let resolved = load(&file(json!({ "tokens": "purlis-dark" })));
    assert!(resolved.refused);
    assert_eq!(
        said(&resolved),
        ["tokens is an object of token names to colours"]
    );
    let bare = load(&json!({ "name": "Bare", "appearance": "light" }));
    assert!(!bare.refused && bare.complaints.is_empty());
    assert_eq!(bare.theme.first().values, light().values);
}

// ---------------------------------------------------------------------------------------
// Motion
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_that_says_nothing_about_motion_moves_as_its_built_in_silently() {
    let resolved = load(&json!({ "name": "quiet", "appearance": "light", "tokens": {} }));
    assert_eq!(resolved.theme.first().motion, light().motion);
    assert!(resolved.complaints.is_empty());
}

#[test]
fn motion_is_taken_where_given_and_filled_token_by_token() {
    let resolved = load(&json!({
        "name": "brisk", "appearance": "dark", "tokens": {},
        "motion": { "duration.enter": 90, "easing.enter": [0, 0, 0.1, 1] },
    }));
    assert!(resolved.complaints.is_empty());
    let motion = &resolved.theme.first().motion;
    assert_eq!(motion.durations["duration.enter"], 90);
    assert_eq!(motion.easings["easing.enter"], [0.0, 0.0, 0.1, 1.0]);
    assert_eq!(
        motion.durations["duration.spin"],
        dark().motion.durations["duration.spin"]
    );
}

#[test]
fn a_duration_is_a_whole_number_of_milliseconds_up_to_the_ceiling() {
    for good in [json!(0), json!(1), json!(160), json!(LONGEST), json!(120.0)] {
        let resolved = load(&json!({ "name": "x", "appearance": "dark", "tokens": {},
            "motion": { "duration.quick": good } }));
        assert!(resolved.complaints.is_empty(), "{good}");
    }
    for bad in [
        json!("150ms"),
        json!(-1),
        json!(12.5),
        json!(LONGEST + 1),
        json!("0ms; } x"),
    ] {
        let resolved = load(&json!({ "name": "x", "appearance": "dark", "tokens": {},
            "motion": { "duration.quick": bad } }));
        assert_eq!(resolved.complaints.len(), 1, "{bad}");
        // Refused whole: the default, Purlis, is drawn in its place, its motion with it.
        assert!(resolved.refused, "{bad}");
        assert_eq!(resolved.theme.first().motion, default_theme().motion);
    }
}

#[test]
fn an_easing_is_four_bounded_numbers() {
    for good in [
        json!([0, 0, 1, 1]),
        json!([0.2, 0, 0, 1]),
        json!([0.34, 1.56, 0.64, 1]),
    ] {
        let resolved = load(&json!({ "name": "x", "appearance": "dark", "tokens": {},
            "motion": { "easing.enter": good } }));
        assert!(resolved.complaints.is_empty(), "{good}");
    }
    for bad in [
        json!([-0.1, 0, 1, 1]),
        json!([0, 0, 1.2, 1]),
        json!([0, 5, 1, 1]),
        json!([0, 0, 1]),
        json!(["0", "0", "1", "1"]),
        json!("ease-out"),
    ] {
        let resolved = load(&json!({ "name": "x", "appearance": "dark", "tokens": {},
            "motion": { "easing.enter": bad } }));
        assert_eq!(resolved.complaints.len(), 1, "{bad}");
    }
}

#[test]
fn a_bad_motion_value_refuses_the_theme_and_names_the_value() {
    let resolved = load(&json!({
        "name": "typo", "appearance": "dark", "tokens": {},
        "motion": { "duration.enter": "160ms", "easing.enter": [2, 0, 0, 1] },
    }));
    assert!(resolved.refused);
    assert_eq!(
        said(&resolved),
        [
            "duration.enter is \"160ms\", which is not a whole number of milliseconds from 0 to \
             10000",
            "easing.enter is [2,0,0,1], which is not [x1, y1, x2, y2] with each x from 0 to 1 \
             and each y from -1 to 2",
        ]
    );
}

#[test]
fn motion_that_is_not_an_object_or_names_no_token_is_said() {
    let resolved = load(&json!({ "name": "x", "appearance": "dark", "motion": [1, 2] }));
    // Refused whole: the default, Purlis, is drawn in its place, its motion with it.
    assert!(resolved.refused);
    assert_eq!(resolved.theme.first().motion, default_theme().motion);
    assert!(
        said(&resolved).contains(&"motion is an object of motion token names to values".to_owned())
    );
    let resolved = load(&json!({ "name": "x", "appearance": "dark",
        "motion": { "duration.200": 200 } }));
    assert!(!resolved.refused);
    assert_eq!(
        said(&resolved),
        ["duration.200 is not a purlis motion token, so it is left out"]
    );
}

#[test]
fn a_theme_may_turn_motion_off() {
    let off: serde_json::Map<String, Value> = DURATIONS
        .iter()
        .map(|token| ((*token).to_owned(), json!(0)))
        .collect();
    let resolved =
        load(&json!({ "name": "still", "appearance": "dark", "tokens": {}, "motion": off }));
    assert!(resolved.complaints.is_empty());
    assert!(
        resolved
            .theme
            .first()
            .motion
            .durations
            .values()
            .all(|ms| *ms == 0)
    );
}

#[test]
fn what_the_window_is_handed_is_written_from_parsed_values_never_from_the_file_s_text() {
    let resolved = load(&json!({
        "name": "hostile", "appearance": "dark",
        "tokens": { "surface.base": "#000; } body { display: none" },
        "motion": {
            "duration.enter": "1ms; } body { display: none",
            "easing.enter": ["0); } body { display: none", 0, 1, 1],
        },
    }));
    let handed = serde_json::to_string(&resolved.theme).unwrap();
    assert!(resolved.refused);
    assert!(!handed.contains("display"), "{handed}");
}

#[test]
fn text_that_is_not_json_is_refused_like_any_theme_that_is_not_one() {
    for text in ["{ not json", "[]", "\"purlis-dark\""] {
        let resolved = load_text(text);
        assert!(resolved.refused, "{text}");
        assert_eq!(&resolved.theme, purlis());
        assert_eq!(resolved.complaints.len(), 1);
    }
    assert!(
        load_text("{ not json").complaints[0]
            .said
            .starts_with("a theme is JSON: ")
    );
}

// ---------------------------------------------------------------------------------------
// The person's own theme file
// ---------------------------------------------------------------------------------------

#[test]
fn the_person_s_theme_is_read_from_their_config_home_and_resolved() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(read_theirs(None), Theirs::default());
    let none = read_theirs(Some(home.path()));
    assert!(!none.found && none.theme.is_none() && none.trouble.is_none());
    assert!(none.path.ends_with("theme.json"), "{}", none.path);

    let dir = crate::machine::dir(home.path());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("theme.json"),
        r##"{ "name": "Mine", "appearance": "light", "tokens": { "accent.base": "#abcdef" } }"##,
    )
    .unwrap();
    let mine = read_theirs(Some(home.path()));
    assert!(mine.found && mine.trouble.is_none());
    let theme = mine.theme.expect("in force");
    assert_eq!(theme.name, "Mine");
    let theme = theme.first();
    assert_eq!(theme.value("accent.base"), "#abcdef");
    assert_eq!(theme.value("surface.base"), light().value("surface.base"));
    assert!(mine.complaints.is_empty());

    // A value outside its type refuses it: not in force, and the trouble names the value.
    std::fs::write(
        dir.join("theme.json"),
        r##"{ "name": "Mine", "appearance": "light", "tokens": { "accent.base": "nope" } }"##,
    )
    .unwrap();
    let bad = read_theirs(Some(home.path()));
    assert!(bad.found && bad.theme.is_none());
    assert!(bad.trouble.unwrap().contains("accent.base is \"nope\""));

    std::fs::write(dir.join("theme.json"), "[1]").unwrap();
    let refused = read_theirs(Some(home.path()));
    assert!(refused.theme.is_none());
    assert!(
        refused
            .trouble
            .unwrap()
            .contains("a theme is a JSON object")
    );
}

// ---------------------------------------------------------------------------------------
// Which theme the window draws
// ---------------------------------------------------------------------------------------

fn offered() -> Arc<[Offered]> {
    let mine = |name: &str, primary: &str| {
        json!({ "name": name, "appearance": "dark", "tokens": { "text.primary": primary } })
            .to_string()
    };
    Arc::from(vec![
        Offered::of("acme".into(), "Night".into(), &mine("Night", "#eeeeee")),
        Offered::of("acme".into(), "Broken".into(), "{ not json"),
        Offered::of("zed".into(), "Zed".into(), &mine("Zed", "#dddddd")),
    ])
}

fn drawn(pick: Option<&str>, theirs: Option<&Theme>, on: On<'_>) -> Drawn {
    let pick = pick.map(|value| Pick::parse(value).expect("a pick"));
    resolve(
        &Inputs {
            pick: pick.as_ref(),
            theirs,
            on,
            system: Appearance::Dark,
            contrast: Contrast::Standard,
        },
        offered,
    )
}

fn name_of(drawn: &Drawn) -> Option<&str> {
    drawn.theme.as_ref().map(|it| it.name.as_str())
}

#[test]
fn a_built_in_pick_draws_that_built_in_by_any_id_it_has_had() {
    // `purlis-dark` and `purlis-light`, by any id they have had, draw Purlis's (#3034);
    // `purlis-classic` draws Classic.
    for (pick, name, id) in [
        ("purlis-light", "Purlis", "light"),
        ("charter-light", "Purlis", "light"),
        ("purlis-dark", "Purlis", "dark"),
        ("purlis-classic", "Purlis Classic", "dark"),
    ] {
        let got = drawn(Some(pick), Some(classic()), On::Every).theme.unwrap();
        assert_eq!((got.name.as_str(), got.id.as_str()), (name, id), "{pick}");
    }
}

#[test]
fn following_the_system_draws_the_default_s_appearance_of_its_tag_and_says_it_follows() {
    let pick = Pick::System;
    for (system, name) in [(Appearance::Light, "Purlis"), (Appearance::Dark, "Purlis")] {
        let it = resolve(
            &Inputs {
                pick: Some(&pick),
                theirs: None,
                on: On::Every,
                system,
                contrast: Contrast::Standard,
            },
            || unreachable!("no survey for a built-in"),
        );
        assert_eq!(name_of(&it), Some(name));
        assert!(it.follows_system);
    }
}

#[test]
fn with_no_pick_the_person_s_own_theme_comes_before_any_extension_s() {
    let mine = load(&file(json!({ "name": "Mine" }))).theme; // a whole theme
    let it = resolve(
        &Inputs {
            pick: None,
            theirs: Some(&mine),
            on: On::Every,
            system: Appearance::Dark,
            contrast: Contrast::Standard,
        },
        || unreachable!("no survey when the person has a theme"),
    );
    assert_eq!(name_of(&it), Some("Mine"));
    assert!(!it.follows_system);
}

#[test]
fn with_neither_the_first_theme_of_an_extension_that_is_on_is_drawn() {
    assert_eq!(name_of(&drawn(None, None, On::Every)), Some("Night"));
    let zed = ["zed".to_owned()];
    assert_eq!(name_of(&drawn(None, None, On::Only(&zed))), Some("Zed"));
    assert_eq!(name_of(&drawn(None, None, On::Only(&[]))), Some("Purlis"));
}

#[test]
fn an_extension_pick_draws_its_theme_only_while_the_extension_is_on() {
    let acme = ["acme".to_owned()];
    let it = drawn(Some("acme/Night"), Some(classic()), On::Only(&acme));
    assert_eq!(name_of(&it), Some("Night"));
    assert_eq!(it.theme.unwrap().value("text.primary"), "#eeeeee");
    assert_eq!(
        name_of(&drawn(Some("acme/Night"), None, On::Only(&[]))),
        Some("Purlis")
    );
    assert_eq!(
        name_of(&drawn(Some("acme/Gone"), None, On::Every)),
        Some("Purlis")
    );
}

#[test]
fn an_extension_theme_that_is_not_json_is_passed_over_unless_it_was_picked() {
    let acme = ["acme".to_owned()];
    let only_broken =
        || -> Arc<[Offered]> { Arc::from(vec![Offered::of("acme".into(), "Broken".into(), "{")]) };
    let unpicked = resolve(
        &Inputs {
            pick: None,
            theirs: None,
            on: On::Only(&acme),
            system: Appearance::Dark,
            contrast: Contrast::Standard,
        },
        only_broken,
    );
    // Nothing else in force: Purlis, with nothing to say about a theme nobody picked.
    assert_eq!(unpicked.theme.as_ref(), Some(default_theme()));
    assert!(unpicked.complaints.is_empty());
    // A refused theme does not stand in front of the next one in force.
    let then_good = || -> Arc<[Offered]> {
        Arc::from(vec![
            Offered::of("acme".into(), "Broken".into(), "{"),
            Offered::of(
                "acme".into(),
                "Good".into(),
                &json!({ "name": "Good", "appearance": "dark" }).to_string(),
            ),
        ])
    };
    let skipped = resolve(
        &Inputs {
            pick: None,
            theirs: None,
            on: On::Only(&acme),
            system: Appearance::Dark,
            contrast: Contrast::Standard,
        },
        then_good,
    );
    assert_eq!(name_of(&skipped), Some("Good"));
    // Picked, it draws Purlis and says why.
    let picked = drawn(Some("acme/Broken"), None, On::Every);
    assert_eq!(name_of(&picked), Some("Purlis"));
    assert_eq!(picked.complaints.len(), 1);
}

#[test]
fn an_extension_theme_s_complaints_come_with_it() {
    let text = json!({ "name": "Loud", "appearance": "dark", "tokens": { "x.y": "#000" } });
    let it = resolve(
        &Inputs {
            pick: None,
            theirs: None,
            on: On::Every,
            system: Appearance::Dark,
            contrast: Contrast::Standard,
        },
        || {
            Arc::from(vec![Offered::of(
                "acme".into(),
                "Loud".into(),
                &text.to_string(),
            )])
        },
    );
    assert_eq!(it.complaints.len(), 1);
}

// ---------------------------------------------------------------------------------------
// The check
// ---------------------------------------------------------------------------------------

#[test]
fn every_built_in_can_be_read_as_it_ships_and_in_every_tint() {
    for theme in built_ins() {
        let report = check(theme);
        assert!(report.passes(), "{}: {:#?}", theme.name, report.findings);
    }
}

#[test]
fn the_check_measures_the_way_wcag_does_and_holds_words_to_aa() {
    // `#777777` on white is WCAG's own worked example: 4.48:1, just under AA's 4.5 for body text.
    let probe = load(&file(json!({ "tokens": {
        "surface.base": "#ffffff", "text.muted": "#777777",
    } })))
    .theme
    .first()
    .clone();
    let report = check(&probe);
    let muted = report
        .findings
        .iter()
        .find_map(|one| match one {
            Finding::Contrast {
                tint: None,
                pair,
                ratio,
            } if pair.front == "text.muted" && pair.back == "surface.base" => {
                Some((*ratio, pair.floor))
            }
            _ => None,
        })
        .expect("muted text on the window is named");
    assert_eq!(muted, (4.48, 4.5));
    // Words at the window's 14px are held to AA's body floor, a mark to 3:1.
    let floor = |front: &str, back: &str| {
        PAIRS
            .iter()
            .find(|pair| pair.front == front && pair.back == back)
            .map(|pair| pair.floor)
    };
    assert_eq!(floor("danger.base", "control.base"), Some(4.5));
    assert_eq!(floor("state.running", "surface.base"), Some(3.0));
    // Black on white is the most there is: nothing the check could find.
    let stark = load(&file(json!({ "tokens": {
        "surface.base": "#ffffff", "text.primary": "#000000",
    } })))
    .theme
    .first()
    .clone();
    assert!(!check(&stark).findings.iter().any(|one| matches!(
        one,
        Finding::Contrast { tint: None, pair, .. }
            if pair.front == "text.primary" && pair.back == "surface.base"
    )));
}

#[test]
fn the_person_s_theme_is_read_once_per_process_until_it_is_forgotten() {
    let home = tempfile::tempdir().unwrap();
    let dir = crate::machine::dir(home.path());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("theme.json"),
        r#"{ "name": "First", "appearance": "dark" }"#,
    )
    .unwrap();
    let kept = TheirsKept::new();
    let first = kept.get(Some(home.path()));
    assert_eq!(
        first.theme.as_ref().map(|it| it.name.as_str()),
        Some("First")
    );

    // A second ask, after the file changed, answers the read the first one took.
    std::fs::write(
        dir.join("theme.json"),
        r#"{ "name": "Second", "appearance": "dark" }"#,
    )
    .unwrap();
    assert_eq!(kept.get(Some(home.path())), first);

    kept.forget();
    let again = kept.get(Some(home.path()));
    assert_eq!(again.theme.map(|it| it.name).as_deref(), Some("Second"));
}

#[test]
fn the_check_names_a_pair_that_cannot_be_read_with_its_ratio() {
    let grey = load(&file(json!({ "tokens": { "text.muted": "#2a2a2a" } })))
        .theme
        .first()
        .clone();
    let report = check(&grey);
    let untinted: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|one| matches!(one, Finding::Contrast { tint: None, .. }))
        .collect();
    assert!(untinted.iter().any(|one| matches!(
        one,
        Finding::Contrast { pair, ratio, .. }
            if pair.front == "text.muted" && pair.back == "surface.base" && *ratio < 4.5
    )));
}

#[test]
fn the_check_holds_the_ansi_colours_off_the_terminal_and_the_selected_row_apart() {
    let flat = load(&file(json!({ "tokens": {
        "terminal.ansi.blue": dark().value("terminal.background"),
        "list.selected": dark().value("surface.hover").to_uppercase(),
    } })))
    .theme
    .first()
    .clone();
    let report = check(&flat);
    assert!(report.findings.contains(&Finding::Ansi {
        tint: None,
        token: "terminal.ansi.blue",
        ratio: 1.0,
        floor: 3.0,
    }));
    assert!(report.findings.contains(&Finding::SelectedNotApart {
        tint: None,
        other: "surface.hover",
    }));
}

#[test]
fn the_check_measures_a_wash_over_what_it_is_laid_on() {
    // White text on an almost clear white wash over the dark window reads; the same wash fully
    // opaque does not.
    let opaque = load(&file(json!({ "tokens": {
        "text.primary": "#ffffff", "danger.wash": "#ffffffff"
    } })))
    .theme
    .first()
    .clone();
    let washed = PAIRS.iter().find(|pair| pair.under.is_some()).unwrap();
    assert!(check(&opaque).findings.iter().any(|one| matches!(
        one,
        Finding::Contrast { pair, tint: None, .. } if pair == washed
    )));
    let clear = load(&file(json!({ "tokens": {
        "text.primary": "#ffffff", "danger.wash": "#ffffff00"
    } })))
    .theme
    .first()
    .clone();
    assert!(!check(&clear).findings.iter().any(|one| matches!(
        one,
        Finding::Contrast { pair, .. } if pair == washed
    )));
}

/// The colours a branch's change marks are drawn in (FM-4): added, deleted, changed, renamed.
const CHANGE_MARKS: [&str; 4] = [
    "state.success",
    "danger.base",
    "state.waiting",
    "accent.base",
];

/// The rows an explorer's change mark is drawn on: the window, the row under the pointer and the
/// selected row.
const CHANGE_MARK_GROUNDS: [&str; 3] = ["surface.base", "surface.hover", "list.selected"];

#[test]
fn the_check_holds_a_change_mark_on_every_row_it_is_drawn_on() {
    // A mark drawn in its row's own colour is gone: the check names it, at a mark's floor or
    // higher, as the theme is and in every tint.
    for mark in CHANGE_MARKS {
        for ground in CHANGE_MARK_GROUNDS {
            let mut tokens = serde_json::Map::new();
            tokens.insert(mark.to_owned(), json!(dark().value(ground)));
            let gone = load(&file(json!({ "tokens": tokens }))).theme;
            let report = check(gone.first());
            assert!(
                report.findings.iter().any(|one| matches!(
                    one,
                    Finding::Contrast { tint: None, pair, ratio }
                        if pair.front == mark && pair.back == ground
                            && pair.floor >= 3.0 && *ratio < 3.0
                )),
                "{mark} on {ground} is not held"
            );
        }
    }
    // The built-ins clear every one of them (the tints too: see above).
    for theme in built_ins() {
        let report = check(theme);
        assert!(!report.findings.iter().any(|one| matches!(
            one,
            Finding::Contrast { pair, .. }
                if CHANGE_MARKS.contains(&pair.front) && CHANGE_MARK_GROUNDS.contains(&pair.back)
        )));
    }
}

#[test]
fn the_check_measures_a_see_through_colour_over_what_it_is_drawn_on() {
    // A mark in a translucent colour is drawn as the colour laid over its row, and that is what
    // is measured: today's added-line wash as a mark on the dark window is 1.85:1, not the 6.93
    // its opaque colour would have.
    let faint = load(&file(json!({ "tokens": { "state.success": "#3fb95055" } }))).theme;
    let report = check(faint.first());
    let ratio = report.findings.iter().find_map(|one| match one {
        Finding::Contrast {
            tint: None,
            pair,
            ratio,
        } if pair.front == "state.success" && pair.back == "surface.base" => Some(*ratio),
        _ => None,
    });
    assert_eq!(ratio, Some(1.85));
    let solid = load(&file(json!({ "tokens": { "state.success": "#3fb950ff" } }))).theme;
    assert!(!check(solid.first()).findings.iter().any(|one| matches!(
        one,
        Finding::Contrast { tint: None, pair, .. }
            if pair.front == "state.success" && pair.back == "surface.base"
    )));
}

#[test]
fn every_pair_names_tokens_that_exist() {
    for pair in PAIRS {
        for token in pair.tokens() {
            assert!(TOKENS.contains(&token), "{token}");
        }
    }
}

// ---------------------------------------------------------------------------------------
// A workspace's colour
// ---------------------------------------------------------------------------------------

/// The fixture the window's `tint.test.ts` reads too: the same inputs, the same answers.
const TINT_CASES: &str = include_str!("tint-cases.json");

#[test]
fn a_tint_turns_each_colour_exactly_as_the_window_does() {
    let cases: Value = serde_json::from_str(TINT_CASES).unwrap();
    for case in cases["hues"].as_array().unwrap() {
        let colour = case["colour"].as_str().unwrap();
        let hue = hue_of(colour);
        match case["hue"].as_f64() {
            None => assert!(hue.is_none(), "{colour}"),
            Some(expected) => assert!((hue.unwrap() - expected).abs() < 1e-9, "{colour}"),
        }
    }
    for case in cases["tints"].as_array().unwrap() {
        let (hex, colour) = (
            case["hex"].as_str().unwrap(),
            case["colour"].as_str().unwrap(),
        );
        assert_eq!(
            tint_hex(hex, hue_of(colour).unwrap()),
            case["tinted"].as_str().unwrap(),
            "{hex} in {colour}"
        );
    }
}

#[test]
fn a_tint_turns_only_the_accent_and_the_tab_shades() {
    let tint = tinted(dark(), "blue");
    for token in TOKENS {
        if TINTED.contains(token) {
            continue;
        }
        assert_eq!(tint.value(token), dark().value(token), "{token}");
    }
    assert_ne!(tint.value("accent.base"), dark().value("accent.base"));
    assert_eq!(&tinted(dark(), "nope"), dark());
    let names: Vec<&str> = PALETTE.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, crate::extension::project::theme::PALETTE);
}
