// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's skill budget** (SK-12, #2775; decision 19, ADR 0102 §1): how many skills, and how
//! many characters of their descriptions, one chat's listing may hold on its harness, and how
//! much of it the chat's skill set spends.
//!
//! The budget's numbers, and what the harness lists of each skill, are each harness's, which its
//! adapter says; this module only counts. A skill in the set costs one skill and its
//! description's characters, up to the harness's cap on one description, and its `SKILL.md`'s
//! path where the harness lists that too. A **manual-only** skill ([`crate::declared`]) costs
//! nothing: every harness keeps one out of its listing, by its own key or by purlis's
//! translation (SK-8, [`crate::ManualOnly`]). It is offered by Use skill… all the same.
//!
//! A set **warns** at [`WARN_AT_PERCENT`] of either limit, before the router has to hide skills
//! behind the index (ADR 0102 as amended 2026-10-21).

use crate::{Declared, Skill};

/// The share of either limit, in percent, at which a chat's set warns.
pub const WARN_AT_PERCENT: usize = 80;

/// The most one chat's listing may hold on a harness, and what the harness lists of a skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// Skills listed.
    pub skills: usize,
    /// Characters of their descriptions, together.
    pub chars: usize,
    /// The most characters of one description the harness lists, where it cuts them.
    pub per_description: Option<usize>,
    /// Whether the harness lists each skill's `SKILL.md` path beside its description, which
    /// then counts as its characters do.
    pub lists_paths: bool,
}

/// What a chat's skill set spends of its budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spent {
    /// Skills listed: every skill in the set but the manual-only ones.
    pub skills: usize,
    /// Characters of what is listed of them, each as the harness lists it.
    pub chars: usize,
    /// Manual-only skills in the set, which cost nothing.
    pub manual: usize,
    /// The budget it is spent against.
    pub budget: Budget,
}

/// Which part of the budget a set warns of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Over {
    /// Its skills are at [`WARN_AT_PERCENT`] of the most the budget lists, or past it.
    Skills,
    /// Its characters are.
    Chars,
    /// Both are.
    Both,
}

impl Spent {
    /// **Which part of the budget the set warns of**: the part at [`WARN_AT_PERCENT`] of its
    /// limit or past it, or `None` below both.
    pub fn warns(&self) -> Option<Over> {
        let near = |spent: usize, most: usize| spent * 100 >= most * WARN_AT_PERCENT;
        match (
            near(self.skills, self.budget.skills),
            near(self.chars, self.budget.chars),
        ) {
            (true, true) => Some(Over::Both),
            (true, false) => Some(Over::Skills),
            (false, true) => Some(Over::Chars),
            (false, false) => None,
        }
    }
}

/// **What `skills`, each with what it declares, spend of `budget`**: one skill and its
/// description's characters (cut at the harness's cap), with its path where the harness lists
/// it, for each the harness lists.
pub fn spent<'a>(
    skills: impl IntoIterator<Item = (&'a Skill, &'a Declared)>,
    budget: Budget,
) -> Spent {
    let mut out = Spent {
        skills: 0,
        chars: 0,
        manual: 0,
        budget,
    };
    for (skill, declared) in skills {
        if declared.manual {
            out.manual += 1;
            continue;
        }
        let chars = skill.description.chars().count();
        let path = if budget.lists_paths {
            skill.path.to_string_lossy().chars().count()
        } else {
            0
        };
        out.skills += 1;
        out.chars += budget.per_description.map_or(chars, |cap| chars.min(cap)) + path;
    }
    out
}

#[cfg(test)]
mod tests;
