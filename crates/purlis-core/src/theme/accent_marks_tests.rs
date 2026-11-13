// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What is drawn in the accent follows a given accent** (#4918): an accent given over Purlis
//! turns the primary's fill (at rest and under the pointer), the focus ring and the selected row's
//! edge to its hue, as it turns the washes (#4666): luminance and alpha kept, chroma no more than
//! the accent's. Each keeps every contrast Purlis gave it, and stays as apart from waiting and
//! danger as the rules ask; where turning would bring it nearer, it stays Purlis's. The active tab
//! is a grey ground in Purlis, not drawn in the accent, and stays Purlis's. What the parent draws
//! as its accent is the accent given, and the link and the first chart colour follow it too, each
//! kept from the state colours (#5157).

use serde_json::json;

use super::check::{PAIRS, contrast, ratio};
use super::person::Person;
use super::tint::{self, APART, oklab_of};
use super::vocabulary::DRAWN_IN_THE_ACCENT;
use super::*;

/// A person's teal, the one #3100's tests give.
const TEAL: &str = "#2bb3a3";

/// A teal dark enough for a light window's words.
const DARK_TEAL: &str = "#0b6359";

fn light(theme: &ResolvedTheme) -> bool {
    tint::luminance_of(theme.value("surface.base")).unwrap() > 0.5
}

/// A colour's OKLCH hue in degrees.
fn hue(hex: &str) -> f64 {
    let [_, a, b] = oklab_of(hex).expect("a colour");
    b.atan2(a).to_degrees().rem_euclid(360.0)
}

fn degrees_apart(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

fn apart(a: &str, b: &str) -> f64 {
    tint::distance(oklab_of(a).unwrap(), oklab_of(b).unwrap())
}

/// `theme` with a person's `accent` laid over it.
fn with_accent(theme: &ResolvedTheme, accent: &str) -> ResolvedTheme {
    let (person, said) = Person::read(Some(&json!({ "colours": { "accent.base": accent } })));
    assert!(said.is_empty(), "{said:?}");
    person.layer().over(theme).0
}

#[test]
fn a_given_teal_turns_the_fill_the_ring_and_the_edge_in_every_purlis_appearance() {
    for theme in &purlis().appearances {
        let given = if light(theme) { DARK_TEAL } else { TEAL };
        let drawn = with_accent(theme, given);
        assert_eq!(drawn.value("accent.base"), given, "{}", theme.id);
        for token in DRAWN_IN_THE_ACCENT {
            let (was, now) = (theme.value(token), drawn.value(token));
            assert_ne!(now, was, "{} {token} stayed Purlis's", theme.id);
            // What Purlis draws as its accent is the accent given (#5157).
            if was == theme.value("accent.base") {
                assert_eq!(now, given, "{} {token}", theme.id);
                continue;
            }
            assert!(
                degrees_apart(hue(now), hue(given)) < 6.0,
                "{} {token}: {now}",
                theme.id
            );
            let (l_was, l_now) = (
                tint::luminance_of(was).unwrap(),
                tint::luminance_of(now).unwrap(),
            );
            assert!(
                (l_was - l_now).abs() < 0.005,
                "{} {token}: {was} → {now}",
                theme.id
            );
        }
        // The active tab is a grey ground in Purlis: nothing of the accent to turn.
        assert_eq!(
            drawn.value("tab.active"),
            theme.value("tab.active"),
            "{}",
            theme.id
        );
    }
}

#[test]
fn what_follows_the_accent_keeps_its_contrasts_and_its_distance_from_waiting_and_danger() {
    // At every tenth degree, an accent of Purlis's own lightness and colour at that hue.
    for theme in purlis().appearances.iter().chain(built_ins()) {
        for degree in (0..360).step_by(10) {
            let given = tint::tint_hex(theme.value("accent.base"), f64::from(degree));
            let drawn = with_accent(theme, &given);
            let id = format!("{} {} at {degree} ({given})", theme.name, theme.id);
            let follows = |token: &str| {
                DRAWN_IN_THE_ACCENT.contains(&token) || tint::SIGNAL_MARKS.contains(&token)
            };
            for pair in PAIRS.iter().filter(|pair| follows(pair.front)) {
                let (Some(was), Some(is)) = (ratio(theme, pair), ratio(&drawn, pair)) else {
                    continue;
                };
                assert!(
                    is >= was.min(pair.floor),
                    "{id}: {} on {} is {is}, {was} as written",
                    pair.front,
                    pair.back
                );
            }
            for fill in ["accent.fill", "accent.fill-hover"] {
                let (was, is) = (
                    contrast(theme.value("accent.fill-text"), theme.value(fill)),
                    contrast(drawn.value("accent.fill-text"), drawn.value(fill)),
                );
                assert!(
                    is >= was.min(4.5),
                    "{id}: the primary's words on {fill} are {is}"
                );
            }
            for rule in APART.iter().filter(|rule| follows(rule.token)) {
                let (was, is) = (
                    apart(theme.value(rule.token), theme.value(rule.from)),
                    apart(drawn.value(rule.token), drawn.value(rule.from)),
                );
                assert!(
                    is >= was.min(rule.distance),
                    "{id}: {} {} is {is} from {} {}",
                    rule.token,
                    drawn.value(rule.token),
                    rule.from,
                    drawn.value(rule.from)
                );
            }
        }
    }
}

#[test]
fn an_orange_accent_s_ring_never_reads_as_waiting() {
    // An orange the colour of Purlis's waiting amber: the ring turned to it would be the waiting
    // edge; it stays Purlis's lavender.
    let dark = purlis().first();
    let drawn = with_accent(dark, dark.value("state.waiting"));
    for token in ["focus.ring", "list.selected-edge"] {
        assert!(
            apart(drawn.value(token), drawn.value("state.waiting")) >= 0.1,
            "{token} {}",
            drawn.value(token)
        );
    }
}

#[test]
fn a_ring_the_theme_gives_is_its_own() {
    let dark = purlis().first();
    let loaded = load(&json!({
        "name": "Teal over Purlis", "extends": "purlis",
        "appearances": [{ "id": dark.id, "tokens": { "accent.base": TEAL, "focus.ring": "#ffffff" } }],
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    let theme = loaded
        .theme
        .appearances
        .into_iter()
        .find(|one| one.id == dark.id)
        .unwrap();
    assert_eq!(theme.value("focus.ring"), "#ffffff");
    assert_ne!(
        theme.value("list.selected-edge"),
        dark.value("list.selected-edge")
    );
}

#[test]
fn a_fill_the_parent_draws_as_its_surface_follows_the_surface_as_turned_not_as_written() {
    // Purlis Classic's primary is filled with its surface (#3177's KIN): an accent given over it
    // alone turns the surface (#4666), and the fill is that turned surface, sharing its hue
    // (#5157), not Classic's teal beside a red surface. The fill under the pointer turns with it;
    // the ring and the edge turn.
    for classic in built_ins() {
        for accent in [
            if light(classic) { DARK_TEAL } else { TEAL },
            if light(classic) { "#5b3f9e" } else { "#9d7cd8" },
        ] {
            let loaded = load(&json!({
                "name": "An accent over Classic",
                "appearance": if light(classic) { "light" } else { "dark" },
                "tokens": { "accent.base": accent },
            }));
            assert!(!loaded.refused, "{:?}", loaded.complaints);
            let theme = loaded.theme.first();
            let id = format!("{} {accent}", classic.id);
            assert_ne!(
                theme.value("accent.surface"),
                classic.value("accent.surface"),
                "{id}"
            );
            assert_eq!(
                theme.value("accent.fill"),
                theme.value("accent.surface"),
                "{id}"
            );
            // Under the pointer the fill keeps the hue it had at rest, at the lightness the
            // parent gave it, as the wash under the pointer does.
            let hover = theme.value("accent.fill-hover");
            assert!(
                degrees_apart(hue(hover), hue(theme.value("accent.fill"))) < 12.0,
                "{id}: fill {} hover {hover}",
                theme.value("accent.fill")
            );
            assert!(
                (tint::luminance_of(hover).unwrap()
                    - tint::luminance_of(classic.value("accent.fill-hover")).unwrap())
                .abs()
                    < 0.005,
                "{id}: hover {hover}"
            );
            for edge in ["focus.ring", "list.selected-edge"] {
                assert_eq!(theme.value(edge), accent, "{id} {edge}");
            }
        }
    }
}

#[test]
fn a_given_blue_turns_the_link_and_the_first_chart_colour_and_keeps_them_from_the_states() {
    // Purlis draws its first chart colour as its accent and its link in its accent's family
    // (#5157): a blue given over it, at Purlis's own lightness, draws both blue, never within 0.1
    // of a state colour.
    for theme in &purlis().appearances {
        let given = tint::tint_hex(theme.value("accent.base"), 235.0);
        let drawn = with_accent(theme, &given);
        for mark in tint::SIGNAL_MARKS {
            let (was, now) = (theme.value(mark), drawn.value(mark));
            let id = format!("{} {mark}: {was} → {now}", theme.id);
            // As its accent, or as near as the other series let it be (#5157).
            if was == theme.value("accent.base") {
                assert!(apart(now, &given) < 0.1, "{id}");
            }
            assert!(degrees_apart(hue(now), hue(&given)) < FAMILY, "{id}");
            for state in tint::SIGNALS {
                assert!(
                    apart(now, drawn.value(state)) >= 0.1,
                    "{id} is {} from {state}",
                    apart(now, drawn.value(state))
                );
            }
        }
    }
}

#[test]
fn a_given_teal_draws_a_teal_link_and_first_chart_colour_clear_of_the_states() {
    // Purlis's link turned to teal at Purlis's lightness would read as success, so the link takes
    // the teal itself, which clears it (#5157, as a tinted link takes the accent's own, #4921).
    for theme in &purlis().appearances {
        // A teal the person's contrast keeps as the accent in each appearance, clear of success.
        let given = match theme.id.as_str() {
            "light" => "#026d6d",
            "light-high-contrast" => DARK_TEAL,
            _ => TEAL,
        };
        let drawn = with_accent(theme, given);
        assert_eq!(drawn.value("accent.base"), given, "{}", theme.id);
        for mark in tint::SIGNAL_MARKS {
            let now = drawn.value(mark);
            let id = format!("{} {mark}: {now}", theme.id);
            // The teal itself, or as near as the other series let it be (#5157).
            assert!(apart(now, given) < 0.05, "{id}");
            for state in tint::SIGNALS {
                assert!(apart(now, drawn.value(state)) >= 0.1, "{id} from {state}");
            }
        }
    }
}

/// The most a nudged mark's hue may be from the accent's, in degrees, with what showing it on the
/// screen can move it by.
const FAMILY: f64 = 34.0;

/// Whether `mark`, as `drawn` draws it for `accent`, is in the accent's family, clear of every
/// state colour and at its contrast floors: what a mark must be, unless nothing can be (#5157).
fn mark_problem(
    theme: &ResolvedTheme,
    drawn: &ResolvedTheme,
    mark: &str,
    accent: &str,
) -> Option<String> {
    let now = drawn.value(mark);
    let [_, a, b] = oklab_of(accent).unwrap();
    // A grey has no hue of its own to stay near, and an accent of the parent's own hue may draw
    // the parent's colour and be in its family all the same.
    let grey = a.hypot(b) <= 0.04;
    if grey && now == theme.value(mark) && now != accent {
        return Some(format!("{now} is the parent's"));
    }
    if !grey && degrees_apart(hue(now), hue(accent)) > FAMILY {
        return Some(format!("{now} left the family of {accent}"));
    }
    for state in tint::SIGNALS {
        let near = apart(now, drawn.value(state));
        if near < 0.1 {
            return Some(format!("{now} is {near} from {state}"));
        }
    }
    for pair in PAIRS.iter().filter(|pair| pair.front == mark) {
        let (Some(was), Some(is)) = (ratio(theme, pair), ratio(drawn, pair)) else {
            continue;
        };
        if is < was.min(pair.floor) {
            return Some(format!(
                "{now} on {} is {is}, floor {}",
                pair.back, pair.floor
            ));
        }
    }
    None
}

/// `theme` with `accent` given as a theme gives it, with no person's layer to leave it out.
fn given_accent(theme: &ResolvedTheme, accent: &str) -> ResolvedTheme {
    let mut given = std::collections::BTreeMap::new();
    given.insert("accent.base", accent.to_owned());
    super::vocabulary::follow_kin(&mut given, &theme.values);
    let mut drawn = theme.clone();
    drawn.values.extend(given);
    drawn
}

#[test]
fn a_mark_too_near_a_state_stays_in_the_accent_s_family_nudged_just_clear() {
    // The person's ruling on #5157: a link or a first chart colour does not fall back to the
    // parent's lavender. Each case took it, before.
    let cases = [
        ("dark", "#2dd4bf"),
        ("dark-high-contrast", "#2dd4bf"),
        ("light", DARK_TEAL),
        ("dark", "#0d9488"),
    ];
    for (id, accent) in cases {
        let theme = purlis()
            .appearances
            .iter()
            .find(|one| one.id == id)
            .unwrap();
        let drawn = given_accent(theme, accent);
        for mark in tint::SIGNAL_MARKS {
            let problem = mark_problem(theme, &drawn, mark, accent);
            assert!(problem.is_none(), "{id} {accent} {mark}: {problem:?}");
            // Just far enough: nearer the accent than a hop past the rule would be.
            assert!(
                apart(drawn.value(mark), accent) < 0.2,
                "{id} {accent} {mark}: {} is far from it",
                drawn.value(mark)
            );
        }
    }
    // A person's accent the layer keeps takes the same road.
    for (id, accent) in &cases[..3] {
        let theme = purlis()
            .appearances
            .iter()
            .find(|one| one.id == *id)
            .unwrap();
        let drawn = with_accent(theme, accent);
        assert_eq!(drawn.value("accent.base"), *accent, "{id}");
        for mark in tint::SIGNAL_MARKS {
            let problem = mark_problem(theme, &drawn, mark, accent);
            assert!(problem.is_none(), "{id} {accent} {mark}: {problem:?}");
        }
    }
}

#[test]
fn an_accent_the_colour_of_success_draws_its_marks_clear_of_success() {
    for theme in &purlis().appearances {
        let accent = theme.value("state.success");
        let drawn = given_accent(theme, accent);
        for mark in tint::SIGNAL_MARKS {
            let problem = mark_problem(theme, &drawn, mark, accent);
            assert!(problem.is_none(), "{} {mark}: {problem:?}", theme.id);
        }
    }
}

/// Every accent a theme could give, and every one a person's layer keeps, in one appearance of
/// Purlis: nothing falls back, since a nudge can always meet the 0.1 and the floor.
fn every_accent_keeps_its_marks_in_the_family(id: &str) {
    let theme = purlis()
        .appearances
        .iter()
        .find(|one| one.id == id)
        .expect("Purlis has the appearance");
    let steps = [0x10, 0x40, 0x70, 0xa0, 0xd0, 0xf0];
    let mut wrong = Vec::new();
    for r in steps {
        for g in steps {
            for b in steps {
                let accent = format!("#{r:02x}{g:02x}{b:02x}");
                let by_theme = given_accent(theme, &accent);
                let by_person = with_accent(theme, &accent);
                let mut drawings = vec![by_theme];
                if by_person.value("accent.base") == accent {
                    drawings.push(by_person);
                }
                for drawn in &drawings {
                    for mark in tint::SIGNAL_MARKS {
                        if let Some(problem) = mark_problem(theme, drawn, mark, &accent) {
                            wrong.push(format!("{} {accent} {mark}: {problem}", theme.id));
                        }
                    }
                }
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{} wrong: {:#?}",
        wrong.len(),
        &wrong[..wrong.len().min(40)]
    );
}

/// The grid of accents in each of Purlis's appearances, one test an appearance so they run side
/// by side (#3597); a test below holds that these are all of them.
mod across_a_grid_of_accents_every_purlis_appearance_keeps_its_marks_in_the_family {
    use super::every_accent_keeps_its_marks_in_the_family as keeps;

    /// The appearances the tests below name.
    const NAMED: [&str; 4] = ["dark", "light", "dark-high-contrast", "light-high-contrast"];

    #[test]
    fn dark() {
        keeps("dark");
    }

    #[test]
    fn light() {
        keeps("light");
    }

    #[test]
    fn dark_high_contrast() {
        keeps("dark-high-contrast");
    }

    #[test]
    fn light_high_contrast() {
        keeps("light-high-contrast");
    }

    #[test]
    fn every_appearance_of_purlis_is_one_of_them() {
        let ids: Vec<&str> = super::purlis()
            .appearances
            .iter()
            .map(|one| one.id.as_str())
            .collect();
        assert_eq!(ids, NAMED);
    }
}

#[test]
fn an_accent_a_person_s_layer_leaves_out_takes_everything_that_followed_it_out_too() {
    // The fill follows the surface, which follows the accent: leaving the accent out reverts the
    // whole chain, not one hop of it (#5157).
    let classic_dark = built_ins().iter().find(|one| !light(one)).unwrap();
    let (person, _) = Person::read(Some(&json!({ "colours": { "accent.base": "#0000ff" } })));
    let (drawn, said) = person.layer().over(classic_dark);
    assert!(
        said.iter().any(|one| one.said.contains("accent.base")),
        "{said:?}"
    );
    assert_eq!(
        drawn.value("accent.fill"),
        classic_dark.value("accent.fill")
    );

    let steps = [0x00, 0x40, 0x80, 0xc0, 0xff];
    for theme in purlis().appearances.iter().chain(built_ins()) {
        for r in steps {
            for g in steps {
                for b in steps {
                    let accent = format!("#{r:02x}{g:02x}{b:02x}");
                    let drawn = with_accent(theme, &accent);
                    if drawn.value("accent.base") != theme.value("accent.base") {
                        continue;
                    }
                    for token in DRAWN_IN_THE_ACCENT
                        .into_iter()
                        .chain(tint::SIGNAL_MARKS)
                        .chain(super::vocabulary::ACCENT_GROUNDS)
                        .chain([super::vocabulary::ACCENT_WORDS, "accent.fill-edge"])
                    {
                        assert_eq!(
                            drawn.value(token),
                            theme.value(token),
                            "{} {} {accent} left out, {token} stayed",
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
fn a_ring_and_an_edge_the_parent_draws_alike_stay_alike_under_a_plain_accent() {
    // The edge has a pair the ring has not (the palette's aimed row), so the two used to part over
    // #007fff on Classic light. Whatever colour they take, they take it together.
    let steps = [0x00, 0x40, 0x7f, 0xc0, 0xff];
    for theme in purlis().appearances.iter().chain(built_ins()) {
        assert_eq!(theme.value("focus.ring"), theme.value("list.selected-edge"));
        for r in steps {
            for g in steps {
                for b in steps {
                    let accent = format!("#{r:02x}{g:02x}{b:02x}");
                    let drawn = given_accent(theme, &accent);
                    assert_eq!(
                        drawn.value("list.selected-edge"),
                        drawn.value("focus.ring"),
                        "{} {} {accent}",
                        theme.name,
                        theme.id
                    );
                }
            }
        }
    }
}

#[test]
fn the_primary_s_fill_under_the_pointer_keeps_the_fill_s_hue() {
    // #abc over Classic dark drew fill #202c3b and, under the pointer, teal #1f3530.
    let classic = built_ins().iter().find(|one| !light(one)).unwrap();
    let drawn = given_accent(classic, "#aabbcc");
    let (fill, hover) = (drawn.value("accent.fill"), drawn.value("accent.fill-hover"));
    assert!(
        degrees_apart(hue(fill), hue(hover)) < 12.0,
        "fill {fill} hover {hover}"
    );
}

/// The series a first chart colour stands beside.
const SERIES: [&str; 7] = [
    "chart.2", "chart.3", "chart.4", "chart.5", "chart.6", "chart.7", "chart.8",
];

#[test]
fn the_first_chart_colour_stays_distinct_from_the_other_series() {
    // Each of these drew chart.1 on, or nearly on, chart.2.
    let purlis_dark = purlis()
        .appearances
        .iter()
        .find(|one| one.id == "dark")
        .unwrap();
    let classic_light = built_ins().iter().find(|one| light(one)).unwrap();
    for (theme, accent) in [(purlis_dark, "#92c4f2"), (classic_light, "#007fbf")] {
        let drawn = given_accent(theme, accent);
        for series in SERIES {
            let was = apart(theme.value("chart.1"), theme.value(series));
            let is = apart(drawn.value("chart.1"), drawn.value(series));
            assert!(
                is >= was.min(super::vocabulary::SERIES_APART),
                "{accent}: chart.1 {} is {is} from {series}, {was} as written",
                drawn.value("chart.1")
            );
        }
    }
    let steps = [0x10, 0x50, 0x90, 0xd0];
    for theme in purlis().appearances.iter().chain(built_ins()) {
        for r in steps {
            for g in steps {
                for b in steps {
                    let accent = format!("#{r:02x}{g:02x}{b:02x}");
                    let drawn = given_accent(theme, &accent);
                    for series in SERIES {
                        let was = apart(theme.value("chart.1"), theme.value(series));
                        let is = apart(drawn.value("chart.1"), drawn.value(series));
                        assert!(
                            is >= was.min(super::vocabulary::SERIES_APART),
                            "{} {} {accent}: {is} from {series}",
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
fn a_theme_s_lavender_over_classic_light_leaves_its_ring_turned_and_the_theme_drawn() {
    // A theme's own accent has no person layer to leave a colour out: the ring drawn as this
    // lavender would be under its 3:1 on a light surface, a hard floor, and refuse the whole
    // theme. It turns to the lavender's hue at Classic's lightness instead (#5157).
    let classic = built_ins()
        .iter()
        .find(|one| light(one))
        .expect("Classic light");
    let lavender = "#b4a7f5";
    let loaded = load(&json!({
        "name": "Lavender over Classic", "appearance": "light",
        "tokens": { "accent.base": lavender },
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    let theme = loaded.theme.first();
    for edge in ["focus.ring", "list.selected-edge"] {
        let now = theme.value(edge);
        assert_ne!(now, lavender, "{edge}");
        assert_ne!(now, classic.value(edge), "{edge}");
        assert!(
            degrees_apart(hue(now), hue(lavender)) < 6.0,
            "{edge}: {now}"
        );
    }
}

#[test]
fn what_follows_any_accent_keeps_every_contrast_it_had_or_its_floor() {
    let steps = [0x10, 0x50, 0x90, 0xd0];
    for theme in purlis().appearances.iter().chain(built_ins()) {
        for r in steps {
            for g in steps {
                for b in steps {
                    let accent = format!("#{r:02x}{g:02x}{b:02x}");
                    let mut given = std::collections::BTreeMap::new();
                    given.insert("accent.base", accent.clone());
                    super::vocabulary::follow_kin(&mut given, &theme.values);
                    let mut drawn = theme.clone();
                    drawn.values.extend(given);
                    let follows = |token: &str| {
                        DRAWN_IN_THE_ACCENT.contains(&token) || tint::SIGNAL_MARKS.contains(&token)
                    };
                    for pair in PAIRS.iter().filter(|pair| pair.tokens().any(follows)) {
                        let (Some(was), Some(is)) = (ratio(theme, pair), ratio(&drawn, pair))
                        else {
                            continue;
                        };
                        assert!(
                            is >= was.min(pair.floor),
                            "{} {} under {accent}: {} on {} is {is}, {was} as written",
                            theme.name,
                            theme.id,
                            pair.front,
                            pair.back
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn a_link_that_follows_the_accent_stays_readable_on_the_launcher_s_deep_surface() {
    // The launcher's links stand on `surface.deep` (#5347). Over Classic light this violet reads
    // on every other ground a link is drawn on and falls under the text floor on that one alone
    // (4.44:1, as #5263's story found it), so the link keeps a colour that reads there, given as a
    // theme gives it or as a person does.
    let theme = classic()
        .of(Appearance::Light)
        .expect("Classic's light appearance");
    let violet = "#7c48d9";
    for ground in [
        "surface.base",
        "surface.raised",
        "surface.overlay",
        "surface.sunken",
    ] {
        assert!(contrast(violet, theme.value(ground)) >= 4.5, "{ground}");
    }
    assert!(contrast(violet, theme.value("surface.deep")) < 4.5);
    for (how, drawn) in [
        ("given", given_accent(theme, violet)),
        ("a person's", with_accent(theme, violet)),
    ] {
        let link = drawn.value("markdown.link");
        let on = contrast(link, drawn.value("surface.deep"));
        assert!(on >= 4.5, "{how}: {link} on surface.deep is {on}");
    }
}
