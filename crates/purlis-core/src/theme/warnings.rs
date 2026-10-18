// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The theme check's warnings as a person reads them** (#2321, #2331): one line per pair, the
//! theme as it is first and the workspace tints after, each combination of appearance and
//! option values named when there is more than one. `purlis theme check` prints these lines and
//! Settings › Theme shows them on a theme's card, so the two say the same thing in the same
//! words. A chat reads the findings as the engine reports them (`purlis theme check --json`).

use super::check::Finding;
use super::{Checked, PALETTE, Theme};
use crate::shown;

/// **Every warning the check found in `checked`**, each a line: a combination is named at the
/// front of its lines only when the theme has more than one to tell apart.
pub fn said(theme: &Theme, checked: &[Checked]) -> Vec<String> {
    let named = checked.len() > 1;
    checked
        .iter()
        .flat_map(|one| {
            let called = combination(theme, one);
            warnings(&one.report.findings)
                .into_iter()
                .map(move |warning| {
                    if named {
                        format!("{called}: {}", warning.line())
                    } else {
                        warning.line()
                    }
                })
        })
        .collect()
}

/// What a combination is called on a warning's line: its appearance, and each option's value
/// in the theme's order when the theme has options (`dark, borders on, accent teal`).
fn combination(theme: &Theme, checked: &Checked) -> String {
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
            Finding::Contrast { tint, pair, ratio } => {
                let over = pair
                    .under
                    .map_or_else(String::new, |under| format!(" over {under}"));
                (
                    format!("{} on {}{over}", pair.front, pair.back),
                    Some(pair.floor),
                    *tint,
                    Some(*ratio),
                )
            }
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
            Finding::SelectionTextAlone => (
                "selection.text is a colour while selection.background is \"platform\", so it \
                 is never drawn"
                    .to_owned(),
                None,
                None,
                None,
            ),
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
