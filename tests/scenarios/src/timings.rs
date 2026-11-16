// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Timings expressions and the durations they name.** A cell's `wait`, `within`, a press's
//! `delay` and a `within` mark's observed range are written in Timings units: `hold+1`,
//! `2*group`, `500ms`, `throttle_window-5s`. A bare number is seconds.
//!
//! **Which names exist, and which pairs are `production_only`, is purlis-core's `Timings`
//! (T1.1-01)**: [`catalogue`] answers from it ([`CoreTimings`]), so a cell names only what core
//! declares, at the values core ships.

use std::collections::{BTreeMap, BTreeSet};

use purlis_core::sandbox::timings::{FIELDS, OUT_OF_PROCESS, PAIRS, Timings, id_of};

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

/// **purlis-core's `Timings` as the cells format reads it**: each field of
/// `Timings::production()` by its name (`throttle_per_chat` a count), each out-of-process duration
/// by its identifier in `OUT_OF_PROCESS`, and the pairs `PAIRS` marks `production_only`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CoreTimings;

impl TimingsCatalogue for CoreTimings {
    fn production_seconds(&self) -> BTreeMap<&'static str, f64> {
        let production = Timings::production();
        FIELDS
            .iter()
            .map(|one| {
                let secs = production
                    .duration(one.field)
                    .map_or(production.throttle_per_chat as f64, |at| at.as_secs_f64());
                (one.field, secs)
            })
            .chain(
                OUT_OF_PROCESS
                    .iter()
                    .map(|(id, _, at)| (*id, at.as_secs_f64())),
            )
            .collect()
    }

    fn unscaled(&self) -> BTreeSet<&'static str> {
        let production = Timings::production();
        FIELDS
            .iter()
            .map(|one| one.field)
            .filter(|field| production.duration(field).is_none())
            .chain(OUT_OF_PROCESS.iter().map(|(id, _, _)| *id))
            .collect()
    }

    fn production_only_pairs(&self) -> Vec<(&'static str, &'static str)> {
        PAIRS
            .iter()
            .filter(|pair| pair.production_only)
            .map(|pair| (id_of(pair.left), id_of(pair.right)))
            .collect()
    }
}

/// The catalogue the validator reads: purlis-core's `Timings`.
pub fn catalogue() -> Box<dyn TimingsCatalogue> {
    Box::new(CoreTimings)
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
