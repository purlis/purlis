// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Timings expressions and the durations they name.** A cell's `wait`, `within`, a press's
//! `delay` and a `within` mark's observed range are written in Timings units: `hold+1`,
//! `2*group`, `500ms`, `throttle_window-5s`. A bare number is seconds.
//!
//! **Which names exist, and which pairs are `production_only`, is ticket 1's (T1.1-01)**, which
//! is built in parallel. Until it lands, [`catalogue`] answers from [`Ticket1Fixture`], the
//! fields, defaults and pairs as T1.1-01's ticket lists them. When `Timings` is in purlis-core,
//! [`TimingsCatalogue`] is implemented over it and [`catalogue`] returns that; nothing else here
//! changes.

use std::collections::{BTreeMap, BTreeSet};

/// **What the cells format needs to know of `Timings`.**
pub trait TimingsCatalogue {
    /// Every name a Timings expression may use, with its production value in seconds: the
    /// in-process fields and the out-of-process durations alike.
    fn production_seconds(&self) -> BTreeMap<&'static str, f64>;

    /// The names that run at real time whatever the scale (the out-of-process durations).
    fn unscaled(&self) -> BTreeSet<&'static str>;

    /// The declared pairs marked `production_only`: a tie (or order) scaling cannot keep.
    fn production_only_pairs(&self) -> Vec<(&'static str, &'static str)>;

    /// Every name a member of some `production_only` pair.
    fn production_only_members(&self) -> BTreeSet<&'static str> {
        self.production_only_pairs()
            .into_iter()
            .flat_map(|(a, b)| [a, b])
            .collect()
    }

    /// Each name's value in seconds at scale `factor` (1.0 is production).
    fn seconds_at(&self, factor: f64) -> BTreeMap<&'static str, f64> {
        let unscaled = self.unscaled();
        self.production_seconds()
            .into_iter()
            .map(|(name, secs)| {
                let secs = if unscaled.contains(name) {
                    secs
                } else {
                    secs * factor
                };
                (name, secs)
            })
            .collect()
    }
}

/// **T1.1-01's `Timings` as its ticket lists it**, standing in until the type exists.
///
/// In-process fields: `hold` 60 s, `group` 2 s, `quiet_hold` 15 min, `throttle_window` 60 s,
/// `throttle_per_chat` 10 (a count), `patience` 30 s, `teller_patience` 2 s, `head` 30 s,
/// `idle` 600 s, `tally_window` 60 s, `tally_tick` 5 s. Out of process, at real time:
/// `HOOK_TIMEOUT` 60 s, `hook_receipt` 250 ms, the Linux supervisor's `supervisor_window`
/// 60 s, and the harness's Bash timeout `bash_timeout` 120 s (Q300). Its `production_only`
/// pairs: `hold` against `HOOK_TIMEOUT`, and `throttle_window` and `hold` against
/// `supervisor_window`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ticket1Fixture;

impl TimingsCatalogue for Ticket1Fixture {
    fn production_seconds(&self) -> BTreeMap<&'static str, f64> {
        [
            ("hold", 60.0),
            ("group", 2.0),
            ("quiet_hold", 900.0),
            ("throttle_window", 60.0),
            ("throttle_per_chat", 10.0),
            ("patience", 30.0),
            ("teller_patience", 2.0),
            ("head", 30.0),
            ("idle", 600.0),
            ("tally_window", 60.0),
            ("tally_tick", 5.0),
            ("HOOK_TIMEOUT", 60.0),
            ("hook_receipt", 0.25),
            ("supervisor_window", 60.0),
            ("bash_timeout", 120.0),
        ]
        .into_iter()
        .collect()
    }

    fn unscaled(&self) -> BTreeSet<&'static str> {
        [
            "throttle_per_chat",
            "HOOK_TIMEOUT",
            "hook_receipt",
            "supervisor_window",
            "bash_timeout",
        ]
        .into_iter()
        .collect()
    }

    fn production_only_pairs(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("hold", "HOOK_TIMEOUT"),
            ("throttle_window", "supervisor_window"),
            ("hold", "supervisor_window"),
        ]
    }
}

/// The catalogue the validator reads. **Hook-up for T1.1-01:** return purlis-core's `Timings`
/// catalogue here and delete [`Ticket1Fixture`].
pub fn catalogue() -> Box<dyn TimingsCatalogue> {
    Box::new(Ticket1Fixture)
}

/// **One term of an expression**: a literal in seconds, or a name times a factor.
#[derive(Debug, Clone, PartialEq)]
enum Term {
    Seconds(f64),
    Name { factor: f64, name: String },
}

/// **A parsed Timings expression**: terms added or subtracted.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    terms: Vec<(f64, Term)>,
}

impl Expr {
    /// `text` parsed, or why not. Names are not checked here; see [`Expr::names`].
    pub fn parse(text: &str) -> Result<Self, String> {
        let squeezed: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        if squeezed.is_empty() {
            return Err("an empty Timings expression".into());
        }
        let mut terms = Vec::new();
        let mut sign = 1.0;
        let mut current = String::new();
        let mut push = |sign: f64, raw: &str| -> Result<(), String> {
            terms.push((
                sign,
                term(raw).ok_or_else(|| format!("'{raw}' in '{text}' is not a Timings term"))?,
            ));
            Ok(())
        };
        for (i, c) in squeezed.char_indices() {
            if (c == '+' || c == '-') && i > 0 {
                push(sign, &current)?;
                current.clear();
                sign = if c == '+' { 1.0 } else { -1.0 };
            } else {
                current.push(c);
            }
        }
        push(sign, &current)?;
        Ok(Expr { terms })
    }

    /// The Timings names it uses.
    pub fn names(&self) -> BTreeSet<&str> {
        self.terms
            .iter()
            .filter_map(|(_, t)| match t {
                Term::Name { name, .. } => Some(name.as_str()),
                Term::Seconds(_) => None,
            })
            .collect()
    }

    /// Its value in seconds, given each name's value; `None` for a name not given.
    pub fn seconds(&self, values: &BTreeMap<&'static str, f64>) -> Option<f64> {
        let mut total = 0.0;
        for (sign, t) in &self.terms {
            total += sign
                * match t {
                    Term::Seconds(s) => *s,
                    Term::Name { factor, name } => factor * values.get(name.as_str())?,
                };
        }
        Some(total)
    }
}

fn term(raw: &str) -> Option<Term> {
    if let Some(secs) = literal(raw) {
        return Some(Term::Seconds(secs));
    }
    let (factor, name) = match raw.split_once('*') {
        Some((n, name)) => (n.parse::<f64>().ok()?, name),
        None => (1.0, raw),
    };
    let mut chars = name.chars();
    let first = chars.next()?;
    (first.is_ascii_alphabetic() && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')).then(
        || Term::Name {
            factor,
            name: name.to_owned(),
        },
    )
}

/// `500ms`, `2s`, `15m` or a bare number of seconds.
fn literal(raw: &str) -> Option<f64> {
    let (number, scale) = if let Some(n) = raw.strip_suffix("ms") {
        (n, 0.001)
    } else if let Some(n) = raw.strip_suffix('s') {
        (n, 1.0)
    } else if let Some(n) = raw.strip_suffix('m') {
        (n, 60.0)
    } else {
        (raw, 1.0)
    };
    let first = number.chars().next()?;
    if !first.is_ascii_digit() {
        return None;
    }
    number.parse::<f64>().ok().map(|n| n * scale)
}
