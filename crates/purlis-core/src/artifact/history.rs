// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An artifact as it was, and two of its versions compared** (spec #5948 story 47, #5966).
//!
//! - **The template's history is git's** ([`versions`]): the last [`MOST_COMMITS`] commits that
//!   changed the artifact's folder, each a version of its own, read with the folder's files
//!   at that commit ([`as_of`]). purlis keeps no copy of a template.
//! - **The data's history is kept here, bounded** ([`keep`]): each Regenerate that placed data
//!   keeps a copy of the declared data files it wrote, and only the newest [`MOST`] are kept
//!   per artifact; an older one is removed as a new one is kept. Data is each person's (ADR 0120
//!   §1), so its copies are **Machine tier**, under the config home ([`DIR`]), keyed as the
//!   page's approval is ([`super::approval::key`]), and **never written to the plane**.
//! - **"As of" a version** pairs the template committed by then with the data kept by then: a
//!   Regenerate's version has the newest commit at or before it, a commit's version the newest
//!   data kept at or before it. A template that no commit holds by then (a LOCAL workspace's,
//!   or one never committed) is read from the folder as it is today, and [`Then::template`] is
//!   none, so the window says so.
//! - **A version is only one this module listed**: an id that is not a kept snapshot or one of
//!   the folder's listed commits is refused, so no name the window sends reaches a path or a
//!   revision git would read as anything else.
//! - **Compare** ([`compare`]) names the template's files that were added, removed or changed,
//!   and each data value that changed, by its path in the JSON, up to [`MOST_CHANGES`].
//!
//! Nothing here renders, and nothing here writes but [`keep`].

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use super::data::Declared;
use super::serve::{self, Snapshot};

/// The folder of kept data, in purlis's folder of the config home ([`crate::machine::dir`]):
/// one folder per artifact, by [`super::approval::key`], and in it one per Regenerate.
pub const DIR: &str = "artifact-snapshots";

/// **How many Regenerates' data are kept per artifact**: the newest twenty. A twenty-first
/// removes the oldest.
pub const MOST: usize = 20;

/// How many of the folder's commits are versions: the newest twenty.
pub const MOST_COMMITS: usize = 20;

/// The version that is the folder as it is now, for [`compare`].
pub const NOW: &str = "now";

/// The most changes [`compare`] names; past it the list is cut and says so.
pub const MOST_CHANGES: usize = 200;

/// The most characters of a value [`compare`] shows.
const VALUE_MOST: usize = 120;

/// How long one git read may take.
const GIT_WITHIN: Duration = Duration::from_secs(10);

/// What a version id starts with: a Regenerate's kept data, or a commit of the template.
const DATA: &str = "data:";
const TEMPLATE: &str = "template:";

/// What a version is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A Regenerate placed new data, which was kept.
    Regenerated,
    /// A commit changed the template.
    TemplateChanged,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Regenerated => "regenerated",
            Self::TemplateChanged => "template-changed",
        }
    }
}

/// One version of an artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    /// What [`as_of`] and [`compare`] are asked for.
    pub id: String,
    /// When, in seconds since 1970: when the data was kept, or the commit's time.
    pub at: i64,
    pub kind: Kind,
    /// The commit's author, for a template's version.
    pub by: Option<String>,
    /// The commit's subject, one line, for a template's version.
    pub said: Option<String>,
}

/// A commit of the template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Committed {
    /// Its id, abbreviated as the window shows it.
    pub commit: String,
    pub at: i64,
}

/// **An artifact as it was**: what its webview is served, and where its page's data reads go.
#[derive(Clone, Debug)]
pub struct Then {
    /// The template then, with the data kept then among its files.
    pub snapshot: Snapshot,
    /// The kept data's folder, which the page's reads are answered from; none when no data was
    /// kept by then, and every read answers `data: null`.
    pub data: Option<PathBuf>,
    /// When the version is.
    pub at: i64,
    /// The commit the template was read at; none when it is today's (no commit by then).
    pub template: Option<Committed>,
    /// When the data was kept, if any was.
    pub data_at: Option<i64>,
}

/// How a template file differs between two versions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    Added,
    Removed,
    Changed,
}

impl How {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Changed => "changed",
        }
    }
}

/// A template file that differs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub how: How,
}

/// A data value that differs: where it is (`data.json › open`), and its JSON before and after,
/// none where there was none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueChange {
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// What [`compare`] found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Compared {
    pub template: Vec<FileChange>,
    pub data: Vec<ValueChange>,
    /// More changed than [`MOST_CHANGES`]: the list is cut.
    pub cut: bool,
}

/// The folder an artifact's kept data is in.
fn kept_dir(config: &Path, workspace: &Path, folder: &str) -> PathBuf {
    crate::machine::dir(config)
        .join(DIR)
        .join(super::approval::key(workspace, folder))
}

/// **Keeps the data a Regenerate just placed** in artifact folder `real` (folder `folder` of
/// `workspace`), the declared files that are there, as kept at `at`; then removes all but the
/// newest [`MOST`]. Answers the version's id. Written into a hidden folder and renamed into
/// place, so a version is whole or not there.
pub fn keep(
    config: &Path,
    workspace: &Path,
    folder: &str,
    real: &Path,
    declared: &Declared,
    at: chrono::DateTime<chrono::Utc>,
) -> Result<String, String> {
    let base = crate::machine::dir(config).join(DIR);
    let dir = base.join(super::approval::key(workspace, folder));
    private_dir(&base).and_then(|()| private_dir(&dir))?;
    let secs = at.timestamp().max(0);
    let id = (0..100)
        .map(|n| format!("{secs:012}-{n:02}"))
        .find(|id| !dir.join(id).exists())
        .ok_or_else(|| "too many Regenerates were kept in one second".to_owned())?;
    let staging = dir.join(format!(".{id}.tmp"));
    let _ = std::fs::remove_dir_all(&staging);
    let kept = stage(real, declared, &staging);
    match kept {
        Ok(0) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err("none of the data files its page declares is there to keep".into());
        }
        Err(why) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(why);
        }
        Ok(_) => {}
    }
    std::fs::rename(&staging, dir.join(&id)).map_err(|e| {
        let _ = std::fs::remove_dir_all(&staging);
        format!("could not keep its data ({})", os(&e))
    })?;
    let listed = kept_in(&dir);
    for (old, _, path) in listed.iter().take(listed.len().saturating_sub(MOST)) {
        if old != &id {
            let _ = std::fs::remove_dir_all(path);
        }
    }
    Ok(format!("{DATA}{id}"))
}

/// Copies the declared data files of `real` that are there into `staging`: how many.
fn stage(real: &Path, declared: &Declared, staging: &Path) -> Result<usize, String> {
    private_dir(staging)?;
    let mut copied = 0;
    for name in &declared.data {
        let bytes = match crate::contain::read_no_link(real, &path_of(real, name)) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(format!("its {name} is a link or not a plain file")),
        };
        let target = path_of(staging, name);
        if let Some(parent) = target.parent() {
            private_dir(parent)?;
        }
        std::fs::write(&target, bytes)
            .map_err(|e| format!("could not keep its {name} ({})", os(&e)))?;
        copied += 1;
    }
    Ok(copied)
}

/// Makes `dir` (and the folders above it) if it is not there, readable by its owner alone.
fn private_dir(dir: &Path) -> Result<(), String> {
    let made = {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir_all(dir)
        }
    };
    made.map_err(|e| format!("could not keep its data ({})", os(&e)))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("could not keep its data ({})", os(&e)))?;
    }
    Ok(())
}

/// Whether `id` is a kept version's folder name: twelve digits of seconds, `-`, two digits.
fn is_kept_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    bytes.len() == 15
        && bytes[12] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(n, b)| n == 12 || b.is_ascii_digit())
}

/// The versions kept in `dir`, oldest first: id, when, and folder. A link is never one.
fn kept_in(dir: &Path) -> Vec<(String, i64, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut kept: Vec<(String, i64, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let id = entry.file_name().to_str()?.to_owned();
            is_kept_id(&id).then_some(())?;
            let at = id[..12].parse().ok()?;
            Some((id, at, entry.path()))
        })
        .collect();
    kept.sort();
    kept
}

/// One commit that changed the folder.
#[derive(Clone, Debug)]
struct Commit {
    hash: String,
    at: i64,
    by: String,
    subject: String,
}

/// The newest [`MOST_COMMITS`] commits that changed artifact folder `real`, newest first; none
/// where git holds none (a LOCAL workspace's folder is ignored).
fn commits(real: &Path) -> Vec<Commit> {
    let most = MOST_COMMITS.to_string();
    let Ok(run) = crate::worktree::git::run(
        real,
        &[
            "log",
            "-n",
            &most,
            "--format=%H%x00%ct%x00%an%x00%s",
            "--",
            ".",
        ],
        GIT_WITHIN,
    ) else {
        return Vec::new();
    };
    if !run.ok() {
        return Vec::new();
    }
    run.out
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(4, '\0');
            let hash = parts.next()?.to_owned();
            let at = parts.next()?.parse().ok()?;
            let by = parts.next()?.to_owned();
            let subject = parts.next().unwrap_or("").to_owned();
            let named = matches!(hash.len(), 40 | 64) && hash.bytes().all(|b| b.is_ascii_hexdigit());
            named.then_some(Commit {
                hash,
                at,
                by,
                subject,
            })
        })
        .collect()
}

/// **The versions of folder `folder` of `workspace`** (its folder at `real`), newest first: each
/// Regenerate this machine kept (with config home `config`; none, none kept) and each listed
/// commit of its template.
pub fn versions(
    config: Option<&Path>,
    workspace: &Path,
    folder: &str,
    real: &Path,
) -> Vec<Version> {
    let kept = config
        .map(|config| kept_in(&kept_dir(config, workspace, folder)))
        .unwrap_or_default();
    let mut versions: Vec<Version> = kept
        .into_iter()
        .map(|(id, at, _)| Version {
            id: format!("{DATA}{id}"),
            at,
            kind: Kind::Regenerated,
            by: None,
            said: None,
        })
        .chain(commits(real).into_iter().map(|commit| Version {
            id: format!("{TEMPLATE}{}", commit.hash),
            at: commit.at,
            kind: Kind::TemplateChanged,
            by: Some(crate::personas::one_line(&commit.by)),
            said: Some(crate::personas::one_line(&commit.subject)),
        }))
        .collect();
    versions.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| b.id.cmp(&a.id)));
    versions
}

/// **Artifact folder `folder` of `workspace` (at `real`) as it was at version `version`**, or
/// the sentence saying why it cannot be shown.
pub fn as_of(
    config: Option<&Path>,
    workspace: &Path,
    folder: &str,
    real: &Path,
    version: &str,
) -> Result<Then, String> {
    let kept = config
        .map(|config| kept_in(&kept_dir(config, workspace, folder)))
        .unwrap_or_default();
    let commits = commits(real);
    let (at, commit, data) = if let Some(id) = version.strip_prefix(DATA) {
        let (_, at, path) = kept
            .iter()
            .find(|(kept, _, _)| kept == id)
            .ok_or_else(|| no_version(version))?;
        let commit = commits.iter().find(|commit| commit.at <= *at);
        (*at, commit, Some((*at, path.clone())))
    } else if let Some(hash) = version.strip_prefix(TEMPLATE) {
        let commit = commits
            .iter()
            .find(|commit| commit.hash == hash)
            .ok_or_else(|| no_version(version))?;
        let data = kept
            .iter()
            .rev()
            .find(|(_, at, _)| *at <= commit.at)
            .map(|(_, at, path)| (*at, path.clone()));
        (commit.at, Some(commit), data)
    } else {
        return Err(no_version(version));
    };
    let mut files = match commit {
        Some(commit) => tree_files(real, &commit.hash, folder)?,
        None => template_today(real, folder)?,
    };
    if let Some((_, path)) = &data {
        kept_files(path, &mut files);
    }
    Ok(Then {
        snapshot: Snapshot::of_files(files, folder)?,
        data: data.as_ref().map(|(_, path)| path.clone()),
        at,
        template: commit.map(|commit| Committed {
            commit: commit.hash.chars().take(12).collect(),
            at: commit.at,
        }),
        data_at: data.map(|(at, _)| at),
    })
}

fn no_version(version: &str) -> String {
    format!(
        "'{}' is not a version of this artifact",
        crate::personas::one_line(version)
    )
}

/// The template's files as they are in the folder today, its data files left out.
fn template_today(real: &Path, folder: &str) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let today = Snapshot::read(real, folder)?;
    Ok(today
        .paths()
        .filter(|path| !serve::is_data(path))
        .filter_map(|path| Some((path.to_owned(), today.get(path)?.to_vec())))
        .collect())
}

/// Whether `path`, as git names it inside the folder, is one a [`Snapshot`] serves: no data
/// file (each person's, never the commit's), no hidden name, and nothing git could not say in
/// UTF-8.
fn served_path(path: &str) -> bool {
    !path.is_empty()
        && !serve::is_data(path)
        && !path.contains(['\\', '\0', '\u{FFFD}'])
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && !segment.starts_with('.'))
}

/// **The template's files at commit `hash`**: every plain file of the folder at `real` in that
/// commit's tree, links and submodules left out, read in one `cat-file --batch`. A folder over
/// [`super::SIZE_CAP`] then is not read.
fn tree_files(real: &Path, hash: &str, folder: &str) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let unreadable = || "purlis could not read the template at that commit".to_owned();
    let listed = crate::worktree::git::run(
        real,
        &["ls-tree", "-r", "-l", "-z", hash, "--", "."],
        GIT_WITHIN,
    )
    .map_err(|e| e.to_string())?;
    if !listed.ok() {
        return Err(unreadable());
    }
    let mut wanted: Vec<(String, String)> = Vec::new();
    let mut total: u64 = 0;
    for entry in listed.out.split('\0').filter(|entry| !entry.is_empty()) {
        let Some((meta, path)) = entry.split_once('\t') else {
            continue;
        };
        let mut meta = meta.split_ascii_whitespace();
        let (Some(mode), Some(kind), Some(oid), Some(size)) =
            (meta.next(), meta.next(), meta.next(), meta.next())
        else {
            continue;
        };
        if kind != "blob" || !matches!(mode, "100644" | "100755") || !served_path(path) {
            continue;
        }
        total = total.saturating_add(size.parse::<u64>().unwrap_or(u64::MAX));
        if total > super::SIZE_CAP {
            return Err(format!(
                "{} at that commit, so none of it is shown",
                super::too_large(folder, total)
            ));
        }
        wanted.push((path.to_owned(), oid.to_owned()));
    }
    let input: Vec<u8> = wanted
        .iter()
        .flat_map(|(_, oid)| format!("{oid}\n").into_bytes())
        .collect();
    let read =
        crate::worktree::git::run_with_input(real, &["cat-file", "--batch"], input, GIT_WITHIN)
            .map_err(|e| e.to_string())?;
    if read.code != Some(0) {
        return Err(unreadable());
    }
    let blobs = batch_blobs(&read.out).ok_or_else(unreadable)?;
    if blobs.len() != wanted.len() {
        return Err(unreadable());
    }
    Ok(wanted
        .into_iter()
        .zip(blobs)
        .map(|((path, _), bytes)| (path, bytes))
        .collect())
}

/// The blobs of a `cat-file --batch` answer, in the order asked; nothing when one was missing
/// or the answer does not read as git's.
fn batch_blobs(mut out: &[u8]) -> Option<Vec<Vec<u8>>> {
    let mut blobs = Vec::new();
    while !out.is_empty() {
        let end = out.iter().position(|&b| b == b'\n')?;
        let header = std::str::from_utf8(&out[..end]).ok()?;
        let mut parts = header.split(' ');
        let (_, kind, size) = (parts.next()?, parts.next()?, parts.next()?);
        if kind != "blob" {
            return None;
        }
        let size: usize = size.parse().ok()?;
        let body = out.get(end + 1..end + 1 + size)?;
        blobs.push(body.to_vec());
        out = out.get(end + 2 + size..)?;
    }
    Some(blobs)
}

/// Adds the data files kept in `dir` to `files`, by their names in the folder: `data.json` and
/// every plain file under `data/`. A link is never read.
fn kept_files(dir: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
    fn walk(
        dir: &Path,
        at: &Path,
        name: &str,
        depth: usize,
        files: &mut BTreeMap<String, Vec<u8>>,
    ) {
        let Ok(meta) = std::fs::symlink_metadata(at) else {
            return;
        };
        if meta.is_file() {
            if let Ok(bytes) = crate::contain::read_no_link(dir, at) {
                files.insert(name.to_owned(), bytes);
            }
        } else if meta.is_dir() && depth < 16 {
            let Ok(entries) = std::fs::read_dir(at) else {
                return;
            };
            for entry in entries.filter_map(Result::ok) {
                if let Some(child) = entry.file_name().to_str() {
                    walk(
                        dir,
                        &entry.path(),
                        &format!("{name}/{child}"),
                        depth + 1,
                        files,
                    );
                }
            }
        }
    }
    walk(dir, &dir.join(super::DATA_FILE), super::DATA_FILE, 0, files);
    walk(dir, &dir.join(super::DATA_DIR), super::DATA_DIR, 0, files);
}

/// **What changed from version `from` to version `to`** of folder `folder` of `workspace` (at
/// `real`): either may be [`NOW`], the folder as it is.
pub fn compare(
    config: Option<&Path>,
    workspace: &Path,
    folder: &str,
    real: &Path,
    from: &str,
    to: &str,
) -> Result<Compared, String> {
    let files = |version: &str| {
        if version == NOW {
            Snapshot::read(real, folder)
        } else {
            as_of(config, workspace, folder, real, version).map(|then| then.snapshot)
        }
    };
    let (before, after) = (files(from)?, files(to)?);
    let paths: BTreeSet<&str> = before.paths().chain(after.paths()).collect();
    let mut compared = Compared::default();
    for path in paths.iter().filter(|path| !serve::is_data(path)) {
        let how = match (before.get(path), after.get(path)) {
            (None, Some(_)) => How::Added,
            (Some(_), None) => How::Removed,
            (Some(a), Some(b)) if a != b => How::Changed,
            _ => continue,
        };
        compared.template.push(FileChange {
            path: (*path).to_owned(),
            how,
        });
    }
    for path in paths.iter().filter(|path| serve::is_data(path)) {
        let (a, b) = (json(before.get(path)), json(after.get(path)));
        if a != b {
            diff(path, a.as_ref(), b.as_ref(), &mut compared);
        }
    }
    Ok(compared)
}

/// A data file's bytes as JSON; one that is not JSON is said so, as a string.
fn json(bytes: Option<&[u8]>) -> Option<Value> {
    bytes.map(|bytes| {
        serde_json::from_slice(bytes).unwrap_or_else(|_| Value::String("(not JSON)".into()))
    })
}

/// Names each value that differs between `a` and `b`, at `path`: objects key by key and arrays
/// place by place, anything else whole.
fn diff(path: &str, a: Option<&Value>, b: Option<&Value>, compared: &mut Compared) {
    if compared.cut {
        return;
    }
    match (a, b) {
        (Some(Value::Object(a)), Some(Value::Object(b))) => {
            let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            for key in keys {
                diff(&format!("{path} › {key}"), a.get(key), b.get(key), compared);
            }
        }
        (Some(Value::Array(a)), Some(Value::Array(b))) => {
            for n in 0..a.len().max(b.len()) {
                diff(&format!("{path} › {n}"), a.get(n), b.get(n), compared);
            }
        }
        _ if a == b => {}
        _ => {
            if compared.data.len() == MOST_CHANGES {
                compared.cut = true;
                return;
            }
            compared.data.push(ValueChange {
                path: path.to_owned(),
                before: a.map(shown),
                after: b.map(shown),
            });
        }
    }
}

/// A value as JSON, cut to [`VALUE_MOST`] characters.
fn shown(value: &Value) -> String {
    let text = value.to_string();
    if text.chars().count() <= VALUE_MOST {
        return text;
    }
    let mut cut: String = text.chars().take(VALUE_MOST).collect();
    cut.push('…');
    cut
}

/// Where data file `name` is inside `folder`.
fn path_of(folder: &Path, name: &str) -> PathBuf {
    name.split('/')
        .fold(folder.to_path_buf(), |at, s| at.join(s))
}

fn os(e: &std::io::Error) -> String {
    crate::rewrite::os_words(e)
}

#[cfg(all(test, unix))]
mod tests;
