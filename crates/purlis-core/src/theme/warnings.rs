// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The theme check's warnings as a person reads them** (#2321, #2331): one line per pair, the
//! theme as it is first and the workspace tints after, each combination of appearance and
//! option values named when there is more than one. `purlis theme check` prints these lines and
//! Settings › Theme shows them on a theme's card, so the two say the same thing in the same
//! words. A chat reads the findings as the engine reports them (`purlis theme check --json`).

use super::check::{Finding, laid};
use super::{Checked, Density, PALETTE, Style, Theme};
use crate::shown;

/// **Every warning the check found in `checked`**, each a line: a combination is named at the
/// front of its lines only when the theme has more than one to tell apart.
pub fn said(theme: &Theme, checked: &[Checked]) -> Vec<String> {
    said_naming(theme, checked, |_, _| None)
}

/// [`said`], with each terminal colour's line naming where to change it: `entry` answers, for
/// an appearance's id and a `terminal.ansi.*` token, the palette entry that holds its colour
/// (an imported terminal's own, which import never tunes, #2589).
pub fn said_naming(
    theme: &Theme,
    checked: &[Checked],
    entry: impl Fn(&str, &str) -> Option<String>,
) -> Vec<String> {
    let named = checked.len() > 1;
    let mut out = Vec::new();
    for one in checked {
        let called = combination(theme, one);
        for warning in warnings(&one.report.findings) {
            let mut line = warning.line();
            if let Some(entry) = warning.ansi.and_then(|token| entry(&one.appearance, token)) {
                line.push_str(&format!(
                    "; it is the palette's `{entry}`: change `{entry}` in theme.json to lift it"
                ));
            }
            out.push(if named {
                format!("{called}: {line}")
            } else {
                line
            });
        }
    }
    out
}

/// **What a combination is called** at the front of a line about it: its appearance, and each
/// option's value in the theme's order when the theme has options (`dark, borders on, accent
/// teal`).
pub fn combination(theme: &Theme, checked: &Checked) -> String {
    let mut parts = vec![checked.appearance.clone()];
    parts.extend(theme.options.iter().filter_map(|option| {
        checked
            .options
            .get(&option.id)
            .map(|value| format!("{} {value}", option.id))
    }));
    shown::short(&parts.join(", "))
}

/// The findings of one pair (or one terminal colour, or the selected row against one other),
/// as the theme is and in each workspace tint it was found in: one line.
struct Warning {
    /// What is drawn on what.
    subject: String,
    /// The floor it is under; `None` for the selected row, which has no ratio.
    floor: Option<f64>,
    /// Found in the theme as it is, with its ratio when it has one.
    plain: Option<Option<f64>>,
    /// Each workspace tint it was found in, with its ratio when it has one.
    tints: Vec<(&'static str, Option<f64>)>,
    /// The terminal colour it is about, for a terminal colour too near its background.
    ansi: Option<&'static str>,
}

impl Warning {
    fn line(&self) -> String {
        let lowest = self
            .tints
            .iter()
            .filter_map(|(_, ratio)| *ratio)
            .fold(f64::INFINITY, f64::min);
        let tints = if self.tints.len() == PALETTE.len() {
            "every workspace tint".to_owned()
        } else {
            let names: Vec<&str> = self.tints.iter().map(|(name, _)| *name).collect();
            format!(
                "the {} {}",
                names.join(", "),
                crate::counts::plural(names.len(), "tint", "tints")
            )
        };
        let floor = self
            .floor
            .map_or_else(String::new, |floor| format!(", under {floor}"));
        match (self.plain, self.tints.is_empty()) {
            (Some(Some(ratio)), true) => format!("{}: {ratio:.2}{floor}", self.subject),
            (Some(Some(ratio)), false) => format!(
                "{}: {ratio:.2}{floor}; in {tints} too, down to {lowest:.2}",
                self.subject
            ),
            (Some(None), true) => self.subject.clone(),
            (Some(None), false) => format!("{}; in {tints} too", self.subject),
            (None, _) if lowest.is_finite() => {
                format!("{}, in {tints}: down to {lowest:.2}{floor}", self.subject)
            }
            (None, _) => format!("{}, in {tints}", self.subject),
        }
    }
}

/// The findings as a person reads them: one line per pair, the theme as it is first and the
/// tints after, in the engine's order.
fn warnings(findings: &[Finding]) -> Vec<Warning> {
    let mut found: Vec<Warning> = Vec::new();
    for finding in findings {
        let (subject, floor, tint, ratio) = match finding {
            Finding::Contrast { tint, pair, ratio } => (
                laid(pair.front, pair.back, [pair.under, pair.beneath]),
                Some(pair.floor),
                *tint,
                Some(*ratio),
            ),
            Finding::ContrastBetweenTints {
                degree,
                pair,
                ratio,
            } => (
                format!(
                    "{}, in a workspace's own colour at {degree}°",
                    laid(pair.front, pair.back, [pair.under, pair.beneath])
                ),
                Some(pair.floor),
                None,
                Some(*ratio),
            ),
            Finding::Ansi {
                tint,
                token,
                ratio,
                floor,
            } => (
                format!("{token} on terminal.background"),
                Some(*floor),
                *tint,
                Some(*ratio),
            ),
            Finding::SelectedNotApart { tint, other } => (
                format!("list.selected is drawn exactly as {other}"),
                None,
                *tint,
                None,
            ),
            Finding::CardNotApart {
                tint,
                card,
                ground,
                ratio,
                floor,
            } => (
                format!("a side view's card row ({card}) on its panel ({ground})"),
                Some(*floor),
                *tint,
                Some(*ratio),
            ),
            Finding::NoticeWayNotApart {
                tint,
                variant,
                ground,
                pointed,
                distance,
                floor,
            } => (
                format!(
                    "a {variant} notice card's secondary way{} on its ground ({ground})",
                    if *pointed { " under the pointer" } else { "" }
                ),
                Some(*floor),
                *tint,
                Some(*distance),
            ),
            Finding::ScrimTooThin { tint, ratio, floor } => (
                "overlay.scrim on the window it lies over".to_owned(),
                Some(*floor),
                *tint,
                Some(*ratio),
            ),
            Finding::SelectionTextAlone => (
                "selection.text is a colour while selection.background is \"platform\", so it \
                 is never drawn"
                    .to_owned(),
                None,
                None,
                None,
            ),
            Finding::LineDoesNotFit {
                density,
                fit,
                line,
                room,
            } => (
                format!(
                    "{} × {} is a {line}px line, taller than {}'s {room}px ({})",
                    fit.size,
                    fit.line_height,
                    fit.height,
                    match density {
                        Density::Comfortable => "comfortable",
                        Density::Compact => "compact",
                    }
                ),
                None,
                None,
                None,
            ),
            Finding::StyleWithoutAFace {
                slot,
                weight,
                style,
            } => (
                format!(
                    "{slot} draws {} text at weight {weight} with no face of its own, so the \
                     browser makes it from another face",
                    match style {
                        Style::Normal => "upright",
                        Style::Italic => "italic",
                    }
                ),
                None,
                None,
                None,
            ),
            Finding::PartFallsBack { part, floor, ratio } => (
                format!("{part} is drawn as its family default: {floor}"),
                None,
                None,
                *ratio,
            ),
            Finding::TierFallsBack {
                tier,
                written,
                drawn,
                ground,
                ratio,
                floor,
                tinted,
                clears,
            } => {
                let tint = if *tinted { " in a workspace tint" } else { "" };
                let as_drawn = if drawn == written {
                    format!("{tier} is drawn as written")
                } else {
                    format!("{tier} is drawn as {drawn}, not {written}")
                };
                (
                    if *clears {
                        format!(
                            "{as_drawn}: {ratio:.2} on {ground}{tint}, under {floor}, so purlis \
                             draws the nearest colour that clears it"
                        )
                    } else {
                        format!(
                            "{as_drawn}: no colour clears every ground, so purlis holds it on the \
                             window's own surfaces; it is {ratio:.2} on {ground}{tint}, under \
                             {floor}"
                        )
                    },
                    None,
                    None,
                    None,
                )
            }
        };
        let at = match found
            .iter()
            .position(|one| one.subject == subject && one.floor == floor)
        {
            Some(at) => at,
            None => {
                found.push(Warning {
                    subject,
                    floor,
                    plain: None,
                    tints: Vec::new(),
                    ansi: match finding {
                        Finding::Ansi { token, .. } => Some(*token),
                        _ => None,
                    },
                });
                found.len() - 1
            }
        };
        match tint {
            None => found[at].plain = Some(ratio),
            Some(hue) => found[at].tints.push((hue, ratio)),
        }
    }
    found
}
