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
}

impl Gained {
    /// What it is, as the end of a sentence: "a script".
    pub fn said(self) -> &'static str {
        match self {
            Self::Script => "a script, or a change to one",
            Self::Command => "a command it runs as it loads",
            Self::Hook => "a hook",
            Self::Tool => "a tool it pre-approves",
            Self::Network => "network access",
            Self::WritesOutside => "a write outside the workspace",
            Self::Hidden => "a character no reader sees",
            Self::NotShown => "a file that is not text",
        }
    }
}

/// The `SKILL.md` among `files`, read: `None` where there is none, or its frontmatter does not
/// read, which purlis hands to no chat (fails closed), so it widens none.
fn frontmatter(files: &[SourceFile]) -> Option<purlis_skills::Frontmatter> {
    let file = files.iter().find(|f| f.parts == [purlis_skills::FILE])?;
    let text = std::str::from_utf8(&file.bytes).ok()?;
    purlis_skills::handed::parse(text).ok()
}

/// What a skill's widening keys gain: `now`'s against `was`'s, or all of `now`'s against none.
fn widening(
    now: Option<&purlis_skills::Frontmatter>,
    was: Option<Option<&purlis_skills::Frontmatter>>,
) -> Vec<Gained> {
    let Some(now) = now else {
        return Vec::new();
    };
    now.widens_beyond(was.flatten())
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
    let was_read = frontmatter(was);
    out.extend(widening(frontmatter(now).as_ref(), Some(was_read.as_ref())));
    for skill in brought {
        out.extend(of_summary(&skill.summary));
        let read = skill
            .files
            .iter()
            .find(|file| file.path == purlis_skills::FILE)
            .and_then(|file| file.text.as_deref())
            .and_then(|text| purlis_skills::handed::parse(text).ok());
        out.extend(widening(read.as_ref(), None));
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests;
