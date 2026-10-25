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
pub const DIR_NAME: &str = "_grades";

/// The version of a row this purlis writes and reads.
pub const VERSION: u32 = 1;

/// The most bytes of one month's file read, and of one row.
pub const MOST_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub const MOST_LINE_BYTES: usize = 1024 * 1024;

/// The most grades of one skill answered, newest first.
pub const MOST_GRADES: usize = 200;

/// The arms the built-in runner compares now: the skill against no skill.
pub const ARMS: [Arm; 2] = [Arm::With, Arm::Without];

/// `personas/_grades/<YYYY-MM>.<device>.jsonl`, `<device>` named as the skill log's is.
pub fn path_for(root: &Path, when: chrono::DateTime<chrono::Utc>, device: &str) -> PathBuf {
    root.join("personas")
        .join(DIR_NAME)
        .join(format!("{}.{device}.jsonl", when.format("%Y-%m")))
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
    /// The copy's level, as its folder relative to the plane (`skills`, `workspaces/ops/skills`),
    /// or `None` for a copy in a workspace that is this machine's alone, whose name never goes in
    /// a committed file.
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
}

impl Graded {
    /// What its copy's folder held.
    pub fn content(&self) -> Option<Content> {
        Content::parse(&self.tree)
    }

    /// **Whether a row read back says only what purlis writes**: a tree or `uncommitted`, a
    /// commit that is a whole hex id, a harness and a profile that are plain names, and a stamp
    /// of a time's shape. A teammate's purlis writes the record, and a person reads these words
    /// in the window and the terminal, so a row that says anything else is skipped.
    fn sound(&self) -> bool {
        let hex = |word: &str| {
            matches!(word.len(), 40 | 64)
                && word
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        let plain = |word: &str| {
            !word.is_empty()
                && word.len() <= 64
                && word
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        };
        self.content().is_some()
            && self.commit.as_deref().is_none_or(hex)
            && self.harness.as_deref().is_none_or(plain)
            && self.profile.as_deref().is_none_or(plain)
            && plain(&self.grade)
            && chrono::DateTime::parse_from_rfc3339(&self.ts).is_ok()
    }
}

/// The level `level` is recorded by: its folder relative to the plane, or `None` for a local
/// workspace's.
pub fn level_word(root: &Path, level: &Level) -> Option<String> {
    if let Some(ws) = level.at.workspace()
        && !crate::wscmd::live_workspaces(root).contains(ws)
    {
        return None;
    }
    level.rel()
}

/// **Appends `graded` to this month's file of the project at `root`**: `None` where the project
/// is one this purlis may not write, or the write was refused.
pub fn record(root: &Path, graded: &Graded, device: &str) -> Option<PathBuf> {
    if crate::compat::read_only(root).is_some() {
        return None;
    }
    let row = serde_json::to_value(graded).ok()?;
    crate::dispatch::append(&path_for(root, chrono::Utc::now(), device), root, &row)
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

/// **The grades of the skill `name`** in the project at `root`, newest first, at most
/// [`MOST_GRADES`]: every copy's, or with `level`, that copy's alone (a row of a local
/// workspace's copy says no level, and is that copy's where its name and content match).
pub fn grades_of(root: &Path, name: &str, level: Option<&Level>) -> Vec<Graded> {
    let dir = root.join("personas").join(DIR_NAME);
    let Ok(reader) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = reader
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    files.sort();
    let wanted = level.map(|level| level_word(root, level));
    let mut out: Vec<Graded> = Vec::new();
    for file in files.iter().rev() {
        let Some(text) = file_text(file) else {
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
    /// The trigger tests left out, by their place in the file, each with why: a query a chat
    /// cannot be started on ([`crate::handoff::bad_message`]).
    pub left_out: Vec<(usize, String)>,
}

/// **The plan of grading the copy of `name` at `level`** in the project at `root`.
pub fn planned(root: &Path, level: &Level, name: &str) -> Result<Planned, String> {
    let folder = find(root, level, name)?;
    let fixtures = grade::fixtures_of(root, Path::new(&rel(root, &folder)), name);
    let mut left_out: Vec<(usize, String)> = Vec::new();
    let sendable: Vec<grade::TriggerTest> = fixtures
        .triggers
        .read()
        .map(|tests| {
            tests
                .iter()
                .enumerate()
                .filter(
                    |(at, test)| match crate::handoff::bad_message(&test.query) {
                        Some(bad) => {
                            left_out.push((*at, bad.say()));
                            false
                        }
                        None => true,
                    },
                )
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
    Ok(Planned {
        skill: name.to_owned(),
        fixtures,
        request,
        plan,
        estimate,
        unseen,
        left_out,
    })
}

/// **What the copy of `name` at `level` holds now, by content, and the commit that is**: the
/// tree and commit a grade is recorded against. A copy with changes not committed is
/// `uncommitted`, with no commit.
pub fn version_of(root: &Path, level: &Level, name: &str) -> (Content, Option<String>) {
    let content = find(root, level, name)
        .ok()
        .and_then(|folder| crate::skilluse::content_at(root, &folder))
        .unwrap_or(Content::Uncommitted);
    let commit = match &content {
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
    };
    (content, commit)
}

#[cfg(test)]
mod tests;
