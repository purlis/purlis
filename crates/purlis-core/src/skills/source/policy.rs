// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The org's controls over skill sources** (SK-11, #2774; spec #2762 stories 51 to 53; ADR
//! 0100 §6 as amended 2026-11-09): `[skill_policy]` in the project's **shared** settings file,
//! the one a team commits and reviews. A project that holds it is an org's: what it installs and
//! updates is held to its lists and its minimum age, and every install, update and removal is
//! audited ([`Audited`]).
//!
//! ```toml
//! [skill_policy]
//! allow = [
//!   { source = "https://github.com/acme/skills" },                 # every skill it offers
//!   { source = "https://github.com/anthropics/skills", skill = "pdf" },  # this one alone
//!   { extension = "acme.registry" },                               # an extension's source
//! ]
//! block = [{ source = "https://github.com/acme/skills", skill = "yolo" }]
//! min_age_days = 7                       # the default; 0 turns the age off
//! min_age_exempt = ["<a full commit>"]   # one commit installed younger, by a reviewed change
//! ```
//!
//! - **Absent `allow`, every source not blocked is allowed**; given, only what it lists. A block
//!   wins over an allow.
//! - **The age is the commit's committer time**, the time the source's history says it was made.
//!   A source that cannot say when (an extension's revision) is too young while the age is on,
//!   unless its revision is exempt.
//! - **Only the shared file is read.** A table in this machine's file is refused, never applied
//!   or merged: one person's machine cannot loosen what the team set.
//! - **A file whose table cannot be read refuses every install and update**, failing closed,
//!   until it is fixed.

use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::Deserialize;

use super::{Origin, Pin};
use crate::settings::{LayerText, Which};

/// The table of the shared settings file the controls are in.
pub const TABLE: &str = "skill_policy";

/// The minimum age of a commit, in days, where the table does not say (spec #2762:
/// `minPackageAge` defaulting to 7 days).
pub const DEFAULT_MIN_AGE_DAYS: u64 = 7;

/// One day.
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// An entry of `allow` or `block`, as the file writes it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Written {
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    extension: Option<String>,
    #[serde(default)]
    skill: Option<String>,
}

/// The table as the file writes it, every key it may hold and no other.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WrittenPolicy {
    #[serde(default)]
    allow: Option<Vec<Written>>,
    #[serde(default)]
    block: Vec<Written>,
    #[serde(default)]
    min_age_days: Option<u64>,
    #[serde(default)]
    min_age_exempt: Vec<String>,
}

/// A source an entry names.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Named {
    Git(String),
    Extension(String),
}

/// One entry of `allow` or `block`: a source, and one skill of it where it names one.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    source: Named,
    skill: Option<String>,
}

impl Rule {
    fn of(written: Written) -> Option<Rule> {
        let source = match (written.source, written.extension) {
            (Some(url), None) if !url.trim().is_empty() => Named::Git(same_url(&url)),
            (None, Some(id)) if !id.trim().is_empty() => Named::Extension(id.trim().to_owned()),
            _ => return None,
        };
        Some(Rule {
            source,
            skill: written.skill.map(|skill| skill.trim().to_owned()),
        })
    }

    /// Whether it names `origin` as a whole.
    fn names_source(&self, origin: &Origin) -> bool {
        match (&self.source, origin) {
            (Named::Git(url), Origin::Git(remote)) => *url == same_url(&remote.url),
            (Named::Extension(id), Origin::Extension(of)) => id == of,
            _ => false,
        }
    }

    /// Whether it covers the skill `skill` of `origin`: the whole source, or that skill of it.
    fn covers(&self, origin: &Origin, skill: &str) -> bool {
        self.names_source(origin) && self.skill.as_deref().is_none_or(|one| one == skill)
    }
}

/// `url` as two spellings of one repository compare: no trailing `/` or `.git`, no case in the
/// scheme and host.
fn same_url(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    match url.split_once("://") {
        Some((scheme, rest)) => {
            let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
            format!(
                "{}://{}/{path}",
                scheme.to_ascii_lowercase(),
                host.to_ascii_lowercase()
            )
        }
        None => url.to_owned(),
    }
}

/// **The org's controls**, as the shared settings file holds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    allow: Option<Vec<Rule>>,
    block: Vec<Rule>,
    min_age: Duration,
    exempt: Vec<String>,
}

/// The sentence a table that cannot be read refuses with.
fn unreadable(plane: &Path) -> String {
    format!(
        "this project's [{TABLE}] in {} could not be read, so nothing is installed or updated \
         until it is fixed",
        Which::Shared.file_at(plane)
    )
}

/// **The controls of the project at `plane`**: `None` where its shared settings file holds no
/// `[skill_policy]`, and why where it holds one that cannot be read, or this machine's file
/// holds one (which only the shared file may).
pub fn of(plane: &Path) -> Result<Option<Policy>, String> {
    if crate::settings::local_layer_holding(plane, TABLE) != LayerText::Nothing {
        return Err(format!(
            "[{TABLE}] is the team's to set, in {}, and this machine's {} holds one too: it is \
             never applied, so nothing is installed or updated until it is taken out",
            Which::Shared.file_at(plane),
            Which::Local.file_at(plane)
        ));
    }
    let LayerText::Text(text) = crate::settings::layer_text(plane, Which::Shared) else {
        return Ok(None);
    };
    let Ok(doc) = toml::from_str::<toml::Table>(&text) else {
        // A file that does not parse holds no table purlis can see; its other readers say so.
        return Ok(None);
    };
    let Some(table) = doc.get(TABLE) else {
        return Ok(None);
    };
    let written: WrittenPolicy = table.clone().try_into().map_err(|_| unreadable(plane))?;
    let rules = |list: Vec<Written>| -> Result<Vec<Rule>, String> {
        list.into_iter()
            .map(|one| Rule::of(one).ok_or_else(|| unreadable(plane)))
            .collect()
    };
    let exempt = written
        .min_age_exempt
        .iter()
        .map(|one| one.trim().to_ascii_lowercase())
        .collect();
    Ok(Some(Policy {
        allow: written.allow.map(rules).transpose()?,
        block: rules(written.block)?,
        min_age: DAY
            * u32::try_from(written.min_age_days.unwrap_or(DEFAULT_MIN_AGE_DAYS))
                .map_err(|_| unreadable(plane))?,
        exempt,
    }))
}

/// `n` days, as a sentence says it.
fn days_said(n: u64) -> String {
    match n {
        1 => "1 day".to_owned(),
        n => format!("{n} days"),
    }
}

impl Policy {
    /// **Whether the skill `skill` of `origin` may be installed or updated**, or why not.
    pub fn admits(&self, origin: &Origin, skill: &str) -> Result<(), String> {
        if let Some(rule) = self.block.iter().find(|rule| rule.covers(origin, skill)) {
            return Err(match rule.skill {
                Some(_) => format!("{skill} from {origin} is on this project's block list"),
                None => format!("{origin} is on this project's block list of skill sources"),
            });
        }
        match &self.allow {
            Some(allow) if !allow.iter().any(|rule| rule.covers(origin, skill)) => {
                Err(if allow.iter().any(|rule| rule.names_source(origin)) {
                    format!(
                        "{skill} is not on this project's allow list: only some skills of \
                         {origin} are"
                    )
                } else {
                    format!("{origin} is not on this project's allow list of skill sources")
                })
            }
            _ => Ok(()),
        }
    }

    /// **Whether what `pin` names is old enough** to install, made at `made` (seconds since
    /// 1970, `None` where the source cannot say), as of `now`.
    pub fn old_enough(&self, pin: &Pin, made: Option<u64>, now: SystemTime) -> Result<(), String> {
        if self.min_age.is_zero() || self.exempt.iter().any(|one| one == pin.as_str()) {
            return Ok(());
        }
        let days = self.min_age.as_secs() / DAY.as_secs();
        let Some(made) = made else {
            return Err(format!(
                "{} cannot say when it was made, and this project installs only what is at least \
                 {days} days old",
                pin.short()
            ));
        };
        let made = SystemTime::UNIX_EPOCH + Duration::from_secs(made);
        let age = now.duration_since(made).unwrap_or_default();
        if age >= self.min_age {
            return Ok(());
        }
        let wait = (self.min_age - age).as_secs().div_ceil(DAY.as_secs());
        Err(format!(
            "{} was committed {} ago, and this project installs only commits at least {days} \
             days old: it can be installed in {}",
            pin.short(),
            days_said(age.as_secs() / DAY.as_secs()),
            days_said(wait)
        ))
    }

    /// [`Self::admits`] for each of `skills`, then [`Self::old_enough`].
    pub fn check<'a>(
        &self,
        origin: &Origin,
        skills: impl IntoIterator<Item = &'a str>,
        pin: &Pin,
        made: Option<u64>,
    ) -> Result<(), String> {
        for skill in skills {
            self.admits(origin, skill)?;
        }
        self.old_enough(pin, made, SystemTime::now())
    }
}

/// **Holds what is about to be installed or updated at `plane` to its controls**, where it has
/// any: each of `skills` of `origin`, at `pin`, made when `made` says. `made` is asked only where
/// the age is in force.
pub fn hold<'a>(
    plane: &Path,
    origin: &Origin,
    skills: impl IntoIterator<Item = &'a str>,
    pin: &Pin,
    made: impl FnOnce() -> Option<u64>,
) -> Result<(), String> {
    match of(plane)? {
        None => Ok(()),
        Some(policy) => {
            let made = if policy.min_age.is_zero() {
                None
            } else {
                made()
            };
            policy.check(origin, skills, pin, made)
        }
    }
}

/// **What an org's project audits of its skill supply** (spec #2762 story 53): one install,
/// update or removal of skills installed from a source. Metadata only (ADR 0075 §4): the level,
/// the skills' names, the source as a person names it, never a folder of this machine or a URL's
/// query, and the commit or revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Audited {
    pub act: Act,
    /// The level, as a sentence names it (`project`, `workspace ops`).
    pub level: String,
    pub skills: Vec<String>,
    pub source: String,
    /// The commit or revision installed or updated to, or, for a removal, the one it was pinned
    /// to.
    pub pin: String,
}

/// The three acts audited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Install,
    Update,
    Remove,
}

/// The kinds the three are recorded as (ADR 0075 §4's registry).
pub const INSTALL_KIND: &str = "skill.install";
pub const UPDATE_KIND: &str = "skill.update";
pub const REMOVE_KIND: &str = "skill.remove";

impl Audited {
    pub fn new(
        act: Act,
        level: &crate::skills::verbs::Level,
        skills: Vec<String>,
        origin: &Origin,
        pin: &Pin,
    ) -> Audited {
        Audited {
            act,
            level: level.to_string(),
            skills,
            source: audited_source(origin),
            pin: pin.as_str().to_owned(),
        }
    }

    /// The kind it is recorded as.
    pub fn kind(&self) -> &'static str {
        match self.act {
            Act::Install => INSTALL_KIND,
            Act::Update => UPDATE_KIND,
            Act::Remove => REMOVE_KIND,
        }
    }

    /// Its body in the event log.
    pub fn body(&self) -> serde_json::Value {
        serde_json::json!({
            "level": self.level,
            "skills": self.skills,
            "source": self.source,
            "pin": self.pin,
        })
    }
}

/// `origin` as an audit names it: an `https://` source's URL without its query or fragment, an
/// extension as `extension:<id>`, and a folder of this machine by its last name alone.
fn audited_source(origin: &Origin) -> String {
    match origin {
        Origin::Git(remote) if remote.url.starts_with("https://") => remote
            .url
            .split(['?', '#'])
            .next()
            .unwrap_or_default()
            .to_owned(),
        Origin::Git(_) => format!("local:{}", origin.label()),
        Origin::Extension(id) => format!("extension:{id}"),
    }
}

/// What a change on an org's project made where nothing can audit it is refused with.
pub const NOT_AUDITED_HERE: &str = "this project audits every skill install, update and removal, \
                                    and only the purlis window writes the audit's log: make the \
                                    change in the Skill studio";

/// **The audit of removing the skill `name` at `level`**, where it was installed from a source:
/// `None` for one made here, whose removal is no part of the skill supply.
pub(in crate::skills) fn removal(
    plane: &Path,
    level: &crate::skills::verbs::Level,
    name: &str,
) -> Option<Audited> {
    let held = super::lock::read(plane, super::lock::Kept::of(level)).ok()?;
    let locked = held.lock.skills.iter().find(|one| one.is(level, name))?;
    Some(Audited::new(
        Act::Remove,
        level,
        vec![name.to_owned()],
        &locked.source,
        &locked.pin,
    ))
}
