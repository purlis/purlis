// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a theme's parts cost a window's creation** (#4337): each appearance's first stylesheet,
//! the one call on the way to the first frame that measures (`windowprefs::at_creation`, through
//! `WindowTheme::from`), for a shipped theme and for two themes that show a tint through a
//! see-through colour. Each run is its own process, so nothing is kept from an earlier one.
//!
//! And the worst case of what stands on a surface's fill (#4607): the popover and a dialog each
//! filled with the see-through wash most opaque that every family drawn on it still reads on, so
//! every look a coarse tint finds near a floor is read again at every degree, with a panel filled
//! too, which always falls ([`PANEL`]); beside it, each wash a step past, which falls and is held
//! again, and the three filled opaque, where only the families' own see-through looks open the
//! tints (D-4607-1).
//!
//! `cargo run -p purlis-core --profile dev-release --example launch_theme_timing`

use std::time::{Duration, Instant};

use purlis_core::theme::parts::{MANIFEST, fallen, stylesheet, stylesheet_by};
use purlis_core::theme::{self, load};
use serde_json::{Map, Value, json};

/// The grounds a recipe turns: #4300's costs were read with these ten.
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

/// A theme whose `parts` draw a see-through wash, with words well above the floor.
fn see_through(name: &str, parts: Map<String, Value>) -> Value {
    json!({
        "name": name, "extends": "purlis-classic",
        "appearances": [
            { "id": "dark", "tokens": { "chart.4": "#ff000040", "chart.3": "#ffffff" } },
            { "id": "light", "tokens": { "chart.4": "#ff000040", "chart.3": "#000000" } },
        ],
        "tint": { "tokens": RECIPE },
        "parts": parts,
    })
}

/// A see-through fill on the root of every family whose root takes one and does not refuse it.
fn every_family() -> Map<String, Value> {
    MANIFEST
        .iter()
        .filter(|family| {
            family.parts[0]
                .properties
                .contains(&theme::parts::Prop::Fill)
        })
        .map(|family| {
            (
                format!("{}.{}.fill", family.name, family.parts[0].name),
                json!("chart.4"),
            )
        })
        .filter(|(key, value)| {
            let one = Map::from_iter([(key.clone(), value.clone())]);
            !load(&see_through("One", one)).refused
        })
        .collect()
}

fn timed(name: &str, document: &Value) {
    timed_in(name, document, None);
}

/// [`timed`], in the appearance named `only`, or in each.
fn timed_in(name: &str, document: &Value, only: Option<&str>) {
    let loaded = load(document);
    assert!(!loaded.refused, "{name} was refused");
    let mut each = Vec::new();
    for appearance in loaded
        .theme
        .appearances
        .iter()
        .filter(|one| only.is_none_or(|id| one.id == id))
    {
        // What the first frame waits: the creation's budget, or nothing with no parts to wait for.
        let started = Instant::now();
        let by = stylesheet_by(appearance, started + Duration::from_millis(50));
        let waited = started.elapsed();
        // What the window's ask then waits for, the pass on the way being the one it shares.
        let started = Instant::now();
        let sheet = stylesheet(appearance);
        let cold = started.elapsed();
        let started = Instant::now();
        let again = stylesheet(appearance);
        let kept = started.elapsed();
        assert_eq!(sheet, again);
        each.push(format!(
            "{}: first frame waited {:.1} ms ({}), the rest {:.1} ms, kept {:.2} ms, {} bytes",
            appearance.id,
            waited.as_secs_f64() * 1e3,
            if by.is_some() {
                "with parts"
            } else {
                "parts later"
            },
            cold.as_secs_f64() * 1e3,
            kept.as_secs_f64() * 1e3,
            sheet.len()
        ));
    }
    println!("{name}\n  {}", each.join("\n  "));
}

/// The three surfaces a family is read on whose fill can stay near a floor together, each with
/// the token its fill is named by.
const NEAR: &[(&str, &str)] = &[
    ("popover", "chart.4"),
    ("panel", "chart.5"),
    ("dialog", "chart.6"),
];

/// The appearance the three are filled in: on Purlis Classic's light one, a badge on a panel
/// falls under any wash that darkens it, so no panel there stays near a floor.
const APPEARANCE: &str = "dark";

/// The washes a surface's fill is looked for in.
const COLOURS: &[&str] = &[
    "#ff0000", "#00ff00", "#0000ff", "#ffffff", "#ffaa00", "#00ffff",
];

/// Each of `washes`' surfaces filled with its wash, edgeless, its own words the primary, so what
/// comes near a floor is what other families draw on it; `seen` a token no part draws, so one
/// theme is never another's kept measure. In [`APPEARANCE`] only: the washes are found there.
fn on_surfaces(washes: &[(&str, String)], seen: &str) -> Value {
    let mut parts = Map::new();
    let mut tokens = Map::new();
    tokens.insert("chart.7".to_owned(), json!(seen));
    for (surface, wash) in washes {
        let (_, token) = NEAR
            .iter()
            .find(|(one, _)| one == surface)
            .expect("a surface");
        tokens.insert((*token).to_owned(), json!(wash));
        parts.insert(format!("{surface}.root.fill"), json!(token));
        parts.insert(format!("{surface}.root.border-width"), json!(0));
        let own: &[&str] = match *surface {
            "popover" => &["root", "header", "footer", "key"],
            "dialog" => &["title", "subtitle", "description", "summary"],
            _ => &["title"],
        };
        for part in own {
            parts.insert(format!("{surface}.{part}.text"), json!("text.primary"));
        }
    }
    json!({
        "name": "Near a floor", "extends": "purlis-classic",
        "appearances": [{ "id": APPEARANCE, "tokens": tokens }],
        "tint": { "tokens": RECIPE },
        "parts": parts,
    })
}

/// Whether nothing falls in [`APPEARANCE`] of `document`.
fn holds(document: &Value) -> bool {
    let loaded = load(document);
    !loaded.refused
        && loaded
            .theme
            .appearances
            .iter()
            .filter(|one| one.id == APPEARANCE)
            .all(|one| fallen(one).is_empty())
}

/// Each surface filled, with its wash.
type Washes = Vec<(&'static str, String)>;

/// A panel's fill, which falls: the grip of the separator between a panel's halves is drawn on it,
/// takes no colour of a theme's, and breaks its floor on any fill a theme sets, so no panel stays
/// near a floor. Its worst case is to fall, which sends the hold round again.
const PANEL: &str = "#ffffff10";

/// **For the popover and the dialog, the wash most opaque that nothing on it falls under**, of
/// the colours each holds the most of, and the next alpha past it; with the panel filled.
fn near_a_floor() -> (Washes, Washes) {
    let panel = ("panel", PANEL.to_owned());
    let (mut holding, mut falling) = (vec![panel.clone()], vec![panel]);
    for (surface, _) in NEAR.iter().filter(|(one, _)| *one != "panel") {
        let at = |colour: &str, alpha: u32| vec![(*surface, format!("{colour}{alpha:02x}"))];
        let (colour, alpha) = COLOURS
            .iter()
            .filter_map(|colour| {
                let alpha = (0x04..0xc0)
                    .take_while(|alpha| holds(&on_surfaces(&at(colour, *alpha), "#000000")))
                    .last()?;
                Some((*colour, alpha))
            })
            .max_by_key(|(_, alpha)| *alpha)
            .unwrap_or_else(|| panic!("no wash holds on the {surface}"));
        holding.extend(at(colour, alpha));
        falling.extend(at(colour, alpha + 1));
    }
    (holding, falling)
}

fn main() {
    timed("purlis", &json!({ "name": "P", "extends": "purlis" }));
    let button = json!({ "button.root.fill": "chart.4", "button.root.text": "chart.3" });
    let Value::Object(button) = button else {
        unreachable!()
    };
    timed("one see-through family", &see_through("One", button));
    let every = every_family();
    let families = every.len();
    timed(
        &format!("{families} see-through families"),
        &see_through("Every", every),
    );
    let (holding, falling) = near_a_floor();
    let said = |washes: &[(&str, String)]| {
        washes
            .iter()
            .map(|(surface, wash)| format!("{surface} {wash}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    assert!(
        holds(&on_surfaces(&holding[1..], "#000000")),
        "the popover and the dialog together fall"
    );
    timed_in(
        &format!("near a floor: {}", said(&holding)),
        &on_surfaces(&holding, "#010101"),
        Some(APPEARANCE),
    );
    timed_in(
        &format!("just past it: {}", said(&falling)),
        &on_surfaces(&falling, "#010101"),
        Some(APPEARANCE),
    );
    let opaque: Vec<_> = NEAR
        .iter()
        .map(|(surface, _)| (*surface, "#f2f2f2".to_owned()))
        .collect();
    timed_in(
        "the three filled opaque",
        &on_surfaces(&opaque, "#010101"),
        Some(APPEARANCE),
    );
}
