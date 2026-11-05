// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the Skill studio started a chat for** (#5185): the skill, its level and the job, kept
//! on the chat's own record so the link outlives a relaunch. The chat's header names it, and the
//! skill's page says the chat while it runs. Once the chat closes, [`super::studio_chats`] keeps
//! it for the skill's Runs.
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
    /// **Whether the job is done**: the chat wrote its draft (#5185). Set once and never unset,
    /// so a draft the person then accepts or discards leaves the chat done, not running again.
    pub done: bool,
    /// **When the studio started the chat** (#5248), as an RFC 3339 time: what the Grades
    /// card's running job counts from. `None` in a record written before it.
    pub started: Option<String>,
    /// **When its job stopped running** (#5248): its draft came, or its program ended. Taken
    /// out again where a later start runs the chat once more and it is not [`Self::done`].
    pub finished: Option<String>,
}

impl StudioJob {
    /// The job `job` of `skill` at `level`, started `now`: not done, not finished.
    pub fn new(
        skill: Option<String>,
        level: Level,
        job: StudioWork,
        now: chrono::DateTime<chrono::Utc>,
    ) -> StudioJob {
        StudioJob {
            skill,
            level,
            job,
            done: false,
            started: Some(time(now)),
            finished: None,
        }
    }

    /// Marks its job stopped `now`, where it had not already: a second stop keeps the first's
    /// time. Whether anything changed.
    pub fn finish(&mut self, now: chrono::DateTime<chrono::Utc>) -> bool {
        if self.finished.is_some() {
            return false;
        }
        self.finished = Some(time(now));
        true
    }

    /// Takes the stop back, as a later start of the chat runs it once more, unless its draft
    /// came: a done job stays finished. Whether anything changed.
    pub fn runs_again(&mut self) -> bool {
        if self.done || self.finished.is_none() {
            return false;
        }
        self.finished = None;
        true
    }
}

/// `now` as the record keeps a time: RFC 3339, in UTC, to the second.
pub fn time(now: chrono::DateTime<chrono::Utc>) -> String {
    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Whether `said` reads as an RFC 3339 time, as the record keeps one.
pub(super) fn reads_as_time(said: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(said).is_ok()
}

/// `said` where it reads as an RFC 3339 time, and `None` for anything else.
fn a_time(said: String) -> Option<String> {
    reads_as_time(&said).then_some(said)
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

    /// Every job, in the record's order.
    pub fn all() -> impl Iterator<Item = StudioWork> {
        Self::WORDS.iter().map(|(work, _)| *work)
    }

    /// The job `word` names, or `None` for a word that names none.
    pub fn read(word: &str) -> Option<StudioWork> {
        Self::WORDS
            .iter()
            .find(|(_, said)| *said == word)
            .map(|(work, _)| *work)
    }
}

/// **A job is written as its word** ([`StudioWork::word`]), as the record and the window name it.
impl serde::Serialize for StudioWork {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.word())
    }
}

/// [`StudioJob`] as `reopen.json` keeps it: `{"job", "skill", "at", "name", "local", "done",
/// "started", "finished"}`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    done: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    started: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    finished: Option<String>,
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
            done: job.done,
            started: job.started.clone(),
            finished: job.finished.clone(),
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
            done: self.done,
            started: self.started.and_then(a_time),
            finished: self.finished.and_then(a_time),
        })
    }
}
