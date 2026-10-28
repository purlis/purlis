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

/// The arms the built-in runner compares now: the skill against no skill.
pub const ARMS: [Arm; 2] = [Arm::With, Arm::Without];

/// The longest one grade's chat runs before purlis ends it: what the plan's "up to" says.
pub const CHAT_TIME: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// The folder of this machine's state the grades of a local copy are kept in, never committed
/// (a copy in `skills.local/`, or in a workspace that is this machine's alone).
pub(crate) const LOCAL_DIR: &str = "skill-grades";

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

/// `<state>/skill-grades/<YYYY-MM>.jsonl`: where a local copy's grades are kept.
pub fn local_path_for(root: &Path, when: chrono::DateTime<chrono::Utc>) -> PathBuf {
    crate::plane::state_dir(root)
        .join(LOCAL_DIR)
        .join(format!("{}.jsonl", when.format("%Y-%m")))
}

/// The level a grade of the copy at `level` is recorded by: its folder relative to the plane.
pub fn recorded_level(level: &Level) -> Option<String> {
    level.rel()
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
}

impl Graded {
    /// What its copy's folder held.
    pub fn content(&self) -> Option<Content> {
        Content::parse(&self.tree)
    }

    /// **Whether a row read back says only what purlis writes**: a tree or `uncommitted`, a
    /// commit that is a whole hex id, a harness and a profile that are plain names, a stamp of a
    /// time's shape, and a spend and a report that hold together ([`report_holds_together`],
    /// #3353). A teammate's purlis writes the record, and a person reads these words in the
    /// window and the terminal, so a row that says anything else is skipped.
    fn sound(&self) -> bool {
        let hex = |word: &str| {
            matches!(word.len(), 40 | 64)
                && word
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        self.content().is_some()
            && self.commit.as_deref().is_none_or(hex)
            && self.harness.as_deref().is_none_or(plain)
            && self.profile.as_deref().is_none_or(plain)
            && plain(&self.grade)
            && chrono::DateTime::parse_from_rfc3339(&self.ts).is_ok()
            && self.spent.cost_usd.is_none_or(amount)
            && self.plugins_off.as_deref().is_none_or(|ids| {
                ids.len() <= MOST_PLUGINS_OFF
                    && ids.iter().all(|id| crate::harness_plugin::id_ok(id))
            })
            && report_holds_together(&self.report, self.runs)
    }
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
const MOST_REASON_BYTES: usize = 2 * 1024;

/// **Whether the report `report` of a grade of `runs` runs holds together** (#3353): what the
/// built-in runner could have written. Its grader is a plain name; each trigger test's query is
/// one a fixture could hold, its triggered runs at most its runs and its runs at most the
/// grade's, its rate what those two make and its pass there exactly where its rate is, each chat
/// a plain id; each arm once, its pass rate a share and its other figures amounts, every spread
/// in order; the difference between two shares; and every cost an amount. Its reason is one
/// line a reader sees whole.
fn report_holds_together(report: &GradeReport, runs: u8) -> bool {
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
            && test.chats.iter().flatten().all(|chat| plain(chat))
    });
    let mut arms = std::collections::HashSet::new();
    let arms_hold = report.arms.iter().all(|arm| {
        arms.insert(arm.arm)
            && arm.pass_rate.as_ref().is_none_or(|one| stat(one, share))
            && arm.tokens.as_ref().is_none_or(|one| stat(one, amount))
            && arm.seconds.as_ref().is_none_or(|one| stat(one, amount))
    });
    plain(&report.grader)
        && report.triggers.len() <= grade::MOST_TRIGGER_TESTS * ARMS_THERE_ARE
        && tests
        && arms_hold
        && report
            .delta
            .is_none_or(|delta| (-1.0..=1.0).contains(&delta))
        && [report.cost_cap_usd, report.spent_usd, report.counted_usd]
            .into_iter()
            .flatten()
            .all(amount)
        && report.priced.as_ref().is_none_or(|priced| {
            amount(priced.usd_per_million)
                && priced.usd_per_million <= price::MOST_READ_USD_PER_MILLION
                && priced.model.as_deref().is_none_or(price::model_id)
        })
        && (report.ended != grade::GradeEnd::OverLimit || report.cost_cap_usd.is_some())
        && report
            .not_started
            .as_deref()
            .is_none_or(|why| why.len() <= MOST_REASON_BYTES && !why.chars().any(char::is_control))
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
        return append_local(&local_path_for(root, chrono::Utc::now()), &row).ok();
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

/// The text of the record file `path`, read through no link and at most [`MOST_FILE_BYTES`].
fn file_text(path: &Path) -> Option<String> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MOST_FILE_BYTES {
        return None;
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let file = crate::contain::nofollow(&mut options).open(path).ok()?;
    let mut text = String::new();
    file.take(MOST_FILE_BYTES).read_to_string(&mut text).ok()?;
    Some(text)
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

/// **The grades of the skill `name`** in the project at `root`, newest first, at most
/// [`MOST_GRADES`]: every copy's, or with `level`, that copy's alone (a row of a local
/// workspace's copy says no level, and is that copy's where its name and content match). The
/// committed record and this machine's are read, newest month first, until enough are held.
pub fn grades_of(root: &Path, name: &str, level: Option<&Level>) -> Vec<Graded> {
    let wanted = level.map(|level| level.rel());
    let mut files = record_files(&root.join("personas").join(DIR_NAME));
    files.extend(record_files(&crate::plane::state_dir(root).join(LOCAL_DIR)));
    // Newest month first, across both folders: a file's name starts with its month.
    files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    let mut out: Vec<Graded> = Vec::new();
    for file in files {
        if out.len() >= MOST_GRADES {
            break;
        }
        let Some(text) = file_text(&file) else {
            continue;
        };
        let mut rows: Vec<Graded> = crate::mdsection::split_lines(&text)
            .into_iter()
            .filter(|line| line.len() <= MOST_LINE_BYTES)
            .filter_map(|line| serde_json::from_str::<Graded>(line).ok())
            .filter(|row| row.v <= VERSION && row.skill == name && row.sound())
            .filter(|row| match &wanted {
                None => true,
                Some(word) => row.level.is_none() || row.level == *word,
            })
            .collect();
        rows.reverse();
        out.extend(rows);
    }
    out.sort_by(|a, b| b.ts.cmp(&a.ts));
    out.truncate(MOST_GRADES);
    out
}

/// What a grade of a copy would cost, from what the copy's last grade spent per chat.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Estimate {
    pub tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    /// The last grade it is worked out from, by when it ran.
    pub chats_then: u32,
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
    pub unseen: Vec<usize>,
    /// The trigger tests left out, each with why: a query a chat cannot be started on
    /// ([`left_out_why`]).
    pub left_out: Vec<LeftOut>,
    /// The token cap the grade is held to ([`cap::token_cap`]).
    pub cap: cap::Cap,
    /// What the person consents to by pressing Grade on this plan ([`consent_of`]): a grade
    /// started on another is refused.
    pub consent: Option<String>,
    /// The copy the consent is a digest of, read once: what a grade started on this plan lays
    /// out every arm from, and records its version as. Never shown, so never sent.
    #[serde(skip)]
    pub copy: Option<SkillCopy>,
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
}

/// **The copy of `name` at `level`, read once** ([`SkillCopy`]). Its content is what git says the
/// folder holds both before and after its files are read: a folder that changed while they were
/// read is `uncommitted`, so a recorded tree is never one the files laid out are not.
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
    let commit = commit_of(root, level, name, &content);
    Ok(SkillCopy {
        snapshot,
        content,
        commit,
    })
}

/// **The plan of grading the copy of `name` at `level`** in the project at `root`.
pub fn planned(root: &Path, level: &Level, name: &str) -> Result<Planned, String> {
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
    let request = (!sendable.is_empty()).then(|| GradeRequest {
        skill: name.to_owned(),
        triggers: sendable,
        outcomes: fixtures.outcomes.read().cloned().unwrap_or_default(),
        arms: ARMS.to_vec(),
        runs: RUNS,
    });
    let plan = request.as_ref().map(grade::plan);
    let estimate = plan.and_then(|plan| {
        let last = grades_of(root, name, Some(level)).into_iter().next()?;
        let chats = u64::from(last.spent.chats.max(1));
        let per = |of: u64| of.div_ceil(chats) * u64::from(plan.chats);
        Some(Estimate {
            tokens: last.spent.tokens.map(per),
            cost_usd: last
                .spent
                .cost_usd
                .map(|cost| cost / chats as f64 * f64::from(plan.chats)),
            chats_then: last.spent.chats,
        })
    });
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
    let copy = match &request {
        Some(_) => Some(copy_of(root, level, name)?),
        None => None,
    };
    let consent = request
        .as_ref()
        .zip(copy.as_ref())
        .map(|(request, copy)| consent_of(request, &copy.content, &copy.snapshot));
    Ok(Planned {
        skill: name.to_owned(),
        fixtures,
        request,
        plan,
        estimate,
        unseen,
        left_out,
        cap: cap::token_cap(root),
        consent,
        copy,
    })
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

/// **What the person consented to**: one digest of the trigger tests the plan showed, what the
/// copy's folder held by content, and the copy's files as they are now. A grade started on a
/// plan that no longer holds is refused, so what runs is what was shown.
pub fn consent_of(request: &GradeRequest, content: &Content, snapshot: &Snapshot) -> String {
    use sha2::Digest as _;
    let mut digest = sha2::Sha256::new();
    digest.update(serde_json::to_vec(&request.triggers).unwrap_or_default());
    digest.update(b"\0");
    digest.update(content.word().as_bytes());
    for (path, text) in &snapshot.files {
        digest.update(b"\0");
        digest.update(path.as_bytes());
        digest.update(b"\0");
        digest.update(text.as_bytes());
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
    let mut ready =
        crate::sandbox::policy::holding_no_kept_grants(|| crate::start::ready(start, root))?;
    chat_refused(&ready)?;
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

/// **The commit whose version of the copy of `name` at `level` is `content`**: what a grade is
/// recorded against beside the tree. A copy with changes not committed has none.
fn commit_of(root: &Path, level: &Level, name: &str, content: &Content) -> Option<String> {
    match content {
        Content::Uncommitted => None,
        Content::Tree(tree) => {
            super::history::history(root, level, name)
                .ok()
                .and_then(|history| {
                    history
                        .versions
                        .into_iter()
                        .find(|version| &version.tree == tree)
                        .map(|version| version.commit)
                })
        }
    }
}

pub mod cap;
pub mod price;
pub mod underway;

#[cfg(test)]
mod tests;
