// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The install review** (ADR 0100 §3, decision 39), and Browse.
//!
//! The review leads with each skill's summary ([`purlis_skills::review`]): the scripts it carries,
//! the network hosts its files name, its `!` lines, the writes it would make outside the
//! workspace, every character no reader sees and every file that is not text, the risky ones
//! marked. Then every file, one click away, its text shown with every invisible character
//! escaped. The skills a skill refers to come along in the same review, each after the one that
//! brought it.

use std::path::Path;

use purlis_skills::review as summary;
use purlis_skills::tree::LeftOut;
use serde::{Deserialize, Serialize};

use super::fetch::{Fetched, Found, NotASkill, SourceFile, fetch};
use super::{Commit, Source, lock};
use crate::skills::verbs::Level;

/// One file of a skill, as the review shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewFile {
    pub path: String,
    pub size: u64,
    pub executable: bool,
    /// Its text with every invisible character escaped ([`summary::escaped`]), `None` for a file
    /// that is not text, which the review lists as not shown.
    pub text: Option<String>,
    /// Whether the summary marks something in it.
    pub risky: bool,
}

/// **One skill in an install review**: what it is, what it does beyond its instructions, every
/// file it installs, and every entry of its folder that is not installed and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillReview {
    pub name: String,
    pub description: String,
    /// Its folder in the source.
    pub folder: String,
    /// The skill whose install brings it along, as one it refers to.
    pub brought_by: Option<String>,
    pub summary: summary::Summary,
    pub files: Vec<ReviewFile>,
    pub left_out: Vec<LeftOut>,
    /// What the lint says of its instructions, and each skill it refers to that the source does
    /// not hold.
    pub lint: Vec<String>,
    /// The digest of what it installs, which the install is handed back.
    pub digest: String,
}

/// **An install review**: the source, the commit it was read at, and each skill, those it refers
/// to after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Review {
    pub source: Source,
    pub commit: Commit,
    pub skills: Vec<SkillReview>,
}

/// One skill of what was accepted: its name and the digest the review showed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AcceptedSkill {
    pub name: String,
    pub digest: String,
}

/// **What an install or an update was accepted as**: the commit and each skill's digest, as the
/// review showed them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accepted {
    pub commit: Commit,
    pub skills: Vec<AcceptedSkill>,
}

impl Accepted {
    /// What accepting `skills` at `commit` says.
    pub fn of(commit: &Commit, skills: &[SkillReview]) -> Accepted {
        Accepted {
            commit: commit.clone(),
            skills: skills
                .iter()
                .map(|skill| AcceptedSkill {
                    name: skill.name.clone(),
                    digest: skill.digest.clone(),
                })
                .collect(),
        }
    }

    /// Whether `now` is what was accepted, whatever the order.
    pub(super) fn is(&self, now: &Accepted) -> bool {
        let mut a = self.skills.clone();
        let mut b = now.skills.clone();
        a.sort();
        b.sort();
        self.commit == now.commit && a == b
    }
}

impl Review {
    /// What accepting it says.
    pub fn accepted(&self) -> Accepted {
        Accepted::of(&self.commit, &self.skills)
    }
}

/// `sha256:` and the hex digest of `bytes`.
pub(super) fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    format!(
        "sha256:{}",
        crate::extension::hex(&sha2::Sha256::digest(bytes))
    )
}

/// **The digest of a copy** whose files are `files`: over each file's path, mode and digest, in
/// path order, so the same bytes at the same paths are the same digest wherever they lie.
pub(crate) fn digest_of(files: &[lock::Wrote]) -> String {
    let mut sorted: Vec<&lock::Wrote> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut all = String::new();
    for file in sorted {
        all.push_str(&format!(
            "{}\0{}\0{}\n",
            file.path,
            if file.executable { "755" } else { "644" },
            file.sha256
        ));
    }
    sha256(all.as_bytes())
}

/// What a lockfile records of `files`.
pub(super) fn wrote(files: &[SourceFile]) -> Vec<lock::Wrote> {
    files
        .iter()
        .map(|file| lock::Wrote {
            path: file.path(),
            executable: file.executable,
            sha256: sha256(&file.bytes),
        })
        .collect()
}

/// The summary of `files`.
pub(super) fn summary_of(files: &[SourceFile]) -> summary::Summary {
    let paths: Vec<String> = files.iter().map(SourceFile::path).collect();
    let read: Vec<summary::FileIn<'_>> = files
        .iter()
        .zip(&paths)
        .map(|(file, path)| summary::FileIn {
            path,
            executable: file.executable,
            bytes: &file.bytes,
        })
        .collect();
    summary::summarize(&read)
}

/// The text of `file`, `None` where the summary reads it as not text.
fn text_of(file: &SourceFile) -> Option<&str> {
    summary::FileIn {
        path: "",
        executable: file.executable,
        bytes: &file.bytes,
    }
    .text()
}

/// The skills `files` refer to (`skills:<name>`, ADR 0099), by name, each once.
pub(super) fn referenced(files: &[SourceFile]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for text in files.iter().filter_map(text_of) {
        for name in purlis_skills::change::referenced(text, crate::skills::LINKED_PLUGIN) {
            if purlis_skills::change::name_problem(&name).is_none() && !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

/// The review of the skill `found`, whose files are `files` and whose left-out entries are
/// `left`, brought along by `brought_by`, in a source that holds the skills `held`.
pub(super) fn review_of(
    found: &Found,
    files: &[SourceFile],
    left: Vec<LeftOut>,
    brought_by: Option<String>,
    held: &[String],
) -> SkillReview {
    let summary = summary_of(files);
    let marked = |path: &str| {
        summary
            .scripts
            .iter()
            .any(|s| s.path == path && !s.risks.is_empty())
            || summary.commands.iter().any(|c| c.path == path)
            || summary.outside.iter().any(|o| o.path == path)
            || summary.unseen.iter().any(|u| u.path == path)
            || summary
                .hosts
                .iter()
                .any(|h| h.risky && h.files.iter().any(|f| f == path))
    };
    let mut lint: Vec<String> = files
        .iter()
        .find(|file| file.parts == [purlis_skills::FILE])
        .and_then(text_of)
        .map(|text| {
            purlis_skills::change::check(text, &found.name)
                .iter()
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    for name in referenced(files) {
        if name != found.name && !held.contains(&name) {
            lint.push(format!(
                "it refers to the skill {name}, which this source does not hold"
            ));
        }
    }
    SkillReview {
        name: found.name.clone(),
        description: found.description.clone(),
        folder: found.folder.clone(),
        brought_by,
        files: files
            .iter()
            .map(|file| ReviewFile {
                path: file.path(),
                size: file.bytes.len() as u64,
                executable: file.executable,
                risky: marked(&file.path()) || file.executable || text_of(file).is_none(),
                text: text_of(file).map(summary::escaped),
            })
            .collect(),
        left_out: left,
        lint,
        digest: digest_of(&wrote(files)),
        summary,
    }
}

/// **The review of `names` from `source`**, and every skill they refer to that the source holds
/// and `have` does not, each right after the one that brought it, so one that was also named is
/// still marked as brought along and an install's second review reads the same. One review for
/// every source, the git one and each extension's (ADR 0101 §1): what is reviewed is what the
/// source answered, and the review is purlis's.
pub(super) fn reviews_of(
    source: &dyn super::SkillSource,
    names: &[String],
    have: &dyn Fn(&str) -> bool,
) -> Result<Vec<SkillReview>, String> {
    let (found, _) = source.skills()?;
    let held: Vec<String> = found.iter().map(|f| f.name.clone()).collect();
    let mut queue: Vec<(String, Option<String>)> =
        names.iter().map(|name| (name.clone(), None)).collect();
    let mut skills: Vec<SkillReview> = Vec::new();
    while !queue.is_empty() {
        let (name, by) = queue.remove(0);
        if skills.iter().any(|s| s.name == name) {
            continue;
        }
        let Some(one) = found.iter().find(|f| f.name == name) else {
            return Err(format!("{} holds no skill named {name:?}", source.shown()));
        };
        let (files, left) = source.files(one)?;
        let mut next: Vec<(String, Option<String>)> = referenced(&files)
            .into_iter()
            .filter(|other| *other != name && held.contains(other) && !have(other))
            .map(|other| (other, Some(name.clone())))
            .collect();
        next.append(&mut queue);
        queue = next;
        skills.push(review_of(one, &files, left, by, &held));
    }
    Ok(skills)
}

impl Fetched {
    /// **The review of `names`** from this commit ([`reviews_of`]).
    pub(super) fn review_with(
        &self,
        names: &[String],
        have: &dyn Fn(&str) -> bool,
    ) -> Result<Review, String> {
        Ok(Review {
            source: self.source.clone(),
            commit: self.commit.clone(),
            skills: reviews_of(self, names, have)?,
        })
    }

    /// **The review of installing `names`** from this commit, and every skill they refer to.
    pub fn review(&self, names: &[String]) -> Result<Review, String> {
        self.review_with(names, &|_| false)
    }
}

/// **The review of installing `names` from `source`** at its newest commit.
pub fn review(plane: &Path, source: &Source, names: &[String]) -> Result<Review, String> {
    fetch(plane, source, None)?.review(names)
}

/// One skill a source offers, as the studio lists it to browse, and where it is installed here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Offered {
    #[serde(flatten)]
    pub skill: Found,
    /// The levels it is installed at from this source.
    pub installed: Vec<Level>,
}

/// **What a source offers**: its newest commit, its skills, and each folder that is not a skill
/// purlis offers, with why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Browse {
    pub commit: Commit,
    pub skills: Vec<Offered>,
    pub not_skills: Vec<NotASkill>,
}

/// **Browses `source`** at its newest commit.
pub fn browse(plane: &Path, source: &Source) -> Result<Browse, String> {
    let fetched = fetch(plane, source, None)?;
    let (found, not_skills) = fetched.skills()?;
    let (locked, _) = lock::every(plane);
    Ok(Browse {
        commit: fetched.commit.clone(),
        skills: found
            .into_iter()
            .map(|skill| Offered {
                installed: locked
                    .iter()
                    .filter(|(_, one)| one.name == skill.name && one.source == source.remote)
                    .map(|(_, one)| one.level.0.clone())
                    .collect(),
                skill,
            })
            .collect(),
        not_skills,
    })
}
