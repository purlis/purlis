// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Purlis, the theme built from the foundation** (#2336), at the engine's interface: what it
//! draws, that every combination of it clears the check and every consent floor, that it is the
//! default while a pick of Purlis Classic keeps drawing Classic (#3034), and that its faces are
//! served.

use std::sync::Arc;

use serde_json::Value;

use super::gallery::{self, Shelf};
use super::levels::Reach;
use super::*;
use crate::extension::project::theme::Pick;

fn no_survey() -> Arc<[Offered]> {
    Arc::from(Vec::new())
}

/// Purlis's `theme.json` as its folder holds it.
fn document() -> Value {
    let files = purlis_files();
    serde_json::from_slice(&files[THEME_JSON]).unwrap()
}

/// What the levels draw for `levels`, with no theme folder anywhere.
fn drawn_for(levels: &Levels, contrast: Contrast) -> Drawn {
    resolve_levels(
        &LevelInputs {
            levels,
            folders: &Folders::of(None, None, None),
            theirs: None,
            on: On::Every,
            system: Appearance::Dark,
            contrast,
        },
        no_survey,
    )
}

#[test]
fn purlis_is_a_dark_a_light_and_a_high_contrast_dark_appearance() {
    let theme = purlis();
    assert_eq!(theme.name, "Purlis");
    let shape: Vec<(&str, Appearance, bool)> = theme
        .appearances
        .iter()
        .map(|one| (one.id.as_str(), one.appearance, one.high_contrast))
        .collect();
    assert_eq!(
        shape,
        [
            ("dark", Appearance::Dark, false),
            ("light", Appearance::Light, false),
            ("dark-high-contrast", Appearance::Dark, true),
        ]
    );
}

#[test]
fn every_appearance_names_every_token_itself_so_nothing_is_purlis_classic_s() {
    // A token an appearance does not name would be Purlis Classic's, silently: Purlis names each.
    let classic: Value = serde_json::from_str(CLASSIC_THEME_JSON).unwrap();
    let every: Vec<&String> = classic["appearances"][0]["tokens"]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    let document = document();
    for one in document["appearances"].as_array().unwrap() {
        let tokens = one["tokens"].as_object().unwrap();
        for token in &every {
            assert!(tokens.contains_key(*token), "{} has no {token}", one["id"]);
        }
        assert!(one["motion"].is_object(), "{} has no motion", one["id"]);
    }
}

#[test]
fn dark_draws_the_foundation_s_colours_fonts_and_icons() {
    let dark = purlis().of(Appearance::Dark).unwrap();
    for (token, value) in [
        ("window.background", "#121215"),
        ("surface.deep", "#141417"),
        ("surface.base", "#17171b"),
        ("surface.raised", "#1d1d22"),
        ("surface.overlay", "#24242a"),
        ("terminal.background", "#202026"),
        ("text.primary", "#e6e4ec"),
        ("text.secondary", "#c9c7d2"),
        ("accent.base", "#a99cf0"),
        ("focus.ring", "#a99cf0"),
        // One step under the frame's `#2b2a37`, so quiet and faint clear it (#2902).
        ("list.selected", "#2a2936"),
        ("state.waiting", "#e5c08a"),
        ("state.success", "#8fc79a"),
        ("syntax.string", "#9fcfa8"),
        ("syntax.property", "#c3baf5"),
        ("markdown.link", "#b3a8f2"),
    ] {
        assert_eq!(dark.value(token), value, "{token}");
    }
    assert_eq!(dark.fonts.ui.family.as_deref(), Some("IBM Plex Sans"));
    let faces = |font: &Font| -> Vec<(u16, Style)> {
        font.faces
            .iter()
            .map(|one| (one.weight, one.style))
            .collect()
    };
    assert_eq!(
        faces(&dark.fonts.ui),
        [
            (400, Style::Normal),
            (400, Style::Italic),
            (500, Style::Normal),
            (500, Style::Italic),
            (600, Style::Normal),
            (600, Style::Italic),
        ]
    );
    let mono = [
        (400, Style::Normal),
        (400, Style::Italic),
        (700, Style::Normal),
    ];
    assert_eq!(faces(&dark.fonts.mono), mono);
    assert_eq!(faces(&dark.fonts.terminal), mono);
    assert_eq!(dark.fonts.mono.family.as_deref(), Some("JetBrains Mono"));
    assert_eq!(
        dark.fonts.terminal.family.as_deref(),
        Some("JetBrains Mono")
    );
    assert!(!dark.icons.is_empty(), "Purlis draws Lucide's icons");
}

#[test]
fn every_combination_clears_the_check_and_holds_no_consent_floor() {
    let theme = purlis();
    let checked = check_every(theme);
    assert_eq!(checked.len(), theme.combinations());
    for one in &checked {
        assert!(
            one.report.passes(),
            "{}: {:?}",
            one.appearance,
            one.report.findings
        );
        assert_eq!(one.held, None, "{} holds a consent floor", one.appearance);
    }
    assert!(warnings_said(theme, &checked).is_empty());
    for one in &theme.appearances {
        assert_eq!(draw(one).held, None, "{}", one.id);
    }
}

#[test]
fn its_faces_are_served_by_their_digest_from_the_files_compiled_in() {
    let dark = purlis().first();
    let files = purlis_files();
    let every = dark
        .fonts
        .ui
        .faces
        .iter()
        .chain(&dark.fonts.mono.faces)
        .chain(&dark.fonts.terminal.faces);
    for face in every {
        let bytes = served(&face.asset).unwrap();
        assert!(bytes.starts_with(b"wOF2"));
        assert!(files.values().any(|one| *one == bytes));
    }
}

fn style(slot: &'static str, weight: u16, style: Style) -> StyleDrawn {
    StyleDrawn {
        slot,
        weight,
        style,
    }
}

#[test]
fn each_style_it_draws_has_a_real_face_in_every_appearance() {
    // Comments, emphasis, headings and terminal bold are drawn in faces of their own (#2709),
    // never ones the browser fakes from the upright regular.
    for one in &purlis().appearances {
        let drawn = styles_drawn(one);
        for wanted in [
            style("mono", 400, Style::Italic),
            style("mono", BOLD, Style::Normal),
            style("terminal", BOLD, Style::Normal),
            style("terminal", 400, Style::Italic),
            style("ui", 400, Style::Italic),
            style("ui", 600, Style::Normal),
            style("ui", 600, Style::Italic),
        ] {
            assert!(
                drawn.contains(&wanted),
                "{}: {wanted:?} in {drawn:?}",
                one.id
            );
        }
        assert_eq!(without_a_face(one), [], "{}", one.id);
    }
}

#[test]
fn a_style_drawn_with_no_face_of_its_own_is_named() {
    let mut thin = purlis().first().clone();
    thin.fonts
        .mono
        .faces
        .retain(|one| one.weight == 400 && one.style == Style::Normal);
    assert_eq!(
        without_a_face(&thin),
        [
            style("mono", 400, Style::Italic),
            style("mono", BOLD, Style::Normal)
        ]
    );
    // A font with no face is the platform's own, whose styles are its own: Purlis Classic's.
    assert_eq!(without_a_face(classic().first()), []);
}

#[test]
fn it_is_the_default_and_every_stored_pick_but_purlis_classic_draws_it() {
    assert_eq!(Pick::parse(PURLIS), Some(Pick::BuiltIn(PURLIS)));
    assert_eq!(DEFAULT, PURLIS);
    assert_eq!(default_theme(), purlis().first());
    // Nothing chose: Purlis's dark, and its high-contrast dark for more contrast (#3034).
    let nothing = drawn_for(&Levels::default(), Contrast::Standard);
    assert_eq!(nothing.level, Some(Level::Purlis));
    assert_eq!(nothing.theme.as_ref(), Some(purlis().first()));
    let sharp = drawn_for(&Levels::default(), Contrast::More).theme.unwrap();
    assert_eq!(
        (sharp.name.as_str(), sharp.id.as_str()),
        ("Purlis", "dark-high-contrast")
    );
    // Each stored value, read as the operator ruled (#3034): `purlis-dark`, `purlis-light`, the
    // ids they had before the rename and the old `system` were picks of the default's dark,
    // light or following the system, so they draw Purlis's; only `purlis-classic` draws Classic.
    // At every level, in the standard contrast and in more; no file is rewritten.
    let dark = ("Purlis", "dark", "dark-high-contrast");
    let light = ("Purlis", "light", "light");
    for (stored, appearance, (name, standard, more)) in [
        (CLASSIC, None, ("Purlis Classic", "dark", "dark")),
        (CLASSIC, Some("light"), ("Purlis Classic", "light", "light")),
        (CLASSIC, Some("system"), ("Purlis Classic", "dark", "dark")),
        ("purlis-dark", None, dark),
        ("charter-dark", None, dark),
        ("purlis-light", None, light),
        ("charter-light", None, light),
        ("system", None, dark),
    ] {
        for level in 0..5 {
            let choice = Choice::new(stored, appearance);
            let levels = match level {
                0 => Levels {
                    workspace_choice: choice,
                    ..Levels::default()
                },
                1 => Levels {
                    workspace_default: choice,
                    ..Levels::default()
                },
                2 => Levels {
                    project_choice: choice,
                    ..Levels::default()
                },
                3 => Levels {
                    project_default: choice,
                    ..Levels::default()
                },
                _ => Levels {
                    machine_default: choice,
                    ..Levels::default()
                },
            };
            for (contrast, id) in [(Contrast::Standard, standard), (Contrast::More, more)] {
                let got = drawn_for(&levels, contrast).theme.unwrap();
                assert_eq!(
                    (got.name.as_str(), got.id.as_str()),
                    (name, id),
                    "{stored} {appearance:?} at {level}, {contrast:?}"
                );
            }
        }
    }
    let chosen = Levels {
        project_choice: Choice::new(PURLIS, None),
        ..Levels::default()
    };
    let drawn = drawn_for(&chosen, Contrast::Standard);
    let theme = drawn.theme.unwrap();
    assert_eq!((theme.name.as_str(), theme.id.as_str()), ("Purlis", "dark"));
    // More contrast asked for draws its high-contrast dark.
    let sharp = drawn_for(&chosen, Contrast::More).theme.unwrap();
    assert_eq!(sharp.id, "dark-high-contrast");
    // An appearance chosen by its id is drawn, and the window asks the engine for it.
    let light = Levels {
        project_choice: Choice::new(PURLIS, Some("light")),
        ..Levels::default()
    };
    assert_eq!(
        drawn_for(&light, Contrast::Standard).theme.unwrap().id,
        "light"
    );
    assert_eq!(light.project_choice.unwrap().told(), PURLIS);
}

#[test]
fn no_theme_folder_takes_its_id_and_a_theme_may_extend_it() {
    assert!(is_reserved(PURLIS));
    assert!(!is_folder_id(PURLIS));
    let child = serde_json::json!({
        "name": "Mine",
        "extends": PURLIS,
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": { "accent.base": "#b3a8f2" } }]
    });
    let read = Library::new().load(None, &child);
    assert!(!read.refused, "{:?}", read.complaints);
    let dark = read.theme.of(Appearance::Dark).unwrap();
    assert_eq!(dark.value("accent.base"), "#b3a8f2");
    assert_eq!(dark.value("surface.base"), "#17171b");
    assert_eq!(dark.fonts, purlis().first().fonts);
    assert_eq!(read.theme.appearances.len(), 3);
}

#[test]
fn the_gallery_shows_it_first_as_the_default_and_purlis_classic_beside_it() {
    let folders = Folders::of(None, None, None);
    let cards = gallery::cards(&Shelf {
        folders: &folders,
        reach: Reach::Machine,
        on: On::Every,
        offered: &[],
    });
    let built_in: Vec<(&str, bool)> = cards
        .iter()
        .take(2)
        .map(|one| (one.id.as_str(), one.preview))
        .collect();
    assert_eq!(built_in, [(PURLIS, false), (CLASSIC, false)]);
    assert!(!is_preview(PURLIS) && !is_preview(CLASSIC));
    let purlis = &cards[0];
    assert_eq!(purlis.name, "Purlis");
    assert!(purlis.warnings.is_empty() && purlis.floors.is_empty());
    assert!(purlis.duplicable && !purlis.folder);
}

/// **The light appearance's rule, re-derived** (D-2336-12), so a change to dark cannot leave
/// light behind: each colour's OKLCH lightness reflected about the foundation's deepest colour,
/// hue, chroma and alpha kept (chroma given up in steps of 0.001 until it fits sRGB); the scrim,
/// the shadow and the scrollbar's track kept as they are; and a reflected colour that misses a
/// floor of the check stepped darker by 0.01 until it clears it.
mod light_rule {
    fn to_linear(c: f64) -> f64 {
        let c = c / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    fn to_srgb(c: f64) -> f64 {
        let c = if c <= 0.003_130_8 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        c * 255.0
    }

    fn rgb(hex: &str) -> [f64; 3] {
        let byte = |at: usize| f64::from(u8::from_str_radix(&hex[at..at + 2], 16).unwrap());
        [byte(1), byte(3), byte(5)]
    }

    fn hex([r, g, b]: [f64; 3]) -> String {
        let one = |v: f64| (v + 0.5).floor().clamp(0.0, 255.0) as u8;
        format!("#{:02x}{:02x}{:02x}", one(r), one(g), one(b))
    }

    fn cbrt(v: f64) -> f64 {
        v.abs().powf(1.0 / 3.0).copysign(v)
    }

    /// `hex` as OKLCH: lightness, chroma, hue in degrees.
    pub(super) fn oklch_of(hex: &str) -> (f64, f64, f64) {
        let [r, g, b] = rgb(hex).map(to_linear);
        let l = cbrt(0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b);
        let m = cbrt(0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b);
        let s = cbrt(0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b);
        let lightness = 0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s;
        let a = 1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s;
        let b = 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s;
        (
            lightness,
            a.hypot(b),
            b.atan2(a).to_degrees().rem_euclid(360.0),
        )
    }

    /// The colour of OKLCH `lightness`, `chroma` and `hue`, chroma given up until it fits sRGB.
    pub(super) fn oklch(lightness: f64, mut chroma: f64, hue: f64) -> String {
        loop {
            let (a, b) = (
                chroma * hue.to_radians().cos(),
                chroma * hue.to_radians().sin(),
            );
            let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
            let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
            let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);
            let linear = [
                4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
                -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
                -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s,
            ];
            if linear.iter().all(|v| (-1e-6..=1.0 + 1e-6).contains(v)) || chroma <= 0.0 {
                return hex(linear.map(|v| to_srgb(v.clamp(0.0, 1.0))));
            }
            chroma = (chroma - 0.001).max(0.0);
        }
    }

    fn luminance(hex: &str) -> f64 {
        let [r, g, b] = rgb(hex).map(to_linear);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    pub(super) fn contrast(a: &str, b: &str) -> f64 {
        let (x, y) = (luminance(a), luminance(b));
        let (light, dark) = if x >= y { (x, y) } else { (y, x) };
        let ratio = (light + 0.05) / (dark + 0.05);
        (ratio * 100.0).round() / 100.0
    }

    /// `value` reflected about lightness `deepest`, its alpha kept; a value that is not a colour
    /// as it is.
    pub(super) fn reflect(value: &str, deepest: f64) -> String {
        if !value.starts_with('#') {
            return value.to_owned();
        }
        let (body, alpha) = value.split_at(7);
        let (lightness, chroma, hue) = oklch_of(body);
        format!("{}{alpha}", oklch(1.0 - (lightness - deepest), chroma, hue))
    }

    /// `value` stepped darker by 0.01 of lightness until it clears `floor` on `ground`.
    pub(super) fn cleared(mut value: String, ground: &str, floor: f64) -> String {
        let (mut lightness, chroma, hue) = oklch_of(&value);
        while contrast(&value, ground) < floor {
            lightness -= 0.01;
            value = oklch(lightness, chroma, hue);
        }
        value
    }
}

#[test]
fn light_is_dark_reflected_by_its_one_rule() {
    let theme = purlis();
    let dark = theme.of(Appearance::Dark).unwrap();
    let light = theme.of(Appearance::Light).unwrap();
    // The foundation's deepest colour: what the window is drawn on, behind everything.
    let deepest = light_rule::oklch_of(dark.value("window.background")).0;
    let kept = ["overlay.scrim", "overlay.shadow", "scrollbar.track"];
    // The pairs a reflected colour missed, and the ground it is stepped away from (#2700).
    let stepped = [
        ("syntax.comment", "surface.hover", 3.0),
        ("syntax.punctuation", "surface.hover", 3.0),
        // The quiet tier, the placeholder that is drawn in it, and faint (#2902).
        ("text.quiet", "surface.hover", 4.5),
        ("placeholder", "surface.hover", 4.5),
        ("text.faint", "surface.hover", 3.0),
    ];
    let mut derived: std::collections::BTreeMap<&str, String> = dark
        .values
        .iter()
        .map(|(token, value)| {
            let value = if kept.contains(token) {
                value.clone()
            } else {
                light_rule::reflect(value, deepest)
            };
            (*token, value)
        })
        .collect();
    for (token, ground, floor) in stepped {
        let ground = derived[ground].clone();
        let value = derived[token].clone();
        derived.insert(token, light_rule::cleared(value, &ground, floor));
    }
    for (token, value) in &light.values {
        assert_eq!(value, &derived[token], "{token}");
    }
    assert_eq!(light.typed, dark.typed);
    assert_eq!(light.geometry, dark.geometry);
    assert_eq!(light.motion, dark.motion);
}

#[test]
fn the_old_system_id_follows_the_system_with_the_person_s_own_theme_else_purlis() {
    // #2615 kept, #3034: `system` is the person's own theme when they have one, else Purlis,
    // following the operating system.
    let system = Levels {
        project_default: Choice::new("system", None),
        ..Levels::default()
    };
    let drawn = |theirs: Option<&Theme>, os: Appearance, contrast| {
        resolve_levels(
            &LevelInputs {
                levels: &system,
                folders: &Folders::of(None, None, None),
                theirs,
                on: On::Every,
                system: os,
                contrast,
            },
            no_survey,
        )
    };
    let light = drawn(None, Appearance::Light, Contrast::Standard);
    assert!(light.follows_system);
    let light = light.theme.unwrap();
    assert_eq!(
        (light.name.as_str(), light.id.as_str()),
        ("Purlis", "light")
    );
    let dark = drawn(None, Appearance::Dark, Contrast::More).theme.unwrap();
    assert_eq!(dark.id, "dark-high-contrast");
    let mine = load(&serde_json::json!({
        "name": "Mine",
        "appearances": [
            { "id": "dark", "appearance": "dark" },
            { "id": "light", "appearance": "light" }
        ]
    }))
    .theme;
    let theirs = drawn(Some(&mine), Appearance::Light, Contrast::Standard);
    assert!(theirs.theirs);
    assert_eq!(theirs.theme.unwrap().name, "Mine");
}

#[test]
fn a_pick_that_cannot_be_drawn_draws_the_default_for_the_contrast_asked() {
    // An extension's theme that is not in force, and a theme folder that is refused, draw Purlis
    // as nothing chosen does: its high-contrast dark when more contrast is asked for (#3034).
    let gone = Levels {
        project_choice: Choice::new("acme/Night", None),
        ..Levels::default()
    };
    let more = drawn_for(&gone, Contrast::More);
    assert_eq!(more.complaints.len(), 1, "{:?}", more.complaints);
    assert_eq!(more.theme.as_ref(), Some(default_for(Contrast::More)));
    assert_eq!(
        drawn_for(&gone, Contrast::Standard).theme.as_ref(),
        Some(default_theme())
    );
}

/// **How Purlis draws each workspace colour** (#3034), as numbers: per appearance and palette
/// colour, the most chroma the accent and its kin are given to stay apart from danger
/// (`apart_chroma`: no limit, an amount, or `None`, drawn untinted), the accent drawn, and the
/// workspace strip's shade, which keeps the colour's full hue whatever the accent is given. A
/// change to Purlis's tokens has to say here what it does to workspace colours.
#[test]
fn purlis_draws_each_workspace_colour_as_these_numbers_and_every_strip_keeps_its_hue() {
    let want: &[(&str, &str, Option<f64>, &str, &str)] = &[
        ("dark", "red", Some(0.015), "#b1a4a2", "#251212"),
        ("dark", "orange", Some(0.02), "#b1a59b", "#22140b"),
        ("dark", "yellow", Some(f64::INFINITY), "#bea643", "#1c1706"),
        ("dark", "green", Some(f64::INFINITY), "#70b773", "#0e1a0f"),
        ("dark", "teal", Some(f64::INFINITY), "#17bab2", "#031b19"),
        ("dark", "blue", Some(f64::INFINITY), "#71aaf1", "#0e1824"),
        ("dark", "purple", Some(f64::INFINITY), "#bc97e7", "#1c1423"),
        ("dark", "pink", Some(0.015), "#afa4a9", "#23121b"),
        ("light", "red", None, "#554691", "#fef6f4"),
        ("light", "orange", None, "#554691", "#fff6f0"),
        ("light", "yellow", Some(f64::INFINITY), "#625001", "#fdf8e1"),
        ("light", "green", Some(f64::INFINITY), "#165e20", "#eafdea"),
        ("light", "teal", Some(f64::INFINITY), "#015b59", "#e0fefa"),
        ("light", "blue", Some(f64::INFINITY), "#1d5291", "#f3f8ff"),
        ("light", "purple", Some(f64::INFINITY), "#654189", "#faf6fe"),
        ("light", "pink", None, "#554691", "#fff5f9"),
        ("dark-high-contrast", "red", None, "#c3baf5", "#251212"),
        ("dark-high-contrast", "orange", None, "#c3baf5", "#22140b"),
        (
            "dark-high-contrast",
            "yellow",
            Some(f64::INFINITY),
            "#d1c183",
            "#1c1706",
        ),
        (
            "dark-high-contrast",
            "green",
            Some(f64::INFINITY),
            "#9ecd9d",
            "#0e1a0f",
        ),
        (
            "dark-high-contrast",
            "teal",
            Some(f64::INFINITY),
            "#7bd0c9",
            "#031b19",
        ),
        (
            "dark-high-contrast",
            "blue",
            Some(f64::INFINITY),
            "#9dc4f7",
            "#0e1824",
        ),
        (
            "dark-high-contrast",
            "purple",
            Some(f64::INFINITY),
            "#cfb7ef",
            "#1c1423",
        ),
        ("dark-high-contrast", "pink", None, "#c3baf5", "#23121b"),
    ];
    let mut got = Vec::new();
    for one in &purlis().appearances {
        for hue in &one.tint.hues {
            let tint = tinted(one, hue.name);
            got.push((
                one.id.as_str(),
                hue.name,
                apart_chroma(one, hue.hue),
                tint.value("accent.base").to_owned(),
                tint.value("layer.workspace").to_owned(),
            ));
            // Every colour tints the strip, whatever its accent is given.
            assert_ne!(
                tint.value("layer.workspace"),
                one.value("layer.workspace"),
                "{} {}",
                one.id,
                hue.name
            );
        }
    }
    let got: Vec<(&str, &str, Option<f64>, &str, &str)> = got
        .iter()
        .map(|(id, name, most, accent, strip)| (*id, *name, *most, accent.as_str(), strip.as_str()))
        .collect();
    assert_eq!(got, want);
}
