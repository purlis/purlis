// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a see-through fill shows of a tint** (#4257): a tint keeps a token's luminance, but a
//! see-through fill lets the turned colour under it through, so words that read as a theme is
//! written can fall in a tint. These hold every part a theme can colour that way, and say what
//! the default-coloured title-bar count does between the named tints.

use serde_json::{Value, json};

use super::super::{load, tint};
use super::*;

/// The recipe: every ground a family stands on, so the fill has something turned to show.
const RECIPE: &[&str] = &[
    "surface.base",
    "surface.sunken",
    "surface.deep",
    "surface.raised",
    "surface.overlay",
    "surface.hover",
    "control.base",
    "control.hover",
    "tab.active",
    "list.selected",
];

const WASHES: &[&str] = &["#ff000040"];
const HUES: std::ops::RangeInclusive<i32> = 0..=11;

fn themed(wash: &str, grey: u8, parts: &Value) -> Option<ResolvedTheme> {
    let loaded = load(&json!({
        "name": "See-through", "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": {
            "chart.4": wash, "chart.3": format!("#{grey:02x}{grey:02x}{grey:02x}")
        } }],
        "tint": { "tokens": RECIPE },
        "parts": parts,
    }));
    if loaded.refused {
        return None;
    }
    loaded
        .theme
        .appearances
        .into_iter()
        .find(|one| one.id == "dark")
}

/// `theme` measured as written alone, whatever its recipe.
fn untinted(theme: &ResolvedTheme) -> ResolvedTheme {
    let mut alone = theme.clone();
    alone.tint.on = false;
    alone
}

fn falls(theme: &ResolvedTheme, family: &str) -> bool {
    fallen(theme)
        .iter()
        .any(|one| one.part.starts_with(&format!("{family}.")))
}

/// The dimmest grey that reads on the wash as written, where a dimmer one falls.
fn just_reading(family: &str, wash: &str, parts: &Value) -> Option<u8> {
    let at = |grey| themed(wash, grey, parts).map(|one| falls(&untinted(&one), family));
    if at(0xff)? || !at(0)? {
        return None;
    }
    let (mut low, mut high) = (0_u8, 0xff_u8);
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if at(middle)? {
            low = middle;
        } else {
            high = middle;
        }
    }
    Some(high)
}

#[test]
fn a_part_that_only_reads_untinted_falls_in_every_family() {
    let mut caught = Vec::new();
    let mut missed = Vec::new();
    for family in MANIFEST {
        let root = family.parts[0];
        for part in family.parts {
            if !part.properties.contains(&Prop::Text) {
                continue;
            }
            let fill_at = if part.properties.contains(&Prop::Fill) {
                part.name
            } else if root.properties.contains(&Prop::Fill) {
                root.name
            } else {
                continue;
            };
            let parts = json!({
                format!("{}.{fill_at}.fill", family.name): "chart.4",
                format!("{}.{}.text", family.name, part.name): "chart.3",
            });
            for wash in WASHES {
                let Some(grey) = just_reading(family.name, wash, &parts) else {
                    continue;
                };
                let theme = themed(wash, grey, &parts).unwrap();
                let breaks = HUES.clone().any(|step| {
                    falls(
                        &untinted(&tint::tinted_at(&theme, f64::from(step * 30))),
                        family.name,
                    )
                });
                if breaks {
                    let name = format!("{}.{}", family.name, part.name);
                    if falls(&theme, family.name) {
                        caught.push(name);
                    } else {
                        missed.push(name);
                    }
                }
            }
        }
    }
    assert!(missed.is_empty(), "not held in a tint: {missed:?}");
    // The sweep found cases to hold: an empty one proves nothing.
    assert!(caught.len() > 20, "{caught:?}");
}

/// The lowest contrast of the count's default colours, over its ground: the pair the token check
/// holds (`check.rs`).
fn default_count(theme: &ResolvedTheme) -> f64 {
    lowest(
        theme,
        &laid(theme, &["surface.deep"]),
        "wash.warning",
        "state.waiting",
    )
    .unwrap()
}

#[test]
fn a_default_count_reads_at_every_degree_of_a_recipe_that_moves_its_ground() {
    for base in ["purlis", "purlis-classic"] {
        let loaded = load(&json!({
            "name": "Moved", "extends": base, "tint": { "tokens": ["surface.deep"] }
        }));
        for appearance in &loaded.theme.appearances {
            for degree in 0..360 {
                let tinted = tint::tinted_at(appearance, f64::from(degree));
                let ratio = default_count(&tinted);
                assert!(
                    ratio >= 4.5,
                    "{base} {} at {degree}: {ratio}",
                    appearance.id
                );
            }
        }
    }
}

#[test]
fn a_shipped_theme_and_an_opaque_count_build_no_tints_for_this() {
    // What pays for the tints is a look the theme changed that draws a see-through colour: every
    // family asked of the rules as held, in every state, as the hold asks.
    let shows = |theme: &ResolvedTheme| {
        let rules = held(theme).0;
        MANIFEST
            .iter()
            .any(|family| shows_through(theme, &rules, family))
    };
    for base in ["purlis", "purlis-classic"] {
        let loaded = load(&json!({ "name": "Plain", "extends": base }));
        for appearance in &loaded.theme.appearances {
            assert!(!shows(appearance), "{base} {}", appearance.id);
        }
    }
    let count = |fill: &str| {
        themed(
            fill,
            0xff,
            &json!({ "titlebar.count.fill": "chart.4", "titlebar.count.text": "chart.3" }),
        )
        .unwrap()
    };
    assert!(!shows(&count("#202020")));
    let see_through = count("#ff000040");
    assert!(shows(&see_through), "{:?}", fallen(&see_through));
}

/// A theme with these tokens and keys, on a recipe that turns the grounds.
fn built(id: &str, tokens: Value, parts: Value) -> Option<ResolvedTheme> {
    let loaded = load(&json!({
        "name": "See-through", "extends": "purlis-classic",
        "appearances": [{ "id": id, "tokens": tokens }],
        "tint": { "tokens": RECIPE },
        "parts": parts,
    }));
    if loaded.refused {
        return None;
    }
    loaded
        .theme
        .appearances
        .into_iter()
        .find(|one| one.id == id)
}

/// The washes a case is looked for on: the turned ground shows through each differently.
const CASES: &[&str] = &[
    "#ff000040",
    "#00ff0060",
    "#0000ff80",
    "#ff00ffa0",
    "#ffaa0030",
    "#00000040",
    "#ffffff20",
    "#ff0000c0",
    "#00ff00a0",
    "#ff8000a0",
    "#ffffffa0",
    "#ff00ffd0",
];

/// The first of `build`'s themes that holds as written, next to one that falls, and breaks in a
/// tint: a case with something to hold. `build` takes a wash, and a byte it turns the colour
/// under test by.
fn breaking_case(
    family: &str,
    build: &dyn Fn(&str, u8) -> Option<ResolvedTheme>,
) -> Option<ResolvedTheme> {
    for wash in CASES {
        let mut before = None;
        for byte in 0..=0xff_u8 {
            let Some(theme) = build(wash, byte) else {
                continue;
            };
            let holds = !falls(&untinted(&theme), family);
            if before == Some(!holds) && holds {
                let breaks = (0..360).step_by(15).any(|degree| {
                    falls(
                        &untinted(&tint::tinted_at(&theme, f64::from(degree))),
                        family,
                    )
                });
                if breaks {
                    return Some(theme);
                }
            }
            before = Some(holds);
        }
    }
    None
}

fn grey(byte: u8) -> String {
    format!("#{byte:02x}{byte:02x}{byte:02x}")
}

/// Holds what `build` makes, and says what it found: `missing` when no byte gives a case.
fn held_in_a_tint(family: &str, build: &dyn Fn(&str, u8) -> Option<ResolvedTheme>) {
    let theme = breaking_case(family, build).expect("a case that holds as written and breaks");
    assert!(falls(&theme, family), "{family} is read as written alone");
}

#[test]
fn a_see_through_edge_of_a_refused_box_is_read_in_each_tint() {
    held_in_a_tint("select", &|wash, alpha| {
        built(
            "dark",
            json!({ "chart.4": format!("{}{alpha:02x}", &wash[..7]) }),
            json!({ "select.root.invalid.border-colour": "chart.4" }),
        )
    });
}

#[test]
fn a_see_through_edge_of_a_ghost_is_read_in_each_tint() {
    held_in_a_tint("button", &|wash, alpha| {
        built(
            "dark",
            json!({ "chart.4": format!("{}{alpha:02x}", &wash[..7]) }),
            json!({ "button.root.ghost.border-colour": "chart.4" }),
        )
    });
}

#[test]
fn words_on_a_see_through_ground_token_are_read_in_each_tint() {
    held_in_a_tint("list", &|wash, byte| {
        built(
            "dark",
            json!({ "surface.overlay": wash, "chart.3": grey(byte) }),
            json!({ "list.label.text": "chart.3" }),
        )
    });
}

#[test]
fn a_wash_drawn_only_in_a_combined_set_is_read_in_each_tint() {
    held_in_a_tint("button", &|wash, byte| {
        built(
            "dark",
            json!({ "tab.active": wash, "chart.3": grey(byte) }),
            json!({ "button.root.secondary.text": "chart.3" }),
        )
    });
}

#[test]
fn a_mark_on_a_see_through_track_is_read_in_each_tint() {
    held_in_a_tint("meter", &|wash, byte| {
        built(
            "dark",
            json!({ "border.subtle": wash, "chart.3": grey(byte) }),
            json!({ "meter.fill.fill": "chart.3" }),
        )
    });
}

#[test]
fn an_opaque_colour_on_a_default_wash_in_a_state_is_read_in_each_tint() {
    held_in_a_tint("titlebar", &|wash, byte| {
        built(
            "dark",
            json!({ "accent.wash": wash, "chart.3": grey(byte) }),
            json!({ "titlebar.workspace.selected.text": "chart.3" }),
        )
    });
}

#[test]
fn a_key_in_a_state_is_read_in_each_tint() {
    held_in_a_tint("tabs", &|wash, byte| {
        built(
            "dark",
            json!({ "chart.4": wash, "chart.3": grey(byte) }),
            json!({ "tabs.tab.selected.fill": "chart.4", "tabs.tab.selected.text": "chart.3" }),
        )
    });
}

#[test]
fn what_a_tint_makes_fall_is_read_again_and_every_look_left_reads() {
    let build = |wash: &str, byte| {
        built(
            "dark",
            json!({ "accent.wash": wash, "chart.3": grey(byte) }),
            json!({ "titlebar.workspace.selected.text": "chart.3" }),
        )
    };
    let theme = breaking_case("titlebar", &build).expect("a case");
    assert!(held_again(&theme).is_empty(), "{:?}", held_again(&theme));
    let held_rules = held(&theme).0;
    let family = family("titlebar").unwrap();
    let sets: Vec<Vec<State>> = std::iter::once(Vec::new())
        .chain(states_of(family).into_iter().map(|one| vec![one]))
        .collect();
    for variant in family.looks() {
        for states in &sets {
            let look = drawn(&held_rules, family, variant, states);
            if look == drawn(&[], family, variant, states) {
                continue;
            }
            for degree in (0..360).step_by(15) {
                let tinted = tint::tinted_at(&theme, f64::from(degree));
                let on = grounds(&tinted, family);
                assert!(
                    unreadable(&tinted, &on, &held_rules, family, variant, states, &look).is_none(),
                    "{variant} {states:?} at {degree}"
                );
            }
        }
    }
}

#[test]
fn a_look_that_falls_only_between_the_coarse_tints_is_read_at_every_degree() {
    // Digits only just reading on a dark wash: found by a search, and pinned here. It reads at
    // the theme's own colours and every 10 degrees, and first breaks at 121.
    let parts = json!({ "titlebar.count.fill": "chart.4", "titlebar.count.text": "chart.3" });
    let theme = themed("#000000c8", 0x76, &parts).unwrap();
    let family = "titlebar";
    assert!(!falls(&untinted(&theme), family));
    assert!(
        !theme
            .tint
            .hues
            .iter()
            .any(|one| falls(&untinted(&tint::tinted(&theme, one.name)), family))
    );
    let breaks = |degree: i32| {
        falls(
            &untinted(&tint::tinted_at(&theme, f64::from(degree))),
            family,
        )
    };
    assert!(!(0..360).step_by(COARSE_DEGREES).any(breaks));
    assert!(breaks(121));
    assert!(falls(&theme, family), "{:?}", fallen(&theme));
}

/// A theme whose measure passes over its tints, far longer than no wait at all.
fn measured_in_its_tints() -> ResolvedTheme {
    themed(
        "#ff000040",
        0xff,
        &json!({ "button.root.fill": "chart.4", "button.root.text": "chart.3" }),
    )
    .expect("a theme")
}

#[test]
fn a_first_frame_past_its_wait_goes_on_without_the_parts_and_the_answer_is_the_same() {
    // #4337: with no wait the creation is told there is no sheet yet; the sheet the window then
    // asks for is the one a wait would have had.
    let theme = measured_in_its_tints();
    let now = std::time::Instant::now();
    assert_eq!(stylesheet_by(&theme, now), None);
    let far = now + std::time::Duration::from_secs(3600);
    let waited = stylesheet_by(&theme, far).expect("written before the deadline");
    assert!(!waited.is_empty());
    assert_eq!(stylesheet(&theme), waited);
}

#[test]
fn a_measure_that_is_kept_is_answered_past_any_deadline_with_no_second_pass() {
    // #4337: a cached answer never waits, and never starts a thread to find it.
    let theme = measured_in_its_tints();
    let sheet = stylesheet(&theme);
    let passes = super::passes();
    assert_eq!(
        stylesheet_by(&theme, std::time::Instant::now()).as_deref(),
        Some(sheet.as_str())
    );
    assert_eq!(super::passes(), passes);
}

#[test]
fn a_theme_with_no_parts_is_answered_at_once() {
    let mut theme = measured_in_its_tints();
    theme.parts.clear();
    assert_eq!(
        stylesheet_by(&theme, std::time::Instant::now()).as_deref(),
        Some("")
    );
}

#[test]
fn two_askers_of_one_theme_share_one_pass() {
    // #4337: the window's ask finds its creation still measuring: it waits, it does not pass again.
    let theme = measured_in_its_tints();
    let place = {
        let mut kept = measured();
        let place = std::sync::Arc::new(std::sync::OnceLock::new());
        kept.push_front((theme.clone(), std::sync::Arc::clone(&place)));
        place
    };
    assert!(!measure_is_kept(&theme), "still measuring");
    let answer = std::sync::Arc::new((Vec::new(), Vec::new()));
    place.set(std::sync::Arc::clone(&answer)).unwrap();
    assert!(measure_is_kept(&theme));
    let passes = super::passes();
    assert!(std::sync::Arc::ptr_eq(&held_once(&theme), &answer));
    assert_eq!(super::passes(), passes);
}

#[test]
fn two_threads_asking_at_once_make_one_pass_and_hold_the_one_answer() {
    // #4337: creation's measuring thread and a window's ask, as the app's threads share the place.
    let theme = measured_in_its_tints();
    let kept = kept_here();
    let gate = std::sync::Barrier::new(2);
    let (one, two) = std::thread::scope(|scope| {
        let ask = || {
            keep_in(kept);
            gate.wait();
            let held = held_once(&theme);
            (held, super::passes())
        };
        let (one, two) = (scope.spawn(ask), scope.spawn(ask));
        (one.join().unwrap(), two.join().unwrap())
    });
    assert_eq!(one.1 + two.1, 1, "passes made");
    assert!(std::sync::Arc::ptr_eq(&one.0, &two.0));
}
