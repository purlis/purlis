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
//! [`crate::skilluse::By::Version`]). Grades join each version the same way (SK-16, #2779): a
//! grade in the eval record says the tree its copy's folder held ([`super::grades`]).
//!
//! **It follows the skill across a rename and a move** (#3918): where the oldest commit of its
//! folder's log brought the folder in by a rename git finds (the rename and move commits the core
//! makes, a deferred save that carried one, or a `git mv` by hand), the log goes on from that
//! commit's parent in the folder the skill had before, under the name it had there, at most
//! [`MOST_MOVES`] times. So a version from before keeps its runs and grades, which were recorded
//! by the name and level it had then ([`Was`]). Each step is one `git log` and one
//! `git diff-tree` of one commit, read only where a history is asked for, never as a chat opens.
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

/// The most renames and moves a history follows back: past them it lists what it has.
pub const MOST_MOVES: usize = 16;

/// **A version's grade** (SK-16, #2779): how many of its trigger tests passed with the skill in
/// its newest grade, of those that ran.
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
    /// Its newest grade, where it was graded (SK-16): the eval record's row whose folder held
    /// this version's tree.
    pub grade: Option<Grade>,
    /// The skill's folder relative to the plane at it, and the name it had there (#3918): the
    /// folder it has now, or one it had before a rename or a move.
    #[serde(skip)]
    pub folder: String,
    #[serde(skip)]
    pub name: String,
    /// **Where it is from before a rename or a move** (#4017): the name and the folder the skill
    /// had at it, which a revert brings back under the name and in the folder it has now. `None`
    /// for a version of the place it has now.
    pub before: Option<Before>,
}

/// The name and the folder a skill had at a version from before a rename or a move (#4017).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Before {
    pub name: String,
    /// Its folder then, relative to the plane.
    pub folder: String,
}

/// **A place a skill had** (#3918): its folder relative to the plane, the name it had there,
/// and the level whose folder that is (`None` for a folder no level's rule reads). The place it
/// has now first, then each it had before a rename or a move, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Was {
    pub folder: String,
    pub name: String,
    pub level: Option<Level>,
    /// The trees its folder held at its versions: what a grade or a run of a place it had before
    /// is matched by, since a skill that took the name there since is another.
    pub trees: Vec<String>,
}

impl Was {
    /// Whether `version` is of this place: its folder, under its name.
    fn holds(&self, version: &Version) -> bool {
        version.folder == self.folder && version.name == self.name
    }
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
/// changes not committed. The versions from before a rename or a move are listed too (#3918),
/// with the runs and grades of the name and level they had. Empty for a skill git does not
/// track, such as a local one; refused where there is no such skill.
pub fn history(plane: &Path, level: &Level, name: &str) -> Result<History, String> {
    let (versions, was) = walked(plane, level, name)?;
    let mut history = History {
        versions,
        uncommitted: Totals::default(),
    };
    count_runs(plane, &was, &mut history);
    join_grades(plane, &was, &mut history.versions);
    Ok(history)
}

/// **The places the skill named `name` at `level` had** (#3918): where it is now, then each
/// place it had before a rename or a move that git holds, newest first ([`Was`]). What its
/// Grades read their rows by, so a grade from before follows it.
pub fn places(plane: &Path, level: &Level, name: &str) -> Result<Vec<Was>, String> {
    walked(plane, level, name).map(|(_, was)| was)
}

/// The versions of the skill `name` at `level`, newest first, across its renames and moves, and
/// the places it had ([`Was`]), its runs and grades not yet joined.
fn walked(plane: &Path, level: &Level, name: &str) -> Result<(Vec<Version>, Vec<Was>), String> {
    let (_, folder_rel) = folder_of(plane, level, name)?;
    let mut was = vec![Was {
        folder: folder_rel,
        name: name.to_owned(),
        level: Some(level.clone()),
        trees: Vec::new(),
    }];
    let mut versions: Vec<Version> = Vec::new();
    // Where the log of the place before starts: the parent of the commit that left it.
    let mut before: Option<String> = None;
    // The renames and moves followed: a name changed within a folder is none.
    let mut moves = 0;
    while let Some(here) = was.last().cloned() {
        let left = MOST_VERSIONS - versions.len();
        let mut found = logged(plane, &here, before.as_deref(), left);
        let oldest = found.last().map(|version| version.commit.clone());
        let whole = found.len() < left;
        trees(plane, &here.folder, &mut found);
        // A commit that took the folder away holds no tree of it: it is no version to go back to.
        found.retain(|version| !version.tree.is_empty());
        // A rename that kept the folder changed only its `name:` (#4021): each name it had there
        // is a place of its own, so its versions keep the name, runs and grades they had.
        let mut named = named_apart(plane, &here.name, &mut found).into_iter();
        if let (Some(last), Some((_, trees))) = (was.last_mut(), named.next()) {
            last.trees = trees;
        }
        was.extend(named.map(|(name, trees)| Was {
            folder: here.folder.clone(),
            name,
            level: here.level.clone(),
            trees,
        }));
        versions.extend(found);
        let Some(oldest) = oldest.filter(|_| whole && moves < MOST_MOVES) else {
            break;
        };
        let Some(folder) = moved_from(plane, &oldest, &here.folder) else {
            break;
        };
        let parent = format!("{oldest}^");
        // A folder whose `SKILL.md` gives no plain name there is taken by its folder's name.
        let name = name_at(plane, &parent, &folder).unwrap_or_else(|| {
            folder
                .rsplit_once('/')
                .map_or(folder.as_str(), |(_, last)| last)
                .to_owned()
        });
        let level = folder
            .rsplit_once('/')
            .and_then(|(level_rel, _)| Level::of_rel(level_rel));
        was.push(Was {
            folder,
            name,
            level,
            trees: Vec::new(),
        });
        moves += 1;
        before = Some(parent);
    }
    if let Some(now) = was.first() {
        for version in versions.iter_mut().filter(|version| !now.holds(version)) {
            version.before = Some(Before {
                name: version.name.clone(),
                folder: version.folder.clone(),
            });
        }
    }
    Ok((versions, was))
}

/// The newest `most` commits from `start` (`HEAD` where `None`) that changed the folder of
/// `here`, as versions with no tree yet.
fn logged(plane: &Path, here: &Was, start: Option<&str>, most: usize) -> Vec<Version> {
    if most == 0 {
        return Vec::new();
    }
    let most = most.to_string();
    let folder_spec = literally(&here.folder);
    let mut args = vec![
        "log",
        "-n",
        most.as_str(),
        "--no-color",
        "--format=%H%x1f%ct%x1f%an%x1f%s%x1e",
    ];
    args.extend(start);
    args.extend(["--", folder_spec.as_str()]);
    let log = git(plane, &args).unwrap_or_default();
    log.split('\u{1e}')
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
                folder: here.folder.clone(),
                name: here.name.clone(),
                before: None,
            })
        })
        .collect()
}

/// **The folder `commit` moved the skill's folder `folder` from** (#3918), where it brought it
/// in by a rename git finds: a file of `folder` renamed from the same path below another folder,
/// its `SKILL.md` first. `None` where it made the folder some other way.
fn moved_from(plane: &Path, commit: &str, folder: &str) -> Option<String> {
    if !whole_id(commit) {
        return None;
    }
    let out = git(
        plane,
        &[
            "diff-tree",
            "-r",
            "-z",
            "-M",
            "--no-commit-id",
            "--name-status",
            "--diff-filter=R",
            commit,
        ],
    )
    .ok()?;
    moved_from_said(&out, folder)
}

/// [`moved_from`] read from `out`, what `git diff-tree -z --name-status --diff-filter=R` said:
/// `R<score>`, the path it was, the path it is, each ended by a NUL.
fn moved_from_said(out: &str, folder: &str) -> Option<String> {
    let mut words = out.split('\0');
    let mut found: Option<String> = None;
    while let Some(status) = words.next() {
        if !status.starts_with('R') {
            continue;
        }
        let (Some(was), Some(is)) = (words.next(), words.next()) else {
            break;
        };
        let Some(below) = is
            .strip_prefix(folder)
            .and_then(|rest| rest.strip_prefix('/'))
        else {
            continue;
        };
        let Some(earlier) = was
            .strip_suffix(below)
            .and_then(|rest| rest.strip_suffix('/'))
            .filter(|earlier| !earlier.is_empty() && *earlier != folder)
        else {
            continue;
        };
        if below == "SKILL.md" {
            return Some(earlier.to_owned());
        }
        found.get_or_insert_with(|| earlier.to_owned());
    }
    found
}

/// The name the `SKILL.md` in `folder` at `commit` gives the skill, where it gives a plain one.
fn name_at(plane: &Path, commit: &str, folder: &str) -> Option<String> {
    let text = git(plane, &["show", &format!("{commit}:{folder}/SKILL.md")]).ok()?;
    name_in(&text)
}

/// The name `text`, a `SKILL.md`, gives the skill, where it gives a plain one.
fn name_in(text: &str) -> Option<String> {
    purlis_skills::frontmatter(text)
        .into_iter()
        .find(|(key, _)| key == "name")
        .map(|(_, name)| name)
        .filter(|name| purlis_skills::change::name_problem(name).is_none())
}

/// **The names one folder's `versions` had, newest first** (#4021): each name with the trees of
/// its versions, and each version given its name. Their `SKILL.md`s are read in one
/// `git cat-file --batch`, by each version's tree ([`named_by`]).
fn named_apart(plane: &Path, name: &str, versions: &mut [Version]) -> Vec<(String, Vec<String>)> {
    let trees: Vec<&str> = versions
        .iter()
        .map(|version| version.tree.as_str())
        .collect();
    let said = names_of_trees(plane, &trees);
    named_by(name, versions, &said)
}

/// [`named_apart`], each of `versions` having said the name in `said` (`None` for none read), in
/// order. The first name is `name`, the place's: a version whose `SKILL.md` names the skill
/// otherwise than the one after it (or, the newest, than the place) starts the next, older name,
/// so versions committed before a rename that is not committed yet keep the name they had. That
/// first name may then hold no version. A version whose `SKILL.md` gives no plain name keeps the
/// name of the one after it.
fn named_by(
    name: &str,
    versions: &mut [Version],
    said: &[Option<String>],
) -> Vec<(String, Vec<String>)> {
    let mut names: Vec<(String, Vec<String>)> = vec![(name.to_owned(), Vec::new())];
    let mut last_said: Option<&str> = Some(name);
    for (at, version) in versions.iter_mut().enumerate() {
        if let Some(read) = said.get(at).and_then(Option::as_deref) {
            if last_said.is_some_and(|last| last != read) {
                names.push((read.to_owned(), Vec::new()));
            }
            last_said = Some(read);
        }
        if let Some((name, trees)) = names.last_mut() {
            version.name.clone_from(name);
            trees.push(version.tree.clone());
        }
    }
    names
}

/// The most bytes of a `SKILL.md` whose name is read: past this it gives none.
const MOST_NAMED_BYTES: usize = 1024 * 1024;

/// The name the `SKILL.md` of each of `trees` (a skill's folder) gives, in order, from one
/// `git cat-file --batch`: `None` where it holds none, gives no plain name, or git answers
/// nothing.
fn names_of_trees(plane: &Path, trees: &[&str]) -> Vec<Option<String>> {
    // Only a whole id is asked for, so a line is never one git could read as more.
    let asked: Vec<&str> = trees
        .iter()
        .copied()
        .filter(|tree| whole_id(tree))
        .collect();
    if asked.is_empty() {
        return vec![None; trees.len()];
    }
    let input: String = asked
        .iter()
        .map(|tree| format!("{tree}:SKILL.md\n"))
        .collect();
    let mut texts = crate::worktree::git::run_with_input(
        plane,
        &["cat-file", "--batch"],
        input.into_bytes(),
        crate::worktree::git::READ,
    )
    .ok()
    .filter(|run| run.code == Some(0))
    .map(|run| batch_texts(&run.out))
    .unwrap_or_default()
    .into_iter();
    trees
        .iter()
        .map(|tree| {
            if !whole_id(tree) {
                return None;
            }
            texts.next().flatten().as_deref().and_then(name_in)
        })
        .collect()
}

/// Each object `git cat-file --batch` answered with, in order, as text: `None` for one missing,
/// not a file, not text, or past [`MOST_NAMED_BYTES`]. It stops at an answer that does not read.
fn batch_texts(mut out: &[u8]) -> Vec<Option<String>> {
    let mut texts = Vec::new();
    while let Some(end) = out.iter().position(|&b| b == b'\n') {
        let header = String::from_utf8_lossy(&out[..end]).into_owned();
        out = &out[end + 1..];
        let words: Vec<&str> = header.split(' ').collect();
        let (kind, size) = match words.as_slice() {
            [_, "missing" | "ambiguous"] => {
                texts.push(None);
                continue;
            }
            [_, kind, size] => match size.parse::<usize>() {
                Ok(size) => (*kind, size),
                Err(_) => break,
            },
            _ => break,
        };
        // The object's bytes, then a newline.
        let Some(body) = out.get(..size) else {
            break;
        };
        out = out.get(size + 1..).unwrap_or_default();
        texts.push(
            (kind == "blob" && size <= MOST_NAMED_BYTES)
                .then(|| String::from_utf8(body.to_vec()).ok())
                .flatten(),
        );
    }
    texts
}

/// Gives each of `versions` of the skill that had the places `was` its newest grade: the eval
/// record's row of the place it had then whose folder held the version's tree, read once.
fn join_grades(plane: &Path, was: &[Was], versions: &mut [Version]) {
    if versions.is_empty() {
        return;
    }
    let grades = super::grades::read_of(plane, was).grades;
    for version in versions {
        let Some(place) = was.iter().find(|place| place.holds(version)) else {
            continue;
        };
        let level = place.level.as_ref().and_then(Level::rel);
        version.grade = grades
            .iter()
            .find(|graded| {
                graded.tree == version.tree
                    && graded.skill == place.name
                    && (graded.level.is_none() || graded.level == level)
            })
            .map(|graded| {
                let (passed, fixtures) = graded.report.passed_with();
                Grade { passed, fixtures }
            });
    }
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
        version.tree = git(
            plane,
            &["rev-parse", "--verify", "-q", "--end-of-options", one],
        )
        .map(|out| out.trim().to_owned())
        .ok()
        .filter(|tree| whole_id(tree))
        .unwrap_or_default();
    }
}

/// Totals onto each version of `history` the runs of the skill at the place it had then
/// ([`Was`]) whose start recorded its folder's tree, and onto [`History::uncommitted`] the runs
/// of its copy now on changes not committed. A run is of a place by the name and the copy it
/// recorded.
fn count_runs(plane: &Path, was: &[Was], history: &mut History) {
    // Each run goes to the first place it is of, read in one pass.
    let mut ours: Vec<Vec<crate::skilluse::Run>> = vec![Vec::new(); was.len()];
    for run in crate::skilluse::runs(plane) {
        if let Some(at) = was.iter().position(|place| ran_at(&run, place)) {
            ours[at].push(run);
        }
    }
    for (at, (place, runs)) in was.iter().zip(&ours).enumerate() {
        let by_tree: std::collections::HashMap<Group, Totals> =
            crate::skilluse::stats(runs, By::Version)
                .into_iter()
                .map(|row| (row.group, row.totals))
                .collect();
        for version in history
            .versions
            .iter_mut()
            .filter(|version| place.holds(version))
        {
            if let Some(totals) = by_tree.get(&Group::Named(version.tree.clone())) {
                version.runs = *totals;
            }
        }
        if at == 0 {
            history.uncommitted = by_tree.get(&Group::None).copied().unwrap_or_default();
        }
    }
}

/// Whether `run` is of the skill's copy at `place`: by the name and the copy it recorded.
fn ran_at(run: &crate::skilluse::Run, place: &Was) -> bool {
    let Some(level) = &place.level else {
        return false;
    };
    run.is_of(&place.name)
        && run.copy.as_ref().is_some_and(|one| {
            // A persona's copy goes with it into every workspace: it is its persona's runs,
            // wherever they ran (SK-32).
            one.level == copy_of(level)
                && match &level.at {
                    super::verbs::Whose::Persona(persona) => {
                        run.persona.as_deref() == Some(persona.as_str())
                    }
                    _ => one.workspace.as_deref() == level.at.workspace(),
                }
        })
}

/// One version's diff, as the tab and the command line show it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Diff {
    /// What the version changed, as a unified diff, each character that draws as nothing shown
    /// as `⟦U+XXXX⟧`.
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
    // Its place alone is wanted, not its runs and grades.
    walked(plane, level, name)?
        .0
        .into_iter()
        .find(|version| version.commit == commit)
        .ok_or_else(|| format!("{name} has no version {}", short(commit)))
}

/// The version `commit` of the skill `name` at `level`, the folder the skill has now, and that
/// folder relative to the plane.
fn version_here(
    plane: &Path,
    level: &Level,
    name: &str,
    commit: &str,
) -> Result<(Version, PathBuf, String), String> {
    let version = version_of(plane, level, name, commit)?;
    let (folder, folder_rel) = folder_of(plane, level, name)?;
    Ok((version, folder, folder_rel))
}

/// A commit's short id, as a sentence or a line of the command line names it.
pub fn short(commit: &str) -> &str {
    commit.get(..12).unwrap_or(commit)
}

/// **What version `commit` of the skill `name` at `level` changed**, against the version before
/// it: a unified diff of its folder. The version that renamed or moved it shows that (#3918),
/// against the folder it had before. Refused for a commit that is none of its history's.
pub fn diff(plane: &Path, level: &Level, name: &str, commit: &str) -> Result<Diff, String> {
    if !whole_id(commit) {
        return Err(format!("{commit:?} is no version of {name}"));
    }
    let (versions, _) = walked(plane, level, name)?;
    let at = versions
        .iter()
        .position(|version| version.commit == commit)
        .ok_or_else(|| format!("{name} has no version {}", short(commit)))?;
    // A version from before a rename or a move changed the folder the skill had then.
    let mut specs = vec![literally(&versions[at].folder)];
    if let Some(older) = versions.get(at + 1)
        && older.folder != versions[at].folder
    {
        specs.push(literally(&older.folder));
    }
    let mut args = vec![
        "show",
        "--format=",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "-M",
        commit,
        "--",
    ];
    args.extend(specs.iter().map(String::as_str));
    let raw = git(plane, &args).map_err(|why| {
        format!(
            "{name}'s version {} could not be read: {why}",
            short(commit)
        )
    })?;
    Ok(shown_diff(&raw))
}

/// `raw` as it is shown: cut at [`MOST_DIFF`] bytes on a character's edge, each character that
/// draws as nothing (a tab aside) as its mark, `⟦U+XXXX⟧`, and each line holding one said.
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
        // purlis's one rule and its one mark (SK-49): a tab is the diff's own layout.
        let first = purlis_unicode::hidden(body, purlis_unicode::Breaks::Kept).next();
        text.push_str(&purlis_unicode::marked(body, purlis_unicode::Breaks::Kept));
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

/// **What the instructions of `version` of the skill `name` say**, from git: as it holds them in
/// the folder the skill had then, and as a revert brings them back. A version from before a
/// rename (#4017) comes back with its `name:` line changed to the name the skill has now.
/// Refused where they are no whole skill of this name.
fn instructions_of(
    plane: &Path,
    version: &Version,
    name: &str,
) -> Result<(String, String), String> {
    let commit = version.commit.as_str();
    let then = git(
        plane,
        &["show", &format!("{commit}:{}/SKILL.md", version.folder)],
    )
    .map_err(|_| {
        format!(
            "version {} of {name} has no instructions to go back to",
            short(commit)
        )
    })?;
    let back = if name_in(&then).as_deref() == Some(name) {
        then.clone()
    } else {
        purlis_skills::change::with_name(&then, name).ok_or_else(|| {
            format!(
                "version {} of {name} has no name: line to change to {name}, so nothing was \
                 reverted",
                short(commit)
            )
        })?
    };
    let problems = purlis_skills::change::check(&back, name);
    if !problems.is_empty() {
        let lines: Vec<String> = problems.iter().map(ToString::to_string).collect();
        return Err(format!(
            "version {} of {name} is not a whole skill of this name, so nothing was reverted: {}",
            short(commit),
            lines.join("; ")
        ));
    }
    Ok((then, back))
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
    let (version, _, _) = version_here(plane, level, name, commit)?;
    let (_, then) = instructions_of(plane, &version, name)?;
    let widening: Vec<purlis_skills::Field> = purlis_skills::handed::parse(&then)
        .map(|read| {
            read.fields()
                .into_iter()
                .filter(|field| field.widens())
                .collect()
        })
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
    let listed = git(
        plane,
        &[
            "ls-tree",
            "-r",
            "-z",
            "--end-of-options",
            commit,
            "--",
            &spec,
        ],
    )
    .map_err(|why| {
        format!(
            "version {} of {name} could not be read: {why}",
            short(commit)
        )
    })?;
    for entry in listed.split('\0').filter(|one| !one.is_empty()) {
        let (head, path) = entry.split_once('\t').unwrap_or((entry, ""));
        let mode = head.split(' ').next().unwrap_or_default();
        if mode == "120000" || mode == "160000" {
            let shown = path
                .strip_prefix(folder_rel)
                .unwrap_or(path)
                .trim_start_matches('/');
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
/// grant, lifecycle state and owners) is left as it is: those change by their own controls. A
/// version from before a move or a rename (#4017) comes back in the folder the skill has now, and
/// under the name it has now ([`instructions_of`]). Refused for a commit that is none of its history's, a version whose instructions are not a
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
    let (version, folder, folder_rel) = version_here(plane, level, name, commit)?;
    if !pending_in(plane, &[&folder_rel])?.is_empty() {
        return Err(format!(
            "{name} holds changes that are not committed yet, and a revert would lose them: \
             save them first"
        ));
    }
    instructions_of(plane, &version, name)?;
    no_links_at(plane, &version.folder, name, commit)?;
    put_back(plane, &version, &folder_rel, name).inspect_err(|_| {
        as_committed(plane, &folder_rel);
    })?;
    // From before a rename: its `name:` line says the name it has now, changed in the file as
    // git wrote it out (line ends and filters applied), as a rename changes it.
    named_now(plane, &folder, &folder_rel, name)
        .inspect_err(|_| as_committed(plane, &folder_rel))
        .map_err(|why| format!("{name} was not reverted: {why}"))?;
    if pending_in(plane, &[&folder_rel])?.is_empty() {
        (cx.say)(crate::repocmd::Say::Info(
            format!(
                "{name} is already as version {} has it, so nothing changed.",
                short(commit)
            )
            .into(),
        ));
        return Ok(folder);
    }
    cx.done(match &version.before {
        None => format!("{name} is as version {} has it", short(commit)),
        Some(before) => format!(
            "{name} is as version {} has it, from when it was {} in {}",
            short(commit),
            before.name,
            before.folder
        ),
    });
    cx.commit(
        &[folder_rel],
        &format!("skill revert {name} {}", short(commit)),
    );
    Ok(folder)
}

/// Changes the `name:` line of the instructions in `folder` to `name`, where they give another.
fn named_now(plane: &Path, folder: &Path, folder_rel: &str, name: &str) -> Result<(), String> {
    let what = format!("{name}'s instructions");
    let file = format!("{folder_rel}/SKILL.md");
    let text = super::verbs::text_at(plane, &folder.join("SKILL.md"), &what)?;
    if name_in(&text).as_deref() == Some(name) {
        return Ok(());
    }
    let named = purlis_skills::change::with_name(&text, name)
        .ok_or_else(|| format!("{what} has no name: line to change"))?;
    super::verbs::write(plane, &file, &what, Some(&text), &named, usize::MAX)
}

/// **Puts the folder at `folder_rel` back as `version` holds the skill's folder**: by
/// `git restore` where it is the same folder, else (a version from before a move or a rename
/// that moved the folder, #4017) its tree read into an index of its own under `folder_rel` and
/// checked out from there, every file git tracks in the folder now that the version has not
/// taken away first. Nothing is written through a link: git checks each path out as git does.
fn put_back(plane: &Path, version: &Version, folder_rel: &str, name: &str) -> Result<(), String> {
    let commit = version.commit.as_str();
    let not = |why: String| format!("{name} was not reverted: {why}");
    if version.folder == folder_rel {
        let source = format!("--source={commit}");
        let spec = literally(folder_rel);
        return git(plane, &["restore", &source, "--worktree", "--", &spec])
            .map(|_| ())
            .map_err(not);
    }
    if !whole_id(&version.tree) {
        return Err(not(format!("version {} holds no folder", short(commit))));
    }
    let index = git(
        plane,
        &[
            "rev-parse",
            "--git-path",
            &format!(
                "purlis-revert-{}-{}.index",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|at| at.as_nanos())
                    .unwrap_or_default()
            ),
        ],
    )
    .map(|out| plane.join(out.trim()))
    .map_err(not)?;
    let put = put_back_by_index(plane, &index, &version.tree, folder_rel);
    let _ = std::fs::remove_file(&index);
    put.map_err(not)
}

/// [`put_back`] from another folder: `tree` read into the index at `index` below `folder_rel`,
/// what the folder tracks now and the tree has not removed, then the tree checked out.
fn put_back_by_index(
    plane: &Path,
    index: &Path,
    tree: &str,
    folder_rel: &str,
) -> Result<(), String> {
    let with = |args: &[&str]| -> Result<String, String> {
        let run = crate::worktree::git::run_with_index(plane, index, args)
            .map_err(|why| why.to_string())?;
        if run.ok() {
            Ok(run.out)
        } else {
            Err(crate::shown::short(run.err.trim()))
        }
    };
    with(&["read-tree", &format!("--prefix={folder_rel}/"), tree])?;
    let then: std::collections::HashSet<String> = with(&["ls-files", "-z"])?
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect();
    let now = git(plane, &["ls-files", "-z", "--", &literally(folder_rel)])?;
    for gone in now
        .split('\0')
        .filter(|path| !path.is_empty() && !then.contains(*path))
    {
        removed(plane, gone)?;
        // A folder it leaves empty goes too, up to the skill's own, as a revert in place leaves
        // none of the version's.
        let mut up = Path::new(gone).parent();
        while let Some(dir) =
            up.filter(|dir| dir.starts_with(folder_rel) && *dir != Path::new(folder_rel))
        {
            if std::fs::remove_dir(plane.join(dir)).is_err() {
                break;
            }
            up = dir.parent();
        }
    }
    with(&["checkout-index", "-a", "-f"])?;
    Ok(())
}

/// Removes the file git tracks at `rel` in the plane, reached through folders alone: where a
/// folder on its way is a link, nothing is removed and that is said.
fn removed(plane: &Path, rel: &str) -> Result<(), String> {
    let mut at = plane.to_path_buf();
    let mut parts = rel.split('/').peekable();
    while let Some(part) = parts.next() {
        at.push(part);
        if parts.peek().is_some() && !std::fs::symlink_metadata(&at).is_ok_and(|meta| meta.is_dir())
        {
            return Err(format!(
                "{} is not a folder, so nothing was removed below it",
                crate::shown::one_line(rel, 120)
            ));
        }
    }
    match std::fs::remove_file(&at) {
        Err(why) if why.kind() != std::io::ErrorKind::NotFound => Err(format!(
            "{} could not be removed: {why}",
            crate::shown::one_line(rel, 120)
        )),
        _ => Ok(()),
    }
}

/// Puts the folder at `folder_rel` back as `HEAD` has it, after a revert that failed part way:
/// what it changed is restored and what it added is removed. The folder held nothing not
/// committed before it.
fn as_committed(plane: &Path, folder_rel: &str) {
    let spec = literally(folder_rel);
    let _ = git(
        plane,
        &["restore", "--source=HEAD", "--worktree", "--", &spec],
    );
    let _ = git(plane, &["clean", "-f", "-d", "-q", "--", &spec]);
}

/// The most bytes of one file of an earlier version read for a grade (SK-57): a file past it is
/// left out, as a skill's file that large is no instruction a chat reads whole.
pub const MOST_VERSION_FILE_BYTES: u64 = 1024 * 1024;

/// **The skill whose folder is `folder_rel` as version `commit` holds it** (SK-57, #3237): what a
/// grade lays its previous version's arm out from. It is read from git's tree of that commit,
/// never from the folder, so nothing a chat could write since reaches it: every plain file git
/// holds there, as text, at most [`super::verbs::drafts::MOST_FILES`] of them and
/// [`super::verbs::drafts::DEEPEST`] folders deep, as Try it copies the folder. A link or another
/// repository is never followed, and is left out with the fixtures (`evals/`), every dot-name, a
/// file past [`MOST_VERSION_FILE_BYTES`], and a file that is not text.
pub(super) fn files_of_version(
    plane: &Path,
    folder_rel: &str,
    name: &str,
    commit: &str,
) -> Result<super::verbs::drafts::Snapshot, String> {
    use super::verbs::drafts::{DEEPEST, MOST_FILES, Snapshot};
    let unread = |why: &str| {
        format!(
            "version {} of {name} could not be read: {why}",
            short(commit)
        )
    };
    if !whole_id(commit) {
        return Err(unread("it is no commit"));
    }
    let spec = literally(folder_rel);
    let listed = git(
        plane,
        &[
            "ls-tree",
            "-r",
            "-z",
            "-l",
            "--full-tree",
            "--end-of-options",
            commit,
            "--",
            &spec,
        ],
    )
    .map_err(|why| unread(&why))?;
    let mut files = std::collections::BTreeMap::new();
    for entry in listed.split('\0').filter(|one| !one.is_empty()) {
        if files.len() >= MOST_FILES {
            break;
        }
        // `<mode> <type> <object> <size>\t<path>`, the size padded to the left.
        let Some((head, path)) = entry.split_once('\t') else {
            continue;
        };
        let mut words = head.split_whitespace();
        let (Some(mode), Some(kind), Some(object), Some(size)) =
            (words.next(), words.next(), words.next(), words.next())
        else {
            continue;
        };
        // A plain file alone: a link (120000) or another repository (160000) is never followed.
        if kind != "blob" || !matches!(mode, "100644" | "100755") || !whole_id(object) {
            continue;
        }
        let Some(below) = path
            .strip_prefix(folder_rel)
            .and_then(|rest| rest.strip_prefix('/'))
        else {
            continue;
        };
        let parts: Vec<&str> = below.split('/').collect();
        if parts.first() == Some(&purlis_skills::grade::EVALS_DIR)
            || parts.len() > DEEPEST
            || parts
                .iter()
                .any(|part| part.is_empty() || part.starts_with('.'))
        {
            continue;
        }
        if size
            .parse::<u64>()
            .map_or(true, |size| size > MOST_VERSION_FILE_BYTES)
        {
            continue;
        }
        let run = crate::worktree::git::run_raw(
            plane,
            &["cat-file", "blob", object],
            crate::worktree::git::READ,
        )
        .map_err(|why| unread(&why.to_string()))?;
        if run.code != Some(0) {
            return Err(unread(&crate::shown::short(&String::from_utf8_lossy(
                &run.err,
            ))));
        }
        // A file that is not text is left out, as Try it leaves it out of the folder.
        if let Ok(text) = String::from_utf8(run.out) {
            files.insert(below.to_owned(), text);
        }
    }
    let Some(instructions) = files.get("SKILL.md") else {
        return Err(unread("it holds no SKILL.md"));
    };
    // A version that is no whole skill of this name (one from before a rename) would be started,
    // if at all, by another name, and every test would read as one it did not trigger.
    if !purlis_skills::change::check(instructions, name).is_empty() {
        return Err(format!(
            "version {} of {name} is not a whole skill of this name",
            short(commit)
        ));
    }
    Ok(Snapshot {
        name: name.to_owned(),
        files,
    })
}

/// **The version a grade of the copy of `name` whose folder holds `tree` compares with** (SK-57,
/// #3237): the newest of `history`'s versions whose folder held something else. For a copy with
/// changes not committed (`tree` is `None`) that is its newest version; for one committed, the
/// version before it. `None` for a skill with no other version, and where that version is from
/// before a rename (#3918): it was another skill's name, which no chat would start it by.
pub fn previous_of<'a>(
    history: &'a History,
    name: &str,
    tree: Option<&str>,
) -> Option<&'a Version> {
    history
        .versions
        .iter()
        .find(|version| tree != Some(version.tree.as_str()))
        .filter(|version| version.name == name)
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
