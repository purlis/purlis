// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`[policy.limits]`: every number purlis enforces, typed against the key table** (M75 slice
//! 12, #7021; spec §5.1).
//!
//! The table is [`super::LIMITS`] (`limits.toml`): each key with its shipped value and the
//! direction that loosens it. A key the table doesn't name makes the file that says it broken
//! (`levels` reads each file against [`spec`]).
//!
//! **The value in force is the stricter of every word said** ([`in_force`]): the shipped value, the
//! Organisation's, the project as committed and the copy the person last accepted. Raising a cap
//! or lowering a retention loosens, so it has no effect until the press: the accepted side still
//! says the old number, and the stricter wins. Tightening applies at once.
//!
//! Under `limits = log` a limit is counted, never applied ([`verdict`]). The would-have-blocked
//! line it writes is slice 13's.

use std::sync::OnceLock;

use crate::enforcement::Mode;

/// Which way a limit's number moves when it loosens (spec §5.1, review round 1, S-B8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// A bigger number loosens: a cap.
    Raised,
    /// A smaller number loosens: a retention, or a wait that protects.
    Lowered,
}

/// What a key's shipped default is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shipped {
    /// A whole number.
    Number(i64),
    /// No cap: off until someone sets it.
    Unset,
    /// Read from the device's memory, where a number said here can only tighten.
    ByMemory,
}

/// One key of the table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    /// `<family>.<kebab>`.
    pub key: String,
    /// Its shipped value.
    pub shipped: Shipped,
    /// The direction that loosens it.
    pub loosens: Direction,
}

/// Every key of the table, in the table's order.
///
/// # Panics
/// Never for the shipped table: a test reads every row.
pub fn table() -> &'static [Spec] {
    static TABLE: OnceLock<Vec<Spec>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let doc = super::LIMITS
            .parse::<toml_edit::DocumentMut>()
            .expect("the limits key table is TOML");
        doc["limit"]
            .as_array_of_tables()
            .into_iter()
            .flat_map(|tables| tables.iter())
            .map(|row| {
                let text = |name: &str| row.get(name).and_then(toml_edit::Item::as_str);
                let key = text("key").expect("a row names its key").to_owned();
                let shipped = match row.get("shipped") {
                    Some(item) if item.as_integer().is_some() => {
                        Shipped::Number(item.as_integer().expect("an integer"))
                    }
                    Some(item) if item.as_str() == Some("by-memory") => Shipped::ByMemory,
                    Some(item) if item.as_str() == Some("none") => Shipped::Unset,
                    other => panic!(
                        "limits.toml: {key}'s shipped is {other:?}, not a whole number, \"none\" or \"by-memory\""
                    ),
                };
                let loosens = match text("loosens") {
                    Some("lowered") => Direction::Lowered,
                    Some("raised") => Direction::Raised,
                    other => panic!(
                        "limits.toml: {key}'s loosens is {other:?}, not \"raised\" or \"lowered\""
                    ),
                };
                Spec {
                    key,
                    shipped,
                    loosens,
                }
            })
            .collect()
    })
}

/// The table's row for `key`, or none where the table doesn't name it.
pub fn spec(key: &str) -> Option<&'static Spec> {
    table().iter().find(|spec| spec.key == key)
}

/// The stricter of two numbers in `direction`; no number is the loosest there is.
fn stricter(direction: Direction, a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(match direction {
            Direction::Raised => a.min(b),
            Direction::Lowered => a.max(b),
        }),
        (a, b) => a.or(b),
    }
}

/// The stricter of two numbers said for `spec`, in its declared direction.
pub(super) fn stricter_number(spec: &Spec, a: i64, b: i64) -> i64 {
    stricter(spec.loosens, Some(a), Some(b)).expect("two numbers")
}

/// The strictest number a key can have in its direction: what a side that does not read says,
/// since it counts as the strictest policy there is, never as none.
pub fn strictest(spec: &Spec) -> i64 {
    match spec.loosens {
        Direction::Raised => 0,
        Direction::Lowered => i64::MAX,
    }
}

/// **The value in force for `spec`**: the stricter, in its declared direction, of the
/// Organisation's number (where it says one) and each of `sides`, the numbers the project says as
/// committed, as the person last accepted it and as each version of the accepted floor held it.
/// A side that is silent says the shipped value, so a cap raised only on the committed side stays
/// at what the accepted side says (the shipped value until the first press). `None` is no cap
/// (or, for [`Shipped::ByMemory`], the device's own).
pub fn in_force(
    spec: &Spec,
    organisation: Option<i64>,
    sides: impl IntoIterator<Item = Option<i64>>,
) -> Option<i64> {
    let shipped = match spec.shipped {
        Shipped::Number(n) => Some(n),
        Shipped::Unset | Shipped::ByMemory => None,
    };
    organisation
        .map(Some)
        .into_iter()
        .chain(sides.into_iter().map(|said| said.or(shipped)))
        .reduce(|a, b| stricter(spec.loosens, a, b))
        .unwrap_or(shipped)
}

/// What a limit does to the thing that met it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Within the cap, or the family is off: nothing happens.
    Within,
    /// Past the cap under `enforce`: refuse.
    Refuse,
    /// Past the cap under `log`: count it and let it through.
    Counted,
}

impl Verdict {
    /// The stricter of two verdicts: refusing beats counting, which beats nothing.
    pub fn stricter(self, other: Verdict) -> Verdict {
        let rank = |v: Verdict| match v {
            Verdict::Within => 0,
            Verdict::Counted => 1,
            Verdict::Refuse => 2,
        };
        if rank(other) > rank(self) {
            other
        } else {
            self
        }
    }
}

/// **What `used` does against the cap `cap`** under the `limits` family's `mode` (Q308b). Under
/// `log` an exceeded cap is counted, not applied; `off` neither. A cap of `None` is never met.
pub fn verdict(mode: Mode, cap: Option<i64>, used: i64) -> Verdict {
    if !cap.is_some_and(|cap| used > cap) {
        return Verdict::Within;
    }
    match mode {
        Mode::Off => Verdict::Within,
        Mode::Log => Verdict::Counted,
        Mode::Enforce => Verdict::Refuse,
    }
}

#[cfg(test)]
#[path = "limits_tests.rs"]
mod tests;
