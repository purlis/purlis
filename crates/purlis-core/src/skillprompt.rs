// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A prompt purlis fills in to run one skill** (#5186; ADR 0061 as amended 2026-11-05): the
//! neutral "start this skill with these words", which the chat's harness adapter renders in its
//! own syntax, and the sentence that names the skill where that harness has no start that reaches
//! it. What this module says is typed and never sent, as every curation prompt is.
//!
//! **Which of the two a chat gets is the adapter's word**
//! ([`crate::harness::HarnessAdapter::own_start_reaches`]), and a harness purlis has no adapter
//! for, a project's declared one, gets the sentence.
//!
//! **A direct start holds its run** ([`SkillPrompt::run_held`]). The harness expands it into the
//! skill with no call to its Skill tool, so no tool call would ever open the run.

use purlis_skills::runs::{Level, StartedBy};

use crate::harness::{Harness, SkillToStart};
use crate::skills::{Given, Origin};
use crate::skillspan::{PromptMarks, PromptSalt, Typed};

/// One skill to start and what it is asked to do, on any harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillPrompt {
    /// The skill's own name, with no harness's namespace.
    pub skill: String,
    /// Where the chat's set takes it from, which says how its harness names it.
    pub origin: Origin,
    /// What follows the skill's name in a direct start: its arguments.
    pub words: String,
    /// The whole prompt where the harness has no direct start.
    pub sentence: String,
    /// Whether the skill can be started directly at all: false for a draft Try it found cannot
    /// (`drafts::trying`), which gets the sentence on every harness.
    pub direct: bool,
}

impl SkillPrompt {
    /// Skill `skill` of purlis's own, asked `words`, said as `sentence` where it cannot be started.
    pub fn built_in(skill: &str, words: String, sentence: String) -> Self {
        Self {
            skill: skill.to_owned(),
            origin: Origin::BuiltIn,
            words,
            sentence,
            direct: true,
        }
    }

    /// What starts it on `harness`, where `harness` has a start that reaches it.
    pub fn direct_on(&self, harness: Option<Harness>) -> Option<String> {
        let harness = harness.filter(|_| self.direct)?;
        harness.adapter().own_start_reaches(self.origin).then(|| {
            harness.skill_start_text(&SkillToStart {
                name: &self.skill,
                origin: self.origin,
                given: &Given::Raw(self.words.clone()),
            })
        })
    }

    /// **The prompt typed into a chat on `harness`**: its direct start, or the sentence.
    pub fn typed_on(&self, harness: Option<Harness>) -> String {
        self.direct_on(harness)
            .unwrap_or_else(|| self.sentence.clone())
    }

    /// **The run a direct start on `harness` opens once it is sent**, `by` saying what asked for
    /// it, under the chat's `salt`, or `None` where `harness` has no direct start: there the chat
    /// starts the skill its own way, and that way opens the run.
    pub fn run_held(
        &self,
        harness: Option<Harness>,
        salt: &PromptSalt,
        workspace: Option<String>,
        by: StartedBy,
    ) -> Option<Typed> {
        let text = self.direct_on(harness)?;
        let harness = harness?;
        Some(Typed {
            skill: harness.skill_name(&self.skill, self.origin),
            run: ulid::Ulid::generate(),
            workspace,
            level: Some(Level {
                origin: self.origin,
                local: false,
            }),
            text: PromptMarks::lead(&text, salt),
            confirmed: harness.typed_start(),
            started_by: by,
            waits: true,
        })
    }
}

#[cfg(test)]
mod tests;
