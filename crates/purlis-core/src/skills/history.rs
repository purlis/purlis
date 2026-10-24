// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A skill's history** (SK-19, #2782; spec #2762 decision 23): every version git holds of one
//! skill, newest first, each with what changed in it, a revert to it, and the runs it had. The
//! one list the skill's tab and `purlis skill history` draw.
//!
//! A version is a commit that changed the skill's folder; a commit that changed only the file
//! beside it (its grant, lifecycle state or owners) is none. Its runs are matched **by content**:
//! each run's start records the tree git holds of the skill's folder as the chat started it
//! ([`crate::skilluse::Content`]), so a run is the version's whose folder had that tree, whenever
//! the run was and on whichever machine, a teammate's that pulled the version later included.
//! A run on a folder with changes not committed is counted apart, as on changes not committed.
//! The runs are totalled by the one reading of the log ([`crate::skilluse::stats`], by
//! [`crate::skilluse::By::Version`]). Grades join each version when grading lands (SK-16,
//! #2779), tied to its commit; until then the slot is empty.
//!
//! What git says is read, never trusted: a commit is taken only as a whole hex id, a diff is cut
//! at [`MOST_DIFF`] bytes, and a character that draws as nothing is shown by its code point and
//! said.

use std::path::{Path, PathBuf};

use super::verbs::{Ctx, Level, find, git, pending_in, rel};
use crate::planegit::literally;
use crate::skilluse::{By, Group, Totals};

/// The most versions a history lists: a skill with more lists its newest this many.
pub const MOST_VERSIONS: usize = 100;

/// The most bytes of one version's diff shown: past this the diff is cut, and says so.
pub const MOST_DIFF: usize = 256 * 1024;

/// A version's grade, once grading lands (SK-16, #2779): how many of its fixtures passed. The
/// slot is typed now so the tab and the command line have one place for it; nothing fills it yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Grade {
    pub passed: u32,
    pub fixtures: u32,
}

/// One version of a skill: one commit that changed it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Version {
    /// The commit, as a whole hex id.
    pub commit: String,
    /// When it was committed, in seconds since 1970.
    pub at: u64,
    /// Who committed it, as one line with nothing invisible in it.
    pub author: String,
    /// Its subject, as one line with nothing invisible in it.
    pub subject: String,
    /// The tree git holds of the skill's folder at it: what a run's start is matched to.
    pub tree: String,
    /// What the runs that started on it add up to.
    pub runs: Totals,
    /// Its grade, once grading lands (SK-16). Always `None` for now.
    pub grade: Option<Grade>,
}

/// Whether `word` is a whole commit id: 40 or 64 lowercase hex digits.
fn whole_id(word: &str) -> bool {
    matches!(word.len(), 40 | 64)
        && word
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The skill named `name` at `level`'s folder, and that folder relative to the plane.
fn folder_of(plane: &Path, level: &Level, name: &str) -> Result<(PathBuf, String), String> {
    let folder = find(plane, level, name)?;
    let folder_rel = rel(plane, &folder);
    Ok((folder, folder_rel))
}

/// A skill's history: its versions, and the runs on changes not committed.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct History {
    /// Its versions, newest first.
    pub versions: Vec<Version>,
    /// What the runs on its folder while it held changes not committed add up to.
    pub uncommitted: Totals,
}

/// **Every version git holds of the skill named `name` at `level`**, newest first, at most
/// [`MOST_VERSIONS`], each with the runs whose start recorded its folder's tree, and the runs on
/// changes not committed. Empty for a skill git does not track, such as a local one; refused
/// where there is no such skill.
pub fn history(plane: &Path, level: &Level, name: &str) -> Result<History, String> {
    let (_, folder_rel) = folder_of(plane, level, name)?;
    let most = MOST_VERSIONS.to_string();
    let folder_spec = literally(&folder_rel);
    let log = git(
        plane,
        &[
            "log",
            "-n",
            &most,
            "--no-color",
            "--format=%H%x1f%ct%x1f%an%x1f%s%x1e",
            "--",
            &folder_spec,
        ],
    )
    .unwrap_or_default();
    let mut versions: Vec<Version> = log
        .split('\u{1e}')
        .filter_map(|record| {
            let mut fields = record.trim_start_matches('\n').split('\u{1f}');
            let commit = fields.next()?.trim();
            let at = fields.next()?.trim().parse::<u64>().ok()?;
            let author = fields.next()?;
            let subject = fields.next()?;
            whole_id(commit).then(|| Version {
                commit: commit.to_owned(),
                at,
                author: crate::shown::one_line(author, 80),
                subject: crate::shown::one_line(subject, 160),
                tree: String::new(),
                runs: Totals::default(),
                grade: None,
            })
        })
        .collect();
    trees(plane, &folder_rel, &mut versions);
    // A commit that took the folder away holds no tree of it: it is no version to go back to.
    versions.retain(|version| !version.tree.is_empty());
    let mut history = History {
        versions,
        uncommitted: Totals::default(),
    };
    count_runs(plane, level, name, &mut history);
    Ok(history)
}

/// Fills each version's [`Version::tree`]: the tree git holds of the folder at `folder_rel` at
/// its commit, in one `git rev-parse` where every commit has the folder, else one per commit.
fn trees(plane: &Path, folder_rel: &str, versions: &mut [Version]) {
    let asked: Vec<String> = versions
        .iter()
        .map(|version| format!("{}:{folder_rel}", version.commit))
        .collect();
    let mut args = vec!["rev-parse", "--end-of-options"];
    args.extend(asked.iter().map(String::as_str));
    if let Ok(out) = git(plane, &args) {
        let lines: Vec<&str> = out.lines().map(str::trim).collect();
        if lines.len() == versions.len() && lines.iter().all(|line| whole_id(line)) {
            for (version, tree) in versions.iter_mut().zip(lines) {
                version.tree = tree.to_owned();
            }
            return;
        }
    }
    for (version, one) in versions.iter_mut().zip(&asked) {
        version.tree = git(plane, &["rev-parse", "--verify", "-q", "--end-of-options", one])
            .map(|out| out.trim().to_owned())
            .ok()
            .filter(|tree| whole_id(tree))
            .unwrap_or_default();
    }
}

/// Totals onto each version of `history` the runs of this copy of the skill `name` whose start
/// recorded its folder's tree, and onto [`History::uncommitted`] the runs on changes not committed.
fn count_runs(plane: &Path, level: &Level, name: &str, history: &mut History) {
    let copy = copy_of(level);
    let ours: Vec<crate::skilluse::Run> = crate::skilluse::runs(plane)
        .into_iter()
        .filter(|run| {
            run.is_of(name)
                && run.copy.as_ref().is_some_and(|one| {
                    one.level == copy && one.workspace.as_deref() == level.workspace_name()
                })
        })
        .collect();
    let by_tree: std::collections::HashMap<Group, Totals> = crate::skilluse::stats(&ours, By::Version)
        .into_iter()
        .map(|row| (row.group, row.totals))
        .collect();
    for version in &mut history.versions {
        if let Some(totals) = by_tree.get(&Group::Named(version.tree.clone())) {
            version.runs = *totals;
        }
    }
    history.uncommitted = by_tree.get(&Group::None).copied().unwrap_or_default();
}

/// One version's diff, as the tab and the command line show it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Diff {
    /// What the version changed, as a unified diff, each character that draws as nothing shown
    /// as `<U+XXXX>`.
    pub text: String,
    /// Whether it was cut at [`MOST_DIFF`] bytes.
    pub cut: bool,
    /// A line for each line of the diff that holds a character that draws as nothing.
    pub unseen: Vec<String>,
}

/// The version `commit` of the skill `name` at `level`, or why it is none of its history's.
fn version_of(plane: &Path, level: &Level, name: &str, commit: &str) -> Result<Version, String> {
    if !whole_id(commit) {
        return Err(format!("{commit:?} is no version of {name}"));
    }
    history(plane, level, name)?
        .versions
        .into_iter()
        .find(|version| version.commit == commit)
        .ok_or_else(|| format!("{name} has no version {}", short(commit)))
}

/// A commit's short id, as a sentence or a line of the command line names it.
pub fn short(commit: &str) -> &str {
    commit.get(..12).unwrap_or(commit)
}

/// **What version `commit` of the skill `name` at `level` changed**, against the version before
/// it: a unified diff of its folder. Refused for a commit that is none of its history's.
pub fn diff(plane: &Path, level: &Level, name: &str, commit: &str) -> Result<Diff, String> {
    version_of(plane, level, name, commit)?;
    let (_, folder_rel) = folder_of(plane, level, name)?;
    let folder_spec = literally(&folder_rel);
    let raw = git(
        plane,
        &[
            "show",
            "--format=",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            commit,
            "--",
            &folder_spec,
        ],
    )
    .map_err(|why| {
        format!(
            "{name}'s version {} could not be read: {why}",
            short(commit)
        )
    })?;
    Ok(shown_diff(&raw))
}

/// `raw` as it is shown: cut at [`MOST_DIFF`] bytes on a character's edge, each character that
/// draws as nothing (a tab aside) as its code point, and each line holding one said.
fn shown_diff(raw: &str) -> Diff {
    let cut = raw.len() > MOST_DIFF;
    let mut end = raw.len().min(MOST_DIFF);
    while !raw.is_char_boundary(end) {
        end -= 1;
    }
    let mut text = String::with_capacity(end);
    let mut unseen = Vec::new();
    for (at, line) in raw[..end].split_inclusive('\n').enumerate() {
        let (body, newline) = match line.strip_suffix('\n') {
            Some(body) => (body, "\n"),
            None => (line, ""),
        };
        let mut first: Option<char> = None;
        for c in body.chars() {
            if c != '\t' && crate::shown::invisible(c) {
                first.get_or_insert(c);
                text.push_str(&format!("<U+{:04X}>", u32::from(c)));
            } else {
                text.push(c);
            }
        }
        text.push_str(newline);
        if let Some(c) = first {
            unseen.push(format!(
                "line {} holds a character that draws as nothing (U+{:04X}), shown by its code \
                 point",
                at + 1,
                u32::from(c)
            ));
        }
    }
    Diff { text, cut, unseen }
}

/// What the instructions of version `commit` of the skill `name` say, from git, refused where
/// they are no whole skill of this name.
fn instructions_at(plane: &Path, folder_rel: &str, name: &str, commit: &str) -> Result<String, String> {
    let then = git(plane, &["show", &format!("{commit}:{folder_rel}/SKILL.md")]).map_err(|_| {
        format!(
            "version {} of {name} has no instructions to go back to",
            short(commit)
        )
    })?;
    let problems = purlis_skills::change::check(&then, name);
    if !problems.is_empty() {
        let lines: Vec<String> = problems.iter().map(ToString::to_string).collect();
        return Err(format!(
            "version {} of {name} is not a whole skill of this name, so nothing was reverted: {}",
            short(commit),
            lines.join("; ")
        ));
    }
    Ok(then)
}

/// **What a revert of the skill `name` at `level` to version `commit` would bring back that widens
/// a chat** (SK-8): the one sentence that says so ([`purlis_skills::widening_said`]), where that
/// version's frontmatter asks for it, which the revert's confirm shows. purlis still leaves it out
/// when it hands the skill to a chat. `None` where nothing widens.
pub fn revert_widens(
    plane: &Path,
    level: &Level,
    name: &str,
    commit: &str,
) -> Result<Option<String>, String> {
    version_of(plane, level, name, commit)?;
    let (_, folder_rel) = folder_of(plane, level, name)?;
    let then = instructions_at(plane, &folder_rel, name, commit)?;
    let widening: Vec<purlis_skills::Field> = purlis_skills::handed::parse(&then)
        .map(|read| read.fields().into_iter().filter(|field| field.widens()).collect())
        .unwrap_or_default();
    Ok((!widening.is_empty()).then(|| {
        format!(
            "{} purlis leaves them out when it hands this skill to a chat.",
            purlis_skills::widening_said(&widening)
        )
    }))
}

/// Refused where version `commit` of the folder at `folder_rel` holds a link or another
/// repository, by its path: a revert never brings a link back, so nothing is restored.
fn no_links_at(plane: &Path, folder_rel: &str, name: &str, commit: &str) -> Result<(), String> {
    let spec = literally(folder_rel);
    let listed = git(plane, &["ls-tree", "-r", "-z", "--end-of-options", commit, "--", &spec])
        .map_err(|why| format!("version {} of {name} could not be read: {why}", short(commit)))?;
    for entry in listed.split('\0').filter(|one| !one.is_empty()) {
        let (head, path) = entry.split_once('\t').unwrap_or((entry, ""));
        let mode = head.split(' ').next().unwrap_or_default();
        if mode == "120000" || mode == "160000" {
            let shown = path.strip_prefix(folder_rel).unwrap_or(path).trim_start_matches('/');
            return Err(format!(
                "version {} of {name} holds a link ({}), and a revert never brings a link back, \
                 so nothing was reverted",
                short(commit),
                crate::shown::one_line(shown, 120)
            ));
        }
    }
    Ok(())
}

/// **Reverts the skill `name` at `level` to version `commit`** (decision 23): its folder as that
/// commit has it, its instructions and every other file, as one commit. The file beside it (its
/// grant, lifecycle state and owners) is left as it is: those change by their own controls.
/// Refused for a commit that is none of its history's, a version whose instructions are not a
/// whole skill of this name, a version that holds a link, and a skill holding changes not
/// committed yet, which the revert would lose. What would widen a chat is the confirm's to show
/// ([`revert_widens`]).
pub fn revert(
    cx: &mut Ctx<'_>,
    level: &Level,
    name: &str,
    commit: &str,
) -> Result<PathBuf, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    version_of(plane, level, name, commit)?;
    let (folder, folder_rel) = folder_of(plane, level, name)?;
    if !pending_in(plane, &[&folder_rel])?.is_empty() {
        return Err(format!(
            "{name} holds changes that are not committed yet, and a revert would lose them: \
             save them first"
        ));
    }
    instructions_at(plane, &folder_rel, name, commit)?;
    no_links_at(plane, &folder_rel, name, commit)?;
    let source = format!("--source={commit}");
    let spec = literally(&folder_rel);
    git(plane, &["restore", &source, "--worktree", "--", &spec])
        .map_err(|why| format!("{name} was not reverted: {why}"))?;
    if pending_in(plane, &[&folder_rel])?.is_empty() {
        (cx.say)(crate::repocmd::Say::Info(format!(
            "{name} is already as version {} has it, so nothing changed.",
            short(commit)
        )));
        return Ok(folder);
    }
    cx.done(format!("{name} is as version {} has it", short(commit)));
    cx.commit(
        &[folder_rel],
        &format!("skill revert {name} {}", short(commit)),
    );
    Ok(folder)
}

/// The copy of a skill at `level`, as its runs record it (SK-13).
pub(super) fn copy_of(level: &Level) -> purlis_skills::runs::Level {
    purlis_skills::runs::Level {
        origin: level.origin(),
        local: level.local,
    }
}

#[cfg(test)]
mod tests;
