// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's skill budget** (SK-12, #2775; decision 19, ADR 0102 §1): how many skills, and how
//! many characters of their descriptions, one chat's listing may hold on its harness, and how
//! much of it the chat's skill set spends.
//!
//! The budget's numbers, and what the harness lists of each skill, are each harness's, which its
//! adapter says; this module only counts. A skill in the set costs one skill and its
//! description's characters, up to the harness's cap on one description, and its `SKILL.md`'s
//! path where the harness lists that too. Where the harness's own limit counts whole lines
//! ([`Budget::per_line`]), it also costs its name, its `when_to_use` and the line around them. A
//! **manual-only** skill ([`crate::declared`]) costs
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
    /// Characters of what is listed of them, together: their descriptions, and what else
    /// [`Self::lists_paths`] and [`Self::per_line`] say counts.
    pub chars: usize,
    /// The most characters of one description the harness lists, where it cuts them.
    pub per_description: Option<usize>,
    /// Whether the harness lists each skill's `SKILL.md` path beside its description, which
    /// then counts as its characters do.
    pub lists_paths: bool,
    /// The characters a skill's line takes around its name and description, where the harness's
    /// own limit counts whole lines. The name and any `when_to_use` then count too, the
    /// `when_to_use` inside [`Self::per_description`]. `None` where only a description counts.
    pub per_line: Option<usize>,
}

impl Budget {
    /// **This budget, with the numbers `set` writes over it**: the setting overrides the
    /// adapter's numbers one by one, and what the harness lists of a skill stays the harness's.
    pub fn with(self, set: Set) -> Self {
        Self {
            skills: set.skills.unwrap_or(self.skills),
            chars: set.chars.unwrap_or(self.chars),
            ..self
        }
    }

    /// What one line costs of [`Self::chars`], for a skill called `name` with `described`
    /// characters of description, not counting a path.
    pub(crate) fn line(&self, name: &str, described: usize) -> usize {
        let described = self
            .per_description
            .map_or(described, |cap| described.min(cap));
        match self.per_line {
            Some(around) => described + name.chars().count() + around,
            None => described,
        }
    }
}

/// **What `settings.skills.budget` writes**: either number, or both (ADR 0102 as amended
/// 2026-11-02). A number not written stays the adapter's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Set {
    pub skills: Option<usize>,
    pub chars: Option<usize>,
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
    /// **Whether the set is over its budget**: more skills or more characters than it holds,
    /// where a router hides skills behind the index (ADR 0102 §2).
    pub(crate) fn over(&self) -> bool {
        self.skills > self.budget.skills || self.chars > self.budget.chars
    }

    /// **Which part of the budget the set warns of**: the part at [`WARN_AT_PERCENT`] of its
    /// limit or past it, or `None` below both.
    pub fn warns(&self) -> Option<Over> {
        let near = |spent: usize, most: usize| {
            spent.saturating_mul(100) >= most.saturating_mul(WARN_AT_PERCENT)
        };
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
/// it and its name, `when_to_use` and line where the harness's limit counts them, for each the
/// harness lists.
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
        let joined = if budget.per_line.is_some() {
            declared.when_to_use.as_ref()
        } else {
            None
        };
        let chars =
            skill.description.chars().count() + joined.map_or(0, |more| more.chars().count());
        let path = if budget.lists_paths {
            skill.path.to_string_lossy().chars().count()
        } else {
            0
        };
        out.skills += 1;
        out.chars += budget.line(&skill.name, chars) + path;
    }
    out
}

#[cfg(test)]
mod tests;
