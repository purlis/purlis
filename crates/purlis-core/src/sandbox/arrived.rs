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
//! - **By where it is, too.** A name already there that is now in another folder of the same path,
//!   or a `.git` or a folder that is another one, is new ([`Pin`]). A file rewritten where it
//!   stands is not: a person's editor does that.
//! - **Nothing empty.** An empty file or folder runs nothing, and on Linux a harness's sandbox
//!   may hold a path by putting an empty one there while its chat runs ([`has_content`]).
//!
//! The walk has a bound ([`LOOKED_AT`], [`MOST_NAMES`]). At it, the record says the look was cut
//! short, and the Notice says the check is incomplete: a look that was cut short never reads as
//! "nothing appeared". A workspace's many branch folders can hold millions of entries, so the
//! bound is met in ordinary use: the walk goes level by level, so it covers the top of every
//! folder first, and a name it first reaches below where it has been whole is taken as there, not
//! as new. The person is told once a set of folders, until a look there is whole again, that the
//! check there is incomplete. A folder it can't read is named, as what it holds can't be said.
//!
//! # What may never happen here
//!
//! **Nothing found is run.** No link is followed, no program is started, and git is never run:
//! a repository's own config or hooks would run with it. What this reads of a repository it reads
//! by gitoxide, opened [isolated](gix::open::Options::isolated), and only for repositories it
//! knows by something a chat cannot write ([`Known`]). A file that is not a regular file is not
//! opened by this code, and all reading of a repository is held to [`CHECKS_TAKE_AT_MOST`] on a
//! thread of its own: past that, every name it was asked about is told.
//!
//! # The app's own work, and the person's
//!
//! What the app checks out carries these names too, and the look tells it apart by what is on
//! disk, so it holds for every way a checkout is made, the person's own terminal included
//! ([`Known`]). Only a new clone is said by the code that makes it ([`made_by_the_app`]), as nothing
//! on disk tells one from a repository that arrived another way. One it can't tell apart is told:
//! this says what appeared and never who put it there.
//!
//! # The record
//!
//! `<state>/app/arrived/<chat>.json`, by the chat's own id, so it outlives a restart. Every harness
//! is compiled to deny a chat reading or writing it: no chat learns what is recorded, or holds the
//! folder's lock.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

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

/// How much one look reads of what names hold ([`contents_of`]). Past it a file counts by its size
/// and its last change.
const MOST_CONTENT_READ: u64 = 64 * 1024 * 1024;

/// How many entries one look goes through in what names hold ([`contents_of`]).
const MOST_CONTENT_ENTRIES: usize = 100_000;

/// The file of the clones the app made ([`made_by_the_app`]), beside the records.
const MADE: &str = "made-by-the-app";

/// The sections of a repository's config ordinary work rewrites, which [`stamp`] leaves out.
const ORDINARY_SECTIONS: [&str; 8] = [
    "branch",
    "remote",
    "user",
    "gc",
    "maintenance",
    "pull",
    "push",
    "fetch",
];

/// The audit event of what a look found.
pub const AUDITED: &str = "trust.sandbox.arrived";

/// How long a chat's start waits for [`begin`] before it goes on, as a person feels a longer one.
pub const BEGIN_WAITS_AT_MOST: Duration = Duration::from_millis(250);

/// How long one look's checks against repositories may take. What holds them longer is told.
pub const CHECKS_TAKE_AT_MOST: Duration = Duration::from_secs(30);

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
    /// Whether what a name holds is told when it changes, in the folders walked: where the chat's
    /// sandbox doesn't refuse a write to a name already there (Claude Code's on Linux, below the
    /// chat's folder). Elsewhere only a person or a tool outside it can change one.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub contents: bool,
    /// The names a person lifted for this chat ([`super::grant::lift`]): the chat may write
    /// them, so they say nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifted: Vec<String>,
    /// The git folders in or below these folders that the chat's sandbox refuses it writing at
    /// all, such as the clone of a branch folder it works in. A repository whose git folder is
    /// in these folders and not one of these vouches for nothing ([`Known`]): the chat could
    /// have written its branches and its index.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub held_git: Vec<PathBuf>,
}

/// **Which thing is at a path**: its device and inode, its folder's, and when it last changed. A
/// path says nothing when what is there can be another thing under the same path.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Pin {
    own: [u64; 2],
    folder: [u64; 2],
    changed: i64,
    /// For a `.git`, what of it says what runs ([`stamp`]).
    #[serde(default, skip_serializing_if = "is_zero")]
    inner: u64,
    /// What it holds ([`contents_of`]), where the look is told to weigh it. 0 where it isn't.
    #[serde(default, skip_serializing_if = "is_zero")]
    content: u64,
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

/// Whether `path` is named `.git`, as a disk that folds case takes it.
fn is_git(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| folded(&name.to_string_lossy()) == ".git")
}

/// **What of a `.git` says what runs**: a `.git` file's words; a folder's hooks by name, size and
/// change, and its config without the sections ordinary work rewrites. So a repository that went
/// away and came back with other hooks is another one, though it is the same inode. Nothing here
/// is opened but a regular file.
fn stamp(dot_git: &Path) -> u64 {
    let mut words = String::new();
    if is_regular(dot_git) {
        words.push_str(&small_file(dot_git).unwrap_or_default());
    } else {
        // Read as git reads it, by gitoxide's parser: no spelling hides a section. What doesn't
        // parse git refuses too, and counts whole.
        let config = small_file(&dot_git.join("config")).unwrap_or_default();
        match gix::config::File::try_from(config.as_str()) {
            Ok(parsed) => {
                for section in parsed.sections() {
                    let name = section.header().name().to_string();
                    if !ORDINARY_SECTIONS
                        .iter()
                        .any(|ordinary| name.eq_ignore_ascii_case(ordinary))
                    {
                        words.push_str(&section.to_bstring().to_string());
                    }
                }
            }
            Err(_) => words.push_str(&config),
        }
        for other in [
            "config.worktree",
            "commondir",
            "objects/info/alternates",
            "info/attributes",
        ] {
            words.push_str(&small_file(&dot_git.join(other)).unwrap_or_default());
        }
        if let Ok(hooks) = std::fs::read_dir(dot_git.join("hooks")) {
            let mut listed: Vec<String> = hooks
                .flatten()
                .map(|hook| {
                    let kind = hook.metadata().ok();
                    format!(
                        "{} {} {}\n",
                        hook.file_name().to_string_lossy(),
                        kind.as_ref().map_or(0, std::fs::Metadata::len),
                        kind.as_ref().map_or(0, ctime),
                    )
                })
                .collect();
            listed.sort();
            listed.truncate(MOST_FILES_CHECKED);
            words.push_str(&listed.concat());
        }
    }
    short_hash(words.as_bytes())
}

/// `bytes`, hashed short, and never 0, which says nothing was taken.
fn short_hash(bytes: &[u8]) -> u64 {
    gix::objs::compute_hash(gix::hash::Kind::Sha1, gix::objs::Kind::Blob, bytes)
        .ok()
        .and_then(|id| id.as_bytes().first_chunk::<8>().copied())
        .map_or(1, u64::from_le_bytes)
        .max(1)
}

/// How much of [`MOST_CONTENT_READ`] and [`MOST_CONTENT_ENTRIES`] one look has used.
#[derive(Debug, Default)]
struct Weighing {
    read: u64,
    entries: usize,
}

/// **What a name holds**: a file by its bytes, a link by what it says, a folder by every entry
/// below it, in name order. By content, so a checkout that writes the same bytes changes nothing.
/// No link is followed and only a regular file is opened ([`read_regular`]). Past
/// [`MOST_FILES_CHECKED`] entries, or the look's own bounds, the rest doesn't count, and that name
/// comes back with any folder not read whole, to be said.
fn contents_of(path: &Path, weighing: &mut Weighing) -> (u64, Vec<PathBuf>) {
    let mut words = String::new();
    let mut unread = Vec::new();
    let mut entries = 0;
    let mut waiting = vec![path.to_path_buf()];
    while let Some(at) = waiting.pop() {
        entries += 1;
        weighing.entries += 1;
        if entries > MOST_FILES_CHECKED || weighing.entries > MOST_CONTENT_ENTRIES {
            unread.push(path.to_path_buf());
            break;
        }
        let Ok(kind) = std::fs::symlink_metadata(&at) else {
            continue;
        };
        let name = at.strip_prefix(path).unwrap_or(&at).display().to_string();
        if kind.is_dir() {
            words.push_str(&format!("d {name}\n"));
            let Ok(found) = std::fs::read_dir(&at) else {
                unread.push(at);
                continue;
            };
            let mut below: Vec<PathBuf> = found.flatten().map(|entry| entry.path()).collect();
            below.sort();
            waiting.extend(below.into_iter().rev());
        } else if kind.is_symlink() {
            let says = std::fs::read_link(&at).unwrap_or_default();
            words.push_str(&format!("l {name} {}\n", says.display()));
        } else if kind.is_file() {
            let size = kind.len();
            let fits = size <= LARGEST_CHECKED && weighing.read + size <= MOST_CONTENT_READ;
            match fits.then(|| read_regular(&at, LARGEST_CHECKED)).flatten() {
                Some(bytes) => {
                    weighing.read += size;
                    words.push_str(&format!("f {name} {}\n", short_hash(&bytes)));
                }
                None => words.push_str(&format!("s {name} {size} {}\n", ctime(&kind))),
            }
        }
    }
    (short_hash(words.as_bytes()), unread)
}

/// When `kind` last changed, which, unlike its modified time, no program sets.
#[cfg(unix)]
fn ctime(kind: &std::fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    kind.ctime()
}

#[cfg(not(unix))]
fn ctime(_: &std::fs::Metadata) -> i64 {
    0
}

impl Pin {
    /// What is at `path`, without following a link there.
    #[cfg(unix)]
    fn of(path: &Path) -> Option<Self> {
        use std::os::unix::fs::MetadataExt;
        let own = std::fs::symlink_metadata(path).ok()?;
        let folder = std::fs::symlink_metadata(path.parent()?).ok()?;
        Some(Self {
            own: [own.dev(), own.ino()],
            folder: [folder.dev(), folder.ino()],
            changed: ctime(&own),
            inner: if is_git(path) { stamp(path) } else { 0 },
            content: 0,
        })
    }

    #[cfg(not(unix))]
    fn of(path: &Path) -> Option<Self> {
        std::fs::symlink_metadata(path)
            .ok()
            .map(|_| Self::default())
    }

    /// Whether what is at `path` now, `now`, is the thing this pin was taken of. A file may be
    /// rewritten where it stands, and an editor's save gives it a new inode. A folder or a `.git`
    /// isn't: another one is another thing.
    fn holds(&self, now: &Self, path: &Path) -> bool {
        let is_itself =
            is_git(path) || std::fs::symlink_metadata(path).is_ok_and(|kind| kind.is_dir());
        let same_content = self.content == 0 || now.content == 0 || self.content == now.content;
        self.folder == now.folder
            && self.inner == now.inner
            && same_content
            && (!is_itself || self.own == now.own)
    }

    /// Whether `now` is this thing, holding something else: what no repository vouches for, as
    /// a chat that writes a `.git` can make one vouch for what it likes.
    fn only_content_differs(&self, now: &Self, path: &Path) -> bool {
        !self.holds(now, path)
            && Self {
                content: now.content,
                ..*self
            }
            .holds(now, path)
    }

    /// Whether `now`, at a repository's `.git`, is the `.git` this pin was taken of.
    fn same_repository(&self, now: &Self) -> bool {
        self.own == now.own && self.inner == now.inner
    }

    /// Whether `now` is this very file, unchanged since it was told.
    fn same_file(&self, now: &Self) -> bool {
        self.own == now.own && self.changed == now.changed
    }
}

/// One chat's record.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Record {
    v: u32,
    /// What the chat was last called, for the Notice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    watched: Watched,
    /// The names there as the chat started, and what the look took for the app's own since.
    baseline: BTreeMap<String, Pin>,
    /// The repositories whose `.git` was there as the chat started, by the folder each is in, and
    /// those the app has made since: what is ever read of a repository ([`Known`]).
    repos: BTreeMap<String, Pin>,
    /// What is on the Notice now.
    arrived: BTreeSet<String>,
    /// The folders the last look could not read, on the Notice.
    unread: BTreeSet<String>,
    /// What was told and put away, here or on another chat's Notice.
    put_away: BTreeMap<String, Pin>,
    /// Whether the last look was cut short.
    cut_short: bool,
    /// Whether the person has put away a Notice that said so, since a look was last whole.
    cut_short_told: bool,
    /// By watched folder, where no look has read all of it: the deepest level below it some look
    /// has read whole. A name found deeper was not known to be missing before, so it says nothing
    /// about what is new.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    whole_for: BTreeMap<String, usize>,
}

/// What one look found, for the audit: paths and no contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Looked {
    /// What this look put on the Notice.
    pub new: Vec<String>,
    /// The folders it newly could not read.
    pub unread: Vec<String>,
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
            "unread": self.unread,
            "cut_short": self.cut_short,
        })
    }

    /// Whether this look has anything for the audit.
    pub fn says_anything(&self) -> bool {
        !self.new.is_empty() || !self.unread.is_empty()
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
    /// Each folder that could not be read.
    pub unread: Vec<String>,
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
        let list = |paths: &[String]| {
            let shown: Vec<String> = paths
                .iter()
                .take(MOST_SHOWN)
                .map(|path| {
                    let path = Path::new(path);
                    crate::shown::line(&path.strip_prefix(plane).unwrap_or(path).to_string_lossy())
                })
                .collect();
            let more = paths.len().saturating_sub(MOST_SHOWN);
            let rest = if more > 0 {
                format!(", and {more} more")
            } else {
                String::new()
            };
            format!("{}{rest}", shown.join(", "))
        };
        let mut out = format!("{chat}:");
        if !self.paths.is_empty() {
            out.push_str(&format!(
                " these appeared in a folder it can write: {}. They can run outside the sandbox \
                 if you or a tool work in that folder. purlis changed nothing.",
                list(&self.paths)
            ));
        }
        if !self.unread.is_empty() {
            out.push_str(&format!(
                " purlis could not read {}, so it can't say what they hold.",
                list(&self.unread)
            ));
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

/// The folder of the records: `<state>/app/arrived/`, which every harness denies a chat reading
/// and writing ([`super::Denied::of`]).
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
/// ([`BEGIN_WAITS_AT_MOST`]). A chat that comes back keeps the record it had, so what appeared
/// while the app was closed is told at its next look; a folder it may write now that it could not
/// before is added as it is.
pub fn begin(plane: &Path, chat: &str, name: Option<&str>, watched: &Watched) -> io::Result<()> {
    let path = file(plane, chat)?;
    let had = read(&path).ok().flatten().unwrap_or_default();
    let new_folders: Vec<PathBuf> = watched
        .folders
        .iter()
        .filter(|folder| !had.watched.folders.contains(folder))
        .cloned()
        .collect();
    let walked = walk(&Watched {
        folders: new_folders.clone(),
        home: watched.home.clone().filter(|_| had.watched.home.is_none()),
        contents: watched.contents,
        lifted: watched.lifted.clone(),
        held_git: watched.held_git.clone(),
    });
    let repos = repos_at_start(&new_folders, &walked);
    holding(plane, || {
        let mut record = read(&path).ok().flatten().unwrap_or_default();
        record.v = 1;
        record.name = name.map(str::to_owned).or(record.name);
        for folder in &watched.folders {
            if !record.watched.folders.contains(folder) {
                record.watched.folders.push(folder.clone());
            }
        }
        record.watched.home = watched.home.clone().or(record.watched.home);
        record.watched.contents = watched.contents;
        record.watched.lifted.clone_from(&watched.lifted);
        record.watched.held_git.clone_from(&watched.held_git);
        record
            .baseline
            .extend(walked.found.iter().map(|(p, found)| (key(p), found.pin)));
        record.repos.extend(repos.clone());
        record.cut_short |= walked.cut_short;
        if let Some(whole_to) = walked.bound_at {
            for folder in &new_folders {
                record.whole_for.insert(key(folder), whole_to);
            }
        }
        write(plane, &path, &record)
    })
}

/// The repositories known by being there as a chat starts: each `.git` the walk found, and each
/// above a folder, with what it is.
fn repos_at_start(folders: &[PathBuf], walked: &Walked) -> BTreeMap<String, Pin> {
    let found = walked
        .found
        .keys()
        .filter(|path| is_git(path))
        .filter_map(|path| path.parent().map(Path::to_path_buf));
    let above = folders
        .iter()
        .flat_map(|folder| folder.ancestors().map(Path::to_path_buf));
    let mut repos = BTreeMap::new();
    for top in found.chain(above) {
        let dot_git = top.join(".git");
        let Some(pin) = Pin::of(&dot_git) else {
            continue;
        };
        // And the repository a branch folder is of, wherever that is.
        if let Some(common) = pointed(&dot_git).as_deref().and_then(common_of)
            && let (Some(clone), Some(pin)) = (common.parent(), Pin::of(&common))
        {
            repos.insert(key(clone), pin);
        }
        repos.insert(key(&top), pin);
    }
    repos
}

/// The `.git` a `git_dir` of a branch folder or a submodule belongs to, read as `commondir` names
/// it: a directory named `.git`, or none.
fn common_of(git_dir: &Path) -> Option<PathBuf> {
    let common = match small_file(&git_dir.join("commondir")) {
        Some(text) => super::planted::lexical(&git_dir.join(text.trim())),
        None => git_dir.ancestors().find(|dir| is_git(dir))?.to_path_buf(),
    };
    is_git(&common).then_some(common)
}

/// **A clone the app has just made in `top`**, said by the code that made it, as nothing on disk
/// tells a clone from a repository that arrived another way. It is held to the `.git` that was
/// there when this was said ([`Known`]). Only outside a sandbox can this be written.
pub fn made_by_the_app(plane: &Path, top: &Path) -> io::Result<()> {
    // No chat has a record here, so the next one to start finds this clone there as it starts.
    if !dir(plane).is_dir() {
        return Ok(());
    }
    let Some(pin) = Pin::of(&top.join(".git")) else {
        return Ok(());
    };
    let path = dir(plane).join(MADE);
    holding(plane, || {
        let mut made = read_made(plane);
        made.insert(key(top), pin);
        let bytes = serde_json::to_vec(&made).map_err(io::Error::other)?;
        crate::plane::write_private(plane, &path, &bytes)
    })
}

fn read_made(plane: &Path) -> BTreeMap<String, Pin> {
    std::fs::read(dir(plane).join(MADE))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// **At the end of a turn of chat `chat`**: what has appeared since its last look, or `None` for a
/// chat with no record, which is one purlis started with no sandbox. Each path on one Notice only:
/// one another chat's Notice has told, and that has not changed since, goes on none here.
pub fn look(plane: &Path, chat: &str, name: Option<&str>) -> io::Result<Option<Looked>> {
    let path = file(plane, chat)?;
    let Some(before) = read(&path)? else {
        return Ok(None);
    };
    let walked = walk(&before.watched);
    // What no look has been sure of before says nothing, and is known from here on.
    let mut first_reached = BTreeSet::new();
    let mut unseen = BTreeSet::new();
    let mut changed = BTreeSet::new();
    let mut first_weighed = BTreeMap::new();
    for (found, seen) in &walked.found {
        let at = key(found);
        // Weighed for the first time, as a chat from before this was asked: known from here on.
        if before.watched.contents
            && seen.pin.content != 0
            && before
                .baseline
                .get(&at)
                .is_some_and(|pin| pin.content == 0 && pin.holds(&seen.pin, found))
        {
            first_weighed.insert(at.clone(), seen.pin);
        }
        // Put away, it stays away only while it is the thing that was put away.
        let told_of = before.arrived.contains(&at)
            || before
                .put_away
                .get(&at)
                .is_some_and(|pin| pin.holds(&seen.pin, found));
        let known = before.baseline.get(&at);
        if told_of || known.is_some_and(|pin| pin.holds(&seen.pin, found)) {
            continue;
        }
        if known.is_some_and(|pin| pin.only_content_differs(&seen.pin, found)) {
            changed.insert(found.clone());
            continue;
        }
        // Known and now another thing is new however deep it is.
        let beyond = known.is_none()
            && before
                .whole_for
                .iter()
                .any(|(folder, whole_to)| found.starts_with(folder) && seen.depth > *whole_to);
        if beyond {
            first_reached.insert(found.clone());
        } else {
            unseen.insert(found.clone());
        }
    }
    let (ours, mut theirs) = checked(plane, &before, unseen);
    theirs.extend(changed);
    holding(plane, || {
        let Some(mut record) = read(&path)? else {
            return Ok(None);
        };
        let was = record.clone();
        record.baseline.extend(first_weighed.clone());
        let (others_showing, others_put_away) = told_by_others(plane, chat);
        let pin_of = |at: &Path| {
            walked
                .found
                .get(at)
                .map(|seen| seen.pin)
                .unwrap_or_default()
        };
        let mut new = Vec::new();
        for at in ours.iter().chain(&first_reached) {
            record.baseline.insert(key(at), pin_of(at));
            if ours.contains(at)
                && is_git(at)
                && let Some(top) = at.parent()
            {
                record.repos.insert(key(top), pin_of(at));
            }
        }
        for at in &theirs {
            let found = key(at);
            record.baseline.remove(&found);
            record.put_away.remove(&found);
            let told_elsewhere = others_showing.contains(&found)
                || others_put_away
                    .iter()
                    .any(|(path, pin)| *path == found && pin.same_file(&pin_of(at)));
            if told_elsewhere {
                record.put_away.insert(found, pin_of(at));
            } else if record.arrived.insert(found.clone()) {
                new.push(found);
            }
        }
        // A folder not read is said once, and said again if it is read and then is not.
        let mut unread = Vec::new();
        record.unread.clear();
        for at in &walked.unread {
            let at = key(at);
            if !record.put_away.contains_key(&at) {
                if !was.unread.contains(&at) {
                    unread.push(at.clone());
                }
                record.unread.insert(at);
            }
        }
        // What has gone says nothing now, and is told again if it comes back. Only a whole look
        // can say something has gone, and only of a folder it knew to look in.
        if !walked.cut_short {
            let there: BTreeSet<String> = walked.found.keys().map(|p| key(p)).collect();
            let mut in_scope: Vec<&PathBuf> = before.watched.folders.iter().collect();
            in_scope.extend(before.watched.home.iter());
            let stays = |path: &String| {
                there.contains(path)
                    || !in_scope
                        .iter()
                        .any(|folder| Path::new(path).starts_with(folder))
            };
            record.baseline.retain(|path, _| stays(path));
            record.arrived.retain(|path| stays(path));
            record.put_away.retain(|path, _| stays(path));
            record.whole_for.clear();
            record.cut_short_told = false;
            // Whole for every chat on these folders, and none is left saying otherwise.
            for (other, mut theirs) in records(plane) {
                if other != chat
                    && theirs.watched.folders == record.watched.folders
                    && (theirs.cut_short || theirs.cut_short_told)
                {
                    theirs.cut_short = false;
                    theirs.cut_short_told = false;
                    write(plane, &file(plane, &other)?, &theirs)?;
                }
            }
        } else {
            match walked.bound_at {
                Some(bound_at) => {
                    for whole_to in record.whole_for.values_mut() {
                        *whole_to = (*whole_to).max(bound_at);
                    }
                }
                None => record.whole_for.clear(),
            }
        }
        record.name = name.map(str::to_owned).or(record.name);
        record.cut_short = walked.cut_short;
        if record != was {
            write(plane, &path, &record)?;
        }
        Ok(Some(Looked {
            new,
            unread,
            cut_short: walked.cut_short,
        }))
    })
}

/// [`Known::split`], on a thread of its own and for [`CHECKS_TAKE_AT_MOST`] at most. What a chat
/// can make a read wait on, or a panic, costs this look its checks: every name is told.
fn checked(
    plane: &Path,
    record: &Record,
    unseen: BTreeSet<PathBuf>,
) -> (BTreeSet<PathBuf>, BTreeSet<PathBuf>) {
    if unseen.is_empty() {
        return (BTreeSet::new(), unseen);
    }
    let (done, answer) = std::sync::mpsc::channel();
    let (plane, record, asked) = (plane.to_path_buf(), record.clone(), unseen.clone());
    let spawned = std::thread::Builder::new()
        .name("arrived-checks".to_owned())
        .spawn(move || {
            let _ = done.send(Known::of(&plane, &record).split(asked));
        });
    if spawned.is_err() {
        return (BTreeSet::new(), unseen);
    }
    answer
        .recv_timeout(CHECKS_TAKE_AT_MOST)
        .unwrap_or((BTreeSet::new(), unseen))
}

/// What every other chat's record shows now, and what each had put away.
fn told_by_others(plane: &Path, chat: &str) -> (BTreeSet<String>, Vec<(String, Pin)>) {
    let mut showing = BTreeSet::new();
    let mut put_away = Vec::new();
    for (other, record) in records(plane) {
        if other != chat {
            showing.extend(record.arrived);
            put_away.extend(record.put_away);
        }
    }
    (showing, put_away)
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
                || !record.unread.is_empty()
                || (record.cut_short && !cut_short_told_for.contains(&record.watched.folders))
        })
        .map(|(chat, record)| Told {
            chat,
            name: record.name,
            paths: record.arrived.into_iter().collect(),
            unread: record.unread.into_iter().collect(),
            cut_short: record.cut_short,
        })
        .collect()
}

/// **Put away on chat `chat`'s Notice.** What it named stays on disk, and is not told again. Nor
/// is a look cut short in these folders, for this chat or another, until one is whole.
pub fn put_away(plane: &Path, chat: &str) -> io::Result<()> {
    let path = file(plane, chat)?;
    holding(plane, || {
        let Some(mut record) = read(&path)? else {
            return Ok(());
        };
        let arrived = std::mem::take(&mut record.arrived);
        let unread = std::mem::take(&mut record.unread);
        for at in arrived.into_iter().chain(unread) {
            let mut pin = Pin::of(Path::new(&at)).unwrap_or_default();
            if record.watched.contents && !is_git(Path::new(&at)) {
                pin.content = contents_of(Path::new(&at), &mut Weighing::default()).0;
            }
            record.put_away.insert(at, pin);
        }
        record.cut_short_told = record.cut_short_told || record.cut_short;
        write(plane, &path, &record)
    })
}

/// A path as a record keeps it.
fn key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// `name` as a disk that folds case compares it, Unicode's folding and not ASCII's: such a disk
/// opens `.vſcode` as `.vscode`.
fn folded(name: &str) -> String {
    if name.is_ascii() {
        name.to_ascii_lowercase()
    } else {
        name.to_uppercase().to_lowercase()
    }
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

/// [`names`], by its last part folded, each as folded parts.
fn by_last_part(lifted: &[String]) -> HashMap<String, Vec<Vec<String>>> {
    let mut out: HashMap<String, Vec<Vec<String>>> = HashMap::new();
    for name in names(lifted) {
        let parts: Vec<String> = name.split('/').map(folded).collect();
        let last = parts.last().cloned().unwrap_or_default();
        out.entry(last).or_default().push(parts);
    }
    out
}

/// Whether `path`, whose last part folded is `last`, is one of `asked`.
fn is_asked(path: &Path, last: &str, asked: &HashMap<String, Vec<Vec<String>>>) -> bool {
    let Some(wanted) = asked.get(last) else {
        return false;
    };
    wanted.iter().any(|parts| {
        parts.len() == 1
            || (path.iter().count() >= parts.len()
                && parts
                    .iter()
                    .rev()
                    .zip(path.iter().rev())
                    .all(|(part, on_disk)| folded(&on_disk.to_string_lossy()) == *part))
    })
}

/// One name a walk found.
#[derive(Debug, Clone, Copy)]
struct Found {
    /// How many levels below its folder it is.
    depth: usize,
    pin: Pin,
}

/// What a walk found.
#[derive(Debug, Default)]
struct Walked {
    found: BTreeMap<PathBuf, Found>,
    /// Each folder it could not read, a watched folder that isn't there among them.
    unread: BTreeSet<PathBuf>,
    cut_short: bool,
    /// Where it met its bound, the deepest level it read whole. A folder not read is said by name,
    /// and says nothing of the levels of any other.
    bound_at: Option<usize>,
    /// How much it has weighed of what names hold.
    weighing: Weighing,
}

impl Walked {
    fn not_read(&mut self, folder: PathBuf) {
        self.unread.insert(folder);
        self.cut_short = true;
    }

    fn keep(&mut self, path: PathBuf, depth: usize, contents: bool) {
        if has_content(&path)
            && let Some(mut pin) = Pin::of(&path)
        {
            if contents && !is_git(&path) {
                let (content, unread) = contents_of(&path, &mut self.weighing);
                pin.content = content;
                for folder in unread {
                    self.not_read(folder);
                }
            }
            self.found.insert(path, Found { depth, pin });
        }
    }
}

/// **The walk**: the home's own names, then shallowest first, each folder in name order so that one
/// bound stops at the same place twice. No link is followed: one named as a name is itself found,
/// and one that isn't says nothing. A watched folder that is a link or isn't there is not read, and
/// says so.
fn walk(watched: &Watched) -> Walked {
    let asked = by_last_part(&watched.lifted);
    let mut walked = Walked::default();
    if let Some(home) = &watched.home {
        for name in names(&watched.lifted) {
            walked.keep(home.join(name), 0, false);
        }
    }
    let mut level = Vec::new();
    for folder in &watched.folders {
        if std::fs::symlink_metadata(folder).is_ok_and(|kind| kind.is_dir()) {
            level.push(folder.clone());
        } else {
            walked.not_read(folder.clone());
        }
    }
    let mut looked = 0;
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
                    walked.not_read(dir);
                    continue;
                }
            };
            // No more of one folder is held than the bound allows.
            let left = LOOKED_AT + 1 - looked.min(LOOKED_AT);
            let mut entries: Vec<_> = entries.flatten().take(left).collect();
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                looked += 1;
                if looked > LOOKED_AT || walked.found.len() >= MOST_NAMES {
                    walked.cut_short = true;
                    walked.bound_at = Some(depth - 1);
                    return walked;
                }
                let name = folded(&entry.file_name().to_string_lossy());
                let path = entry.path();
                if is_asked(&path, &name, &asked) {
                    // The home's own names are rewritten by a person's programs all the time,
                    // and stay as the walk of the home kept them.
                    let contents = watched.contents && !walked.found.contains_key(&path);
                    if contents || !walked.found.contains_key(&path) {
                        walked.keep(path, depth, contents);
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

/// Whether `path` is a regular file, not a link or a FIFO.
fn is_regular(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|kind| kind.is_file())
}

/// A file read up to [`LARGEST_READ`], where it is a regular file.
fn small_file(path: &Path) -> Option<String> {
    String::from_utf8(read_regular(path, LARGEST_READ)?).ok()
}

/// At most `limit` bytes of the regular file at `path`, or `None` for anything else. Asked of the
/// file once it is open, with no link followed and no wait, so swapping a name for a link or a pipe
/// after a look at it reaches nothing and holds no look up.
pub(super) fn read_regular(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    #[cfg(not(unix))]
    if !is_regular(path) {
        return None;
    }
    let file = options.open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(limit).read_to_end(&mut bytes).ok()?;
    Some(bytes)
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
/// - **A new clone**: a `.git` folder the app said it made ([`made_by_the_app`]).
/// - **A file of a known repository**: one whose every file is what that repository has at that
///   path ([`Repo`]). The name is not enough, nor is what a chat could have committed.
///
/// A repository is known where the record pinned its `.git`, as the chat started or as one of the
/// above made it, and it is still that `.git`. A `.git` file's git directory must be in the
/// `worktrees` or `modules` of a `.git` so pinned. Anything else is never opened.
///
/// **Nor is a repository whose git folder the chat may write**: one in the folders it watches,
/// unless its sandbox holds that git folder whole ([`Watched::held_git`]). Its branches, its
/// index and its `worktrees` and `modules` are the chat's to write, so they vouch for nothing.
struct Known {
    tops: Vec<(PathBuf, Repo)>,
    git_dirs: Vec<PathBuf>,
    pinned: BTreeMap<String, Pin>,
    /// The folders the chat may write, and the git folders in them its sandbox holds.
    folders: Vec<PathBuf>,
    held_git: Vec<PathBuf>,
}

impl Known {
    fn of(plane: &Path, record: &Record) -> Self {
        let mut pinned = read_made(plane);
        pinned.extend(record.repos.clone());
        let mut known = Self {
            tops: Vec::new(),
            git_dirs: Vec::new(),
            pinned,
            folders: record.watched.folders.clone(),
            held_git: record.watched.held_git.clone(),
        };
        for top in record.repos.keys() {
            known.add_as_it_is(Path::new(top));
        }
        known
    }

    /// Whether the `.git` of `top` is the one pinned for it.
    fn is_pinned(&self, top: &Path) -> bool {
        self.pinned.get(&key(top)).is_some_and(|pin| {
            Pin::of(&top.join(".git")).is_some_and(|now| pin.same_repository(&now))
        })
    }

    /// Whether the chat may write the git folder `git`: it is in a folder the chat may write,
    /// by its path or the kernel's, and not in one its sandbox holds.
    fn chat_may_write(&self, git: &Path) -> bool {
        let real_git = std::fs::canonicalize(git).unwrap_or_else(|_| git.to_path_buf());
        let within = |folder: &PathBuf| {
            git.starts_with(folder)
                || std::fs::canonicalize(folder).is_ok_and(|folder| real_git.starts_with(folder))
        };
        self.folders.iter().any(within) && !self.held_git.iter().any(within)
    }

    /// `top`, read as its `.git` says, where that is a repository whose git folder the chat
    /// may not write.
    fn add_as_it_is(&mut self, top: &Path) {
        let dot_git = top.join(".git");
        let Ok(kind) = std::fs::symlink_metadata(&dot_git) else {
            return;
        };
        if !self.is_pinned(top) {
            return;
        }
        if kind.is_dir() {
            if self.chat_may_write(&dot_git) {
                return;
            }
            self.git_dirs.push(dot_git);
            self.tops.push((top.to_path_buf(), Repo::Clone));
        } else if let Some(git_dir) = pointed(&dot_git)
            && let Some(common) = common_of(&git_dir)
            && common.parent().is_some_and(|clone| self.is_pinned(clone))
            && !self.chat_may_write(&common)
            && !self.chat_may_write(&git_dir)
        {
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
    /// the app made.
    fn vouches_for_git(&self, dot_git: &Path) -> bool {
        let Some(top) = dot_git.parent() else {
            return false;
        };
        let Ok(kind) = std::fs::symlink_metadata(dot_git) else {
            return false;
        };
        if kind.is_dir() {
            return self.is_pinned(top);
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
                // The submodule's config names this folder back.
                named.starts_with(git_dir.join("modules"))
                    && named.join("HEAD").is_file()
                    && small_file(&named.join("config")).is_some_and(|config| {
                        config.lines().any(|line| {
                            line.split_once('=').is_some_and(|(word, value)| {
                                word.trim().eq_ignore_ascii_case("worktree")
                                    && same_path(
                                        &super::planted::lexical(&named.join(value.trim())),
                                        top,
                                    )
                            })
                        })
                    })
            }
        })
    }

    /// `unseen`, split: what the app or a repository vouches for, then the rest.
    fn split(mut self, unseen: BTreeSet<PathBuf>) -> (BTreeSet<PathBuf>, BTreeSet<PathBuf>) {
        let mut ours = BTreeSet::new();
        let mut theirs = BTreeSet::new();
        let (gits, others): (Vec<PathBuf>, Vec<PathBuf>) =
            unseen.into_iter().partition(|path| is_git(path));
        let mut checked = 0;
        for dot_git in gits {
            checked += 1;
            if checked <= MOST_CHECKED && self.vouches_for_git(&dot_git) {
                if let (Some(top), Some(pin)) = (dot_git.parent(), Pin::of(&dot_git)) {
                    self.pinned.insert(key(top), pin);
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
                        is_regular(index)
                            .then(|| {
                                gix::index::File::at(
                                    index,
                                    gix::hash::Kind::Sha1,
                                    false,
                                    gix::index::decode::Options::default(),
                                )
                                .ok()
                            })
                            .flatten()
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
                .or_insert_with(|| {
                    let git = top.join(".git");
                    // What an opening reads first, each a regular file or no repository.
                    (is_regular(&git.join("config")) && is_regular(&git.join("HEAD")))
                        .then(|| gix::open_opts(top, gix::open::Options::isolated()).ok())
                        .flatten()
                })
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
    let kind = std::fs::symlink_metadata(path).ok()?;
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
