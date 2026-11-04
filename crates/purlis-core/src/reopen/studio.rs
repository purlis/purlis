// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the Skill studio started a chat for** (#5185): the skill, its level and the job, kept
//! on the chat's own record so the link outlives a relaunch. The chat's header names it, and the
//! skill's page says the chat while it runs.
//!
//! **The window's state, kept for it.** Nothing starts, ends or allows anything by it: a value
//! off the file that does not read whole is no link, and the chat comes back as any other.

use crate::skills::verbs::{Level, Whose};

/// What the studio asked a chat for, of which skill at which level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioJob {
    /// The skill, by its name: `None` for a new skill's draft, which has none until it is written.
    pub skill: Option<String>,
    /// Where the skill lives, and so where the chat started (#5184).
    pub level: Level,
    pub job: StudioWork,
}

/// The studio's jobs a chat is started for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudioWork {
    /// Ask a chat to draft a skill, and New skill…'s Ask a chat to draft it.
    Draft,
    /// Ask a chat to write a skill's trigger tests.
    TriggerTests,
    /// Ask a chat to write a skill's outcome evals.
    OutcomeEvals,
    /// Ask a chat to write both kinds of a skill's checks.
    Checks,
    /// Improve from runs.
    Improve,
    /// Ask a chat to redo a draft whose chat is closed.
    Redo,
}

impl StudioWork {
    const WORDS: [(StudioWork, &'static str); 6] = [
        (StudioWork::Draft, "draft"),
        (StudioWork::TriggerTests, "trigger-tests"),
        (StudioWork::OutcomeEvals, "outcome-evals"),
        (StudioWork::Checks, "checks"),
        (StudioWork::Improve, "improve"),
        (StudioWork::Redo, "redo"),
    ];

    /// The word the record and the window name it by.
    pub fn word(self) -> &'static str {
        Self::WORDS
            .iter()
            .find(|(work, _)| *work == self)
            .map_or("draft", |(_, word)| word)
    }

    /// The job `word` names, or `None` for a word that names none.
    pub fn read(word: &str) -> Option<StudioWork> {
        Self::WORDS
            .iter()
            .find(|(_, said)| *said == word)
            .map(|(work, _)| *work)
    }
}

/// [`StudioJob`] as `reopen.json` keeps it: `{"job", "skill", "at", "name", "local"}`.
#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct StudioOnDisk {
    job: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    skill: String,
    /// `project`, `workspace` or `persona`.
    at: String,
    /// The workspace's or the persona's name; absent for the project.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    name: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    local: bool,
}

impl StudioOnDisk {
    pub(super) fn of(job: &StudioJob) -> StudioOnDisk {
        let (at, name) = match &job.level.at {
            Whose::Project => ("project", ""),
            Whose::Workspace(name) => ("workspace", name.as_str()),
            Whose::Persona(name) => ("persona", name.as_str()),
        };
        StudioOnDisk {
            job: job.job.word().to_owned(),
            skill: job.skill.clone().unwrap_or_default(),
            at: at.to_owned(),
            name: name.to_owned(),
            local: job.level.local,
        }
    }

    /// The link it names, or `None` where any part of it is not one the studio could have made.
    pub(super) fn sound(self) -> Option<StudioJob> {
        let job = StudioWork::read(&self.job)?;
        let skill = match self.skill.as_str() {
            "" => None,
            name if purlis_skills::change::name_problem(name).is_none() => Some(self.skill),
            _ => return None,
        };
        let level = match self.at.as_str() {
            "project" => Level::project(),
            "workspace" if crate::contain::workspace_name_ok(&self.name) => {
                Level::workspace(&self.name)
            }
            "persona" if crate::personas::valid_name(&self.name) => Level::persona(&self.name),
            _ => return None,
        };
        Some(StudioJob {
            skill,
            level: level.local(self.local),
            job,
        })
    }
}
