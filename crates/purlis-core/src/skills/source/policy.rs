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

use serde::{Deserialize, Serialize};

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
            skill: match written.skill.map(|skill| skill.trim().to_owned()) {
                Some(skill) if skill.is_empty() => return None,
                skill => skill,
            },
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

    /// The entry as a sentence names it (#6031): `<url>`, `<skill> from <url>`, `the extension
    /// <id>`, `<skill> from the extension <id>`.
    fn shown(&self) -> String {
        let source = match &self.source {
            Named::Git(url) => url.clone(),
            Named::Extension(id) => format!("the extension {id}"),
        };
        match &self.skill {
            Some(skill) => format!("{skill} from {source}"),
            None => source,
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

/// The sentence a table that cannot be read refuses with, `why` what the reader said of it.
fn unreadable(plane: &Path, why: &str) -> String {
    format!(
        "this project's [{TABLE}] in {} could not be read ({why}), so nothing is installed or \
         updated until it is fixed",
        Which::Shared.file_at(plane)
    )
}

/// What an entry of `allow` or `block` that names no one source is refused as.
const NO_SOURCE: &str = "an entry names neither one source nor one extension, or an empty skill";

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
    let doc = match toml::from_str::<toml::Table>(&text) {
        Ok(doc) => doc,
        // A file that does not parse and may hold the table fails closed: its controls cannot
        // be told from none. One that never names it is a project without controls, and its
        // other readers say what is wrong with it.
        Err(why) if text.contains(TABLE) => {
            return Err(unreadable(plane, why.message()));
        }
        Err(_) => return Ok(None),
    };
    let Some(table) = doc.get(TABLE) else {
        return Ok(None);
    };
    let written: WrittenPolicy = table
        .clone()
        .try_into()
        .map_err(|why: toml::de::Error| unreadable(plane, why.message()))?;
    let rules = |list: Vec<Written>| -> Result<Vec<Rule>, String> {
        list.into_iter()
            .map(|one| Rule::of(one).ok_or_else(|| unreadable(plane, NO_SOURCE)))
            .collect()
    };
    let exempt = written
        .min_age_exempt
        .iter()
        .map(|one| one.trim().to_owned())
        .collect();
    Ok(Some(Policy {
        allow: written.allow.map(rules).transpose()?,
        block: rules(written.block)?,
        min_age: DAY
            * u32::try_from(written.min_age_days.unwrap_or(DEFAULT_MIN_AGE_DAYS))
                .map_err(|_| unreadable(plane, "min_age_days is too large"))?,
        exempt,
    }))
}

/// **Which of the controls refuses** (#6031): what a row's mark says in a word or two, beside the
/// sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// On the block list.
    Blocked,
    /// Not on the allow list, or only some of its source is.
    NotAllowed,
    /// Younger than the minimum age, or unable to say when it was made.
    TooYoung,
    /// The table cannot be read, so everything is refused until it is fixed.
    Unread,
}

/// **What the controls would refuse before a review** (#6031): the sentence the review and the
/// install refuse with, and, where it is refused for its age, when it is old enough.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refused {
    pub kind: Kind,
    pub why: String,
    /// When it is old enough to take, in seconds since 1970; `None` for any other refusal, or
    /// where its source cannot say when it was made.
    pub until: Option<u64>,
}

/// **Why the controls do not allow every skill of a source** (#6031): the sentence, and the
/// skills of it they do allow, none where they allow none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceRefused {
    pub kind: Kind,
    pub why: String,
    pub only: Vec<String>,
}

/// **The controls as the team set them** (#6031), each entry as a sentence names it, for Settings
/// to show read only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Shown {
    /// `None` where the table names no allow list, so every source not blocked is allowed.
    pub allow: Option<Vec<String>>,
    pub block: Vec<String>,
    pub min_age_days: u64,
    pub exempt: Vec<String>,
}

/// `names` as a sentence lists them: `a`, `a and b`, `a, b and c`.
fn listed(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// `n` days, as a sentence says it.
fn days_said(n: u64) -> String {
    crate::counts::counted(n, "day", "days")
}

impl Policy {
    /// **Whether the skill `skill` of `origin` may be installed or updated**, or why not.
    pub fn admits(&self, origin: &Origin, skill: &str) -> Result<(), String> {
        self.admitted(origin, skill).map_err(|(_, why)| why)
    }

    /// [`Self::admits`], with which list refuses it.
    fn admitted(&self, origin: &Origin, skill: &str) -> Result<(), (Kind, String)> {
        if let Some(rule) = self.block.iter().find(|rule| rule.covers(origin, skill)) {
            return Err((
                Kind::Blocked,
                match rule.skill {
                    Some(_) => format!("{skill} from {origin} is on this project's block list"),
                    None => format!("{origin} is on this project's block list of skill sources"),
                },
            ));
        }
        match &self.allow {
            Some(allow) if !allow.iter().any(|rule| rule.covers(origin, skill)) => Err((
                Kind::NotAllowed,
                if allow.iter().any(|rule| rule.names_source(origin)) {
                    format!(
                        "{skill} is not on this project's allow list: only some skills of \
                         {origin} are"
                    )
                } else {
                    format!("{origin} is not on this project's allow list of skill sources")
                },
            )),
            _ => Ok(()),
        }
    }

    /// **Whether what `pin` names is old enough** to install, made at `made` (seconds since
    /// 1970, `None` where the source cannot say), as of `now`.
    pub fn old_enough(&self, pin: &Pin, made: Option<u64>, now: SystemTime) -> Result<(), String> {
        // A commit is named in either case; a revision is the extension's own, exactly.
        let exempt = |one: &String| match pin {
            Pin::Commit(commit) => one.eq_ignore_ascii_case(commit.as_str()),
            Pin::Revision(revision) => one == revision.as_str(),
        };
        if self.min_age.is_zero() || self.exempt.iter().any(exempt) {
            return Ok(());
        }
        let days = days_said(self.min_age.as_secs() / DAY.as_secs());
        let Some(made) = made else {
            return Err(format!(
                "{} cannot say when it was made, and this project installs only what is at least \
                 {days} old",
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
             old: it can be installed in {}",
            pin.short(),
            days_said(age.as_secs() / DAY.as_secs()),
            days_said(wait)
        ))
    }

    /// **When what `pin` names, made at `made`, is old enough** (#6031), in seconds since 1970:
    /// `None` where it is already, the age is off, `pin` is exempt, or `made` is not known.
    pub fn old_enough_at(&self, pin: &Pin, made: Option<u64>, now: SystemTime) -> Option<u64> {
        self.old_enough(pin, made, now).err()?;
        made.map(|made| made + self.min_age.as_secs())
    }

    /// Whether the minimum age is in force: what is installed must say when it was made.
    pub fn ages(&self) -> bool {
        !self.min_age.is_zero()
    }

    /// **What the controls would refuse of the skill `skill` of `origin` at `pin`**, made at
    /// `made`, as of `now` (#6031): [`Self::admits`], then [`Self::old_enough`], with when it is
    /// old enough; `None` where they refuse nothing.
    pub fn refuses(
        &self,
        origin: &Origin,
        skill: &str,
        pin: &Pin,
        made: Option<u64>,
        now: SystemTime,
    ) -> Option<Refused> {
        if let Err((kind, why)) = self.admitted(origin, skill) {
            return Some(Refused {
                kind,
                why,
                until: None,
            });
        }
        let why = self.old_enough(pin, made, now).err()?;
        Some(Refused {
            kind: Kind::TooYoung,
            why,
            until: self.old_enough_at(pin, made, now),
        })
    }

    /// **Whether the controls allow every skill of `origin`** (#6031), or why not and which of
    /// them they do allow: a block of the whole source, or an allow list that does not name it,
    /// allows none; one that names some of its skills alone allows those.
    pub fn admits_source(&self, origin: &Origin) -> Result<(), SourceRefused> {
        if self
            .block
            .iter()
            .any(|rule| rule.skill.is_none() && rule.names_source(origin))
        {
            return Err(SourceRefused {
                kind: Kind::Blocked,
                why: format!("{origin} is on this project's block list of skill sources"),
                only: Vec::new(),
            });
        }
        let Some(allow) = &self.allow else {
            return Ok(());
        };
        let of: Vec<&Rule> = allow
            .iter()
            .filter(|rule| rule.names_source(origin))
            .collect();
        if of.iter().any(|rule| rule.skill.is_none()) {
            return Ok(());
        }
        let only: Vec<String> = of.iter().filter_map(|rule| rule.skill.clone()).collect();
        Err(SourceRefused {
            kind: Kind::NotAllowed,
            why: if only.is_empty() {
                format!("{origin} is not on this project's allow list of skill sources")
            } else {
                format!(
                    "{origin} has only {} on this project's allow list",
                    listed(&only)
                )
            },
            only,
        })
    }

    /// **The controls as the team set them** (#6031), for Settings to show read only.
    pub fn shown(&self) -> Shown {
        Shown {
            allow: self
                .allow
                .as_ref()
                .map(|allow| allow.iter().map(Rule::shown).collect()),
            block: self.block.iter().map(Rule::shown).collect(),
            min_age_days: self.min_age.as_secs() / DAY.as_secs(),
            exempt: self.exempt.clone(),
        }
    }

    /// **What the controls do, in one line** (#6031): what Sources says of them before anything
    /// is reviewed.
    pub fn said(&self) -> String {
        let allowed = match &self.allow {
            None => "every source not blocked is allowed".to_owned(),
            Some(allow) => format!(
                "{} allowed",
                crate::counts::counted(allow.len(), "source", "sources")
            ),
        };
        let blocked = match self.block.len() {
            0 => "nothing blocked".to_owned(),
            n => {
                let skills = self
                    .block
                    .iter()
                    .filter(|rule| rule.skill.is_some())
                    .count();
                let sources = n - skills;
                let mut parts = Vec::new();
                if sources > 0 {
                    parts.push(crate::counts::counted(sources, "source", "sources"));
                }
                if skills > 0 {
                    parts.push(crate::counts::counted(skills, "skill", "skills"));
                }
                format!("{} blocked", parts.join(" and "))
            }
        };
        let age = if self.min_age.is_zero() {
            "no minimum age".to_owned()
        } else {
            format!(
                "commits at least {} old",
                days_said(self.min_age.as_secs() / DAY.as_secs())
            )
        };
        format!("This project's controls apply: {allowed}, {blocked}, {age}.")
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

/// **What the controls `policy` (as [`of`] read them) would refuse of the skill `skill` of
/// `origin` at `pin`, made at `made`**, before a review (#6031): every one where they cannot be
/// read, as the review would refuse it; nothing where there are none.
pub fn refused_before(
    policy: &Result<Option<Policy>, String>,
    origin: &Origin,
    skill: &str,
    pin: &Pin,
    made: Option<u64>,
) -> Option<Refused> {
    match policy {
        Err(why) => Some(Refused {
            kind: Kind::Unread,
            why: why.clone(),
            until: None,
        }),
        Ok(None) => None,
        Ok(Some(policy)) => policy.refuses(origin, skill, pin, made, SystemTime::now()),
    }
}

/// [`refused_before`] of the lists alone, where when it was made could not be read here: the
/// review reads it, and says.
pub fn listed_before(
    policy: &Result<Option<Policy>, String>,
    origin: &Origin,
    skill: &str,
) -> Option<Refused> {
    match policy {
        Err(why) => Some(Refused {
            kind: Kind::Unread,
            why: why.clone(),
            until: None,
        }),
        Ok(None) => None,
        Ok(Some(policy)) => policy
            .admitted(origin, skill)
            .err()
            .map(|(kind, why)| Refused {
                kind,
                why,
                until: None,
            }),
    }
}

/// Whether the controls `policy` hold a minimum age, so what is installed must say when it was
/// made.
pub fn asks_age(policy: &Result<Option<Policy>, String>) -> bool {
    matches!(policy, Ok(Some(policy)) if policy.ages())
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

    /// Its body in the event log: who made it, the person on the scope it was `made` at (ADR
    /// 0075's actor, as the other audited acts say it), then what was changed.
    pub fn body(&self, made: MadeAt) -> serde_json::Value {
        serde_json::json!({
            "actor_kind": "human",
            "actor": "operator",
            "scope": made.scope(),
            "level": self.level,
            "skills": self.skills,
            "source": self.source,
            "pin": self.pin,
        })
    }

    /// **The audit a terminal's ask carries** (1.82, SK-11a, #6030), which the session host
    /// writes as the terminal's: who made it is never the terminal's to say.
    pub fn told(&self) -> purlis_session_protocol::session::SkillChangeAudit {
        use purlis_session_protocol::session::SkillAct;
        purlis_session_protocol::session::SkillChangeAudit {
            act: match self.act {
                Act::Install => SkillAct::Install,
                Act::Update => SkillAct::Update,
                Act::Remove => SkillAct::Remove,
            },
            level: self.level.clone(),
            skills: self.skills.clone(),
            source: self.source.clone(),
            pin: self.pin.clone(),
        }
    }

    /// The audit a terminal's ask `told` the session host.
    pub fn from_told(told: &purlis_session_protocol::session::SkillChangeAudit) -> Audited {
        use purlis_session_protocol::session::SkillAct;
        Audited {
            act: match told.act {
                SkillAct::Install => Act::Install,
                SkillAct::Update => Act::Update,
                SkillAct::Remove => Act::Remove,
            },
            level: told.level.clone(),
            skills: told.skills.clone(),
            source: told.source.clone(),
            pin: told.pin.clone(),
        }
    }
}

/// **Where an audited change was made** (ADR 0100 as amended 2026-11-10): the entry's `scope`,
/// the client scope the person made it on. Either is a person's scope, whose credential no chat's
/// sandbox reaches (session protocol, V16a), so either entry is `human`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MadeAt {
    /// The Skill studio, in the window, written through the window's `record`.
    Window,
    /// `purlis skill` at the person's terminal, written by the session host on the ask of a link
    /// it admitted as `terminal` (session protocol 1.82).
    Terminal,
}

impl MadeAt {
    /// The scope's word in an entry.
    pub fn scope(self) -> &'static str {
        match self {
            MadeAt::Window => "local-ui",
            MadeAt::Terminal => "terminal",
        }
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
                                    and nothing here can write the audit's log: make the change \
                                    in the Skill studio, or with `purlis skill` in your own \
                                    terminal";

/// What the session host refuses a terminal's skill change with on a project without controls.
pub const NO_CONTROLS: &str = "this project's shared settings hold no [skill_policy], so it \
                               audits no skill change";

/// **The audit of `act` on the skill `name` at `level`**, by the pin its lockfile keeps:
/// `None` for one made here, which is no part of the skill supply. A lockfile that cannot be
/// read is refused on an org's project, which could not tell; elsewhere nothing is audited.
pub(in crate::skills) fn of_installed(
    plane: &Path,
    act: Act,
    level: &crate::skills::verbs::Level,
    name: &str,
) -> Result<Option<Audited>, String> {
    let held = match super::lock::read(plane, super::lock::Kept::of(level)) {
        Ok(held) => held,
        Err(why) if of(plane)?.is_some() => return Err(why),
        Err(_) => return Ok(None),
    };
    Ok(held
        .lock
        .skills
        .iter()
        .find(|one| one.is(level, name))
        .map(|locked| {
            Audited::new(
                act,
                level,
                vec![name.to_owned()],
                &locked.source,
                &locked.pin,
            )
        }))
}

/// **Holds a skill coming back to `level` as `name` to the controls** where it was installed
/// from a source: its source and name to the lists, as an install's are. Its commit is the one
/// it was pinned to, already past any age, so the age is not asked again.
pub(in crate::skills) fn hold_restored(
    plane: &Path,
    level: &crate::skills::verbs::Level,
    name: &str,
) -> Result<Option<Audited>, String> {
    let Some(policy) = of(plane)? else {
        return Ok(None);
    };
    let held = super::lock::read(plane, super::lock::Kept::of(level))?;
    let Some(locked) = held.lock.skills.iter().find(|one| one.is(level, name)) else {
        return Ok(None);
    };
    policy.admits(&locked.source, locked.at_source())?;
    Ok(Some(Audited::new(
        Act::Install,
        level,
        vec![name.to_owned()],
        &locked.source,
        &locked.pin,
    )))
}
