// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The pairs the check reports and nothing else acts on** (#3735): a meter's fills on its track,
//! held for a palette alone, at the engine's interface: a theme in, the report and the drawn
//! theme out.

use serde_json::{Value, json};

use super::super::{Finding, PAIRS, Report, ResolvedTheme, check, classic, floors, load};
use super::REPORTED;

/// What a meter draws on its track (`ui/meter/meter.css`): the subject, the baseline and the
/// previous version.
const FILLS: [&str; 3] = ["accent.base", "state.waiting", "text.muted"];

/// A format 2 theme with one dark appearance that sets `tokens`: its appearance as read.
fn dark(tokens: Value) -> ResolvedTheme {
    let resolved = load(&json!({
        "name": "Probe",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }]
    }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    resolved.theme.first().clone()
}

/// Whether the check found `front` on the meter's track over `ground`, as the theme is.
fn lost_on_track(report: &Report, front: &str, ground: &str) -> bool {
    report.findings.iter().any(|one| {
        matches!(one, Finding::Contrast { tint: None, pair, .. }
            if pair.front == front && pair.back == "border.subtle" && pair.under == Some(ground))
    })
}

/// A dark theme whose track, `border.subtle`, is drawn in `fill`'s own colour: the bar is lost.
fn track_as(fill: &str) -> ResolvedTheme {
    let colour = classic().first().value(fill).to_owned();
    dark(json!({ "border.subtle": colour }))
}

#[test]
fn a_palette_that_loses_a_meter_fill_on_its_track_gets_a_finding() {
    for fill in FILLS {
        let report = check(&track_as(fill));
        for ground in ["surface.base", "surface.raised"] {
            assert!(
                lost_on_track(&report, fill, ground),
                "{fill} on its track over {ground}: {:#?}",
                report.findings
            );
        }
    }
}

#[test]
fn a_reported_pair_is_never_a_pair_floors_tiers_or_a_person_s_colour_read() {
    // The consent floors, the import's tier walk and the person's colours each read `PAIRS`.
    for pair in REPORTED {
        assert!(!PAIRS.contains(pair), "{pair:?} is in PAIRS");
        assert!(
            floors::pairs().all(|(_, held)| held != pair),
            "{pair:?} is a consent floor"
        );
    }
}

#[test]
fn a_waiting_bar_lost_on_its_track_leaves_the_waiting_colour_as_the_theme_writes_it() {
    let theme = track_as("state.waiting");
    let (drawn, held) = floors::hold(&theme);
    assert_eq!(held, None);
    assert_eq!(drawn.value("state.waiting"), theme.value("state.waiting"));
}
