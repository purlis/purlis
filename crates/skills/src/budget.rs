// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's skill budget** (SK-12, #2775; decision 19, ADR 0102 §1): how many skills, and how
//! many characters of their descriptions, one chat's listing may hold on its harness, and how
//! much of it the chat's skill set spends.
//!
//! The budget's numbers are each harness's, which its adapter says; this module only counts. A
//! skill in the set costs one skill and its description's characters, up to the harness's cap on
//! one description. A **manual-only** skill ([`crate::declared`]) costs nothing: its description
//! stays out of the listing, and no harness picks it by itself. It is still offered by Use skill….

use crate::{Declared, Skill, SkillSet};

/// The most one chat's listing may hold on a harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// Skills listed.
    pub skills: usize,
    /// Characters of their descriptions, together.
    pub chars: usize,
    /// The most characters of one description the harness lists, where it cuts them.
    pub per_description: Option<usize>,
}

/// What a chat's skill set spends of its budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spent {
    /// Skills listed: every skill in the set but the manual-only ones.
    pub skills: usize,
    /// Characters of their descriptions, each as the harness lists it.
    pub chars: usize,
    /// Manual-only skills in the set, which cost nothing.
    pub manual: usize,
    /// The budget it is spent against.
    pub budget: Budget,
}

/// Which part of the budget a set is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Over {
    /// More skills than the budget lists.
    Skills,
    /// More description characters than it holds.
    Chars,
    /// Both.
    Both,
}

impl Spent {
    /// Which part of the budget the set is over, or `None` within it: what the chat warns of.
    pub fn over(&self) -> Option<Over> {
        match (
            self.skills > self.budget.skills,
            self.chars > self.budget.chars,
        ) {
            (true, true) => Some(Over::Both),
            (true, false) => Some(Over::Skills),
            (false, true) => Some(Over::Chars),
            (false, false) => None,
        }
    }
}

/// **What `skills`, each with what it declares, spend of `budget`**: one skill and its
/// description's characters (cut at the harness's cap) for each that is not manual-only.
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
        out.skills += 1;
        out.chars += budget.per_description.map_or(chars, |cap| chars.min(cap));
    }
    out
}

/// [`spent`] for every skill a chat's `set` gets, each read for what it declares.
pub fn spent_by(set: &SkillSet, budget: Budget) -> Spent {
    let declared: Vec<Declared> = set
        .effective
        .iter()
        .map(|effective| crate::declared(&effective.skill))
        .collect();
    spent(
        set.effective
            .iter()
            .map(|effective| &effective.skill)
            .zip(&declared),
        budget,
    )
}

#[cfg(test)]
mod tests;
