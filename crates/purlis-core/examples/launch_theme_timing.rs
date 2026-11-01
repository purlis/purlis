// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a theme's parts cost a window's creation** (#4337): each appearance's first stylesheet,
//! the one call on the way to the first frame that measures (`windowprefs::at_creation`, through
//! `WindowTheme::from`), for a shipped theme and for two themes that show a tint through a
//! see-through colour. Each run is its own process, so nothing is kept from an earlier one.
//!
//! `cargo run -p purlis-core --profile dev-release --example launch_theme_timing`

use std::time::{Duration, Instant};

use purlis_core::theme::parts::{MANIFEST, stylesheet, stylesheet_by};
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
    let loaded = load(document);
    assert!(!loaded.refused, "{name} was refused");
    let mut each = Vec::new();
    for appearance in &loaded.theme.appearances {
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
}
