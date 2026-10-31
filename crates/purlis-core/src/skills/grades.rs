// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A skill's grades: the plan before one runs, and the eval record after** (SK-16, #2779;
//! spec #2762 decisions 8, 21, 22, 23 and 43; ADR 0101 §5, ADR 0105).
//!
//! - **The plan comes first** ([`planned`]): the copy's fixtures as they read, the chats a grade
//!   would run, and what the copy's last grade spent per chat, so the person sees the cost before
//!   anything runs. Nothing is estimated that was never measured: a project with no grade yet is
//!   told how many chats, and that each is one turn.
//! - **The eval record** is `personas/_grades/<YYYY-MM>.<device>.jsonl` beside the skill log,
//!   append-only and committed, one row per grade ([`Graded`]): the copy, what its folder held by
//!   content (the tree git holds of it, as a skill run's start records it, or that it held changes
//!   not committed), the commit that tree is, the harness and profile its chats ran on, and the
//!   report. So a grade is tied to the skill's version by content, wherever and whenever it ran,
//!   and a teammate who pulls the record sees it on the same version ([`history`] joins them).
//! - Results never go in the skill's folder (decision 21): its `evals/` holds fixtures alone.
//!
//! The record is read as a teammate's purlis may have written it: each file at most
//! [`MOST_FILE_BYTES`], read through no link, each line at most [`MOST_LINE_BYTES`] and parsed as
//! a row or skipped, a row of a newer version skipped, and at most [`MOST_GRADES`] answered.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use purlis_skills::grade::{self, Arm, Fixtures, GradeReport, GradeRequest, Plan, RUNS};

use super::verbs::{Level, find, rel};
use crate::skilluse::Content;

/// The directory under `personas/` the eval record lives in, beside the skill log's.
const DIR_NAME: &str = "_grades";

/// The version of a row this purlis writes and reads.
pub const VERSION: u32 = 1;

/// The most bytes of one month's file read, and of one row.
const MOST_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MOST_LINE_BYTES: usize = 1024 * 1024;

/// The most grades of one skill answered, newest first.
const MOST_GRADES: usize = 200;

/// The arms the built-in runner compares a skill with no other version in: the skill against no
/// skill.
pub const ARMS: [Arm; 2] = [Arm::With, Arm::Without];

/// The arms it compares a skill that has a previous version in (SK-57, #3237): the skill, no
/// skill, and the skill as its previous version had it, which is then the baseline.
pub const ARMS_WITH_PREVIOUS: [Arm; 3] = [Arm::With, Arm::Without, Arm::Previous];

/// The longest one grade's chat runs before purlis ends it: what the plan's "up to" says.
pub const CHAT_TIME: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// The folder of this machine's app state of the project ([`crate::appstate`]) the grades of a
/// local copy are kept in, never committed (a copy in `skills.local/`, or in a workspace that is
/// this machine's alone).
pub const LOCAL_DIR: &str = "skill-grades";

/// The most files of one record folder looked at: a folder with more is read no further.
const MOST_FILES: usize = 600;

/// The first characters a chat program reads as its own command, a flag, a shell line, a memory
/// or a file's mention, rather than as a prompt: a query that starts with one is left out of a
/// grade, never sent (the review of #3229).
const COMMAND_STARTS: [char; 5] = ['-', '/', '!', '#', '@'];

/// `personas/_grades/<YYYY-MM>.<device>.jsonl`, `<device>` named as the skill log's is.
fn path_for(root: &Path, when: chrono::DateTime<chrono::Utc>, device: &str) -> PathBuf {
    root.join("personas")
        .join(DIR_NAME)
        .join(format!("{}.{device}.jsonl", when.format("%Y-%m")))
}

/// `<data>/app-state/<clone key>/skill-grades/<YYYY-MM>.jsonl`: where a local copy's grades are
/// kept. `None` where this machine has no data home.
pub fn local_path_for(root: &Path, when: chrono::DateTime<chrono::Utc>) -> Option<PathBuf> {
    Some(
        crate::appstate::dir(root)?
            .join(LOCAL_DIR)
            .join(format!("{}.jsonl", when.format("%Y-%m"))),
    )
}

/// The level a grade of the copy at `level` is recorded by: its folder relative to the plane.
pub fn recorded_level(level: &Level) -> Option<String> {
    level.rel()
}

/// **The level a grade was recorded by** ([`recorded_level`]), read back: what a grade's
/// scorecard says the copy it graded was (#3978). `None` for a word no level's folder is.
pub fn level_of_recorded(word: &str) -> Option<Level> {
    Level::of_rel(word)
}

/// **Whether the copy at `level` is this machine's alone**: a `skills.local/` copy, or one in a
/// workspace that is this machine's alone. Its grades never go in a committed file.
pub fn is_local(root: &Path, level: &Level) -> bool {
    level.local
        || level
            .at
            .workspace()
            .is_some_and(|ws| !crate::wscmd::live_workspaces(root).contains(ws))
}

/// What a grade's chats spent in all, where their harness said.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Spent {
    /// The chats that ran.
    pub chats: u32,
    pub tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    /// **The chats a grade purlis stopped under asked for before it stopped** (#3602), ran or
    /// not: what an estimate shares its tokens and cost over, as they hold the spend of a chat
    /// ended at an ask, at a cap or on Stop too. Said by its marker
    /// ([`underway::Marker::spent`]) alone: a scored row's arms count them (`chats` +
    /// `not_run`), so it is `None` there, and in an interrupted row written before #3602.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asked_for: Option<u32>,
}

/// **One grade, as the eval record holds it.**
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Graded {
    pub v: u32,
    /// The grade's id.
    pub grade: String,
    pub skill: String,
    /// The copy's level, as its folder relative to the plane (`skills`, `workspaces/ops/skills`).
    /// A copy that is this machine's alone is recorded in this machine's state, never in the
    /// committed file ([`is_local`]).
    pub level: Option<String>,
    /// What the copy's folder held, by content: the tree's id, or `uncommitted`.
    pub tree: String,
    /// The commit whose version of the skill that tree is, where it is one.
    pub commit: Option<String>,
    /// The harness its chats ran on, and the profile.
    pub harness: Option<String>,
    pub profile: Option<String>,
    pub ts: String,
    pub runs: u8,
    pub report: GradeReport,
    pub spent: Spent,
    /// The harness's own plugins its chats were handed off, by the harness's id (P8, #3468):
    /// every one it had installed, purlis's own kept on and left out here. `None` in a row
    /// written before #3468, in one whose grade no chat of started, and in the row held while a
    /// grade is under way (so an interrupted grade's record has none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins_off: Option<Vec<String>>,
    /// **The model its chats ran on, as they reported it** (SK-60, #3240; ADR 0087 §6): the one
    /// the first chat that said its model said, by the harness's own id for it. `None` in a row
    /// written before #3240, and where no chat said ([`reported_model`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// **The version it was compared with** (SK-57, #3237): the commit whose tree its previous
    /// version's arm was laid out from, where it had one. `None` in a row written before #3237,
    /// and for a skill with no other version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
    /// **Whether a saved edit started it** (SK-58, #3238), under the opt-in, rather than a press
    /// of Grade. `false` in a row written before #3238.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub after_edit: bool,
}

impl Graded {
    /// **The version it was compared with, where it compared one** (SK-57): its commit, from a
    /// row whose report has the `previous` arm.
    pub fn compared_with(&self) -> Option<&str> {
        self.previous
            .as_deref()
            .filter(|_| self.report.arm(Arm::Previous).is_some())
    }

    /// What its copy's folder held.
    pub fn content(&self) -> Option<Content> {
        Content::parse(&self.tree)
    }

    /// **Whether a row read back says only what purlis writes**: a tree or `uncommitted`, a
    /// commit that is a whole hex id, a harness and a profile that are plain names, a stamp of a
    /// time's shape, and a spend and a report that hold together ([`report_holds_together`],
    /// #3353). A teammate's purlis writes the record, and a person reads these words in the
    /// window and the terminal, so a row that says anything else is skipped.
    fn sound(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        let hex = |word: &str| {
            matches!(word.len(), 40 | 64)
                && word
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        self.content().is_some()
            && self.commit.as_deref().is_none_or(hex)
            && self.previous.as_deref().is_none_or(hex)
            // A previous version's arm is of a version the row names.
            && (self.previous.is_some() || self.report.arm(Arm::Previous).is_none())
            && self.harness.as_deref().is_none_or(plain)
            && self.profile.as_deref().is_none_or(plain)
            && self.level.as_deref().is_none_or(level_word)
            && plain(&self.grade)
            && stamped_by(&self.ts, now)
            && (1..=MOST_RUNS).contains(&self.runs)
            && self.spent.tokens.is_none_or(tokens)
            && self.spent.cost_usd.is_none_or(dollars)
            && self.plugins_off.as_deref().is_none_or(|ids| {
                ids.len() <= MOST_PLUGINS_OFF
                    && ids.iter().all(|id| crate::harness_plugin::id_ok(id))
            })
            && self.model.as_deref().is_none_or(price::model_id)
            && report_holds_together(&self.report, self.runs)
            // A skill's arms alone: a row scored in a router's grade's arms is a router's.
            && self
                .report
                .arms
                .iter()
                .all(|arm| matches!(arm.arm, Arm::With | Arm::Without | Arm::Previous))
            && spent_is_the_reports(&self.spent, &self.report, self.runs)
    }
}

/// **The model a grade records its chats ran on** (SK-60, #3240), from `said`, what each chat
/// reported in order: the first that is a model's id as a harness names one, so the record
/// never holds words no reader could take for one.
pub fn reported_model<'a>(said: impl IntoIterator<Item = &'a str>) -> Option<String> {
    said.into_iter()
        .map(str::trim)
        .find(|model| price::model_id(model))
        .map(str::to_owned)
}

/// The most runs a row read back says each test had in each arm: past any grade's.
const MOST_RUNS: u8 = 16;

/// The most tokens a figure of a row read back says: ten times the largest cap, as a chat may
/// pass the cap a little before it is ended.
const MOST_TOKENS: u64 = 10 * cap::MOST;

/// The most dollars a figure of a row read back says: ten times the largest cap at the dearest
/// price a row may name.
const MOST_USD: f64 = 10.0 * (cap::MOST as f64) * price::MOST_READ_USD_PER_MILLION / 1_000_000.0;

/// The most seconds a row read back says a chat ran: a day, far past a grade's chat's time.
const MOST_SECONDS: f64 = 24.0 * 60.0 * 60.0;

/// How far past this machine's clock a row read back may be stamped: a teammate's clock may run
/// a little ahead, but a row stamped later than this would stay newest and push real ones out
/// of the [`MOST_GRADES`] read.
const MOST_AHEAD: chrono::TimeDelta = chrono::TimeDelta::hours(1);

/// Whether `ts` is a time's stamp, no later than [`MOST_AHEAD`] past `now`.
fn stamped_by(ts: &str, now: chrono::DateTime<chrono::Utc>) -> bool {
    chrono::DateTime::parse_from_rfc3339(ts).is_ok_and(|at| {
        now.checked_add_signed(MOST_AHEAD)
            .is_none_or(|latest| at <= latest)
    })
}

/// A level as purlis records it: a folder of a level, spelled as that level spells its own.
fn level_word(word: &str) -> bool {
    word.len() <= 256
        && Level::of_rel(word).is_some_and(|level| level.rel().as_deref() == Some(word))
}

/// A count of tokens no larger than [`MOST_TOKENS`].
fn tokens(value: u64) -> bool {
    value <= MOST_TOKENS
}

/// An amount of dollars no larger than [`MOST_USD`].
fn dollars(value: f64) -> bool {
    amount(value) && value <= MOST_USD
}

/// **Whether a row's spend `spent` is what its report `report` says it spent** (#3273): the
/// chats that ran, as the arms count them, and the tokens, as the report adds them up; and the
/// cost, where the report adds it up (a row before #3469 does not). A grade purlis stopped under
/// before it was scored has no arms, and says what its chats spent all the same (#3582): as many
/// chats as a grade of `runs` runs could have run, at most, and as many asked for (#3602), at
/// least those that ran. A scored row's arms count what it asked for, so it says no more.
fn spent_is_the_reports(spent: &Spent, report: &GradeReport, runs: u8) -> bool {
    let chats = if stopped_unscored(report) {
        let most = u64::from(runs) * (grade::MOST_TRIGGER_TESTS * ARMS_THERE_ARE) as u64;
        u64::from(spent.chats) <= most
            && spent.asked_for.is_none_or(|asked_for| {
                (u64::from(spent.chats)..=most).contains(&u64::from(asked_for))
            })
    } else {
        let ran: u64 = report.arms.iter().map(|arm| u64::from(arm.chats)).sum();
        u64::from(spent.chats) == ran && spent.asked_for.is_none()
    };
    chats
        && spent.tokens == report.spent_tokens
        && report.spent_usd.is_none_or(|said| {
            spent
                .cost_usd
                .is_some_and(|cost| (cost - said).abs() <= 1e-9 * said.max(1.0))
        })
}

/// Whether `report` is of a grade purlis stopped under before it was scored: interrupted, with
/// no arm to count its chats.
fn stopped_unscored(report: &GradeReport) -> bool {
    report.ended == grade::GradeEnd::Interrupted && report.arms.is_empty()
}

/// The most plugin ids a row read back names as off: far past any machine's installs.
const MOST_PLUGINS_OFF: usize = 256;

/// A plain name: at most 64 of letters, digits, `-`, `_` and `.`.
fn plain(word: &str) -> bool {
    !word.is_empty()
        && word.len() <= 64
        && word
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

/// An amount: a finite number, not below nothing.
fn amount(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

/// A share: a finite number from nothing to the whole.
fn share(value: f64) -> bool {
    (0.0..=1.0).contains(&value)
}

/// The longest reason a report gives why its chats stopped starting, as read back.
pub(crate) const MOST_REASON_BYTES: usize = 2 * 1024;

/// **Whether the report `report` of a grade of `runs` runs holds together** (#3353): what the
/// built-in runner could have written. Its grader is a plain name; each trigger test's query is
/// one a fixture could hold, its triggered runs at most its runs and its runs at most the
/// grade's, its rate what those two make and its pass there exactly where its rate is, each chat
/// a plain id; each arm once, its pass rate a share and its other figures amounts, every spread
/// in order; the difference between two shares; and every cost an amount. Its reason is one
/// line a reader sees whole.
pub(crate) fn report_holds_together(report: &GradeReport, runs: u8) -> bool {
    let stat = |stat: &grade::Stat, fits: fn(f64) -> bool| {
        [stat.mean, stat.min, stat.max].into_iter().all(fits)
            && amount(stat.stddev)
            && stat.min <= stat.mean
            && stat.mean <= stat.max
    };
    let tests = report.triggers.iter().all(|test| {
        let rate = match (test.runs, test.rate) {
            (0, None) => true,
            (0, Some(_)) | (_, None) => false,
            (ran, Some(rate)) => {
                share(rate) && (rate - f64::from(test.triggered) / f64::from(ran)).abs() < 1e-9
            }
        };
        !test.query.trim().is_empty()
            && test.query.len() <= grade::MOST_PROMPT_BYTES
            && test.triggered <= test.runs
            && test.runs <= u32::from(runs)
            && rate
            && test.pass.is_some() == test.rate.is_some()
            && test.chats.len() <= usize::from(runs)
            && u64::from(test.runs) + u64::from(test.asked) <= u64::from(runs)
            && test.chats.iter().flatten().all(|chat| plain(chat))
    });
    let mut arms = std::collections::HashSet::new();
    let arms_hold = report.arms.iter().all(|arm| {
        // What its tests say of this arm: the runs that ran, and every chat asked for.
        let of_arm = || report.triggers.iter().filter(|test| test.arm == arm.arm);
        let ran: u64 = of_arm().map(|test| u64::from(test.runs)).sum();
        let asked: u64 = of_arm().map(|test| test.chats.len() as u64).sum();
        arms.insert(arm.arm)
            && arm.pass_rate.as_ref().is_none_or(|one| stat(one, share))
            && arm
                .tokens
                .as_ref()
                .is_none_or(|one| stat(one, |value| amount(value) && value <= MOST_TOKENS as f64))
            && arm
                .seconds
                .as_ref()
                .is_none_or(|one| stat(one, |value| amount(value) && value <= MOST_SECONDS))
            && u64::from(arm.chats) == ran
            && u64::from(arm.chats) + u64::from(arm.not_run) == asked
    });
    // Every test is of an arm the report scores.
    let tests_of_its_arms = report
        .triggers
        .iter()
        .all(|test| report.arm(test.arm).is_some());
    plain(&report.grader)
        && report.triggers.len() <= grade::MOST_TRIGGER_TESTS * ARMS_THERE_ARE
        && tests
        && arms_hold
        && tests_of_its_arms
        && report.cap.is_none_or(|cap| cap <= cap::MOST)
        && report.spent_tokens.is_none_or(tokens)
        && report
            .delta
            .is_none_or(|delta| (-1.0..=1.0).contains(&delta))
        && [report.cost_cap_usd, report.spent_usd, report.counted_usd]
            .into_iter()
            .flatten()
            .all(dollars)
        && report.priced.as_ref().is_none_or(|priced| {
            amount(priced.usd_per_million)
                && priced.usd_per_million <= price::MOST_READ_USD_PER_MILLION
                && priced.model.as_deref().is_none_or(price::model_id)
        })
        && (report.ended != grade::GradeEnd::OverLimit || report.cost_cap_usd.is_some())
        && [report.not_started.as_deref(), report.failed.as_deref()]
            .into_iter()
            .flatten()
            .all(|why| why.len() <= MOST_REASON_BYTES && !why.chars().any(char::is_control))
        // A Grader's failure says why, and only a Grader's failure does (#3236).
        && report.failed.is_some() == (report.ended == grade::GradeEnd::GraderFailed)
}

/// Every arm a grade can compare: the skill, no skill and its previous version.
const ARMS_THERE_ARE: usize = 3;

/// **Appends `graded`, a grade of the copy at `level`, to this month's file of the project at
/// `root`**: the committed record, or, for a copy that is this machine's alone ([`is_local`]),
/// this machine's state. `None` where the project is one this purlis may not write, or the write
/// was refused.
pub fn record(root: &Path, level: &Level, graded: &Graded, device: &str) -> Option<PathBuf> {
    record_to(root, is_local(root, level), graded, || device.to_owned())
}

/// [`record`], to this machine's state where `local`, else to the committed record of the
/// device `device` names: asked only then, as naming it reads this machine's store.
fn record_to(
    root: &Path,
    local: bool,
    graded: &Graded,
    device: impl FnOnce() -> String,
) -> Option<PathBuf> {
    let row = serde_json::to_value(graded).ok()?;
    if local {
        crate::appstate::made(root).ok()?;
        return append_local(&local_path_for(root, chrono::Utc::now())?, &row).ok();
    }
    if crate::compat::read_only(root).is_some() {
        return None;
    }
    crate::dispatch::append(&path_for(root, chrono::Utc::now(), &device()), root, &row)
}

/// **Appends `row` to this machine's record file `path`**, in a folder of this machine's state
/// made private, opened following no link, one line with its keys sorted, as the committed
/// record's are.
fn append_local(path: &Path, row: &serde_json::Value) -> std::io::Result<PathBuf> {
    use std::io::Write as _;
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("the record has no folder"))?;
    std::fs::create_dir_all(dir)?;
    if !std::fs::symlink_metadata(dir)?.is_dir() {
        return Err(std::io::Error::other("the record's folder is not a folder"));
    }
    let mut options = std::fs::OpenOptions::new();
    options.append(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = crate::contain::nofollow(&mut options).open(path)?;
    file.write_all(format!("{}\n", crate::pyjson::dumps_sorted(row)).as_bytes())?;
    Ok(path.to_path_buf())
}

/// Why the record file `path` was not read: it is not a file of its own, or it is larger than
/// [`MOST_FILE_BYTES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unread {
    NotAFile,
    TooLarge,
}

/// The text of the record file `path`, read through no link and at most [`MOST_FILE_BYTES`], or
/// why not. A byte that is not text spoils only the row it is in, never the file.
fn file_text(path: &Path) -> Result<String, Unread> {
    let meta = std::fs::symlink_metadata(path).map_err(|_| Unread::NotAFile)?;
    if !meta.is_file() {
        return Err(Unread::NotAFile);
    }
    if meta.len() > MOST_FILE_BYTES {
        return Err(Unread::TooLarge);
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let file = crate::contain::nofollow(&mut options)
        .open(path)
        .map_err(|_| Unread::NotAFile)?;
    let mut bytes = Vec::new();
    file.take(MOST_FILE_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|_| Unread::NotAFile)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The record files of folder `dir`, newest month first: only where `dir` is a folder of its
/// own (never a link), and none of a folder holding more than [`MOST_FILES`] names.
fn record_files(dir: &Path) -> Vec<PathBuf> {
    if !std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir()) {
        return Vec::new();
    }
    let Ok(reader) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let names: Vec<PathBuf> = reader
        .take(MOST_FILES + 1)
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    if names.len() > MOST_FILES {
        return Vec::new();
    }
    let mut files: Vec<PathBuf> = names
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    files.sort();
    files.reverse();
    files
}

/// **A skill's grades as the record was read** (#3273): the rows, newest first, and what was
/// left out of them, so a row or a file that was not taken is said, never dropped unseen.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Read {
    pub grades: Vec<Graded>,
    /// The rows of the skill skipped: ones that say what purlis does not write, or are stamped
    /// later than this machine's clock allows ([`MOST_AHEAD`]).
    pub skipped: usize,
    /// The files not read, by their name, each with why.
    pub unread: Vec<String>,
}

impl Read {
    /// **What was left out, in a sentence**, or `None` where nothing was.
    pub fn said(&self) -> Option<String> {
        let mut parts = Vec::new();
        match self.skipped {
            0 => {}
            1 => parts.push(
                "A row of the grade record was skipped, as it says what purlis does not write, or \
                 is stamped ahead of this machine's clock"
                    .to_owned(),
            ),
            many => parts.push(format!(
                "{many} rows of the grade record were skipped, as they say what purlis does not \
                 write, or are stamped ahead of this machine's clock"
            )),
        }
        parts.extend(self.unread.iter().cloned());
        (!parts.is_empty()).then(|| format!("{}.", parts.join("; ")))
    }
}

/// Whether `line`, which is not a row this purlis reads, names the skill `name` as a row would:
/// an object whose `skill` is it, of a version this purlis reads.
fn names_the_skill(line: &str, name: &str) -> bool {
    if line.len() > MOST_LINE_BYTES {
        // Too long to read whole: the key looked for as purlis writes it, or as JSON may.
        return line.contains(&format!("\"skill\": {}", serde_json::Value::from(name)))
            || line.contains(&format!("\"skill\":{}", serde_json::Value::from(name)));
    }
    serde_json::from_str::<serde_json::Value>(line).is_ok_and(|value| {
        value.get("skill").and_then(serde_json::Value::as_str) == Some(name)
            && value
                .get("v")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|v| v <= u64::from(VERSION))
    })
}

/// What the window and the terminal say of the record file `path` that was not read, `why`.
fn unread_said(path: &Path, why: Unread) -> String {
    let name = path
        .file_name()
        .map(|name| crate::shown::short(&name.to_string_lossy()))
        .unwrap_or_default();
    match why {
        Unread::NotAFile => format!("{name} is not a file of its own, so its grades were not read"),
        Unread::TooLarge => format!(
            "{name} is larger than {} MiB, so its grades were not read",
            MOST_FILE_BYTES / (1024 * 1024)
        ),
    }
}

/// **The grades of the skill `name`** in the project at `root`, newest first, at most
/// [`MOST_GRADES`]: [`read`]'s rows.
pub fn grades_of(root: &Path, name: &str, level: Option<&Level>) -> Vec<Graded> {
    read(root, name, level).grades
}

/// **The grades of the skill `name`** in the project at `root`, newest first, at most
/// [`MOST_GRADES`], and what was left out ([`Read`]): every copy's, or with `level`, that copy's
/// alone (a row of a local workspace's copy says no level, and is that copy's where its name and
/// content match). The committed record and this machine's are read, newest month first, until
/// enough are held.
pub fn read(root: &Path, name: &str, level: Option<&Level>) -> Read {
    read_at(root, name, level, chrono::Utc::now())
}

/// **The grades of the skill that had the places `was`** (#3918, [`super::history::places`]):
/// [`read`]'s rows of each place's copy, by the name and level it had there, so the grades from
/// before a rename or a move are read with the copy's own. A row of a place it had before is
/// taken only where its tree is one of that place's versions ([`super::history::Was::trees`]):
/// a skill that took the old name there since is another, and its grades are none of these. A
/// place no level's rule reads is left out.
pub fn read_of(root: &Path, was: &[super::history::Was]) -> Read {
    read_where(root, &wanted_of(was), None, chrono::Utc::now())
}

/// The copies [`read_of`] reads the rows of.
fn wanted_of(was: &[super::history::Was]) -> Vec<Wanted<'_>> {
    was.iter()
        .enumerate()
        .filter_map(|(at, place)| {
            let level = place.level.as_ref()?;
            Some(Wanted {
                name: place.name.as_str(),
                level: Some(level.rel()),
                trees: (at > 0).then_some(place.trees.as_slice()),
                until: None,
                grades: &[],
            })
        })
        .collect()
}

/// **The grades of the copy of `name` at `level`, and of the places it had before a rename or a
/// move** (#3918): what its Grades tab draws. The places git holds ([`super::history::places`]),
/// and those this machine kept where git could not follow, as for a skill only on this machine
/// (#4018, [`earlier`]). Its own alone where it had none.
pub fn read_followed(root: &Path, level: &Level, name: &str) -> Read {
    let was = super::history::places(root, level, name).unwrap_or_default();
    let earlier = earlier::of(root, level, name);
    match followed((level, name), &was, &earlier) {
        Some(copies) => read_where(root, &copies, None, chrono::Utc::now()),
        None => read(root, name, Some(level)),
    }
}

/// The copies whose rows the copy of `name` at `level` reads, where it had the places `was` in
/// git ([`wanted_of`], its own first, or none where its history was not read) and `earlier` on
/// this machine: `None` where it had no place but its own.
fn followed<'a>(
    (level, name): (&Level, &'a str),
    was: &'a [super::history::Was],
    earlier: &'a [earlier::Earlier],
) -> Option<Vec<Wanted<'a>>> {
    if was.len() <= 1 && earlier.is_empty() {
        return None;
    }
    let mut copies = wanted_of(was);
    if was.is_empty() {
        copies.push(Wanted {
            name,
            level: Some(level.rel()),
            trees: None,
            until: None,
            grades: &[],
        });
    }
    copies.extend(earlier.iter().filter_map(|one| {
        Some(Wanted {
            name: one.name.as_str(),
            level: Some(Some(one.level.clone())),
            trees: None,
            until: Some(one.until()?),
            grades: &one.grades,
        })
    }));
    Some(copies)
}

/// A copy whose rows [`read_where`] reads: its skill's name; where `level` is `Some`, its
/// level's folder (`None` in it for a copy no folder holds), else every copy of the name; and
/// where `trees` is `Some`, only rows whose tree is one of them; where `until` is `Some`, only
/// rows recorded before it (a place this machine kept, [`earlier`]), or of `grades`, those under
/// way as it was left (#4104).
struct Wanted<'a> {
    name: &'a str,
    level: Option<Option<String>>,
    trees: Option<&'a [String]>,
    until: Option<chrono::DateTime<chrono::Utc>>,
    grades: &'a [String],
}

impl Wanted<'_> {
    /// Whether `row` is of this copy.
    fn holds(&self, row: &Graded) -> bool {
        row.skill == self.name
            && match &self.level {
                None => true,
                Some(word) => row.level.is_none() || row.level == *word,
            }
            && self.trees.is_none_or(|trees| trees.contains(&row.tree))
            && (self.until.is_none_or(|until| {
                chrono::DateTime::parse_from_rfc3339(&row.ts).is_ok_and(|at| at <= until)
            }) || self.grades.contains(&row.grade))
    }
}

/// [`read`], with this machine's clock at `now`.
fn read_at(
    root: &Path,
    name: &str,
    level: Option<&Level>,
    now: chrono::DateTime<chrono::Utc>,
) -> Read {
    let wanted = Wanted {
        name,
        level: level.map(Level::rel),
        trees: None,
        until: None,
        grades: &[],
    };
    read_where(root, std::slice::from_ref(&wanted), None, now)
}

/// The rows of the record of any of `copies` ([`Wanted`]), and of the grade `grade` alone where
/// it is `Some`, newest first, at most [`MOST_GRADES`].
fn read_where(
    root: &Path,
    copies: &[Wanted<'_>],
    grade: Option<&str>,
    now: chrono::DateTime<chrono::Utc>,
) -> Read {
    let mut files = record_files(&root.join("personas").join(DIR_NAME));
    if let Some(state) = crate::appstate::dir(root) {
        files.extend(record_files(&state.join(LOCAL_DIR)));
    }
    // Newest month first, across both folders: a file's name starts with its month.
    files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    let mut out = Read::default();
    for file in files {
        if out.grades.len() >= MOST_GRADES {
            break;
        }
        let text = match file_text(&file) {
            Ok(text) => text,
            Err(why) => {
                out.unread.push(unread_said(&file, why));
                continue;
            }
        };
        let mut rows: Vec<Graded> = Vec::new();
        for line in crate::mdsection::split_lines(&text) {
            // A line too long or not a row is skipped; it is counted where it names this
            // skill, so a row of it is never dropped unseen.
            let row = (line.len() <= MOST_LINE_BYTES)
                .then(|| serde_json::from_str::<Graded>(line).ok())
                .flatten();
            let Some(row) = row else {
                if copies.iter().any(|one| names_the_skill(line, one.name)) {
                    out.skipped += 1;
                }
                continue;
            };
            let its = row.v <= VERSION
                && grade.is_none_or(|grade| row.grade == grade)
                && copies.iter().any(|one| one.holds(&row));
            if !its {
                continue;
            }
            if row.sound(now) {
                rows.push(row);
            } else {
                out.skipped += 1;
            }
        }
        rows.reverse();
        out.grades.extend(rows);
    }
    out.grades.sort_by(|a, b| b.ts.cmp(&a.ts));
    out.grades.truncate(MOST_GRADES);
    out
}

/// **How a grade ended, as its row and its scorecard say it.**
pub fn ended_said(end: grade::GradeEnd) -> &'static str {
    use grade::GradeEnd;
    match end {
        GradeEnd::Whole => "Finished",
        GradeEnd::Cancelled => "Stopped",
        GradeEnd::ChatsDidNotStart => "Chats did not start",
        GradeEnd::OverCap => "Stopped at its token cap",
        GradeEnd::OverLimit => "Stopped at its dollar limit",
        GradeEnd::SpendNotReported => "Stopped: spend not reported",
        GradeEnd::Interrupted => "Interrupted: not scored",
        GradeEnd::GraderFailed => "Stopped: its Grader failed",
    }
}

/// **The grade `grade` of the skill `name`** in the project at `root`, as the eval record holds
/// it (SK-59, #3239): what its scorecard is drawn from. A grade from before a rename or a move
/// (#3918) is recorded by the name and level a copy of `name` had then, so where no row of `name`
/// is the grade, the places each copy had before are looked in ([`read_followed`]). `None` where
/// no row of it is read.
pub fn grade_of(root: &Path, name: &str, grade: &str) -> Option<Graded> {
    let now = chrono::Utc::now();
    let of = |copies: &[Wanted<'_>]| {
        read_where(root, copies, Some(grade), now)
            .grades
            .into_iter()
            .next()
    };
    let named = Wanted {
        name,
        level: None,
        trees: None,
        until: None,
        grades: &[],
    };
    of(std::slice::from_ref(&named)).or_else(|| {
        Level::copies_of(root, name).find_map(|(level, _)| {
            let was = super::history::places(root, &level, name).unwrap_or_default();
            let earlier = earlier::of(root, &level, name);
            of(&followed((&level, name), &was, &earlier)?)
        })
    })
}

/// **What a grade of a copy would cost**, from what a grade of it before spent per chat
/// ([`estimate_of`]), and which grade that was. Its tokens and cost are both `None` where that
/// grade's chats did not say what they spent: the copy was graded, and the cost is not known.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Estimate {
    pub tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    /// The chats the grade it is worked out from asked for, ran or not: none where none ran,
    /// and in one purlis stopped under before its marker counted them (#3602).
    pub chats_then: u32,
    /// That grade: when it ran, and the profile and the harness its chats ran on.
    pub ts: String,
    pub profile: Option<String>,
    pub harness: Option<String>,
    /// The model its chats reported (SK-60, #3240): a grade on another model spends otherwise,
    /// so the plan names it. `None` in a row from before #3240 and where no chat said.
    pub model: Option<String>,
    /// Whether it ran on the profile this grade runs on (a grade on another one may have spent
    /// otherwise), or `None` where this grade's profile is not known.
    pub same_profile: Option<bool>,
    /// **Whether that grade was interrupted before it was scored** (#3602): its spend is what
    /// its marker said of the chats it asked for before purlis stopped under it, so the plan
    /// says so. `false` where it asked for none to go by.
    pub interrupted: bool,
}

/// **What a grade of `chats` chats on `profile` would cost, from `grades`** (newest first), and
/// which grade it is worked out from (#3273): the newest one on that profile that asked for a
/// chat, else the newest that asked for one on any; where none did, the newest, which says no
/// cost and no chat. `None` where there is none. What it spent is shared over every chat it
/// asked for, a chat stopped, ended at a limit or never started among them, as what a chat
/// stopped spent is counted too.
///
/// **A grade purlis stopped under before it was scored** (#3602) counts the chats its marker
/// said it asked for, once those took a turn of each of its arms: arms take turns, so fewer
/// would be one arm's chats alone (the skill's dearer, the bare one's cheaper) shared over every
/// arm. It is gone by only where no scored grade on the same profile is, as its chats are fewer
/// and stopped where they were; one from before its marker counted them says none.
fn estimate_of(grades: &[Graded], chats: u32, profile: Option<&str>) -> Option<Estimate> {
    let asked = |row: &Graded| -> u64 {
        if stopped_unscored(&row.report) {
            let arms = if row.previous.is_some() {
                ARMS_WITH_PREVIOUS.len()
            } else {
                ARMS.len()
            } as u64;
            return row
                .spent
                .asked_for
                .map(u64::from)
                .filter(|asked_for| *asked_for >= arms)
                .unwrap_or(0);
        }
        row.report
            .arms
            .iter()
            .map(|arm| u64::from(arm.chats) + u64::from(arm.not_run))
            .sum()
    };
    let on_profile = |row: &&Graded| profile.is_some() && row.profile.as_deref() == profile;
    let scored = |row: &&Graded| asked(row) > 0 && !stopped_unscored(&row.report);
    let any = |row: &&Graded| asked(row) > 0;
    let last = grades
        .iter()
        .filter(scored)
        .find(on_profile)
        .or_else(|| grades.iter().filter(any).find(on_profile))
        .or_else(|| grades.iter().find(scored))
        .or_else(|| grades.iter().find(any))
        .or_else(|| grades.first())?;
    let then = asked(last);
    let per = |of: u64| (then > 0).then(|| of.div_ceil(then).saturating_mul(u64::from(chats)));
    Some(Estimate {
        tokens: last.spent.tokens.and_then(per),
        cost_usd: last
            .spent
            .cost_usd
            .filter(|_| then > 0)
            .map(|cost| cost / then as f64 * f64::from(chats)),
        chats_then: u32::try_from(then).unwrap_or(u32::MAX),
        ts: last.ts.clone(),
        profile: last.profile.clone(),
        harness: last.harness.clone(),
        model: last.model.clone(),
        same_profile: profile.map(|profile| last.profile.as_deref() == Some(profile)),
        interrupted: then > 0 && stopped_unscored(&last.report),
    })
}

/// **What grading a copy would run, before anything runs** (decision 22): its fixtures, the
/// request, the plan, the estimate, the trigger tests whose query holds a character no reader
/// would see, by their place, which the plan marks as the install review does, and the ones no
/// chat can be started on, which the grade leaves out.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Planned {
    pub skill: String,
    pub fixtures: Fixtures,
    /// What a grade would ask, where the copy has trigger tests to run.
    pub request: Option<GradeRequest>,
    pub plan: Option<Plan>,
    pub estimate: Option<Estimate>,
    /// What the record left out as it was read ([`Read::said`]).
    pub record_said: Option<String>,
    /// The record of the copy's grades, read once for the plan: what the estimate is worked
    /// out from, and what `purlis skill grade` lists.
    #[serde(skip)]
    pub record: Read,
    pub unseen: Vec<usize>,
    /// The trigger tests left out, each with why: a query a chat cannot be started on
    /// ([`left_out_why`]).
    pub left_out: Vec<LeftOut>,
    /// The token cap the grade is held to ([`cap::token_cap`]).
    pub cap: cap::Cap,
    /// What the person consents to by pressing Grade on this plan ([`consent_of`]): a grade
    /// started on another is refused.
    pub consent: Option<String>,
    /// **The version it compares with** (SK-57, #3237), where the skill has one: the plan says
    /// it. And why a previous version is not compared, where it has one that did not read.
    pub previous: Option<ComparedWith>,
    pub previous_refused: Option<String>,
    /// **The chats each grade after an edit may run** (#4035, [`after_edit::chats_to_accept`]):
    /// with its previous version's arm counted where its first saved edit brings one.
    pub after_edit_chats: Option<u32>,
    /// The copy the consent is a digest of, read once: what a grade started on this plan lays
    /// out every arm from, and records its version as. Never shown, so never sent.
    #[serde(skip)]
    pub copy: Option<SkillCopy>,
}

/// **The version a grade compares with, as its plan says it** (SK-57): its commit, and its
/// subject as one line with nothing invisible in it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ComparedWith {
    pub commit: String,
    pub subject: String,
}

/// **A trigger test a grade leaves out**: its place in the file, from 0, and why
/// ([`left_out_why`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LeftOut {
    pub at: usize,
    pub why: String,
}

/// **The copy of a skill a grade runs, read once** (the review of #3229): its files, what its
/// folder held by content as they were read, and the commit that content is. The consent, every
/// arm and the record's version are all taken from this one read, so what is graded, what was
/// shown and what is recorded are the same files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillCopy {
    pub snapshot: Snapshot,
    pub content: Content,
    pub commit: Option<String>,
    /// **Its previous version** (SK-57, #3237), read from git's tree of that commit, where the
    /// skill has one and it reads; else why not, where it has one that does not read.
    pub previous: Option<Previous>,
    pub previous_refused: Option<String>,
}

/// **A skill's previous version, as a grade compares with it** (SK-57, #3237): the commit, what
/// its folder held there, its subject as one line, and its files as git's tree of that commit
/// holds them ([`super::history::files_of_version`]), never as a folder a chat could write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Previous {
    pub commit: String,
    pub tree: String,
    pub subject: String,
    pub snapshot: Snapshot,
}

/// **The copy of `name` at `level`, read once** ([`SkillCopy`]). Its content is what git says the
/// folder holds both before and after its files are read: a folder that changed while they were
/// read is `uncommitted`, so a recorded tree is never one the files laid out are not. Its
/// history is read once too, for the commit its content is and the version before it.
fn copy_of(root: &Path, level: &Level, name: &str) -> Result<SkillCopy, String> {
    let folder = find(root, level, name)?;
    let content_now = || crate::skilluse::content_at(root, &folder).unwrap_or(Content::Uncommitted);
    let before = content_now();
    let snapshot = super::verbs::drafts::snapshot(root, level, name)?;
    let content = if content_now() == before {
        before
    } else {
        Content::Uncommitted
    };
    let history = super::history::history(root, level, name).ok();
    let tree = match &content {
        Content::Tree(tree) => Some(tree.as_str()),
        Content::Uncommitted => None,
    };
    let commit = tree.and_then(|tree| {
        history.as_ref().and_then(|history| {
            history
                .versions
                .iter()
                .find(|version| version.tree == tree)
                .map(|version| version.commit.clone())
        })
    });
    let (previous, previous_refused) = match history
        .as_ref()
        .and_then(|history| super::history::previous_of(history, name, tree))
    {
        None => (None, None),
        Some(version) => {
            // From before a move, it is read from the folder the skill had then (#3918).
            match super::history::files_of_version(root, &version.folder, name, &version.commit) {
                Ok(files) => (
                    Some(Previous {
                        commit: version.commit.clone(),
                        tree: version.tree.clone(),
                        subject: version.subject.clone(),
                        snapshot: files,
                    }),
                    None,
                ),
                Err(why) => (None, Some(why)),
            }
        }
    };
    Ok(SkillCopy {
        snapshot,
        content,
        commit,
        previous,
        previous_refused,
    })
}

/// **The plan of grading the copy of `name` at `level`** in the project at `root`, its chats on
/// `profile` where it is known: what its estimate is closest to ([`estimate_of`]).
pub fn planned(
    root: &Path,
    level: &Level,
    name: &str,
    profile: Option<&str>,
) -> Result<Planned, String> {
    let folder = find(root, level, name)?;
    let fixtures = grade::fixtures_of(root, Path::new(&rel(root, &folder)), name);
    let mut left_out: Vec<LeftOut> = Vec::new();
    let sendable: Vec<grade::TriggerTest> = fixtures
        .triggers
        .read()
        .map(|tests| {
            tests
                .iter()
                .enumerate()
                .filter(|(at, test)| match left_out_why(&test.query) {
                    Some(why) => {
                        left_out.push(LeftOut { at: *at, why });
                        false
                    }
                    None => true,
                })
                .map(|(_, test)| test.clone())
                .collect()
        })
        .unwrap_or_default();
    let copy = if sendable.is_empty() {
        None
    } else {
        Some(copy_of(root, level, name)?)
    };
    // A skill with a previous version is compared with it too, and it is the baseline (SK-57).
    let arms = match copy.as_ref().and_then(|copy| copy.previous.as_ref()) {
        Some(_) => ARMS_WITH_PREVIOUS.to_vec(),
        None => ARMS.to_vec(),
    };
    let request = (!sendable.is_empty()).then(|| GradeRequest {
        skill: name.to_owned(),
        triggers: sendable,
        outcomes: fixtures.outcomes.read().cloned().unwrap_or_default(),
        arms,
        runs: RUNS,
    });
    let plan = request.as_ref().map(grade::plan);
    let record = read(root, name, Some(level));
    let estimate = plan.and_then(|plan| estimate_of(&record.grades, plan.chats, profile));
    let record_said = record.said();
    let unseen = request
        .as_ref()
        .map(|request| {
            request
                .triggers
                .iter()
                .enumerate()
                .filter(|(_, test)| {
                    purlis_unicode::hides(&test.query, purlis_unicode::Breaks::Kept)
                })
                .map(|(at, _)| at)
                .collect()
        })
        .unwrap_or_default();
    let consent = request
        .as_ref()
        .zip(copy.as_ref())
        .map(|(request, copy)| consent_of(request, copy));
    let previous = copy
        .as_ref()
        .and_then(|copy| copy.previous.as_ref())
        .map(|previous| ComparedWith {
            commit: previous.commit.clone(),
            subject: previous.subject.clone(),
        });
    let previous_refused = copy.as_ref().and_then(|copy| copy.previous_refused.clone());
    Ok(Planned {
        skill: name.to_owned(),
        fixtures,
        request,
        plan,
        estimate,
        record_said,
        record,
        unseen,
        left_out,
        cap: cap::token_cap(root),
        consent,
        previous,
        previous_refused,
        after_edit_chats: plan
            .map(|plan| after_edit::chats_to_accept(&plan, !is_local(root, level))),
        copy,
    })
}

/// **The trigger tests a grade of the copy of `name` at `level` would run**, and why its file was
/// not read where it was not: what a skill's Grades tab asks to know whether there is anything to
/// grade (#3928), without the plan's history, estimate and models. A test no chat can be started
/// on ([`left_out_why`]) is not one a grade runs, as [`planned`] leaves it out.
pub fn trigger_tests(root: &Path, level: &Level, name: &str) -> Result<TriggerTests, String> {
    let folder = find(root, level, name)?;
    let fixtures = grade::fixtures_of(root, Path::new(&rel(root, &folder)), name);
    let runs = fixtures.triggers.read().map_or(0, |tests| {
        tests
            .iter()
            .filter(|test| left_out_why(&test.query).is_none())
            .count()
    });
    Ok(TriggerTests {
        runs,
        refused: fixtures.triggers.refused().cloned(),
    })
}

/// **A copy's trigger tests, counted** ([`trigger_tests`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerTests {
    /// How many a grade runs.
    pub runs: usize,
    /// Why the file could not be read, where it could not.
    pub refused: Option<grade::Refused>,
}

/// **Why the query `query` is left out of a grade**, or `None` for one a chat is started on: one
/// whose first character a chat program reads as its own ([`COMMAND_STARTS`]), or one no first
/// message can carry ([`crate::handoff::bad_message`]).
pub fn left_out_why(query: &str) -> Option<String> {
    // The first character a chat program would act on: past white space and past every
    // character no reader sees (a byte-order mark, a zero-width space), which a program may
    // drop before it reads the line (the review of #3229).
    if let Some(first) = query
        .chars()
        .find(|ch| !ch.is_whitespace() && !purlis_unicode::invisible(*ch))
        && COMMAND_STARTS.contains(&first)
    {
        return Some(format!(
            "it starts with {first:?}, which the chat program would read as a command of its \
             own rather than as a prompt"
        ));
    }
    crate::handoff::bad_message(query).map(|bad| bad.say())
}

/// **What the person consented to**: one digest of the trigger tests the plan showed, the arms
/// it runs, what the copy's folder held by content, the copy's files as they are now, and the
/// previous version it compares with and its files (SK-57). A grade started on a plan that no
/// longer holds is refused, so what runs is what was shown.
pub fn consent_of(request: &GradeRequest, copy: &SkillCopy) -> String {
    use sha2::Digest as _;
    let mut digest = sha2::Sha256::new();
    digest.update(serde_json::to_vec(&request.triggers).unwrap_or_default());
    digest.update(b"\0");
    digest.update(serde_json::to_vec(&request.arms).unwrap_or_default());
    digest.update(b"\0");
    digest.update(copy.content.word().as_bytes());
    let files = |digest: &mut sha2::Sha256, snapshot: &Snapshot| {
        for (path, text) in &snapshot.files {
            digest.update(b"\0");
            digest.update(path.as_bytes());
            digest.update(b"\0");
            digest.update(text.as_bytes());
        }
    };
    files(&mut digest, &copy.snapshot);
    if let Some(previous) = &copy.previous {
        digest.update(b"\0previous\0");
        digest.update(previous.commit.as_bytes());
        files(&mut digest, &previous.snapshot);
    }
    hex_of(digest)
}

/// A digest's bytes as lowercase hex.
fn hex_of(digest: sha2::Sha256) -> String {
    use sha2::Digest as _;
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// **What a grade's plan says it may spend, beside what it grades**: the profile and the model
/// its chats run on, the token cap and the dollar limit, as the plan shows them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Terms<'a> {
    pub profile: Option<&'a str>,
    pub model: Option<&'a str>,
    pub token_cap: u64,
    pub limit_usd: f64,
}

/// **The consent of a plan that shows `terms`**: `consent` (what it grades, [`consent_of`]) and
/// the profile, model and limits it shows, in one digest. A press made on a plan whose profile,
/// model, cap or limit is not the one a grade would start on now is refused, so a grade runs on
/// the terms the person read.
pub fn consent_on(consent: &str, terms: &Terms<'_>) -> String {
    use sha2::Digest as _;
    let mut digest = sha2::Sha256::new();
    digest.update(consent.as_bytes());
    for word in [terms.profile, terms.model] {
        digest.update(b"\0");
        match word {
            Some(word) => {
                digest.update(b"=");
                digest.update(word.as_bytes());
            }
            None => digest.update(b"-"),
        }
    }
    digest.update(b"\0");
    digest.update(terms.token_cap.to_be_bytes());
    digest.update(b"\0");
    digest.update(terms.limit_usd.to_bits().to_be_bytes());
    hex_of(digest)
}

use super::verbs::drafts::Snapshot;

/// **How a grade's chat would start as `start`** in the project at `root` (the operator's
/// rulings of 2026-10-26 on #3229), or why it may not: decided keeping none of the person's own
/// Allows ([`crate::sandbox::policy::holding_no_kept_grants`]), and refused unless it runs at the
/// full sandbox on a profile that keeps its permission prompts ([`chat_refused`]). The one answer
/// a grade's plan and its chats both take.
///
/// **Its chats have only purlis's plugin on** (P8, #3468): every plugin the harness has
/// installed is handed off for the chat alone, by the harness's own per-chat switch, and
/// nothing of the person's own settings is written ([`crate::harness_plugin::only_purlis_on`]),
/// so a grade measures the skill alone. [`plugins_off`] names them for the record.
pub fn ready_to_grade(
    start: &crate::start::Start,
    root: &Path,
) -> Result<crate::start::Ready, String> {
    ready_to_grade_on(start, root, None)
}

/// **[`ready_to_grade`], on the model `model` the person picked** (SK-60, #3240): one of those
/// its harness's adapter offers ([`models_for`]), started by that adapter's own words after the
/// profile's ([`crate::harness::HarnessAdapter::model_words`]), so the chat runs on it and its
/// dollar limit is priced from it ([`priced`]). `None` is the profile's own model. A model the
/// adapter does not offer is refused, so nothing but a name it takes reaches the line.
pub fn ready_to_grade_on(
    start: &crate::start::Start,
    root: &Path,
    model: Option<&str>,
) -> Result<crate::start::Ready, String> {
    let mut ready =
        crate::sandbox::policy::holding_no_kept_grants(|| crate::start::ready(start, root))?;
    chat_refused(&ready)?;
    if let Some(model) = model {
        let adapter = ready.harness.map(crate::harness::Harness::adapter);
        let offered = models_for(&ready);
        match adapter {
            Some(adapter) if offered.contains(&model) => {
                ready.command.extend(adapter.model_words(model));
            }
            _ => return Err(model_not_offered(model, offered)),
        }
    }
    if let Some(harness) = ready.harness {
        let adapter = harness.adapter().plugins();
        ready.plugins.plugins = crate::harness_plugin::only_purlis_on(
            adapter,
            &crate::harness_plugin::Env::of(&ready.env),
        )
        .map_err(|why| {
            format!(
                "{why}, and a grade runs with every plugin but purlis's own off, so no grade \
                 runs on this profile: mend the record with the program's own plugin command, or \
                 pick another profile."
            )
        })?;
    }
    Ok(ready)
}

/// **The models a grade's chat that starts as `ready` can be picked to run on** (SK-60, #3240):
/// what its harness's adapter offers ([`crate::harness::HarnessAdapter::grade_models`]), none
/// for a chat on no harness.
pub fn models_for(ready: &crate::start::Ready) -> &'static [&'static str] {
    ready
        .harness
        .map_or(&[], |harness| harness.adapter().grade_models())
}

/// Why the model `model` is refused for a grade whose harness offers `offered`, in a sentence.
fn model_not_offered(model: &str, offered: &[&str]) -> String {
    let model = crate::shown::one_line(model, 60);
    if offered.is_empty() {
        return format!(
            "A grade on this profile runs on the profile's own model, so it cannot be picked as \
             {model}: pick the profile's own model."
        );
    }
    format!(
        "{model} is not a model a grade on this profile can pick: pick {}, or the profile's own \
         model.",
        offered.join(", ")
    )
}

/// **The harness's own plugins a grade's chat that starts as `ready` is handed off** (#3468):
/// what its record says, purlis's pins left out.
pub fn plugins_off(ready: &crate::start::Ready) -> Vec<String> {
    ready.harness.map_or_else(Vec::new, |harness| {
        crate::harness_plugin::persons_plugins_off(
            harness.adapter().plugins(),
            &ready.plugins.plugins,
        )
    })
}

/// **The price a grade's chat started as `ready` has its dollar limit worked out at** (#3469):
/// that of the model its harness's adapter says it runs on
/// ([`crate::harness::HarnessAdapter::grade_model`]), or the dearest where it says none or its
/// price is not known ([`price::of_model`]).
pub fn priced(ready: &crate::start::Ready) -> grade::Priced {
    let command: Vec<String> = std::iter::once(ready.program.clone())
        .chain(ready.command.iter().cloned())
        .collect();
    let model = ready
        .harness
        .map(crate::harness::Harness::adapter)
        .and_then(|adapter| {
            adapter.grade_model(&command, &crate::harness_plugin::Env::of(&ready.env))
        });
    price::of_model(model.as_deref())
}

/// **Why a grade's chat may not start as `ready` would start it**, or the sandbox level it would
/// run at (the review of #3229, the operator's ruling of 2026-10-26): a grade's chats run only on
/// a harness whose adapter says it can hold a chat to the grade's limits (its spend, its MCP
/// servers: [`crate::harness::HarnessAdapter::can_grade`], #3310); never where that adapter says
/// the profile's command or the person's settings keep the chat from asking
/// ([`crate::harness::HarnessAdapter::grade_refused`]); and only at the full sandbox, never at
/// the floor, with the sandbox lifted, or with no backend.
pub fn chat_refused(ready: &crate::start::Ready) -> Result<crate::sandbox::Level, String> {
    let Some(harness) = ready.harness else {
        return Err(not_on_this_harness());
    };
    let adapter = harness.adapter();
    if let Err(why) = adapter.can_grade() {
        return Err(refused_because(&why));
    }
    let command: Vec<String> = std::iter::once(ready.program.clone())
        .chain(ready.command.iter().cloned())
        .collect();
    if let Some(why) = adapter.grade_refused(&command, &crate::harness_plugin::Env::of(&ready.env))
    {
        return Err(why);
    }
    if let Some(lifted) = &ready.unsandboxed {
        return Err(match &lifted.reason {
            Some(why) => format!(
                "Its chats would start with the sandbox off ({}), and a grade runs only in the \
                 full sandbox.",
                crate::shown::one_line(why, 120)
            ),
            None => "Its chats would start with the sandbox off, as your own level for this \
                     project is off, and a grade runs only in the full sandbox."
                .to_owned(),
        });
    }
    if let Some(why) = &ready.floorless {
        return Err(format!(
            "{} A grade runs only in the full sandbox.",
            crate::shown::one_line(why, 200)
        ));
    }
    match ready.sandbox.as_ref().map(crate::sandbox::Applied::level) {
        Some(crate::sandbox::Level::Full) => Ok(crate::sandbox::Level::Full),
        Some(crate::sandbox::Level::Floor) => Err(
            "This project's chats run at the floor, not the full sandbox, and a grade runs only \
             in the full sandbox: turn the sandbox on for the project, or set your own level to \
             on."
            .to_owned(),
        ),
        None => Err(
            "Its chats would start with no sandbox, and a grade runs only in the full sandbox."
                .to_owned(),
        ),
    }
}

/// **Why a grade's chat is refused on a harness that cannot grade**, in one sentence: its
/// adapter's reason `why` ([`crate::harness::HarnessAdapter::can_grade`]), then the harnesses
/// that can, so the core names none.
pub fn refused_because(why: &str) -> String {
    let can = can_grade_on();
    if can.is_empty() {
        return format!("{why}, and no program this purlis runs can grade yet, so no grade runs.");
    }
    format!(
        "{why}, so a grade runs on a {} profile only: pick one.",
        can.join(" or ")
    )
}

/// The titles of the harnesses that can grade.
fn can_grade_on() -> Vec<&'static str> {
    crate::harness::Harness::ALL
        .into_iter()
        .filter(|harness| harness.adapter().can_grade().is_ok())
        .map(crate::harness::Harness::title)
        .collect()
}

/// **Why a grade's chat is refused on a profile of no harness purlis knows**: named by the
/// harnesses that can ([`crate::harness::HarnessAdapter::can_grade`]), so the core names none.
fn not_on_this_harness() -> String {
    let can = can_grade_on();
    if can.is_empty() {
        return "No program this purlis runs can hold a grade's chats to its limits yet, so no \
                grade runs."
            .to_owned();
    }
    format!(
        "A grade runs its chats on a {} profile only, as only there can purlis hold a chat to \
         the grade's limits: pick one.",
        can.join(" or ")
    )
}

pub mod after_edit;
pub mod cap;
pub mod earlier;
pub mod kept;
pub mod price;
pub mod routers;
pub mod underway;

#[cfg(test)]
mod tests;
