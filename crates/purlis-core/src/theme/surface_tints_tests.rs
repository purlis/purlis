// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What stands on a see-through surface, in a tint** (#4310): a surface's see-through fill lets
//! the turned colour under it through, so another family's words that read on it as written can
//! fall in a tint, before the surface's own do. What #4541 left out is here too (#4607): a
//! see-through look on an opaque fill, a look only a set of states draws, a menu, and the margin on
//! every part, edge and state. A sweep here is held to a control that falls as written, so an empty
//! one cannot pass.

use serde_json::{Value, json};

use super::super::{ResolvedTheme, load, tint};
use super::{
    Drawn, Family, Fell, NEAR_A_FLOOR, Rule, State, broken_on, coarsely_tinted, combined, drawn,
    drawn_apart, drawn_on, edge, every_tinted, fallen, family, floor_of, held, looks_in, lowest,
    rules, sets_read, shows_a_tint, singles_of, surface_ground, surfaces_set,
    tints_reach_a_surface, under_part, unreadable,
};

/// The recipe: every ground a surface stands on, so its fill has something turned to show.
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

const COLOURS: &[&str] = &[
    "#ff0000", "#00ff00", "#0000ff", "#ff00ff", "#ffaa00", "#00ffff", "#000000", "#ffffff",
    "#808080", "#ff8000",
];

fn themed(appearance: &str, tokens: Value, parts: Value) -> Option<ResolvedTheme> {
    let loaded = load(&json!({
        "name": "See-through surface", "extends": "purlis-classic",
        "appearances": [{ "id": appearance, "tokens": tokens }],
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
        .find(|one| one.id == appearance)
}

fn untinted(theme: &ResolvedTheme) -> ResolvedTheme {
    let mut alone = theme.clone();
    alone.tint.on = false;
    alone
}

/// A surface with no edge of its own to fall first, its own words and the switcher's count in
/// the strongest colour, so what falls is what the buttons draw; with `tokens` and `more` keys.
fn standing_on(surface: &str, tokens: Value, more: &Value) -> Option<ResolvedTheme> {
    let own: &[&str] = match surface {
        "popover" => &["root", "header", "footer", "key"],
        "dialog" => &["title", "subtitle", "description", "summary"],
        _ => &["title"],
    };
    let mut parts = json!({
        format!("{surface}.root.fill"): "chart.4",
        format!("{surface}.root.border-width"): 0,
        "titlebar.row-count.text": "text.primary",
    });
    for part in own {
        parts[format!("{surface}.{part}.text")] = json!("text.primary");
    }
    for (key, value) in more.as_object().expect("keys") {
        parts[key] = value.clone();
    }
    themed("light", tokens, parts)
}

fn on_a_surface(surface: &str, wash: &str) -> ResolvedTheme {
    standing_on(surface, json!({ "chart.4": wash }), &json!({})).expect("the theme loads")
}

/// The washes found by a search that read as written, each with a degree they fall at.
const ONLY_IN_A_TINT: &[(&str, &str, i32)] = &[
    ("popover", "#ff000022", 150),
    ("panel", "#00ff008f", 0),
    ("dialog", "#ff000031", 180),
];

const BUTTON_WORDS: &str = "what a button draws on it: the root's words";

fn words_fell(fell: &[Fell]) -> bool {
    fell.iter().any(|one| one.floor.starts_with(BUTTON_WORDS))
}

/// Whether what `family` draws on a surface brought the surface back.
fn fell_on(fell: &[Fell], family: &str) -> bool {
    let on_it = format!("what a {family} draws on it");
    fell.iter().any(|one| one.floor.starts_with(&on_it))
}

fn grey(byte: u8) -> String {
    format!("#{byte:02x}{byte:02x}{byte:02x}")
}

/// **What `name` breaks on `surface`'s fill `chart.4`, as `theme` draws it**: this one reading and
/// not the whole hold, so that a search can ask it often.
fn breaks_on(theme: &ResolvedTheme, name: &str, surface: &str) -> bool {
    let alone = untinted(theme);
    let rules = rules(&alone.parts);
    broken_on(
        &alone,
        &[],
        std::slice::from_ref(&alone),
        &rules,
        family(name).unwrap(),
        family(surface).unwrap(),
        "chart.4",
    )
    .is_some()
}

/// **What `name` breaks on `surface`'s fill `chart.4` through the reading itself**, the coarse tints
/// and every degree as the hold passes them. What a test holds where words this near would fall on
/// the family's own grounds first, and the hold never come to the surface.
fn falls_in_the_tints(theme: &ResolvedTheme, name: &str, surface: &str) -> Option<String> {
    let rules = rules(&theme.parts);
    let mut drawn_in = vec![theme.clone()];
    drawn_in.extend(every_tinted(theme));
    broken_on(
        theme,
        &coarsely_tinted(theme),
        &drawn_in,
        &rules,
        family(name).unwrap(),
        family(surface).unwrap(),
        "chart.4",
    )
    .map(|(floor, _)| floor)
}

fn breaks_in_a_tint(theme: &ResolvedTheme, name: &str, surface: &str) -> bool {
    (0..360)
        .step_by(5)
        .any(|degree| breaks_on(&tint::tinted_at(theme, f64::from(degree)), name, surface))
}

/// The byte next to where `breaks` starts, walking from `holds` to `falls`, on the side that holds.
fn last_holding(mut holds: u8, mut falls: u8, breaks: impl Fn(u8) -> bool) -> Option<u8> {
    if breaks(holds) || !breaks(falls) {
        return None;
    }
    while holds.abs_diff(falls) > 1 {
        let middle = holds.midpoint(falls);
        if breaks(middle) {
            falls = middle;
        } else {
            holds = middle;
        }
    }
    Some(holds)
}

/// The colours a case is looked for in, at each of `alphas`.
fn washes(alphas: &'static [u32]) -> impl Iterator<Item = String> {
    COLOURS[..6].iter().flat_map(move |colour| {
        alphas
            .iter()
            .map(move |alpha| format!("{colour}{alpha:02x}"))
    })
}

/// `keys`, with each button with no box of its own in the strongest words and the secondary with
/// no edge, so that what comes near a floor on a surface is the secondary's fill.
fn secondary_near(mut keys: Value) -> Value {
    keys["button.root.quiet.text"] = json!("text.primary");
    keys["button.root.link.text"] = json!("text.primary");
    keys["button.root.secondary.border-width"] = json!(0);
    keys
}

#[test]
fn a_button_s_words_that_fall_only_in_a_tint_bring_the_surface_back() {
    for (surface, wash, degree) in ONLY_IN_A_TINT {
        let theme = on_a_surface(surface, wash);
        let written = fallen(&untinted(&theme));
        assert!(written.is_empty(), "{surface} {wash}: {written:?}");
        let tinted = fallen(&untinted(&tint::tinted_at(&theme, f64::from(*degree))));
        assert!(
            words_fell(&tinted),
            "{surface} {wash} at {degree}: {tinted:?}"
        );
        let held = fallen(&theme);
        assert!(
            words_fell(&held),
            "{surface} {wash} is read as written alone"
        );
    }
}

#[test]
fn a_surface_whose_words_fall_as_written_is_still_caught_on_the_same_colour() {
    // The control: the same colour, more of it, falls as written, so the washes above are near a
    // floor this reading can reach and not a fill it never reads.
    for (surface, _, _) in ONLY_IN_A_TINT {
        let falling = COLOURS
            .iter()
            .flat_map(|colour| {
                (0x10..0xff_u32)
                    .step_by(8)
                    .map(move |alpha| format!("{colour}{alpha:02x}"))
            })
            .map(|wash| on_a_surface(surface, &wash))
            .find(|theme| words_fell(&fallen(&untinted(theme))))
            .unwrap_or_else(|| panic!("no wash brings {surface}'s button words down"));
        assert!(words_fell(&fallen(&falling)), "{surface}");
    }
}

#[test]
fn the_switcher_s_count_that_falls_only_in_a_tint_brings_the_popover_back() {
    // What stands apart from its root on the popover: the fill, as found by a search.
    let theme = themed(
        "light",
        json!({ "chart.4": "#ff000014" }),
        json!({ "popover.root.fill": "chart.4", "popover.root.border-width": 0 }),
    )
    .expect("the theme loads");
    assert!(fallen(&untinted(&theme)).is_empty());
    let on_it = "what a titlebar draws on it";
    let tinted = fallen(&untinted(&tint::tinted_at(&theme, 120.0)));
    assert!(
        tinted.iter().any(|one| one.floor.starts_with(on_it)),
        "{tinted:?}"
    );
    let held = fallen(&theme);
    assert!(
        held.iter().any(|one| one.floor.starts_with(on_it)),
        "{held:?}"
    );
}

#[test]
fn an_opaque_surface_is_read_as_written_alone() {
    for fill in ["#ffffff", "#f0f0f0"] {
        let theme = on_a_surface("popover", fill);
        assert!(!shows_a_tint(&theme, "chart.4"));
        assert!(fallen(&theme).is_empty(), "{fill}: {:?}", fallen(&theme));
    }
    assert!(shows_a_tint(
        &on_a_surface("popover", "#ff000022"),
        "chart.4"
    ));
}

#[test]
fn a_shipped_theme_builds_no_tints_for_what_stands_on_its_surfaces() {
    for base in ["purlis", "purlis-classic"] {
        let loaded = load(&json!({ "name": "Plain", "extends": base }));
        for appearance in &loaded.theme.appearances {
            let rules = held(appearance).0;
            assert!(
                !tints_reach_a_surface(appearance, &rules),
                "{base} {}",
                appearance.id
            );
            for (surface, _, _, set) in surfaces_set(&rules) {
                assert!(
                    !shows_a_tint(appearance, set),
                    "{base} {} {}",
                    appearance.id,
                    surface.name
                );
            }
        }
    }
}

#[test]
fn the_coarse_reading_comes_within_its_margin_of_every_degree() {
    // What lets a look clear of a floor at the coarse tints stay unread at the rest (#4257's
    // margin, read here on what stands on a surface).
    let button = family("button").unwrap();
    let mut widest: f64 = 0.0;
    let mut read = 0;
    for surface in ["popover"] {
        let surface_family = family(surface).unwrap();
        for colour in &COLOURS[..3] {
            for alpha in [0x14_u32, 0x60] {
                let theme = on_a_surface(surface, &format!("{colour}{alpha:02x}"));
                for variant in button.looks() {
                    let look = drawn(&[], button, variant, &[]);
                    let at = |one: &ResolvedTheme| {
                        let on = surface_ground(one, surface_family, "chart.4");
                        lowest(one, &on, look.grounds[0], look.words[0])
                    };
                    let least = |all: &[ResolvedTheme]| {
                        all.iter().filter_map(at).fold(f64::INFINITY, f64::min)
                    };
                    let (coarse, fine) = (coarsely_tinted(&theme), every_tinted(&theme));
                    let (coarse, fine) = (least(&coarse), least(&fine));
                    if coarse.is_finite() && fine.is_finite() {
                        widest = widest.max(coarse - fine);
                        read += 1;
                    }
                }
            }
        }
    }
    assert!(read > 20, "{read}");
    assert!(widest < NEAR_A_FLOOR, "{widest}");
}

#[test]
fn a_see_through_look_on_an_opaque_surface_is_read_in_the_tints() {
    // The fill is opaque and no tint turns it: what moves is the look, which the recipe turns. On
    // white every default look clears its floors, so only these words come near.
    // Each token is drawn elsewhere only under the primary words, so the wash brings no others near.
    let build = |token: &str, wash: &str, byte: u8| {
        let mut tokens = json!({ "chart.3": grey(byte), "chart.4": "#ffffff" });
        tokens[token] = json!(wash);
        standing_on(
            "popover",
            tokens,
            &secondary_near(json!({
                "button.root.secondary.fill": token,
                "button.root.secondary.text": "chart.3",
            })),
        )
    };
    let mut tried = Vec::new();
    let found = ["list.selected", "surface.hover"]
        .into_iter()
        .flat_map(|token| {
            washes(&[0x30, 0x40, 0x50, 0x60, 0x70, 0x80]).map(move |wash| (token, wash))
        })
        .find_map(|(token, wash)| {
            let breaks = |byte| {
                build(token, &wash, byte).is_none_or(|one| breaks_on(&one, "button", "popover"))
            };
            let byte = last_holding(0, 0xff, breaks);
            tried.push((token, wash.clone(), byte));
            let theme = build(token, &wash, byte?)?;
            breaks_in_a_tint(&theme, "button", "popover").then_some((token, wash, byte?))
        });
    let Some((token, wash, byte)) = found else {
        panic!("no see-through look holds as written and breaks in a tint: {tried:?}");
    };
    let theme = build(token, &wash, byte).unwrap();
    assert!(!shows_a_tint(&theme, "chart.4"), "{wash}");
    assert!(!breaks_on(&theme, "button", "popover"), "{wash} {byte}");
    let floor = falls_in_the_tints(&theme, "button", "popover");
    assert!(
        floor
            .as_ref()
            .is_some_and(|one| one.contains("words contrast")),
        "{wash} {byte}: {floor:?}"
    );
    // The controls: a step lighter the same words fall as written, and black ones in no tint.
    assert!(breaks_on(
        &build(token, &wash, byte + 1).unwrap(),
        "button",
        "popover"
    ));
    let black = falls_in_the_tints(&build(token, &wash, 0).unwrap(), "button", "popover");
    assert_eq!(black, None, "{wash}");
}

/// Whether the secondary reads on `surface`'s fill in every state alone and breaks in a set of them.
fn falls_only_in_a_set(theme: &ResolvedTheme, surface: &str) -> bool {
    let alone = untinted(theme);
    let rules = rules(&alone.parts);
    let button = family("button").unwrap();
    let on = surface_ground(&alone, family(surface).unwrap(), "chart.4");
    let falls = |states: &[State]| {
        button.looks().into_iter().any(|variant| {
            let look = drawn(&rules, button, variant, states);
            unreadable(&alone, &on, &rules, button, variant, states, &look).is_some()
        })
    };
    !singles_of(button).iter().any(|one| falls(one))
        && combined(button).iter().any(|one| falls(one))
}

#[test]
fn a_look_only_a_set_of_states_draws_on_a_surface_is_read() {
    // Hovered and pressed at once: the words of one on the see-through fill of the other, which
    // neither draws alone. On white every default look clears its floors, so only those come near.
    let build = |wash: &str, words: u8| {
        standing_on(
            "popover",
            json!({ "chart.3": grey(words), "chart.4": "#ffffff", "chart.5": wash }),
            &secondary_near(json!({
                "button.root.secondary.hover.text": "chart.3",
                "button.root.secondary.pressed.fill": "chart.5",
            })),
        )
    };
    let mut tried = 0;
    let theme = ["#00000040", "#00000050", "#00000060"]
        .into_iter()
        .flat_map(|wash| (0x38..=0x70_u8).step_by(8).map(move |words| (wash, words)))
        .filter_map(|(wash, words)| build(wash, words))
        .inspect(|_| tried += 1)
        .find(|theme| falls_only_in_a_set(theme, "popover"));
    let Some(theme) = theme else {
        panic!("no look only a set of states draws falls on the popover, of {tried} that loaded");
    };
    assert!(breaks_on(&theme, "button", "popover"));
}

#[test]
fn a_menu_s_words_that_fall_only_in_a_tint_bring_the_popover_back() {
    // A menu is read after a button, so the button has to clear every tint for this to be the
    // menu's: its row's words are the theme's own, and only those come near.
    let build = |wash: &str, byte: u8| {
        standing_on(
            "popover",
            json!({ "chart.3": grey(byte), "chart.4": wash }),
            &json!({ "menu.root.default.text": "chart.3" }),
        )
    };
    let (wash, byte) = washes(&[0x14, 0x20, 0x30, 0x40])
        .find_map(|wash| {
            let breaks =
                |byte| build(&wash, byte).is_none_or(|one| breaks_on(&one, "menu", "popover"));
            let byte = last_holding(0, 0xff, breaks)?;
            let theme = build(&wash, byte)?;
            (breaks_in_a_tint(&theme, "menu", "popover")
                && !breaks_in_a_tint(&theme, "button", "popover")
                && fallen(&untinted(&theme)).is_empty())
            .then_some((wash, byte))
        })
        .expect("menu words that hold as written and break in a tint, a button's clear of it");
    let held = fallen(&build(&wash, byte).unwrap());
    assert!(fell_on(&held, "menu"), "{wash} {byte}: {held:?}");
    assert!(!fell_on(&held, "button"), "{wash} {byte}: {held:?}");
    // The control: a step lighter, the same words fall as written.
    let falling = fallen(&untinted(&build(&wash, byte + 1).unwrap()));
    assert!(fell_on(&falling, "menu"), "{wash} {byte}: {falling:?}");
}

#[test]
fn a_card_or_a_notice_takes_no_fill_a_button_is_not_read_on() {
    // #4541's search found no such theme, and this is why: the load refuses a fill off the
    // button's grounds, see-through or not, and on one of them the button is read already.
    let refusal = "is not a ground the answers";
    for surface in ["card", "notice"] {
        for fill in ["#ff000022", "#202020"] {
            let loaded = load(&json!({
                "name": "Off the grounds", "extends": "purlis-classic",
                "appearances": [{ "id": "light", "tokens": { "chart.4": fill } }],
                "tint": { "tokens": RECIPE },
                "parts": { format!("{surface}.root.fill"): "chart.4" },
            }));
            assert!(
                loaded
                    .complaints
                    .iter()
                    .any(|one| one.said.contains(refusal)),
                "{surface} {fill}: {:?}",
                loaded.complaints
            );
        }
        let theme = themed(
            "light",
            json!({ "surface.sunken": "#ff000022" }),
            json!({ format!("{surface}.root.fill"): "surface.sunken" }),
        )
        .expect("a ground is taken, see-through or not");
        let rules = rules(&theme.parts);
        let set: Vec<_> = surfaces_set(&rules)
            .into_iter()
            .filter(|(one, _, _, _)| one.name == surface)
            .collect();
        assert!(!set.is_empty(), "{surface}");
        for (one, _, default, fill) in set {
            assert!(shows_a_tint(&theme, fill), "{surface}");
            assert_eq!(drawn_on(one, default, fill).count(), 0, "{surface}");
        }
    }
    // The control: the same two lists do put a button and a menu on a popover's fill.
    let theme = on_a_surface("popover", "#ff000022");
    let rules = rules(&theme.parts);
    let on_it: Vec<_> = surfaces_set(&rules)
        .into_iter()
        .filter(|(one, _, _, _)| one.name == "popover")
        .flat_map(|(one, _, default, fill)| drawn_on(one, default, fill).map(|on| on.name))
        .collect();
    assert!(
        on_it.contains(&"button") && on_it.contains(&"menu"),
        "{on_it:?}"
    );
}

/// A look's readings, in the order the floors read them: each part's words on its fill, each with
/// its floor, then the root's edge. A slot not drawn reads `None`.
fn readings(
    one: &ResolvedTheme,
    on: &[String],
    rules: &[Rule],
    family: &Family,
    variant: &str,
    look: &Drawn,
) -> Vec<(f64, Option<f64>)> {
    let mut read: Vec<(f64, Option<f64>)> = family
        .parts
        .iter()
        .enumerate()
        .filter(|(_, part)| drawn_apart(family, part.name).is_none())
        .map(|(at, part)| {
            let under = under_part(one, on, family, at, variant, look);
            (
                one.least.floor(floor_of(family, part)),
                lowest(one, &under, look.grounds[at], look.words[at]),
            )
        })
        .collect();
    let border = edge(rules, family, variant, look);
    read.push((
        one.least.floor(3.0),
        border.and_then(|border| lowest(one, on, look.fill, border)),
    ));
    read
}

/// How far over its floor a reading counts: past this a gap wider than the margin decides nothing.
const WITHIN: f64 = 1.0;

#[test]
fn the_coarse_reading_comes_within_its_margin_on_every_part_edge_and_state() {
    // Every look a family draws on the fill, each reading the check takes of it. The darkest wash
    // falls as written: the control, that a sweep this size does reach a floor.
    let surface = family("popover").unwrap();
    let mut widest: f64 = 0.0;
    let (mut parts, mut edges, mut states, mut fell) = (0, 0, 0, 0);
    for wash in [
        "#ff000022",
        "#00ff0040",
        "#0000ff40",
        "#ff00ff60",
        "#ffaa0060",
        "#00ffff80",
        "#80808080",
        "#00000090",
    ] {
        let theme = on_a_surface("popover", wash);
        let rules = rules(&theme.parts);
        let on_all = |all: Vec<ResolvedTheme>| -> Vec<(ResolvedTheme, Vec<String>)> {
            all.into_iter()
                .map(|one| {
                    let on = surface_ground(&one, surface, "chart.4");
                    (one, on)
                })
                .collect()
        };
        let coarse = on_all(coarsely_tinted(&theme));
        let fine = on_all(every_tinted(&theme));
        let written = surface_ground(&theme, surface, "chart.4");
        for name in ["button", "menu"] {
            let drawn_family = family(name).unwrap();
            let sets = sets_read(&rules, drawn_family);
            for (variant, set, look) in looks_in(&rules, drawn_family, &sets) {
                let least = |all: &[(ResolvedTheme, Vec<String>)]| {
                    let mut low: Vec<(f64, f64)> = Vec::new();
                    for (one, on) in all {
                        let read = readings(one, on, &rules, drawn_family, variant, &look);
                        if low.is_empty() {
                            low = vec![(0.0, f64::INFINITY); read.len()];
                        }
                        for (slot, (floor, ratio)) in low.iter_mut().zip(read) {
                            *slot = (floor, slot.1.min(ratio.unwrap_or(f64::INFINITY)));
                        }
                    }
                    low
                };
                let as_written = readings(&theme, &written, &rules, drawn_family, variant, &look);
                fell += as_written
                    .iter()
                    .filter(|(floor, ratio)| ratio.is_some_and(|ratio| ratio < *floor))
                    .count();
                let at_coarse = least(&coarse);
                // Every degree is the dear reading: a look far over its floors cannot need it.
                if at_coarse
                    .iter()
                    .all(|(floor, low)| *low >= floor + WITHIN + NEAR_A_FLOOR)
                {
                    continue;
                }
                let at_every = least(&fine);
                let last = at_every.len() - 1;
                for (at, ((floor, coarsest), (_, finest))) in
                    at_coarse.iter().zip(&at_every).enumerate()
                {
                    if finest.is_finite() && *finest < floor + WITHIN {
                        widest = widest.max(coarsest - finest);
                        if at == last {
                            edges += 1;
                        } else if set.is_empty() {
                            parts += 1;
                        } else {
                            states += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(widest < NEAR_A_FLOOR, "{widest}");
    assert!(
        parts > 20 && states > 20 && edges > 0,
        "{parts} {states} {edges}"
    );
    assert!(fell > 0, "the sweep reached no floor");
}
