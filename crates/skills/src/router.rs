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
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Router {
    /// purlis's own (ADR 0102 §2): everything below the budget, the pins and the index above it.
    BuiltIn,
    /// An extension that declares `skill-router` (ADR 0102 §5, SK-15b), by its id: asked at a
    /// chat's start which of its candidates to hide ([`SkillSet::routed_by`]).
    Extension(String),
}

impl Router {
    /// The router as a sentence names it: "the built-in router", "the router relevance".
    pub fn shown(&self) -> String {
        match self {
            Self::BuiltIn => "the built-in router".to_owned(),
            Self::Extension(id) => format!("the router {id}"),
        }
    }
}

/// **A router's own one line** for a skill it hid (ADR 0102 §7): what a person is shown after
/// "hidden by the router …". Held where it is read, never a free string: trimmed, one line of
/// at most [`Line::MOST_CHARS`] characters, with no control character and nothing a reader
/// would not see ([`purlis_unicode::hides`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct Line(String);

impl Line {
    /// The most characters of one line.
    pub const MOST_CHARS: usize = 120;

    /// `said`, trimmed, as a line, or why it is not one.
    pub fn new(said: &str) -> Result<Self, String> {
        let said = said.trim();
        if said.is_empty() {
            return Err("a reason that says nothing".to_owned());
        }
        if said.chars().count() > Self::MOST_CHARS {
            return Err(format!(
                "a reason longer than {} characters",
                Self::MOST_CHARS
            ));
        }
        if said
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
            || purlis_unicode::hides(said, purlis_unicode::Breaks::Marked)
        {
            return Err(
                "a reason on more than one line, or holding a character a reader would not see"
                    .to_owned(),
            );
        }
        Ok(Self(said.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Line {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a router hid a skill.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HiddenWhy {
    /// The chat's set is over its skill budget, and the skill is not pinned.
    OverBudget,
    /// An extension router hid it, and said why in its own line (SK-15b).
    ByRouter(Line),
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
        let why = match &self.why {
            HiddenWhy::OverBudget => "over this chat's skill budget, so it is in the index skill",
            HiddenWhy::ByRouter(line) => line.as_str(),
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
    /// Its name: [`INDEX`], or on a route that names purlis's own skills theirs `purlis-skill-index`
    /// ([`Route::built_in_name`]), so a clone's own `skill-index` is another skill beside it.
    pub name: String,
    pub text: String,
}

impl Index {
    /// **The skills an index skill's `text` names**, each with its one line, as
    /// [`SkillSet::index`] wrote them (SK-15c): what a chat's prompt hook asks a router that
    /// reads prompts about, read back from the index the chat was handed, so the hook reads no
    /// skill folder. A line that is not one of its entries is skipped, and so is a name that is
    /// not a skill's name by the open format's grammar. At most [`crate::tree::MOST_SKILLS`].
    pub fn named_in(text: &str) -> Vec<(String, String)> {
        text.lines()
            .filter_map(|line| {
                let rest = line.strip_prefix("- `")?;
                let (name, said) = rest.split_once("`: ")?;
                crate::change::name_problem(name)
                    .is_none()
                    .then(|| (name.to_owned(), said.trim().to_owned()))
            })
            .take(crate::tree::MOST_SKILLS)
            .collect()
    }
}

impl SkillSet {
    /// **The set, routed by the built-in router** for a chat whose budget is `budget` and whose
    /// pins are `pins` (its persona's and its workspace's, ADR 0102 §4). Nothing leaves the set:
    /// a skill it hides is still in [`SkillSet::effective`], and named by [`Self::hidden_by_router`].
    /// A skill the chat's harness finds itself ([`crate::Origin::Harness`]) counts against
    /// nothing (#4192).
    #[must_use]
    pub fn routed(mut self, budget: Budget, pins: &[String]) -> Self {
        let parts = Parts::of(&self, pins);
        let mut hidden = Vec::new();
        let mut still_over = None;
        if cost(parts.listed.iter(), budget).over() {
            let hide: Vec<&str> = parts.hideable().collect();
            still_over = parts.still_over(budget, !hide.is_empty());
            // In the set's order, by name, so a lookup is a search.
            hidden = hide
                .into_iter()
                .map(|name| Hidden {
                    name: name.to_owned(),
                    router: Router::BuiltIn,
                    why: HiddenWhy::OverBudget,
                })
                .collect();
            hidden.sort_by(|a: &Hidden, b: &Hidden| a.name.cmp(&b.name));
        }
        let pinned = parts.pinned;
        self.routing = Some(Routing {
            router: Router::BuiltIn,
            hidden,
            pinned,
            still_over,
        });
        self
    }

    /// **What a router extension is asked to decide for this set** (SK-15b, ADR 0102 §1): the
    /// skills it may hide, each with whether the chat pins it (a pin is never hidden), what the
    /// rest of the listing spends that no router hides, and the chat's `budget`.
    pub fn candidates(&self, budget: Budget, pins: &[String]) -> Candidates {
        let parts = Parts::of(self, pins);
        Candidates {
            skills: parts
                .could
                .iter()
                .map(|(e, _)| Candidate {
                    name: e.skill.name.clone(),
                    description: e.skill.description.clone(),
                    pinned: parts.pins.contains(e.skill.name.as_str()),
                })
                .collect(),
            besides: cost(parts.fixed(), budget),
            budget,
        }
    }

    /// **The set, routed by the extension router `router`**, which answered that `hidden` are
    /// hidden from this chat's listing, each with its own line (SK-15b, ADR 0102 §1, §4, §7), or
    /// why that answer is not one purlis takes, so the chat is routed by the built-in router
    /// instead:
    ///
    /// - it names a skill that is not one of its [`Self::candidates`] (one the set leaves out,
    ///   or one no router hides: a built-in, harness, repo or manual-only skill), or names one
    ///   twice;
    /// - it hides a pin;
    /// - it leaves the listing over `budget`, with skills it could still have hidden.
    ///
    /// Nothing leaves the set: a skill it hides is still in [`SkillSet::effective`].
    pub fn routed_by(
        mut self,
        router: Router,
        budget: Budget,
        pins: &[String],
        hidden: &[(String, Line)],
    ) -> Result<Self, String> {
        let parts = Parts::of(&self, pins);
        let mut named: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (name, _) in hidden {
            let shown = purlis_unicode::marked(name, purlis_unicode::Breaks::Marked);
            if !parts.could.iter().any(|(e, _)| e.skill.name == *name) {
                return Err(format!(
                    "it hid {shown}, which is not a skill of this chat it may hide"
                ));
            }
            if parts.pins.contains(name.as_str()) {
                return Err(format!("it hid {shown}, which this chat pins"));
            }
            if !named.insert(name.as_str()) {
                return Err(format!("it named {shown} twice"));
            }
        }
        let shown_now = parts
            .listed
            .iter()
            .filter(|(e, _)| !named.contains(e.skill.name.as_str()));
        let mut spent = cost(shown_now, budget);
        let index = usize::from(!named.is_empty());
        spent.skills += index;
        spent.chars += index * budget.line(INDEX, INDEX_DESCRIPTION.chars().count());
        let hid_all_it_may = parts.hideable().all(|name| named.contains(name));
        let still_over = if !spent.over() {
            None
        } else if hid_all_it_may {
            parts.still_over(budget, index == 1)
        } else {
            return Err(format!(
                "it left this chat's list over its skill budget ({} of {} skills, {} of {} \
                 characters) with skills it could still have hidden",
                spent.skills, budget.skills, spent.chars, budget.chars
            ));
        };
        let mut out: Vec<Hidden> = hidden
            .iter()
            .map(|(name, line)| Hidden {
                name: name.clone(),
                router: router.clone(),
                why: HiddenWhy::ByRouter(line.clone()),
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        let pinned = parts.pinned;
        self.routing = Some(Routing {
            router,
            hidden: out,
            pinned,
            still_over,
        });
        Ok(self)
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
        // The route purlis hands this set's skills by: every skill it hands has the chat's.
        let name = self
            .effective
            .iter()
            .find(|e| e.route != Route::Own)
            .map_or_else(|| INDEX.to_owned(), |e| e.route.built_in_name(INDEX));
        // Why they are not in the list: the budget, or the router a chat chose (SK-15b).
        let why = match &routing.router {
            Router::BuiltIn => {
                "This chat has more skills than its list holds, so these are not in it."
            }
            Router::Extension(_) => "This chat's router left these skills out of its list.",
        };
        let mut text = format!(
            "---\nname: {name}\ndescription: {INDEX_DESCRIPTION}\n---\n\n# This chat's other \
             skills\n\n{why} When a request matches one, load it with purlis's `skill_load` \
             tool by its name, then follow what it says. `skill_find` finds one by what it \
             does.\n\n"
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
        Some(Index { name, text })
    }
}

/// **What a router extension decides on** ([`SkillSet::candidates`]): the skills it may hide,
/// what the rest of the listing spends, and the budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidates {
    pub skills: Vec<Candidate>,
    /// What the listing spends on what no router hides (the built-in skills, the harness's and
    /// the repos' own), before any of [`Self::skills`] and the index.
    pub besides: crate::Spent,
    pub budget: Budget,
}

/// One skill a router may hide, by its name and description, and whether the chat pins it: a
/// pin is never hidden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub name: String,
    pub description: String,
    pub pinned: bool,
}

/// A set's listing, split by what a router may do with each skill.
struct Parts<'a> {
    pins: std::collections::HashSet<&'a str>,
    /// The chat's pins that name a skill of its set.
    pinned: Vec<String>,
    /// Every skill the listing would show that counts against the budget, each with what it
    /// declares: not one its harness finds itself ([`crate::Origin::Harness`]), which it lists
    /// against its own limit and counts against nothing (ADR 0102 as amended 2026-10-31, #4192).
    listed: Vec<(&'a Effective, crate::Declared)>,
    /// What a router may hide, where nothing pins it.
    could: Vec<(&'a Effective, crate::Declared)>,
}

impl<'a> Parts<'a> {
    fn of(set: &'a SkillSet, pins: &'a [String]) -> Self {
        let pins: std::collections::HashSet<&str> = pins.iter().map(String::as_str).collect();
        let pinned = set
            .effective
            .iter()
            .filter(|e| pins.contains(e.skill.name.as_str()))
            .map(|e| e.skill.name.clone())
            .collect();
        let listed: Vec<(&Effective, crate::Declared)> = set
            .effective
            .iter()
            .filter(|e| e.route != Route::Mcp && e.origin != crate::Origin::Harness)
            .map(|e| (e, crate::declared(&e.skill)))
            .collect();
        let could = listed
            .iter()
            .filter(|(e, declared)| hideable(e, declared))
            .map(|(e, declared)| (*e, declared.clone()))
            .collect();
        Self {
            pins,
            pinned,
            listed,
            could,
        }
    }

    /// What no router hides, whatever is pinned.
    fn fixed(&self) -> impl Iterator<Item = &(&'a Effective, crate::Declared)> + Clone {
        self.listed
            .iter()
            .filter(|(e, declared)| !hideable(e, declared))
    }

    /// What a router may hide that nothing pins, by name, in the set's order.
    fn hideable(&self) -> impl Iterator<Item = &str> {
        self.could
            .iter()
            .map(|(e, _)| e.skill.name.as_str())
            .filter(|name| !self.pins.contains(name))
    }

    /// What is still over `budget` with all a router may hide hidden, and the index listed where
    /// `index` says one is.
    fn still_over(&self, budget: Budget, index: bool) -> Option<StillOver> {
        let index = usize::from(index);
        let with_index = |mut spent: crate::Spent| {
            spent.skills += index;
            spent.chars += index * budget.line(INDEX, INDEX_DESCRIPTION.chars().count());
            spent
        };
        let fixed = self.fixed();
        let pinned = self
            .could
            .iter()
            .filter(|(e, _)| self.pins.contains(e.skill.name.as_str()));
        if with_index(cost(fixed.clone(), budget)).over() {
            Some(StillOver::NotHideable)
        } else if with_index(cost(fixed.chain(pinned), budget)).over() {
            Some(StillOver::Pins)
        } else {
            None
        }
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
