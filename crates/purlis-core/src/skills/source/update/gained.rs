// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What an update newly lets a skill do** (#5354, operator's ruling of 2026-11-05): a review of
//! several updates ticks a plain edit of a skill's text, and leaves an update that gains any of
//! these unticked, to be opened and ticked on its own. Accepting such an update names each kind it
//! gains, and [`super::set::update_set`] refuses one that gains a kind it was not accepted with.
//!
//! Worked out from the files alone, as the summary is ([`purlis_skills::review`]): the pinned
//! commit's files against the newer commit's, and every skill the update brings along read whole,
//! since it is new here.

use purlis_skills::compat::Field;
use purlis_skills::review::{self as summary, Risk, Summary};
use serde::{Deserialize, Serialize};

use super::super::fetch::SourceFile;
use super::super::review::{SkillReview, summary_of};

/// **One kind of thing an update newly lets a skill do**, beyond an edit of its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Gained {
    /// A script it carries is new, or its bytes changed: what runs is not what was reviewed.
    Script,
    /// A `` !`cmd` `` line, which some harnesses run before the model reads the skill.
    Command,
    /// A `hooks` key in its frontmatter, new or changed: commands on the chat's events.
    Hook,
    /// An `allowed-tools` key, new or changed: tools pre-approved while it runs.
    Tool,
    /// A host a script or a `!` line reaches, or a script that reaches the network.
    Network,
    /// A write a script or a `!` line names outside the workspace.
    WritesOutside,
    /// A line holding a character no reader sees.
    Hidden,
    /// A file that is not text, so the review cannot show it.
    NotShown,
    /// A `SKILL.md` whose frontmatter does not read, where it read before: what it asks of a
    /// chat cannot be told, so it is never taken for a plain edit.
    Unread,
}

impl Gained {
    /// What it is, as the end of a sentence: "a script".
    pub fn said(self) -> &'static str {
        match self {
            Self::Script => "a script or a change to one",
            Self::Command => "a command it runs as it loads",
            Self::Hook => "a hook",
            Self::Tool => "a tool it pre-approves",
            Self::Network => "network access",
            Self::WritesOutside => "a write outside the workspace",
            Self::Hidden => "characters no reader sees",
            Self::NotShown => "a file the review can't show",
            Self::Unread => "a frontmatter purlis can't read",
        }
    }
}

/// A skill's frontmatter, as its `SKILL.md` gives it.
enum Front {
    /// No `SKILL.md`.
    Absent,
    /// One whose frontmatter does not read as purlis reads it.
    Unread,
    Parsed(purlis_skills::Frontmatter),
}

impl Front {
    fn of_text(text: Option<&str>) -> Front {
        match text.map(purlis_skills::handed::parse) {
            None => Front::Unread,
            Some(Ok(read)) => Front::Parsed(read),
            Some(Err(_)) => Front::Unread,
        }
    }

    fn of(files: &[SourceFile]) -> Front {
        match files.iter().find(|f| f.parts == [purlis_skills::FILE]) {
            None => Front::Absent,
            Some(file) => Front::of_text(std::str::from_utf8(&file.bytes).ok()),
        }
    }

    fn parsed(&self) -> Option<&purlis_skills::Frontmatter> {
        match self {
            Front::Parsed(read) => Some(read),
            Front::Absent | Front::Unread => None,
        }
    }
}

/// What a skill's frontmatter gains, `now` against `was` (against nothing for a skill new here):
/// each widening key added or changed, and a frontmatter that no longer reads.
fn widening(now: &Front, was: Option<&Front>) -> Vec<Gained> {
    let now = match now {
        Front::Absent => return Vec::new(),
        Front::Unread if was.is_some_and(|was| matches!(was, Front::Unread)) => return Vec::new(),
        Front::Unread => return vec![Gained::Unread],
        Front::Parsed(now) => now,
    };
    now.widens_beyond(was.and_then(Front::parsed))
        .into_iter()
        .filter_map(|field| match field {
            Field::Hooks => Some(Gained::Hook),
            Field::AllowedTools => Some(Gained::Tool),
            _ => None,
        })
        .collect()
}

/// The kinds `added`, what a summary gained, holds.
fn of_summary(added: &Summary) -> Vec<Gained> {
    let mut out = Vec::new();
    if !added.scripts.is_empty() {
        out.push(Gained::Script);
    }
    if !added.commands.is_empty() {
        out.push(Gained::Command);
    }
    if added.hosts.iter().any(|host| host.risky)
        || added
            .scripts
            .iter()
            .any(|script| script.risks.contains(&Risk::Network))
    {
        out.push(Gained::Network);
    }
    if !added.outside.is_empty() {
        out.push(Gained::WritesOutside);
    }
    if !added.unseen.is_empty() {
        out.push(Gained::Hidden);
    }
    if !added.not_text.is_empty() {
        out.push(Gained::NotShown);
    }
    out
}

/// **What updating a skill from `was` to `now` gains**, with the skills it brings along, each
/// kind once and in order. A script whose bytes changed counts as one gained, though its summary
/// line reads the same.
pub(in crate::skills::source) fn gained(
    was: &[SourceFile],
    now: &[SourceFile],
    brought: &[SkillReview],
) -> Vec<Gained> {
    let now_summary = summary_of(now);
    let changes = summary::changes(&summary_of(was), &now_summary);
    let mut out = of_summary(&changes.added);
    let script_changed = now_summary.scripts.iter().any(|script| {
        was.iter()
            .find(|old| old.path() == script.path)
            .is_some_and(|old| {
                now.iter().any(|file| {
                    file.path() == script.path
                        && (file.bytes != old.bytes || file.executable != old.executable)
                })
            })
    });
    if script_changed {
        out.push(Gained::Script);
    }
    out.extend(widening(&Front::of(now), Some(&Front::of(was))));
    for skill in brought {
        out.extend(of_summary(&skill.summary));
        let read = match skill
            .files
            .iter()
            .find(|file| file.path == purlis_skills::FILE)
        {
            None => Front::Absent,
            Some(file) => Front::of_text(file.text.as_deref()),
        };
        out.extend(widening(&read, None));
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests;
