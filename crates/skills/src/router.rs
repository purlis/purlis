// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Skill routing** (SK-15, #2778; ADR 0102): which of a chat's skills its listing shows, and
//! why each one it does not show is hidden.
//!
//! A router only hides: every skill it hides stays in the chat's [`SkillSet`], so `skill_find`,
//! `skill_load` and Use skill… reach it, and it is named in the **index skill** ([`INDEX`]), one
//! always-loaded skill that lists every hidden skill by name and one line. A skill the set leaves
//! out stays out, whatever a router says.
//!
//! **The built-in router** ([`Router::BuiltIn`]) needs no prompt. Below the chat's budget it
//! hides nothing. Above it, it hides every skill purlis hands from the plane that is not pinned,
//! and shows the index. What it never hides:
//!
//! - **a built-in skill**, which comes with purlis's own plugin on every route, whole;
//! - **a harness or repo skill**, which its harness lists itself ([`crate::Route::Own`]);
//! - **a pinned skill** (ADR 0102 §4): pins count first, and where they are over the budget
//!   they are all listed and the set says so ([`Routing::still_over`]);
//! - **a manual-only skill**, which no listing holds and so costs nothing (ADR 0102 as amended
//!   2026-10-22), and which its harness must still be handed for Use skill… to start it.
//!
//! A chat on [`crate::Route::Mcp`] lists nothing: every skill is found with `skill_find`, so
//! nothing is hidden there.

use crate::{Budget, Effective, Route, SkillSet};

/// **The index skill's name** (ADR 0102 §2): a plane skill of this name never loads
/// ([`crate::Reason::IndexName`]).
pub const INDEX: &str = "skill-index";

/// The index skill's `description`: what its harness lists it by, and what it costs.
pub const INDEX_DESCRIPTION: &str = "Lists this chat's other skills by name and one line. Use \
     it when a request may match a skill this list does not name, then load that skill with \
     purlis's skill_load tool.";

/// The most characters of a skill's description the index gives it.
pub(crate) const MOST_LINE_CHARS: usize = 120;

/// **A skill router** (ADR 0102 §1): what decides which skills a chat's listing shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Router {
    /// purlis's own (ADR 0102 §2): everything below the budget, the pins and the index above it.
    BuiltIn,
}

impl Router {
    /// The router as a sentence names it: "the built-in router".
    pub fn shown(self) -> &'static str {
        match self {
            Self::BuiltIn => "the built-in router",
        }
    }
}

/// Why a router hid a skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HiddenWhy {
    /// The chat's set is over its skill budget, and the skill is not pinned.
    OverBudget,
}

/// **One skill a router hid** from a chat's listing, and why. Its [`std::fmt::Display`] is the
/// one line a person is shown (ADR 0102 §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hidden {
    pub name: String,
    pub router: Router,
    pub why: HiddenWhy,
}

impl std::fmt::Display for Hidden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let why = match self.why {
            HiddenWhy::OverBudget => "over this chat's skill budget, so it is in the index skill",
        };
        write!(f, "hidden by {}: {why}", self.router.shown())
    }
}

/// **What is still over the budget once a router hid all it may** (ADR 0102 §4): then all of it
/// is listed, and the chat's budget says which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StillOver {
    /// What no pin decides is over by itself: the built-in skills, the harness's and the repos'
    /// own (and the index), which no router hides.
    NotHideable,
    /// Those fit, and the pins on top of them do not.
    Pins,
}

/// **What a router made of a chat's set**: which router, what it hid, and the pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routing {
    pub router: Router,
    /// Every skill it hid, by name.
    pub hidden: Vec<Hidden>,
    /// The chat's pins that name a skill of its set, by name: never hidden.
    pub pinned: Vec<String>,
    /// What is still over the budget with everything it may hide hidden, where something is.
    pub still_over: Option<StillOver>,
}

/// **The index skill of a chat** (ADR 0102 §2): the `SKILL.md` its route hands it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    pub text: String,
}

impl SkillSet {
    /// **The set, routed by the built-in router** for a chat whose budget is `budget` and whose
    /// pins are `pins` (its persona's and its workspace's, ADR 0102 §4). Nothing leaves the set:
    /// a skill it hides is still in [`SkillSet::effective`], and named by [`Self::hidden_by_router`].
    #[must_use]
    pub fn routed(mut self, budget: Budget, pins: &[String]) -> Self {
        let pins: std::collections::HashSet<&str> = pins.iter().map(String::as_str).collect();
        let pinned: Vec<String> = self
            .effective
            .iter()
            .filter(|e| pins.contains(e.skill.name.as_str()))
            .map(|e| e.skill.name.clone())
            .collect();
        let listed: Vec<(&Effective, crate::Declared)> = self
            .effective
            .iter()
            .filter(|e| e.route != Route::Mcp)
            .map(|e| (e, crate::declared(&e.skill)))
            .collect();
        let mut hidden = Vec::new();
        let mut still_over = None;
        if cost(listed.iter(), budget).over() {
            // Fixed: what no router hides whatever is pinned. Pinned: what only a pin keeps.
            let (could, fixed): (Vec<_>, Vec<_>) = listed
                .iter()
                .partition(|(e, declared)| hideable(e, declared));
            let (pinned_ones, hide): (Vec<_>, Vec<_>) = could
                .into_iter()
                .partition(|(e, _)| pins.contains(e.skill.name.as_str()));
            let index = usize::from(!hide.is_empty());
            let with_index = |mut spent: crate::Spent| {
                spent.skills += index;
                spent.chars += index * INDEX_DESCRIPTION.chars().count();
                spent
            };
            still_over = if with_index(cost(fixed.iter().copied(), budget)).over() {
                Some(StillOver::NotHideable)
            } else if with_index(cost(fixed.iter().chain(&pinned_ones).copied(), budget)).over() {
                Some(StillOver::Pins)
            } else {
                None
            };
            // In the set's order, by name, so a lookup is a search.
            hidden = hide
                .iter()
                .map(|(e, _)| Hidden {
                    name: e.skill.name.clone(),
                    router: Router::BuiltIn,
                    why: HiddenWhy::OverBudget,
                })
                .collect();
            hidden.sort_by(|a: &Hidden, b: &Hidden| a.name.cmp(&b.name));
        }
        self.routing = Some(Routing {
            router: Router::BuiltIn,
            hidden,
            pinned,
            still_over,
        });
        self
    }

    /// **Why a router hid `name` from this chat's listing**, or `None` where it is listed or no
    /// router ran.
    pub fn hidden_by_router(&self, name: &str) -> Option<&Hidden> {
        let hidden = &self.routing.as_ref()?.hidden;
        debug_assert!(
            hidden.is_sorted_by(|a, b| a.name < b.name),
            "a routing's hidden skills are sorted by name, once each"
        );
        hidden
            .binary_search_by(|one| one.name.as_str().cmp(name))
            .ok()
            .map(|at| &hidden[at])
    }

    /// Whether the chat's pins name `name`, a skill of its set.
    pub fn is_pinned(&self, name: &str) -> bool {
        self.routing
            .as_ref()
            .is_some_and(|routing| routing.pinned.iter().any(|one| one == name))
    }

    /// **The skills of the set its listing shows**: every one but those a router hid.
    pub fn listed(&self) -> impl Iterator<Item = &Effective> {
        self.effective
            .iter()
            .filter(|e| self.hidden_by_router(&e.skill.name).is_none())
    }

    /// **The chat's index skill**, where a router hid any skill: every hidden one by name and
    /// one line, the deprecated ones after the rest with their replacements, and how to load one.
    ///
    /// The model reads it, so nothing in it is hidden from a person reading it too: a name is
    /// listed only where it is a skill's name by the open format's grammar (64 characters at
    /// most), and the others are counted; a description is folded onto one line, cut at
    /// [`MOST_LINE_CHARS`], and each character a reader would not see in it (an invisible, bidi
    /// or tag character) is written as its mark ([`purlis_unicode::marked`]).
    pub fn index(&self) -> Option<Index> {
        let routing = self.routing.as_ref()?;
        if routing.hidden.is_empty() {
            return None;
        }
        let (mut hidden, odd): (Vec<&Effective>, Vec<&Effective>) = self
            .effective
            .iter()
            .filter(|e| self.hidden_by_router(&e.skill.name).is_some())
            .partition(|e| crate::change::name_problem(&e.skill.name).is_none());
        hidden.sort_by(|a, b| {
            a.deprecated
                .is_some()
                .cmp(&b.deprecated.is_some())
                .then(a.skill.name.cmp(&b.skill.name))
        });
        let mut text = format!(
            "---\nname: {INDEX}\ndescription: {INDEX_DESCRIPTION}\n---\n\n# This chat's other \
             skills\n\nThis chat has more skills than its list holds, so these are not in it. \
             When a request matches one, load it with purlis's `skill_load` tool by its name, \
             then follow what it says. `skill_find` finds one by what it does.\n\n"
        );
        for e in hidden {
            text.push_str(&format!(
                "- `{}`: {}",
                e.skill.name,
                one_line(&e.skill.description)
            ));
            if let Some(deprecated) = &e.deprecated {
                match deprecated
                    .replacement
                    .as_deref()
                    .filter(|instead| crate::change::name_problem(instead).is_none())
                {
                    Some(instead) => {
                        text.push_str(&format!(" (deprecated: use `{instead}` instead)"));
                    }
                    None => text.push_str(" (deprecated)"),
                }
            }
            text.push('\n');
        }
        match odd.len() {
            0 => {}
            1 => text.push_str(
                "\n1 more skill has a name that is not a skill name, so it is not listed here: \
                 `skill_find` finds it.\n",
            ),
            more => text.push_str(&format!(
                "\n{more} more skills have names that are not skill names, so they are not \
                 listed here: `skill_find` finds them.\n"
            )),
        }
        Some(Index { text })
    }
}

/// What `listed`, each with what it declares, spends of `budget`.
fn cost<'a>(
    listed: impl Iterator<Item = &'a (&'a Effective, crate::Declared)>,
    budget: Budget,
) -> crate::Spent {
    crate::budget::spent(listed.map(|(e, declared)| (&e.skill, declared)), budget)
}

/// Whether the built-in router may hide `e` where nothing pins it: a skill purlis hands from the
/// plane, by a route that lists it, that is not manual-only.
fn hideable(e: &Effective, declared: &crate::Declared) -> bool {
    e.origin.of_the_plane()
        && matches!(e.route, Route::Plugin | Route::Config | Route::Briefing)
        && !declared.manual
}

/// `description` on one line: every run of white space one space, cut at [`MOST_LINE_CHARS`]
/// with `…`, then each character a reader would not see written as its mark.
fn one_line(description: &str) -> String {
    let words: Vec<&str> = description.split_whitespace().collect();
    let line = words.join(" ");
    let cut = if line.chars().count() <= MOST_LINE_CHARS {
        line
    } else {
        let mut cut: String = line.chars().take(MOST_LINE_CHARS).collect();
        cut.truncate(cut.trim_end().len());
        cut.push('…');
        cut
    };
    purlis_unicode::marked(&cut, purlis_unicode::Breaks::Marked)
}

#[cfg(test)]
mod tests;
