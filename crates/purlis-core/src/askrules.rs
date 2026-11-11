// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Ask less** (ADR 0121 decision 7, #6010): the answers the person gave to requests in Asks,
//! the rules they made from an offer, and the cool-offs repeated Don't allow started.
//!
//! - **An offer.** After the same Allow to the same request twice within
//!   [`OFFER_WINDOW_SECS`], the request's next ask offers a rule ([`offer`]). What it needs to
//!   have prevented the asks is said as a [`Scope`]: this chat where every ask was its, this
//!   project where every ask was the project's, this machine otherwise. Which grants the ask may
//!   offer as a rule, and where each is written, are its source's: this module only counts.
//! - **Rules I made.** Each rule made from an offer is kept ([`Rule`]) with the grant it wrote,
//!   so it can be reviewed and undone. How many asks it saved is read, never kept
//!   ([`measures::saved_by`]).
//! - **The measures** (ADR 0121 decision 18, #6021): each project's asks this week, the median
//!   time to answer them, and the asks its rules saved ([`measures`]).
//! - **A cool-off.** A second Don't allow to the same request from the same chat within
//!   [`COOL_WITHIN_SECS`] holds that chat's same ask quietly, neither counted nor notified, for
//!   [`COOL_FOR_SECS`] after the last ([`cooling`]), until the person ends it ([`Store::ended`]).
//!
//! A request is named by its source (`Asked::request`): two asks with the same name are the
//! same request, as identical requests are one row of Asks (#6009).
//!
//! # Where and how
//!
//! `<data>/inbox/rules.json`, one JSON object for the whole machine (an offer may need every
//! project), rewritten whole under the same lock on its folder as the Inbox's updates, `0600`. A
//! write lets go of answers past the window and past [`AT_MOST_ANSWERS`], of endings past a day,
//! and of rules past [`AT_MOST_RULES`], oldest first. A file this build cannot read, or one
//! larger than [`MOST_BYTES`], reads as nothing and is written over: it holds no grant, only
//! the record of one, and the grant itself stays where its source wrote it.

use std::io;
use std::path::{Path, PathBuf};

/// The file, in the Inbox's folder under purlis's data home.
pub const FILE: &str = "rules.json";

/// **How far back the same answer counts towards an offer**: a week.
pub const OFFER_WINDOW_SECS: u64 = 7 * 24 * 60 * 60;

/// **How many times the same Allow to the same request offers a rule.**
pub const OFFER_AFTER: usize = 2;

/// **How many Don't allows from one chat to one request start a cool-off.**
pub const COOL_AFTER: usize = 2;

/// **How close together they must be**: within an hour.
pub const COOL_WITHIN_SECS: u64 = 60 * 60;

/// **How long a cool-off holds the chat's ask**, from the last Don't allow: an hour.
pub const COOL_FOR_SECS: u64 = 60 * 60;

/// How long an ending of a cool-off is kept: past every cool-off it could end.
const ENDED_KEPT_SECS: u64 = 24 * 60 * 60;

/// The most answers kept; the oldest go first. Every answer given in Asks is kept, for the
/// week's measures (#6021), so this is a busy week's, across every project.
pub const AT_MOST_ANSWERS: usize = 2000;

/// The most rules kept; the oldest go first.
pub const AT_MOST_RULES: usize = 200;

/// The most characters of a request's name, an ask's words or an answer's label.
pub const MOST_SAID: usize = 400;

/// The most characters of a tool line a rule keeps: a longer one is not kept, never cut, since a
/// cut line would match no tool to undo.
pub const MOST_LINE: usize = 2000;

/// The most bytes the file is read to.
pub const MOST_BYTES: u64 = 4 * 1024 * 1024;

/// **Who a rule is for**, narrowest first: this chat, this project, this machine.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    Chat,
    Project,
    Machine,
}

/// **One request a chat asks**: its project, by the key its records are named by
/// (`sandbox::Homes::project_key`), its chat, and the request's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    pub project: String,
    pub session: u32,
    pub request: String,
}

/// **One answer the person gave a request**, as it is kept.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Answered {
    pub project: String,
    pub session: u32,
    /// The request's name; empty where the ask names none, which offers no rule and starts no
    /// cool-off, and counts only in the week's measures (#6021).
    pub request: String,
    /// The answer's id, as its ask offered it.
    pub option: String,
    /// Whether it let the request through.
    pub allows: bool,
    /// When, in seconds since 1970.
    pub at: u64,
    /// **When the ask was raised**, in seconds since 1970, where its source knows (#6021): what
    /// its time to answer is counted from, a timed-out ask's too (#6007).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raised: Option<u64>,
}

/// **The person ended a chat's cool-off** for a request.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Ended {
    pub project: String,
    pub session: u32,
    pub request: String,
    pub at: u64,
}

/// **A rule the person made from an offer**: what it answers, where it was written, and when.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rule {
    /// What tells it from every other rule: Undo names it.
    pub id: String,
    /// When it was made, in seconds since 1970.
    pub at: u64,
    pub project: String,
    /// The project's folder, as it was opened: what its grant is undone in.
    pub root: PathBuf,
    /// The chat it was made from.
    pub session: u32,
    pub request: String,
    /// Which source's ask it answered (`sandbox-host`, `run-grant`, …).
    pub source: String,
    /// What the ask said, in one line: data, never markup.
    pub says: String,
    /// The answer it gave, in its ask's words: what the rule allows, and for whom.
    pub label: String,
    /// The answer's id.
    pub option: String,
    pub scope: Scope,
    /// What its source calls the grant it wrote, which Undo takes away; none where it was not
    /// found.
    #[serde(default)]
    pub grant: Option<String>,
    /// **The tool line an Always allow wrote** to the persona's definition, which Undo takes out
    /// (#6177); none for every other rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared: Option<Declared>,
}

/// **A tool line Always allow wrote** (ADR 0112, #6177): the persona whose tools hold it, and the
/// line exactly as written, which Undo takes out only while the tools still hold it so.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Declared {
    pub persona: String,
    pub line: String,
}

/// **Everything kept**: answers, rules and endings.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Record {
    #[serde(default)]
    pub answered: Vec<Answered>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub ended: Vec<Ended>,
}

/// **A rule an ask offers**: the Allow given each time, how many times, and the narrowest scope
/// that would have prevented every one of the asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub answered: String,
    pub times: u32,
    pub needs: Scope,
}

/// **What `asked`'s ask offers** under `record` at `now`: a rule, where the last Allow given its
/// request was given at least [`OFFER_AFTER`] times in the window since any rule was made for
/// it, and no other answer came between.
pub fn offer(record: &Record, asked: &Asked, now: u64) -> Option<Offer> {
    // A rule made for the request starts the count again: what asks after it is another need.
    let since = record
        .rules
        .iter()
        .filter(|rule| rule.request == asked.request)
        .map(|rule| rule.at)
        .max()
        .unwrap_or(0);
    let answers: Vec<&Answered> = record
        .answered
        .iter()
        .filter(|one| {
            !one.request.is_empty()
                && one.request == asked.request
                && one.at > since
                && now.saturating_sub(one.at) <= OFFER_WINDOW_SECS
        })
        .collect();
    let last = answers.iter().max_by_key(|one| one.at)?;
    if !last.allows {
        return None;
    }
    // The same answer each time, and nothing else between: two Allows that differ are no habit.
    let alike: Vec<&&Answered> = answers
        .iter()
        .filter(|one| one.option == last.option)
        .collect();
    if alike.len() < OFFER_AFTER || alike.len() != answers.len() {
        return None;
    }
    let needs = if alike
        .iter()
        .all(|one| one.project == asked.project && one.session == asked.session)
    {
        Scope::Chat
    } else if alike.iter().all(|one| one.project == asked.project) {
        Scope::Project
    } else {
        Scope::Machine
    };
    Some(Offer {
        answered: last.option.clone(),
        times: u32::try_from(alike.len()).unwrap_or(u32::MAX),
        needs,
    })
}

/// **Until when `asked`'s ask is held in a cool-off** under `record` at `now`, if it is: its
/// chat heard Don't allow to its request at least [`COOL_AFTER`] times in a row within
/// [`COOL_WITHIN_SECS`], the last less than [`COOL_FOR_SECS`] ago, and the person has not ended
/// it since.
pub fn cooling(record: &Record, asked: &Asked, now: u64) -> Option<u64> {
    let mut answers: Vec<&Answered> = record
        .answered
        .iter()
        .filter(|one| {
            !one.request.is_empty()
                && one.project == asked.project
                && one.session == asked.session
                && one.request == asked.request
        })
        .collect();
    answers.sort_by_key(|one| one.at);
    // Only the refusals since its last Allow, in a row.
    let refused: Vec<&&Answered> = answers.iter().rev().take_while(|one| !one.allows).collect();
    let last = refused.first()?.at;
    let first = refused.get(COOL_AFTER - 1)?.at;
    if last.saturating_sub(first) > COOL_WITHIN_SECS {
        return None;
    }
    let ended = record.ended.iter().any(|one| {
        one.project == asked.project
            && one.session == asked.session
            && one.request == asked.request
            && one.at >= last
    });
    let until = last.saturating_add(COOL_FOR_SECS);
    (!ended && now < until).then_some(until)
}

/// The machine's record, under a data home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// The record under the data home `data`.
    pub fn in_data(data: &Path) -> Self {
        Self {
            dir: data.join(crate::inboxupdates::DIR),
        }
    }

    /// The record under this machine's data home, where there is one purlis may write in.
    pub fn here() -> Option<Self> {
        let data = crate::datahome::root()?;
        crate::datahome::refusal(&data)
            .is_none()
            .then(|| Self::in_data(&data))
    }

    /// Its file.
    pub fn file(&self) -> PathBuf {
        self.dir.join(FILE)
    }

    /// The record as kept; nothing where it is missing, cannot be read, or is too large.
    pub fn read(&self) -> Record {
        use std::io::Read as _;
        let Ok(file) = std::fs::File::open(self.file()) else {
            return Record::default();
        };
        let mut text = Vec::new();
        if file.take(MOST_BYTES + 1).read_to_end(&mut text).is_err()
            || text.len() as u64 > MOST_BYTES
        {
            return Record::default();
        }
        serde_json::from_slice(&text).unwrap_or_default()
    }

    /// Notes that the person answered `asked`, raised at `raised` where its source knows, with
    /// `option` at `now`.
    pub fn answered(
        &self,
        asked: &Asked,
        option: &str,
        allows: bool,
        raised: Option<u64>,
        now: u64,
    ) -> io::Result<()> {
        self.change(now, |record| {
            record.answered.push(Answered {
                project: cut(&asked.project),
                session: asked.session,
                request: cut(&asked.request),
                option: cut(option),
                allows,
                at: now,
                raised: raised.filter(|raised| *raised <= now),
            });
        })
        .map(|_| ())
    }

    /// Keeps `rule`, its words held to their bounds. Answers it as kept.
    pub fn make(&self, rule: Rule) -> io::Result<Rule> {
        let rule = Rule {
            id: cut(&rule.id),
            project: cut(&rule.project),
            request: cut(&rule.request),
            source: cut(&rule.source),
            says: cut(&rule.says),
            label: cut(&rule.label),
            option: cut(&rule.option),
            grant: rule.grant.as_deref().map(cut),
            declared: rule.declared.filter(|one| {
                one.line.chars().count() <= MOST_LINE && one.persona.chars().count() <= MOST_SAID
            }),
            ..rule
        };
        let kept = rule.clone();
        self.change(rule.at, move |record| {
            record.rules.retain(|one| one.id != rule.id);
            record.rules.push(rule);
        })?;
        Ok(kept)
    }

    /// Lets go of the rule called `id`, answering it, or nothing where there was none. Its grant
    /// is its source's to take away.
    pub fn undo(&self, id: &str) -> io::Result<Option<Rule>> {
        let mut gone = None;
        self.change(0, |record| {
            if let Some(at) = record.rules.iter().position(|one| one.id == id) {
                gone = Some(record.rules.remove(at));
            }
        })?;
        Ok(gone)
    }

    /// **Ends `asked`'s cool-off** at `now`: its next ask is counted and notified again.
    pub fn ended(&self, asked: &Asked, now: u64) -> io::Result<()> {
        self.change(now, |record| {
            record.ended.push(Ended {
                project: cut(&asked.project),
                session: asked.session,
                request: cut(&asked.request),
                at: now,
            });
        })
        .map(|_| ())
    }

    /// Reads the record, lets `change` edit it, bounds it as of `now` (none where `now` is 0),
    /// and writes it back, under the lock on its folder. Answers the record as kept.
    fn change(&self, now: u64, change: impl FnOnce(&mut Record)) -> io::Result<Record> {
        crate::rewrite::create_dir_all(&self.dir)?;
        let _held = crate::rewrite::Lock::bounded(&self.dir)?;
        let mut record = self.read();
        change(&mut record);
        if now > 0 {
            record
                .answered
                .retain(|one| now.saturating_sub(one.at) <= OFFER_WINDOW_SECS);
            record
                .ended
                .retain(|one| now.saturating_sub(one.at) <= ENDED_KEPT_SECS);
        }
        let past = record.answered.len().saturating_sub(AT_MOST_ANSWERS);
        record.answered.drain(..past);
        let past = record.rules.len().saturating_sub(AT_MOST_RULES);
        record.rules.drain(..past);
        let text = serde_json::to_string(&record).map_err(io::Error::other)?;
        crate::rewrite::replace(
            &self.dir,
            &self.file(),
            text.as_bytes(),
            crate::rewrite::Mode::Private,
        )?;
        Ok(record)
    }
}

/// **The key the project at `root` is kept under here**: the digest its other records on this
/// machine are named by.
pub fn project_of(root: &Path) -> String {
    crate::sandbox::Homes::project_key(root)
}

pub mod measures;

fn cut(text: &str) -> String {
    text.chars().take(MOST_SAID).collect()
}

#[cfg(test)]
mod tests;
