// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What can run later and has appeared in a sandboxed chat's folders** (ADR 0067 §5, amended
//! 2026-11-05): told to the person, and never touched.
//!
//! A chat's sandbox refuses a write of a name a program run later loads ([`super::PLANTED`]).
//! This is the second look, for every harness: as a chat starts ([`begin`]) purlis records which
//! of those names are in the folders it may write, and at each end of its turn ([`look`]) it
//! says which have appeared since. Each stays on a Notice ([`told`]) until the person puts it
//! away ([`put_away`]). Nothing here refuses, deletes or changes anything: it says what is
//! there.
//!
//! # What is looked at
//!
//! - **The folders the chat may write** ([`Watched`]): its own, the folders a person granted it,
//!   and at the floor the project's. A temp folder and a package cache are not looked at:
//!   nothing a later program loads comes from one.
//! - **At the floor, the home's own names**, each asked for directly. The home is not walked.
//! - **`node_modules` and `target`** are not entered. A manifest is not one of these names
//!   ([`super::manifests_held`], #1336).
//! - **A name, once.** What is below a name is not looked through, as a whole `.vscode` or `.git`
//!   is one thing to say. So a change inside a name already there is not told.
//! - **Nothing empty.** An empty file or folder runs nothing, and on Linux a harness's sandbox
//!   may hold a path by putting an empty one there while its chat runs ([`has_content`]).
//!
//! The walk has a bound ([`LOOKED_AT`], [`MOST_NAMES`]), and stops for a folder it cannot read.
//! Either way the record says the look was cut short, and the Notice says the check is
//! incomplete: a look that was cut short never reads as "nothing appeared". A workspace's many
//! branch folders can hold millions of entries, so the bound is met in ordinary use: the walk goes
//! level by level, so it covers the top of every folder first, and a name it first reaches
//! below where it has been whole is taken as there, not as new ([`Record::whole_to`]). The person
//! is told once a set of folders that the check there is incomplete.
//!
//! # What may never happen here
//!
//! **Nothing found is run.** No link is followed, no program is started, and git is never run:
//! a repository's own config or hooks would run with it. What this reads of a repository it reads
//! by gitoxide, opened [isolated](gix::open::Options::isolated), and only for repositories it
//! knows by something a chat cannot write ([`Known`]). A file that is not a regular file is never
//! opened, so a FIFO cannot hold the look.
//!
//! # The app's own work, and the person's
//!
//! What the app checks out carries these names too, and the look tells it apart by what is on
//! disk, so it holds for every way a checkout is made, the person's own terminal included, and
//! nothing has to remember to say it acted ([`Known`]). One it can't tell apart is told: this
//! says what appeared and never who put it there.
//!
//! # The record
//!
//! `<state>/app/arrived/<chat>.json`, by the chat's own id, so it outlives a restart. It is
//! under the app's folder of the state, which every harness is compiled to deny a chat writing.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::Metadata;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::{PLANTED, Reach};

/// The name of the folder of the records, in the app's folder of the state.
pub const DIR: &str = "arrived";

/// How many directory entries one look goes through in all, shallowest first.
pub const LOOKED_AT: usize = 500_000;

/// How many names one look keeps. A folder that holds more is not a project anyone can read a
/// list of, and the look says it was cut short.
pub const MOST_NAMES: usize = 20_000;

/// How many new names one look checks against a known repository ([`Known`]). Past it a name is
/// told: checking must not cost more than telling does.
const MOST_CHECKED: usize = 512;

/// How many files below one name are checked against a repository.
const MOST_FILES_CHECKED: usize = 2_000;

/// The largest file checked against a repository.
const LARGEST_CHECKED: u64 = 16 * 1024 * 1024;

/// How many remote-tracking branches of a clone are read for one look.
const MOST_BRANCHES: usize = 256;

/// The most read of a git pointer or a workspace's manifest.
const LARGEST_READ: u64 = 1024 * 1024;

/// The audit event of what a look found.
pub const AUDITED: &str = "trust.sandbox.arrived";

/// How long a chat's start waits for [`begin`] before it goes on, as a person feels a longer one.
pub const BEGIN_WAITS_AT_MOST: std::time::Duration = std::time::Duration::from_millis(250);

/// How many paths one Notice names: the rest it counts.
pub const MOST_SHOWN: usize = 12;

/// The directories a look never enters.
const SKIPPED: [&str; 2] = ["node_modules", "target"];

/// Where one chat's names are looked for.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Watched {
    /// The folders walked: what the chat may write, never a temp folder or a cache.
    pub folders: Vec<PathBuf>,
    /// The home whose own names are asked for, for a chat at the floor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<PathBuf>,
    /// The names a person lifted for this chat ([`super::grant::lift`]): the chat may write
    /// them, so they say nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifted: Vec<String>,
}

/// One chat's record.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Record {
    v: u32,
    /// What the chat was last called, for the Notice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    watched: Watched,
    /// The names there as the chat started, and what the look took for the app's own since.
    baseline: BTreeSet<String>,
    /// What is on the Notice now.
    arrived: BTreeSet<String>,
    /// What was told and put away, here or on another chat's Notice.
    put_away: BTreeSet<String>,
    /// Whether the last look was cut short.
    cut_short: bool,
    /// Whether the person put away a Notice that said so.
    cut_short_told: bool,
    /// Where a look was cut short, the deepest level below a folder that some look has read whole,
    /// and none once one has read all of it. A name found deeper was not known to be missing
    /// before, so it says nothing about what is new.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    whole_to: Option<usize>,
}

/// What one look found, for the audit: paths and no contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Looked {
    /// What this look put on the Notice.
    pub new: Vec<String>,
    /// Whether it stopped at its bound, or at a folder it could not read.
    pub cut_short: bool,
}

impl Looked {
    /// The audit's body: `actor_kind` and `actor` the host's, as purlis looked, and paths with no
    /// contents.
    pub fn body(&self) -> serde_json::Value {
        serde_json::json!({
            "actor_kind": "host",
            "actor": "purlis",
            "paths": self.new,
            "cut_short": self.cut_short,
        })
    }

    /// Whether this look has anything for the audit.
    pub fn says_anything(&self) -> bool {
        !self.new.is_empty()
    }
}

/// One chat's Notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Told {
    /// The chat's id: what [`put_away`] takes.
    pub chat: String,
    /// What it was called.
    pub name: Option<String>,
    /// Each path once, as it is on disk.
    pub paths: Vec<String>,
    pub cut_short: bool,
}

impl Told {
    /// What the Notice says, each path marked so that no character of its name can forge a line
    /// ([`crate::shown::line`]), and below `plane` where it is. A chat chooses these names.
    pub fn words(&self, plane: &Path) -> String {
        let chat = self
            .name
            .as_deref()
            .map_or_else(|| "A chat".to_owned(), crate::shown::line);
        let mut out = String::new();
        if !self.paths.is_empty() {
            let shown: Vec<String> = self
                .paths
                .iter()
                .take(MOST_SHOWN)
                .map(|path| {
                    let path = Path::new(path);
                    crate::shown::line(&path.strip_prefix(plane).unwrap_or(path).to_string_lossy())
                })
                .collect();
            let more = self.paths.len().saturating_sub(MOST_SHOWN);
            let rest = if more > 0 {
                format!(", and {more} more")
            } else {
                String::new()
            };
            out.push_str(&format!(
                "{chat}: these appeared in a folder it can write: {}{rest}. They can run outside \
                 the sandbox if you or a tool work in that folder. purlis changed nothing.",
                shown.join(", ")
            ));
        } else {
            out.push_str(&format!("{chat}:"));
        }
        if self.cut_short {
            out.push_str(
                " purlis stopped before it had looked through all of this chat's folders, so \
                 this check is incomplete.",
            );
        }
        out
    }
}

/// The folder of the records: `<state>/app/arrived/`, which every harness denies a chat writing
/// ([`super::Denied::of`]).
pub fn dir(plane: &Path) -> PathBuf {
    crate::names::state(plane).join("app").join(DIR)
}

fn file(plane: &Path, chat: &str) -> io::Result<PathBuf> {
    crate::contain::mintable(chat)
        .map(|()| dir(plane).join(format!("{chat}.json")))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "not a chat's id"))
}

fn read(path: &Path) -> io::Result<Option<Record>> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|why| io::Error::new(io::ErrorKind::InvalidData, why)),
        Err(why) if why.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(why) => Err(why),
    }
}

fn write(plane: &Path, path: &Path, record: &Record) -> io::Result<()> {
    let bytes = serde_json::to_vec(record).map_err(io::Error::other)?;
    crate::plane::write_private(plane, path, &bytes)
}

/// The folder of the records, made, and held while `then` reads and writes in it: the app's
/// looks and the window's Put away may be different processes.
fn holding<T>(plane: &Path, then: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    let folder = dir(plane);
    std::fs::create_dir_all(&folder)?;
    let _held = crate::rewrite::Lock::bounded(&folder)?;
    then()
}

/// **As chat `chat` starts**: what is in `watched` now is what it started with. Where the walk
/// takes longer than a start may wait, the caller lets it finish beside the chat's first moments
/// ([`BEGIN_WAITS_AT_MOST`]). A chat that comes
/// back keeps the record it had, so what appeared while the app was closed is told at its next
/// look; a folder it may write now that it could not before is added as it is.
pub fn begin(plane: &Path, chat: &str, name: Option<&str>, watched: &Watched) -> io::Result<()> {
    let path = file(plane, chat)?;
    let had = read(&path).ok().flatten();
    let new_folders: Vec<PathBuf> = watched
        .folders
        .iter()
        .filter(|folder| {
            !had.as_ref()
                .is_some_and(|had| had.watched.folders.contains(folder))
        })
        .cloned()
        .collect();
    let walked = walk(&Watched {
        folders: new_folders,
        home: watched.home.clone().filter(|_| had.is_none()),
        lifted: watched.lifted.clone(),
    });
    holding(plane, || {
        let mut record = read(&path).ok().flatten().unwrap_or_else(|| Record {
            v: 1,
            name: None,
            watched: Watched::default(),
            baseline: BTreeSet::new(),
            arrived: BTreeSet::new(),
            put_away: BTreeSet::new(),
            cut_short: false,
            cut_short_told: false,
            whole_to: None,
        });
        record.name = name.map(str::to_owned).or(record.name);
        for folder in &watched.folders {
            if !record.watched.folders.contains(folder) {
                record.watched.folders.push(folder.clone());
            }
        }
        record.watched.home = watched.home.clone().or(record.watched.home);
        record.watched.lifted.clone_from(&watched.lifted);
        record.baseline.extend(walked.found.keys().map(|p| key(p)));
        record.cut_short |= walked.cut_short;
        if walked.cut_short {
            record.whole_to = Some(
                record
                    .whole_to
                    .map_or(walked.whole_to, |had| had.min(walked.whole_to)),
            );
        }
        write(plane, &path, &record)
    })
}

/// **At the end of a turn of chat `chat`**: what has appeared since its last look, or `None` for a
/// chat with no record, which is one purlis started with no sandbox. Each path on one Notice only:
/// one another chat's Notice has told goes on none here.
pub fn look(plane: &Path, chat: &str, name: Option<&str>) -> io::Result<Option<Looked>> {
    let path = file(plane, chat)?;
    let Some(before) = read(&path)? else {
        return Ok(None);
    };
    let walked = walk(&before.watched);
    // What no look has been sure of before says nothing, and is known from here on.
    let (first_reached, unseen): (BTreeSet<PathBuf>, BTreeSet<PathBuf>) = walked
        .found
        .iter()
        .filter(|(found, _)| {
            let found = key(found);
            !before.baseline.contains(&found)
                && !before.arrived.contains(&found)
                && !before.put_away.contains(&found)
        })
        .partition_map_by_depth(before.whole_to);
    let (ours, theirs) = Known::of(plane, &before).split(unseen);
    holding(plane, || {
        let Some(mut record) = read(&path)? else {
            return Ok(None);
        };
        let was = record.clone();
        let told_elsewhere = told_by_others(plane, chat);
        let mut new = Vec::new();
        record.baseline.extend(ours.iter().map(|p| key(p)));
        record.baseline.extend(first_reached.iter().map(|p| key(p)));
        for found in theirs.iter().map(|p| key(p)) {
            if told_elsewhere.contains(&found) {
                record.put_away.insert(found);
            } else if record.arrived.insert(found.clone()) {
                new.push(found);
            }
        }
        // What has gone says nothing now, and is told again if it comes back. Only a whole look
        // can say something has gone.
        if !walked.cut_short {
            let there: BTreeSet<String> = walked.found.keys().map(|p| key(p)).collect();
            for kept in [
                &mut record.baseline,
                &mut record.arrived,
                &mut record.put_away,
            ] {
                kept.retain(|path| there.contains(path));
            }
        }
        record.name = name.map(str::to_owned).or(record.name);
        record.cut_short = walked.cut_short;
        record.whole_to = if walked.cut_short {
            record.whole_to.map(|had| had.max(walked.whole_to))
        } else {
            None
        };
        if record != was {
            write(plane, &path, &record)?;
        }
        Ok(Some(Looked {
            new,
            cut_short: walked.cut_short,
        }))
    })
}

/// What every other chat's record has told.
fn told_by_others(plane: &Path, chat: &str) -> BTreeSet<String> {
    let mut told = BTreeSet::new();
    for (other, record) in records(plane) {
        if other != chat {
            told.extend(record.arrived);
            told.extend(record.put_away);
        }
    }
    told
}

/// Every record of the project, by its chat's id, oldest id first. One that does not read is left
/// out, and the log says so.
fn records(plane: &Path) -> Vec<(String, Record)> {
    let Ok(entries) = std::fs::read_dir(dir(plane)) else {
        return Vec::new();
    };
    let mut out: Vec<(String, Record)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let chat = name.to_str()?.strip_suffix(".json")?.to_owned();
            crate::contain::mintable(&chat).ok()?;
            match read(&entry.path()) {
                Ok(record) => record.map(|record| (chat, record)),
                Err(why) => {
                    tracing::warn!("purlis: {} does not read ({why})", entry.path().display());
                    None
                }
            }
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// **The Notices of the project**: one a chat, for each chat with something to tell. Read as the
/// Inbox is, so it survives a restart with the record it is read from.
pub fn told(plane: &Path) -> Vec<Told> {
    let records = records(plane);
    // Told once a set of folders: a chat there is no more incomplete than the last one was.
    let cut_short_told_for: Vec<Vec<PathBuf>> = records
        .iter()
        .filter(|(_, record)| record.cut_short_told)
        .map(|(_, record)| record.watched.folders.clone())
        .collect();
    records
        .into_iter()
        .filter(|(_, record)| {
            !record.arrived.is_empty()
                || (record.cut_short && !cut_short_told_for.contains(&record.watched.folders))
        })
        .map(|(chat, record)| Told {
            chat,
            name: record.name,
            paths: record.arrived.into_iter().collect(),
            cut_short: record.cut_short,
        })
        .collect()
}

/// **Put away on chat `chat`'s Notice.** What it named stays on disk, and is not told again. Nor
/// is a look cut short in these folders, for this chat or another.
pub fn put_away(plane: &Path, chat: &str) -> io::Result<()> {
    let path = file(plane, chat)?;
    holding(plane, || {
        let Some(mut record) = read(&path)? else {
            return Ok(());
        };
        let arrived = std::mem::take(&mut record.arrived);
        record.put_away.extend(arrived);
        record.cut_short_told = record.cut_short_told || record.cut_short;
        write(plane, &path, &record)
    })
}

/// A path as a record keeps it.
fn key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The names a look asks for: every name of [`PLANTED`] that holds what is below it, and `.git`
/// itself, which brings its own. `.git`'s own parts and a name with a `**` are inside a `.git`,
/// which is one thing to say. A bare `.claude` or `.ssh` is not code: what is code in them is its
/// own name here. What a person lifted is left out.
fn names(lifted: &[String]) -> impl Iterator<Item = &'static str> + '_ {
    PLANTED
        .iter()
        .filter(|planted| {
            !planted.path.contains("**")
                && !planted.path.starts_with(".git/")
                && (planted.reach == Reach::AndBelow || planted.path == ".git")
        })
        .map(|planted| planted.path)
        .filter(|path| !lifted.iter().any(|name| name.eq_ignore_ascii_case(path)))
}

/// [`names`], by its last part in lower case, each as lower-case parts.
fn by_last_part(lifted: &[String]) -> HashMap<String, Vec<Vec<String>>> {
    let mut out: HashMap<String, Vec<Vec<String>>> = HashMap::new();
    for name in names(lifted) {
        let parts: Vec<String> = name.split('/').map(str::to_ascii_lowercase).collect();
        let last = parts.last().cloned().unwrap_or_default();
        out.entry(last).or_default().push(parts);
    }
    out
}

/// Whether `path`, whose last part is `lower`, is one of `asked`.
fn is_asked(path: &Path, lower: &str, asked: &HashMap<String, Vec<Vec<String>>>) -> bool {
    let Some(wanted) = asked.get(lower) else {
        return false;
    };
    wanted.iter().any(|parts| {
        parts.len() == 1
            || parts
                .iter()
                .rev()
                .zip(path.iter().rev())
                .all(|(part, on_disk)| on_disk.to_string_lossy().eq_ignore_ascii_case(part))
                && path.iter().count() >= parts.len()
    })
}

/// What a walk found.
#[derive(Debug, Default)]
struct Walked {
    /// Each name, and how many levels below its folder it is.
    found: BTreeMap<PathBuf, usize>,
    cut_short: bool,
    /// Where it was cut short, the deepest level it read whole.
    whole_to: usize,
}

/// Names split by whether a record cut short at `whole_to` could have seen them before.
trait ByDepth {
    fn partition_map_by_depth(
        self,
        whole_to: Option<usize>,
    ) -> (BTreeSet<PathBuf>, BTreeSet<PathBuf>);
}

impl<'a, I: Iterator<Item = (&'a PathBuf, &'a usize)>> ByDepth for I {
    fn partition_map_by_depth(
        self,
        whole_to: Option<usize>,
    ) -> (BTreeSet<PathBuf>, BTreeSet<PathBuf>) {
        let mut first_reached = BTreeSet::new();
        let mut unseen = BTreeSet::new();
        for (path, depth) in self {
            if whole_to.is_some_and(|whole_to| *depth > whole_to) {
                first_reached.insert(path.clone());
            } else {
                unseen.insert(path.clone());
            }
        }
        (first_reached, unseen)
    }
}

/// **The walk**: shallowest first, each folder in name order so that one bound stops at the same
/// place twice. No link is followed: one named as a name is itself found, and one that isn't says
/// nothing.
fn walk(watched: &Watched) -> Walked {
    let asked = by_last_part(&watched.lifted);
    let mut walked = Walked::default();
    let mut looked = 0;
    let mut level: Vec<PathBuf> = watched
        .folders
        .iter()
        .filter(|folder| std::fs::symlink_metadata(folder).is_ok_and(|kind| kind.is_dir()))
        .cloned()
        .collect();
    let mut depth = 0;
    while !level.is_empty() {
        depth += 1;
        let mut next = Vec::new();
        for dir in level {
            let entries = match std::fs::read_dir(&dir) {
                Ok(entries) => entries,
                // Gone since its folder was read: nothing there to say.
                Err(why) if why.kind() == io::ErrorKind::NotFound => continue,
                // A folder not read may hold anything.
                Err(_) => {
                    if !walked.cut_short {
                        walked.whole_to = depth;
                    }
                    walked.cut_short = true;
                    continue;
                }
            };
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                looked += 1;
                if looked > LOOKED_AT || walked.found.len() >= MOST_NAMES {
                    if !walked.cut_short {
                        walked.whole_to = depth - 1;
                    }
                    walked.cut_short = true;
                    return walked;
                }
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                let path = entry.path();
                if is_asked(&path, &name, &asked) {
                    if has_content(&path) {
                        walked.found.insert(path, depth);
                    }
                    continue;
                }
                let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
                if is_dir && !SKIPPED.iter().any(|skipped| entry.file_name() == *skipped) {
                    next.push(path);
                }
            }
        }
        level = next;
    }
    if let Some(home) = &watched.home {
        for name in names(&watched.lifted) {
            let path = home.join(name);
            if has_content(&path) {
                walked.found.insert(path, 0);
            }
        }
    }
    walked
}

/// Whether what is at `path` could hold anything to run: anything but an empty file or a folder of
/// nothing else. Read without following a link, and without opening anything.
fn has_content(path: &Path) -> bool {
    let Ok(kind) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if kind.is_file() {
        return kind.len() > 0;
    }
    if !kind.is_dir() {
        return true;
    }
    let mut left = MOST_FILES_CHECKED;
    let mut level = vec![path.to_path_buf()];
    while let Some(dir) = level.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return true;
        };
        for entry in entries.flatten() {
            left = left.saturating_sub(1);
            if left == 0 {
                return true;
            }
            let Ok(kind) = entry.metadata() else {
                return true;
            };
            if kind.is_dir() && !kind.is_symlink() {
                level.push(entry.path());
            } else if kind.len() > 0 || !kind.is_file() {
                return true;
            }
        }
    }
    false
}

/// A file read up to [`LARGEST_READ`], where it is a regular file and not a link.
fn small_file(path: &Path) -> Option<String> {
    let kind = std::fs::symlink_metadata(path).ok()?;
    if !kind.is_file() {
        return None;
    }
    let mut text = String::new();
    std::fs::File::open(path)
        .ok()?
        .take(LARGEST_READ)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

/// **A repository this look may read**, known by what no chat can write, and what vouches for a
/// file in it.
#[derive(Debug, Clone)]
enum Repo {
    /// A clone. A chat can commit in one, so its index and its branches say nothing: what vouches
    /// for a file is a remote-tracking branch, which holds what a fetch brought.
    Clone,
    /// A linked worktree: `index`, in its clone's `.git/worktrees`, which no chat writes. What is
    /// staged there was checked out, or committed through the app's brokered commit, which
    /// refuses these names.
    Linked { index: PathBuf },
}

/// **How a look tells the app's own work from what appeared another way**, by what is on disk and
/// no chat can write:
///
/// - **A new branch folder**: a `.git` file naming a directory of a known repository's
///   `.git/worktrees` that names it back. Only git outside a sandbox writes there.
/// - **A new submodule**: a `.git` file naming a directory with a `HEAD` in a known repository's
///   `.git/modules`, which is held the same way.
/// - **A new clone**: a `.git` folder where a workspace's manifest names a repo. Only the app
///   writes a manifest.
/// - **A file of a known repository**: one whose every file is what that repository has at that
///   path ([`Repo`]). The name is not enough, nor is what a chat could have committed.
///
/// A repository is known where a `.git` was there as the chat started, above one of its folders,
/// or as above. Anything else is never opened.
struct Known {
    tops: Vec<(PathBuf, Repo)>,
    git_dirs: Vec<PathBuf>,
    clones: BTreeSet<PathBuf>,
}

impl Known {
    fn of(plane: &Path, record: &Record) -> Self {
        let mut known = Self {
            tops: Vec::new(),
            git_dirs: Vec::new(),
            clones: clones_in_manifests(plane),
        };
        let started = record
            .baseline
            .iter()
            .map(PathBuf::from)
            .filter(|path| path.file_name().is_some_and(|name| name == ".git"))
            .filter_map(|path| path.parent().map(Path::to_path_buf));
        let above = record
            .watched
            .folders
            .iter()
            .flat_map(|folder| folder.ancestors().map(Path::to_path_buf));
        let tops: BTreeSet<PathBuf> = started.chain(above).collect();
        for top in tops {
            known.add_as_it_is(&top);
        }
        known
    }

    /// `top`, read as its `.git` says, where that is a repository.
    fn add_as_it_is(&mut self, top: &Path) {
        let dot_git = top.join(".git");
        let Ok(kind) = std::fs::symlink_metadata(&dot_git) else {
            return;
        };
        if kind.is_dir() {
            self.git_dirs.push(dot_git);
            self.tops.push((top.to_path_buf(), Repo::Clone));
        } else if let Some(git_dir) = pointed(&dot_git) {
            let common = small_file(&git_dir.join("commondir"))
                .map(|text| super::planted::lexical(&git_dir.join(text.trim())))
                .unwrap_or_else(|| git_dir.clone());
            self.git_dirs.push(common);
            self.tops.push((
                top.to_path_buf(),
                Repo::Linked {
                    index: git_dir.join("index"),
                },
            ));
        }
    }

    /// Whether the new `dot_git` is a worktree or a submodule of a known repository, or a clone
    /// a manifest names.
    fn vouches_for_git(&self, dot_git: &Path) -> bool {
        let Some(top) = dot_git.parent() else {
            return false;
        };
        let Ok(kind) = std::fs::symlink_metadata(dot_git) else {
            return false;
        };
        if kind.is_dir() {
            return self.clones.contains(top);
        }
        // Where it points is looked at only inside a repository already known.
        let Some(named) = pointer_text(dot_git) else {
            return false;
        };
        self.git_dirs.iter().any(|git_dir| {
            if named.parent() == Some(&git_dir.join("worktrees")) {
                // The directory names this `.git` back.
                let back = small_file(&named.join("gitdir"));
                back.is_some_and(|back| same_path(Path::new(back.trim()), dot_git))
            } else {
                named.starts_with(git_dir.join("modules")) && named.join("HEAD").is_file()
            }
        })
    }

    /// `unseen`, split: what the app or a repository vouches for, then the rest.
    fn split(mut self, unseen: BTreeSet<PathBuf>) -> (BTreeSet<PathBuf>, BTreeSet<PathBuf>) {
        let mut ours = BTreeSet::new();
        let mut theirs = BTreeSet::new();
        let (gits, others): (Vec<PathBuf>, Vec<PathBuf>) = unseen
            .into_iter()
            .partition(|path| path.file_name().is_some_and(|name| name == ".git"));
        let mut checked = 0;
        for dot_git in gits {
            checked += 1;
            if checked <= MOST_CHECKED && self.vouches_for_git(&dot_git) {
                if let Some(top) = dot_git.parent() {
                    self.add_as_it_is(top);
                }
                ours.insert(dot_git);
            } else {
                theirs.insert(dot_git);
            }
        }
        let mut read = Evidence::default();
        for path in others {
            checked += 1;
            // The nearest repository, and none of what appeared.
            let owner = self
                .tops
                .iter()
                .filter(|(top, _)| path.starts_with(top))
                .max_by_key(|(top, _)| top.as_os_str().len());
            let in_its_own = owner.is_some_and(|(top, repo)| {
                checked <= MOST_CHECKED && read.vouches(top, repo, &path)
            });
            if in_its_own {
                ours.insert(path);
            } else {
                theirs.insert(path);
            }
        }
        (ours, theirs)
    }
}

/// The directory a `.git` file names, lexically, with nothing opened but the file.
fn pointer_text(dot_git: &Path) -> Option<PathBuf> {
    let text = small_file(dot_git)?;
    let named = text.lines().find_map(|line| line.strip_prefix("gitdir:"))?;
    let top = dot_git.parent()?;
    Some(super::planted::lexical(&top.join(named.trim())))
}

/// [`pointer_text`], where it names a git directory.
fn pointed(dot_git: &Path) -> Option<PathBuf> {
    let dir = pointer_text(dot_git)?;
    (std::fs::symlink_metadata(&dir).is_ok_and(|kind| kind.is_dir()) && dir.join("HEAD").is_file())
        .then_some(dir)
}

/// Whether two paths are one, by the kernel's spelling where both are there.
fn same_path(a: &Path, b: &Path) -> bool {
    a == b
        || matches!(
            (std::fs::canonicalize(a), std::fs::canonicalize(b)),
            (Ok(a), Ok(b)) if a == b
        )
}

/// Each repo every workspace's manifest names, as the folder it is cloned to. A manifest is never
/// a chat's to write (ADR 0067, amended 2026-10-21).
fn clones_in_manifests(plane: &Path) -> BTreeSet<PathBuf> {
    let workspaces = plane.join(super::WORKSPACES);
    let Ok(entries) = std::fs::read_dir(&workspaces) else {
        return BTreeSet::new();
    };
    let mut out = BTreeSet::new();
    for entry in entries.flatten() {
        let workspace = entry.path();
        let Some(text) = small_file(&workspace.join("workspace.json")) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let repos = manifest["repos"].as_array().into_iter().flatten();
        let named = repos.filter_map(|repo| repo["name"].as_str());
        for name in named.filter(|name| crate::contain::repo_name_ok(name)) {
            out.insert(workspace.join(name));
        }
    }
    out
}

/// One look's reading of the repositories it needed, each read once.
#[derive(Default)]
struct Evidence {
    /// By the repository's top: the blob each path has, read when first asked.
    index: HashMap<PathBuf, Option<gix::index::File>>,
    /// By the repository's top: what its remote-tracking branches hold.
    repos: HashMap<PathBuf, Option<gix::Repository>>,
}

impl Evidence {
    /// Whether every file below `path` is what `repo` at `top` has at that path.
    fn vouches(&mut self, top: &Path, repo: &Repo, path: &Path) -> bool {
        let Some(files) = files_below(path) else {
            return false;
        };
        !files.is_empty()
            && files.iter().all(|file| {
                let Some(blob) = blob_of(file) else {
                    return false;
                };
                let Ok(relative) = file.strip_prefix(top) else {
                    return false;
                };
                self.has(top, repo, relative, blob)
            })
    }

    fn has(&mut self, top: &Path, repo: &Repo, relative: &Path, blob: gix::ObjectId) -> bool {
        match repo {
            Repo::Linked { index } => {
                let Some(words) = relative.to_str() else {
                    return false;
                };
                self.index
                    .entry(top.to_path_buf())
                    .or_insert_with(|| {
                        gix::index::File::at(
                            index,
                            gix::hash::Kind::Sha1,
                            false,
                            gix::index::decode::Options::default(),
                        )
                        .ok()
                    })
                    .as_ref()
                    .is_some_and(|index| {
                        index
                            .entry_by_path(gix::bstr::BStr::new(words))
                            .is_some_and(|entry| entry.id == blob)
                    })
            }
            Repo::Clone => self
                .repos
                .entry(top.to_path_buf())
                .or_insert_with(|| gix::open_opts(top, gix::open::Options::isolated()).ok())
                .as_ref()
                .is_some_and(|repo| a_remote_branch_has(repo, relative, blob)),
        }
    }
}

/// Whether one of `repo`'s remote-tracking branches ([`MOST_BRANCHES`] of them) has `blob` at
/// `relative`.
fn a_remote_branch_has(repo: &gix::Repository, relative: &Path, blob: gix::ObjectId) -> bool {
    let Ok(references) = repo.references() else {
        return false;
    };
    let Ok(branches) = references.remote_branches() else {
        return false;
    };
    branches.flatten().take(MOST_BRANCHES).any(|mut branch| {
        branch
            .peel_to_tree()
            .ok()
            .and_then(|tree| tree.lookup_entry_by_path(relative).ok().flatten())
            .is_some_and(|entry| entry.object_id() == blob)
    })
}

/// `path`, or each file and link below it, without following one; `None` past
/// [`MOST_FILES_CHECKED`], or for a folder that does not read.
fn files_below(path: &Path) -> Option<Vec<PathBuf>> {
    let kind = std::fs::symlink_metadata(path).ok()?;
    if !kind.is_dir() {
        return Some(vec![path.to_path_buf()]);
    }
    let mut out = Vec::new();
    let mut level = vec![path.to_path_buf()];
    while let Some(dir) = level.pop() {
        for entry in std::fs::read_dir(&dir).ok()? {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                level.push(entry.path());
            } else {
                out.push(entry.path());
                if out.len() > MOST_FILES_CHECKED {
                    return None;
                }
            }
        }
    }
    Some(out)
}

/// The blob git would store for what is at `path`: a regular file's bytes or a link's target. A
/// file too large, or anything else, has none. Nothing but a regular file is opened.
fn blob_of(path: &Path) -> Option<gix::ObjectId> {
    let kind: Metadata = std::fs::symlink_metadata(path).ok()?;
    let bytes = if kind.is_symlink() {
        std::fs::read_link(path)
            .ok()?
            .to_string_lossy()
            .into_owned()
            .into_bytes()
    } else if kind.is_file() && kind.len() <= LARGEST_CHECKED {
        std::fs::read(path).ok()?
    } else {
        return None;
    };
    gix::objs::compute_hash(gix::hash::Kind::Sha1, gix::objs::Kind::Blob, &bytes).ok()
}
