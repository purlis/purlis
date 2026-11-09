// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The accent's grounds follow the accent** (#4666): a chosen item says its words in
//! `accent.text` on `accent.wash` or `accent.surface`, so where the words go the ground goes. An
//! accent given over a parent turns the parent's grounds to its hue, and a workspace's colour
//! turns the washes with the words, but never nearer the waiting or danger wash, as drawn,
//! than the theme's own.

use serde_json::json;

use super::check::{contrast, onto};
use super::person::Person;
use super::tint::{self, oklab_of};
use super::vocabulary::{ACCENT_GROUNDS, ACCENT_WASHES, ACCENT_WORDS, DRAWN_IN_THE_ACCENT, TINTED};
use super::*;

/// A person's teal, the one #3100's tests give.
const TEAL: &str = "#2bb3a3";

/// A teal dark enough for a light window's words.
const DARK_TEAL: &str = "#0b6359";

/// The teal a person would give `theme`: one its window can read.
fn teal_for(theme: &ResolvedTheme) -> &'static str {
    let light = tint::luminance_of(theme.value("surface.base")).unwrap() > 0.5;
    if light { DARK_TEAL } else { TEAL }
}

/// Every appearance purlis ships: Purlis's three and Classic's two.
fn shipped() -> Vec<&'static ResolvedTheme> {
    purlis().appearances.iter().chain(built_ins()).collect()
}

/// A colour's OKLCH hue in degrees and its chroma.
fn hue_chroma(hex: &str) -> (f64, f64) {
    let [_, a, b] = oklab_of(hex).expect("a colour");
    (b.atan2(a).to_degrees().rem_euclid(360.0), a.hypot(b))
}

/// How far two hues are, the short way round.
fn degrees_apart(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// The alpha a value is written with, `ff` for an opaque one.
fn alpha(hex: &str) -> &str {
    if hex.len() == 9 { &hex[7..] } else { "ff" }
}

/// `accent.text` on each ground it is drawn on, as the check measures it.
fn words_on_grounds(theme: &ResolvedTheme) -> Vec<(String, f64)> {
    let words = theme.value(ACCENT_WORDS);
    let mut out = vec![(
        "accent.surface".to_owned(),
        contrast(words, theme.value("accent.surface")),
    )];
    for wash in ACCENT_WASHES {
        for under in ["surface.base", "surface.deep"] {
            let ground = onto(theme.value(wash), theme.value(under));
            out.push((format!("{wash} over {under}"), contrast(words, &ground)));
        }
    }
    out
}

fn over_purlis(id: &str, tokens: serde_json::Value) -> ResolvedTheme {
    let loaded = load(&json!({
        "name": "Teal over Purlis", "extends": "purlis",
        "appearances": [{ "id": id, "tokens": tokens }],
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    loaded
        .theme
        .appearances
        .into_iter()
        .find(|one| one.id == id)
        .expect("the appearance")
}

// ---------------------------------------------------------------------------------------
// An accent given over a parent
// ---------------------------------------------------------------------------------------

#[test]
fn an_accent_given_over_purlis_turns_its_washes_and_surface_to_its_hue() {
    let (teal, _) = hue_chroma(TEAL);
    let dark = purlis().first();
    let theme = over_purlis(&dark.id, json!({ "accent.base": TEAL }));
    assert_eq!(theme.value(ACCENT_WORDS), TEAL);
    // Purlis's wash under the pointer is its most vivid lavender, farther from the waiting and
    // danger washes than any turned one: it stays Purlis's (NO_NEARER).
    assert_eq!(
        theme.value("accent.wash-hover"),
        dark.value("accent.wash-hover")
    );
    for ground in ["accent.surface", "accent.wash"] {
        let (was, now) = (dark.value(ground), theme.value(ground));
        let (hue, _) = hue_chroma(now);
        assert!(degrees_apart(hue, teal) < 6.0, "{ground}: {now} at {hue}");
        // The ground keeps how light it was and how much it lets through.
        assert_eq!(alpha(now), alpha(was), "{ground}");
        let (l_was, l_now) = (
            tint::luminance_of(was).unwrap(),
            tint::luminance_of(now).unwrap(),
        );
        assert!((l_was - l_now).abs() < 0.005, "{ground}: {was} → {now}");
    }
}

#[test]
fn a_ground_the_theme_gives_is_its_own() {
    let dark = purlis().first();
    let theme = over_purlis(
        &dark.id,
        json!({ "accent.base": TEAL, "accent.wash": "#11223344" }),
    );
    assert_eq!(theme.value("accent.wash"), "#11223344");
    // The others still follow.
    assert_ne!(theme.value("accent.surface"), dark.value("accent.surface"));
}

#[test]
fn the_accent_as_its_parent_draws_it_leaves_the_grounds_as_they_are() {
    let dark = purlis().first();
    let theme = over_purlis(
        &dark.id,
        json!({ "accent.base": dark.value("accent.base") }),
    );
    for ground in ACCENT_GROUNDS {
        assert_eq!(theme.value(ground), dark.value(ground), "{ground}");
    }
}

#[test]
fn a_grey_accent_draws_grey_grounds() {
    let dark = purlis().first();
    let theme = over_purlis(&dark.id, json!({ "accent.base": "#9a9a9a" }));
    // A grey ground is nearer the waiting and danger washes than Purlis's lavender, so where it
    // would be, Purlis's stays; where it is not, it is grey.
    for ground in ACCENT_GROUNDS {
        let now = theme.value(ground);
        if now == dark.value(ground) {
            continue;
        }
        let (_, chroma) = hue_chroma(now);
        assert!(chroma < 0.005, "{ground}: {now}");
    }
    let light = purlis()
        .appearances
        .iter()
        .find(|one| one.appearance == Appearance::Light)
        .expect("Purlis light");
    let theme = over_purlis(&light.id, json!({ "accent.base": "#5a5a5a" }));
    let turned = ACCENT_GROUNDS
        .iter()
        .filter(|ground| theme.value(ground) != light.value(ground))
        .count();
    assert!(turned > 0, "a grey turns some ground somewhere");
}

#[test]
fn a_person_s_accent_turns_the_grounds_under_its_words_in_every_shipped_appearance() {
    for theme in shipped() {
        let given = teal_for(theme);
        let (person, said) = Person::read(Some(&json!({ "colours": { "accent.base": given } })));
        assert!(said.is_empty(), "{said:?}");
        let (teal, _) = hue_chroma(given);
        let (drawn, said) = person.layer().over(theme);
        let id = format!("{} {}", theme.name, theme.id);
        assert!(said.is_empty(), "{id}: {said:?}");
        assert_eq!(drawn.value(ACCENT_WORDS), given, "{id}");
        // Each ground turns to the teal, or stays the theme's where turning would bring it nearer
        // the waiting or danger wash; the chosen item's surface always turns.
        for ground in ACCENT_GROUNDS {
            if ground != "accent.surface" && drawn.value(ground) == theme.value(ground) {
                continue;
            }
            let (hue, _) = hue_chroma(drawn.value(ground));
            assert!(degrees_apart(hue, teal) < 6.0, "{id} {ground}");
        }
    }
}

#[test]
fn a_person_s_accent_left_out_takes_its_grounds_with_it() {
    // An accent as dark as Purlis's ground: its words would vanish, so it is left out.
    let (person, _) = Person::read(Some(&json!({ "colours": { "accent.base": "#1a1a1f" } })));
    let dark = purlis().first();
    let (drawn, said) = person.layer().over(dark);
    assert!(!said.is_empty());
    assert!(
        said.iter().all(|one| !one.said.contains("accent.wash")),
        "{said:?}"
    );
    for ground in ACCENT_GROUNDS {
        assert_eq!(drawn.value(ground), dark.value(ground), "{ground}");
    }
}

// ---------------------------------------------------------------------------------------
// A workspace's colour
// ---------------------------------------------------------------------------------------

#[test]
fn a_workspace_s_colour_turns_the_washes_by_default() {
    for wash in ACCENT_WASHES {
        assert!(TINTED.contains(&wash), "{wash}");
    }
}

#[test]
fn the_washes_turn_only_where_the_words_do_and_at_their_chroma() {
    // The words turn by their own rule (#3100); a wash turns with them, at their chroma, or stays
    // as written where turning would bring it nearer the waiting or danger wash (#4666).
    for theme in shipped() {
        for degree in 0..360 {
            let most = tint::most_at(theme, f64::from(degree));
            for wash in ACCENT_WASHES {
                let (wash_at, words_at) = (
                    tint::chroma_of(wash, most),
                    tint::chroma_of(ACCENT_WORDS, most),
                );
                assert!(
                    wash_at.is_none() || wash_at == words_at,
                    "{} {} at {degree}: {wash} at {wash_at:?}, words at {words_at:?}",
                    theme.name,
                    theme.id
                );
            }
        }
    }
}

#[test]
fn the_words_hold_four_and_a_half_to_one_on_their_grounds_at_every_hue() {
    for theme in shipped() {
        for degree in 0..360 {
            let tinted = tint::tinted_at(theme, f64::from(degree));
            for (ground, ratio) in words_on_grounds(&tinted) {
                assert!(
                    ratio >= 4.5,
                    "{} {} at {degree}: accent.text on {ground} is {ratio}",
                    theme.name,
                    theme.id
                );
            }
        }
    }
}

/// The waiting and danger washes, which a chosen item's ground must not be taken for.
const SIGNAL_WASHES: [&str; 2] = ["wash.warning", "danger.wash"];

/// The grounds a wash is laid on.
const UNDER: [&str; 2] = ["surface.base", "surface.deep"];

/// The closest `ground`, drawn as `value` in `theme`, comes to the waiting or danger wash, each
/// laid on each of [`UNDER`] as the screen draws them: what a person sees side by side.
fn nearest_drawn(theme: &ResolvedTheme, value: &str) -> (f64, String) {
    let mut nearest = (f64::INFINITY, String::new());
    for under in UNDER {
        let under_drawn = theme.value(under);
        for from in SIGNAL_WASHES {
            let apart = oklab_distance(
                &onto(value, under_drawn),
                &onto(theme.value(from), under_drawn),
            );
            if apart < nearest.0 {
                nearest = (apart, format!("{from} over {under}"));
            }
        }
    }
    nearest
}

#[test]
fn a_turned_ground_comes_no_nearer_the_drawn_waiting_and_danger_washes_than_it_was() {
    // Measured as drawn, alpha and all, on the grounds each is laid on: an alpha-free measure
    // cannot see two see-through washes meet (#4666's review, F1).
    for theme in shipped() {
        for degree in 0..360 {
            let tinted = tint::tinted_at(theme, f64::from(degree));
            for ground in ACCENT_GROUNDS {
                let (now, was) = (tinted.value(ground), theme.value(ground));
                if now == was {
                    continue;
                }
                let (turned, near) = nearest_drawn(&tinted, now);
                let (unturned, _) = nearest_drawn(&tinted, was);
                assert!(
                    turned >= unturned,
                    "{} {} at {degree}: {ground} {was} → {now} is {turned} from {near}, \
                     {unturned} as written",
                    theme.name,
                    theme.id
                );
            }
        }
    }
}

/// Purlis Classic's dark appearance.
fn classic_dark() -> &'static ResolvedTheme {
    built_ins()
        .iter()
        .find(|one| one.appearance == Appearance::Dark && !one.high_contrast)
        .expect("Classic's dark")
}

#[test]
fn a_yellow_workspace_keeps_classic_dark_s_wash_off_the_waiting_wash() {
    // The review's case: in yellow, Classic dark's wash turned to #312b18 beside the waiting
    // wash's #2f2918, 0.008 apart, where as written it keeps 0.043.
    let theme = classic_dark();
    let tinted = tint::tinted(theme, "yellow");
    for wash in ACCENT_WASHES {
        let (turned, near) = nearest_drawn(&tinted, tinted.value(wash));
        let (written, _) = nearest_drawn(&tinted, theme.value(wash));
        assert!(
            turned >= written,
            "{wash} {} is {turned} from {near}, {written} as written",
            tinted.value(wash)
        );
    }
    // The words keep their own rule.
    assert_eq!(
        tinted.value(ACCENT_WORDS) == theme.value(ACCENT_WORDS),
        tint::chroma_of(ACCENT_WORDS, tint::most_at(theme, 95.0)).is_none()
    );
}

#[test]
fn a_red_workspace_keeps_classic_dark_s_wash_off_the_danger_wash() {
    let theme = classic_dark();
    let tinted = tint::tinted(theme, "red");
    for ground in ACCENT_GROUNDS {
        let (turned, near) = nearest_drawn(&tinted, tinted.value(ground));
        let (written, _) = nearest_drawn(&tinted, theme.value(ground));
        assert!(
            turned >= written,
            "{ground} {} is {turned} from {near}, {written} as written",
            tinted.value(ground)
        );
    }
}

#[test]
fn a_theme_whose_wash_meets_the_waiting_wash_under_a_tint_keeps_the_wash_as_written() {
    // The wash drawn as the waiting wash turned half way round: apart as written, the waiting
    // wash itself under a workspace of the waiting wash's hue.
    let dark = purlis().first();
    let warning = dark.value("wash.warning");
    let (amber, _) = hue_chroma(warning);
    let wash = tint::tint_hex(warning, amber + 180.0);
    let theme = over_purlis(
        &dark.id,
        json!({ "accent.wash": wash, "accent.wash-hover": wash }),
    );
    // Turned to the waiting wash's hue, it would be drawn nearer it than as written.
    assert!(
        nearest_drawn(&theme, &tint::tint_hex(&wash, amber)).0 < nearest_drawn(&theme, &wash).0
    );
    let tinted = tint::tinted_at(&theme, amber);
    assert_eq!(tinted.value("accent.wash"), theme.value("accent.wash"));
    assert_eq!(
        tinted.value("accent.wash-hover"),
        theme.value("accent.wash-hover")
    );
}

// ---------------------------------------------------------------------------------------
// The grounds that follow a given accent
// ---------------------------------------------------------------------------------------

#[test]
fn a_person_s_accent_never_draws_its_wash_as_the_danger_wash() {
    // The review's case (F2): #c08880 clears the danger floor over Purlis dark, but its wash,
    // turned from Purlis's, drew #31292c, the danger wash to the pixel.
    let dark = purlis().first();
    let (person, said) = Person::read(Some(&json!({ "colours": { "accent.base": "#c08880" } })));
    assert!(said.is_empty(), "{said:?}");
    let (drawn, said) = person.layer().over(dark);
    assert!(said.is_empty(), "{said:?}");
    assert_eq!(drawn.value("accent.base"), "#c08880");
    for under in UNDER {
        let under = drawn.value(under);
        for wash in ACCENT_WASHES {
            assert_ne!(
                onto(drawn.value(wash), under),
                onto(drawn.value("danger.wash"), under),
                "{wash} over {under}"
            );
        }
    }
    for ground in ACCENT_GROUNDS {
        let (now, _) = nearest_drawn(&drawn, drawn.value(ground));
        let (was, _) = nearest_drawn(&drawn, dark.value(ground));
        assert!(
            now >= was,
            "{ground} {}: {now} < {was}",
            drawn.value(ground)
        );
    }
}

#[test]
fn a_followed_ground_comes_no_nearer_the_drawn_signal_washes_than_its_parent_s() {
    // Every accent on a coarse grid, over every shipped appearance: the grounds follow_kin
    // turns toward it come no nearer the waiting and danger washes, as drawn, than the parent's.
    // Its first half turns them, and its second writes none of them
    // (`follow_kin_s_grounds_are_its_first_half_s`), so the grid asks only the first.
    let steps = [0x10, 0x30, 0x50, 0x70, 0x90, 0xb0, 0xd0, 0xf0];
    for theme in shipped() {
        for r in steps {
            for g in steps {
                for b in steps {
                    let accent = format!("#{r:02x}{g:02x}{b:02x}");
                    let mut given = std::collections::BTreeMap::new();
                    given.insert("accent.base", accent.clone());
                    super::vocabulary::follow_grounds(&mut given, &theme.values);
                    for ground in ACCENT_GROUNDS {
                        let Some(now) = given.get(ground) else {
                            continue;
                        };
                        let (turned, near) = nearest_drawn(theme, now);
                        let (was, _) = nearest_drawn(theme, theme.value(ground));
                        assert!(
                            turned >= was,
                            "{} {} under {accent}: {ground} {now} is {turned} from {near}, \
                             {was} as the parent draws it",
                            theme.name,
                            theme.id
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn a_recipe_that_turns_the_accent_turns_its_washes_and_leaving_the_accent_leaves_them() {
    let loaded = load(&json!({
        "name": "Accent only", "extends": "purlis",
        "tint": { "tokens": ["accent.base"] },
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    let tabs = loaded.theme.first().tint.tabs();
    for token in [
        "accent.base",
        ACCENT_WORDS,
        "accent.wash",
        "accent.wash-hover",
    ] {
        assert!(tabs.contains(&token), "{token}: {tabs:?}");
    }
    let mut recipe = tint::Tint::default();
    let left = recipe.leave(&["accent.base"]);
    for token in [
        "accent.base",
        ACCENT_WORDS,
        "accent.wash",
        "accent.wash-hover",
    ] {
        assert!(left.contains(&token), "{token}: {left:?}");
        assert!(!recipe.tabs().contains(&token), "{token}");
    }
}

fn oklab_distance(a: &str, b: &str) -> f64 {
    let (a, b) = (oklab_of(a).unwrap(), oklab_of(b).unwrap());
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn follow_kin_s_grounds_are_its_first_half_s() {
    // What is drawn in the accent, the second half, is never one of its grounds...
    for token in DRAWN_IN_THE_ACCENT.into_iter().chain(tint::SIGNAL_MARKS) {
        assert!(!ACCENT_GROUNDS.contains(&token), "{token}");
    }
    // ...so the whole gives each ground what the first half gives it, here over every shipped
    // appearance, for an accent of each kind: grey, a state's hue, the parent's family, and far.
    for theme in shipped() {
        for accent in [
            "#808080", "#d03030", "#30a050", "#7060e0", "#2bb3a3", "#f0f0f0",
        ] {
            let given = std::collections::BTreeMap::from([("accent.base", accent.to_owned())]);
            let (mut whole, mut half) = (given.clone(), given);
            super::vocabulary::follow_kin(&mut whole, &theme.values);
            super::vocabulary::follow_grounds(&mut half, &theme.values);
            for ground in ACCENT_GROUNDS {
                assert_eq!(
                    whole.get(ground),
                    half.get(ground),
                    "{} {accent} {ground}",
                    theme.id
                );
            }
        }
    }
}
