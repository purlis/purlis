// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A save commits a file only as the one name it has on disk** (the plane's save and a
//! repo's, and each commit made as a save makes one): each file is staged from one open
//! descriptor, and each staged path that is not a deletion is looked at again once it is staged.
//! One the readers would not read is not staged, or is taken out of the index again, and is not
//! committed.
//!
//! The readers ([`crate::held`], [`crate::files`]) already refuse a file with a second name, a
//! hard link, since its bytes can be another file's. A save holds itself to the same rule, and
//! to one more: what is staged is the file on disk now, so a file swapped or removed after it
//! was staged is not committed on the strength of bytes nobody looks at again.
//!
//! **Every save stages each file from one open descriptor** ([`stage_named`]): the file is
//! opened without following a link, its descriptor is asked that it is a regular file with one
//! name, and the bytes git stores are read from that descriptor. A file with a second name is
//! never hashed, so its bytes never reach the repository's object store, and the bytes
//! committed are the bytes that were checked, whatever has the name a moment later. A link, a
//! folder (a nested repository) and a deletion are staged by `git add`, which reads no file's
//! bytes.
//!
//! **Asked before the staging, and again after it.** Before: each file the staging would take
//! is looked at, and one with a second name refuses the save with nothing staged
//! ([`second_names_ahead`]). After: the index is what the commit carries, and it can hold rows
//! staged before the save ran, which a listing of the working tree does not see, and a file can
//! change once it is staged ([`not_as_staged`]). The blob of a row refused then is taken out of
//! the object store again where it is a loose object nothing else reaches ([`refused`]); a
//! packed object, or one a ref, a reflog or an index reaches, is never touched.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::shown;
use crate::worktree::git;

/// Why a staged path is not committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Why {
    /// A regular file with more than one name.
    SecondName,
    /// The file on disk is not the one that was staged: written, swapped or touched since.
    Changed,
    /// No file on disk by that name any more.
    Gone,
    /// Git is told not to look at the file on disk (`skip-worktree` or `assume-unchanged`), so
    /// whether it is what was staged cannot be asked.
    NotLooked,
}

impl Why {
    /// The clause after the path, in the command line's words.
    pub(crate) fn words(self) -> &'static str {
        match self {
            Why::SecondName => "has a second name (a hard link), so purlis does not save it",
            Why::Changed => "changed on disk after it was staged, so purlis does not save it",
            Why::Gone => "is no longer on disk as it was staged, so purlis does not save it",
            Why::NotLooked => {
                "is marked for git not to look at it on disk, so purlis cannot tell that what \
                 is staged is this file, and does not save it"
            }
        }
    }
}

/// One staged path a save does not commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Held {
    pub(crate) path: String,
    pub(crate) why: Why,
}

/// The NUL-separated paths `args` prints in `root`, or what git said where it could not.
fn listed(root: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    let run = git::run(root, args, git::READ).map_err(|e| e.to_string())?;
    if !run.ok() {
        return Err(super::tail(&run));
    }
    Ok(run
        .out
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect())
}

/// **Each path staged in the repository at `root` that a save does not commit**, and why: a
/// regular file with a second name, one that is no longer on disk or not the file that was
/// staged, and one git is told not to look at on disk. A deletion carries no file, and a
/// folder is a nested repository's commit id, so neither is asked about. `Err` with git's
/// words where git could not say, which a save refuses as it refuses a finding.
pub(crate) fn not_as_staged(root: &Path) -> Result<Vec<Held>, String> {
    let staged = listed(
        root,
        &[
            "diff",
            "--cached",
            "--name-only",
            "-z",
            "--no-renames",
            "--diff-filter=d",
        ],
    )?;
    if staged.is_empty() {
        return Ok(Vec::new());
    }
    // Every stat field compared, whatever the repository's config relaxes: a file swapped
    // for another, or moved away and back, keeps no inode or change time of the one staged.
    let changed: HashSet<String> = listed(
        root,
        &[
            "-c",
            "core.checkStat=default",
            "-c",
            "core.trustctime=true",
            "diff-files",
            "--name-only",
            "-z",
            "--no-renames",
            "--ignore-submodules=all",
        ],
    )?
    .into_iter()
    .collect();
    // `ls-files -v` tags a `skip-worktree` row `S` and an `assume-unchanged` row in lower case;
    // `diff-files` reports neither, whatever is on disk.
    let not_looked: HashSet<String> = listed(root, &["ls-files", "-v", "-z"])?
        .into_iter()
        .filter_map(|row| {
            let (tag, path) = row.split_once(' ')?;
            (tag == "S" || tag.chars().all(|c| c.is_ascii_lowercase())).then(|| path.to_owned())
        })
        .collect();
    let mut held = Vec::new();
    for path in staged {
        // Asked of the name itself, never through a link: a link is committed as the link.
        let why = match std::fs::symlink_metadata(root.join(&path)) {
            Err(_) => Some(Why::Gone),
            Ok(found) if found.is_dir() => None,
            Ok(found) if found.is_file() && links(&found) > 1 => Some(Why::SecondName),
            Ok(_) if not_looked.contains(&path) => Some(Why::NotLooked),
            Ok(_) if changed.contains(&path) => Some(Why::Changed),
            Ok(_) => None,
        };
        if let Some(why) = why {
            held.push(Held { path, why });
        }
    }
    Ok(held)
}

/// **Each file a `git add` of `pathspec` (all of the tree where it is empty) would stage that
/// has a second name**, asked before the add: a tracked file that changed and a file git does
/// not track or ignore, each looked at by its own name, never through a link. `Err` with git's
/// words where git could not list them, which a save refuses with nothing staged.
pub(crate) fn second_names_ahead(root: &Path, pathspec: &[&str]) -> Result<Vec<Held>, String> {
    let mut args = vec![
        "ls-files",
        "-z",
        "--modified",
        "--others",
        "--exclude-standard",
        "--deduplicate",
        "--",
    ];
    args.extend_from_slice(pathspec);
    Ok(listed(root, &args)?
        .into_iter()
        .filter(|path| {
            std::fs::symlink_metadata(root.join(path))
                .is_ok_and(|found| found.is_file() && links(&found) > 1)
        })
        .map(|path| Held {
            path,
            why: Why::SecondName,
        })
        .collect())
}

/// [`second_names_ahead`] for `paths` named relative to `root`, each read letter for letter
/// (`:(literal)`), a few at a time so a long list stays well inside what a start holds.
pub(crate) fn second_names_of(root: &Path, paths: &[String]) -> Result<Vec<Held>, String> {
    let mut held = Vec::new();
    for part in paths.chunks(UNSTAGED_AT_ONCE) {
        let specs: Vec<String> = part.iter().map(|path| super::literally(path)).collect();
        let specs: Vec<&str> = specs.iter().map(String::as_str).collect();
        held.extend(second_names_ahead(root, &specs)?);
    }
    Ok(held)
}

/// The paths after the `--` of the `git add` arguments `add`: none where there is no `--`,
/// which is all of the tree the add is run in.
pub(crate) fn pathspec_of<'a>(add: &[&'a str]) -> Vec<&'a str> {
    add.iter()
        .position(|arg| *arg == "--")
        .map(|at| add[at + 1..].to_vec())
        .unwrap_or_default()
}

/// How many names the file `found` describes has.
#[cfg(unix)]
fn links(found: &std::fs::Metadata) -> u64 {
    std::os::unix::fs::MetadataExt::nlink(found)
}

/// Where the count of a file's names cannot be read, one.
#[cfg(not(unix))]
fn links(_: &std::fs::Metadata) -> u64 {
    1
}

/// The most paths one `git reset` is handed, so a long list stays well inside what a start
/// holds.
const UNSTAGED_AT_ONCE: usize = 256;

/// Takes each of `held` out of the index of the repository at `root` again, as `HEAD` has it,
/// so a later commit made without purlis does not carry it either. Nothing else is touched:
/// the files on disk stay as they are.
fn unstage(root: &Path, held: &[Held]) -> Result<(), String> {
    for part in held.chunks(UNSTAGED_AT_ONCE) {
        let specs: Vec<String> = part.iter().map(|one| super::literally(&one.path)).collect();
        let mut args = vec!["reset", "-q", "--"];
        args.extend(specs.iter().map(String::as_str));
        let run = git::run(root, &args, super::WRITE).map_err(|e| e.to_string())?;
        if !run.ok() {
            return Err(super::tail(&run));
        }
    }
    Ok(())
}

/// What a save says where it holds paths back ([`refused`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Refusal {
    /// One line for each path, or for why none could be asked about.
    pub(crate) named: Vec<String>,
    /// What became of them.
    pub(crate) then: String,
}

/// How a refusal puts back the rows it holds back ([`refused_into`]).
#[derive(Debug, Clone, Copy)]
pub(crate) enum PutBack<'a> {
    /// As `HEAD` has them (`git reset`): a save's or a commit's.
    AsHead,
    /// As the conflict git left them in, from the index's own record of it (`update-index
    /// --unresolve`): a stopped rebase's resolved files, where `HEAD` is not what the paused
    /// commit would carry. It carries every file that was staged, all put back where git could
    /// not say which are held.
    Conflicted(&'a [String]),
}

/// **What a save of the repository at `root` says where [`not_as_staged`] answered `found`**,
/// or `None` where it holds nothing back. Each path held back is taken out of the index again
/// first ([`unstage`]); where that fails, it is said.
pub(crate) fn refused(root: &Path, found: &Result<Vec<Held>, String>) -> Option<Refusal> {
    refused_into(root, found, PutBack::AsHead)
}

/// **`found` split into the rows a save takes out before its secret guard reads any blob, and
/// the rest.** A row with a second name, or one changed since it was staged, holds bytes nobody
/// checked as this file's, and a row that is gone has no file left to stand for: each is named
/// from the row alone, and its blob leaves the object store first. A row git is told not to
/// look at holds what was staged under its own name: the guard reads it and names what it
/// finds, and the row is taken out after it ([`refused`] of the second). Where git could not
/// say, that is the first, and the second is empty.
pub(crate) fn split_for_guard(
    found: Result<Vec<Held>, String>,
) -> (Result<Vec<Held>, String>, Vec<Held>) {
    match found {
        Ok(held) => {
            let (first, after): (Vec<Held>, Vec<Held>) = held
                .into_iter()
                .partition(|one| !matches!(one.why, Why::NotLooked));
            (Ok(first), after)
        }
        Err(said) => (Err(said), Vec::new()),
    }
}

impl Refusal {
    /// `first` and `then` said as one refusal: every path, then what became of them.
    pub(crate) fn joined(first: Option<Refusal>, then: Option<Refusal>) -> Option<Refusal> {
        match (first, then) {
            (Some(mut first), Some(then)) => {
                first.named.extend(then.named);
                if first.then != then.then {
                    first.then = format!("{} {}", first.then, then.then);
                }
                Some(first)
            }
            (first, then) => first.or(then),
        }
    }
}

/// [`refused`], each path held back put back as `back` says.
pub(crate) fn refused_into(
    root: &Path,
    found: &Result<Vec<Held>, String>,
    back: PutBack<'_>,
) -> Option<Refusal> {
    let held = match found {
        Ok(held) if held.is_empty() => return None,
        Ok(held) => held,
        Err(said) => {
            let then = match back {
                // Nothing was taken out of the index, so no blob is left unstaged by this.
                PutBack::AsHead => "Nothing was committed.".to_owned(),
                // Every file staged is put back in conflict, and the blob each was staged as
                // taken out of the object store where nothing else reaches it, as below.
                PutBack::Conflicted(staged) => {
                    let all: Vec<Held> = staged
                        .iter()
                        .map(|path| Held {
                            path: path.clone(),
                            why: Why::Changed,
                        })
                        .collect();
                    put_back_and_forget(root, &all, back)
                }
            };
            return Some(Refusal {
                named: vec![format!(
                    "purlis could not tell whether what is staged is the file on disk: {said}"
                )],
                then,
            });
        }
    };
    Some(Refusal {
        named: named(held),
        then: put_back_and_forget(root, held, back),
    })
}

/// [`put_back`], and where every one of `held` went back, the blob each was staged as taken out
/// of the object store where nothing else reaches it ([`forget`]): what became of them, in a
/// sentence.
fn put_back_and_forget(root: &Path, held: &[Held], back: PutBack<'_>) -> String {
    // Asked before the index forgets them: once unstaged, nothing says which blob each was.
    let blobs = staged_blobs(root, held);
    let (back_all, then) = put_back(root, held, back);
    if back_all {
        forget(root, &blobs);
    }
    then
}

/// Puts each of `held` back in the index of the repository at `root` as `back` says: whether
/// they all went back, and what became of them in a sentence.
fn put_back(root: &Path, held: &[Held], back: PutBack<'_>) -> (bool, String) {
    let then = match (back, unstage_as(root, held, back)) {
        (PutBack::AsHead, Ok(())) => {
            return (
                true,
                "Nothing was committed, and those files are no longer staged.".to_owned(),
            );
        }
        (PutBack::Conflicted(_), Ok(())) => {
            return (
                true,
                "Nothing was committed, and those files are marked as conflicted again.".to_owned(),
            );
        }
        (PutBack::AsHead, Err(said)) => format!(
            "Nothing was committed, and git could not take those files out of the index again \
             ({said}). Unstage them with `git reset -- <path>` before committing."
        ),
        (PutBack::Conflicted(_), Err(said)) => format!(
            "Nothing was committed, and git could not mark those files as conflicted again \
             ({said}). Finish or abort the rebase from your own terminal."
        ),
    };
    (false, then)
}

/// [`unstage`], or for a stopped rebase's resolved files their conflict restored.
fn unstage_as(root: &Path, held: &[Held], back: PutBack<'_>) -> Result<(), String> {
    match back {
        PutBack::AsHead => unstage(root, held),
        // `update-index` takes paths, not pathspecs: each is the file of that name.
        PutBack::Conflicted(_) => {
            for part in held.chunks(UNSTAGED_AT_ONCE) {
                let mut args = vec!["update-index", "--unresolve", "--"];
                args.extend(part.iter().map(|one| one.path.as_str()));
                run_ok(root, &args)?;
            }
            Ok(())
        }
    }
}

impl Refusal {
    /// The refusal in one sentence, for an answer that is one: each path, then what became of
    /// them.
    pub(crate) fn sentence(&self) -> String {
        format!("{}. {}", self.named.join("; "), self.then)
    }
}

/// One line for each of `held`: its path, and why it is not saved.
fn named(held: &[Held]) -> Vec<String> {
    held.iter()
        .map(|one| {
            format!(
                "{} {}",
                shown::readable(&one.path, shown::DISPLAY_LIMIT),
                one.why.words()
            )
        })
        .collect()
}

/// What a save says where [`second_names_ahead`] answered `found` before anything was staged,
/// or `None` where it found nothing.
pub(crate) fn refused_ahead(found: &Result<Vec<Held>, String>) -> Option<Refusal> {
    let named = match found {
        Ok(held) if held.is_empty() => return None,
        Ok(held) => named(held),
        Err(said) => vec![format!(
            "purlis could not tell whether each file to be saved has one name on disk: {said}"
        )],
    };
    Some(Refusal {
        named,
        then: "Nothing was staged or committed.".into(),
    })
}

/// **Stages `pathspec` (all of the tree where it is empty) in the repository at `root` as a
/// save does, for a commit made outside a save** (a publish, a new project's first commit, a
/// doctor fix): [`second_names_ahead`] before anything is staged, then each file staged from one
/// open descriptor ([`stage_named`]), then [`not_as_staged`] of the whole index, with what
/// [`refused`] takes back. `Err` with `failed_as`, a colon and git's words where git could not
/// stage them, or with `refused_as`, a colon, then each path's line and what became of them;
/// nothing is committed either way.
pub(crate) fn add_checked(
    root: &Path,
    pathspec: &[&str],
    refused_as: &str,
    failed_as: &str,
) -> Result<(), String> {
    let refusal = match refused_ahead(&second_names_ahead(root, pathspec)) {
        Some(refusal) => refusal,
        None => {
            #[cfg(test)]
            super::tests::before_the_add(root);
            let held = stage_named(root, pathspec, |_| false)
                .map_err(|said| format!("{failed_as}: {said}"))?;
            #[cfg(test)]
            super::tests::after_the_add(root);
            match refused(root, &Ok(held)).or_else(|| refused(root, &not_as_staged(root))) {
                Some(refusal) => refusal,
                None => return Ok(()),
            }
        }
    };
    Err(format!(
        "{refused_as}: {}. {}",
        refusal.named.join("; "),
        refusal.then
    ))
}

/// The blob each of `held` is staged as, from the index: a nested repository's commit (mode
/// `160000`) is not an object of this repository's, and is left out. None where git could not
/// say, and then nothing is taken out of the object store.
fn staged_blobs(root: &Path, held: &[Held]) -> Vec<String> {
    let mut blobs = Vec::new();
    for part in held.chunks(UNSTAGED_AT_ONCE) {
        let specs: Vec<String> = part.iter().map(|one| super::literally(&one.path)).collect();
        let mut args = vec!["ls-files", "-s", "-z", "--"];
        args.extend(specs.iter().map(String::as_str));
        let Ok(rows) = listed(root, &args) else {
            return Vec::new();
        };
        blobs.extend(rows.iter().filter_map(|row| {
            let (meta, _path) = row.split_once('\t')?;
            let mut fields = meta.split(' ');
            let mode = fields.next()?;
            let sha = fields.next()?;
            (mode != "160000" && object_id(sha)).then(|| sha.to_owned())
        }));
    }
    blobs.sort();
    blobs.dedup();
    blobs
}

/// Whether `sha` is spelled as an object id: forty or sixty-four lower-case hex digits.
fn object_id(sha: &str) -> bool {
    matches!(sha.len(), 40 | 64) && sha.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// **Takes each of `blobs` out of the object store of the repository at `root` where it is a
/// loose object nothing reaches**: no ref, no reflog and no index of any of the repository's
/// work trees. A packed object, or one anything reaches, stays; where git cannot say what is
/// reached, every one stays.
fn forget(root: &Path, blobs: &[String]) {
    let Some(objects) = objects_dir(root) else {
        return;
    };
    let mut unreached: HashSet<&str> = blobs
        .iter()
        .map(String::as_str)
        .filter(|sha| loose_at(&objects, sha))
        .collect();
    if unreached.is_empty() {
        return;
    }
    let reached = git::run(
        root,
        &[
            "rev-list",
            "--objects",
            "--all",
            "--reflog",
            "--indexed-objects",
        ],
        super::WRITE,
    );
    let Some(reached) = reached.ok().filter(git::Run::ok) else {
        return;
    };
    for line in reached.out.lines() {
        unreached.remove(line.split(' ').next().unwrap_or_default());
        if unreached.is_empty() {
            return;
        }
    }
    for sha in unreached {
        remove_loose(&objects, sha);
    }
}

/// The repository's object store, as git names it for the tree at `root`.
fn objects_dir(root: &Path) -> Option<std::path::PathBuf> {
    let run = git::run(root, &["rev-parse", "--git-path", "objects"], git::READ).ok()?;
    let named = run.line().trim();
    (run.ok() && !named.is_empty()).then(|| root.join(named))
}

/// Whether `sha` is a loose object in `objects`: a regular file at `objects/xx/<rest>`, each
/// looked at by its own name, never through a link.
fn loose_at(objects: &Path, sha: &str) -> bool {
    if !object_id(sha) {
        return false;
    }
    let (fan, rest) = sha.split_at(2);
    let dir = objects.join(fan);
    std::fs::symlink_metadata(&dir).is_ok_and(|found| found.is_dir())
        && std::fs::symlink_metadata(dir.join(rest)).is_ok_and(|found| found.is_file())
}

/// Unlinks the loose object `sha` from `objects`, through the folder it is in held open without
/// following a link, and only where it is a regular file there.
#[cfg(unix)]
fn remove_loose(objects: &Path, sha: &str) {
    let (fan, rest) = sha.split_at(2);
    let Ok(Some(dir)) = crate::held::Store::project(objects)
        .map_err(|_| ())
        .and_then(|store| store.sub(fan, false).map_err(|_| ()))
    else {
        return;
    };
    let plain = dir.entry(rest).is_ok_and(|entry| {
        entry.is_some_and(|entry| entry.kind == rustix::fs::FileType::RegularFile)
    });
    if plain {
        let _ = dir.remove(rest);
    }
}

/// Where a folder cannot be held open, nothing is removed.
#[cfg(not(unix))]
fn remove_loose(_: &Path, _: &str) {}

/// What a file a save stages is, asked of one open descriptor ([`opened`]).
pub(super) enum OnDisk {
    /// A regular file with one name, open to read, and whether its owner may run it.
    File(std::fs::File, bool),
    /// A regular file with more than one name.
    SecondName,
    /// Anything else, or nothing: a link, a folder (a nested repository), a removed file. Each
    /// is staged by `git add`, which stages a link as the link and reads no file's bytes.
    Other,
}

/// What `path` (relative to `root`) is, opened one folder at a time and never through a link.
#[cfg(unix)]
pub(super) fn opened(root: &Path, path: &str) -> OnDisk {
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let Some((name, folders)) = parts.split_last() else {
        return OnDisk::Other;
    };
    let Ok(mut dir) = crate::held::Store::project(root) else {
        return OnDisk::Other;
    };
    for folder in folders {
        dir = match dir.sub(folder, false) {
            Ok(Some(below)) => below,
            _ => return OnDisk::Other,
        };
    }
    match dir.open_checked(name) {
        Ok(Some((file, entry))) if entry.kind == rustix::fs::FileType::RegularFile => {
            if entry.links > 1 {
                OnDisk::SecondName
            } else {
                OnDisk::File(file, entry.mode.contains(rustix::fs::Mode::XUSR))
            }
        }
        _ => OnDisk::Other,
    }
}

/// Where a folder cannot be held open, each path is staged by `git add`.
#[cfg(not(unix))]
pub(super) fn opened(_: &Path, _: &str) -> OnDisk {
    OnDisk::Other
}

/// **Stages what a `git add -A` of the pathspecs `specs` (all of the tree where there are none)
/// would, each regular file from one open descriptor**, leaving out each path `kept_out` names. A file whose descriptor says
/// it has a second name is not staged, and is answered, for the save to refuse as it refuses
/// one [`not_as_staged`] finds. A link, a folder or a removed file is staged by `git add`.
/// `Err` with git's words where git could not list or stage them.
pub(crate) fn stage_named(
    root: &Path,
    specs: &[&str],
    kept_out: impl Fn(&str) -> bool,
) -> Result<Vec<Held>, String> {
    let mut args = vec![
        "ls-files",
        "-z",
        "--modified",
        "--others",
        "--deleted",
        "--exclude-standard",
        "--deduplicate",
        "--",
    ];
    args.extend_from_slice(specs);
    let mut files = Vec::new();
    let mut by_git = Vec::new();
    let mut held = Vec::new();
    for path in listed(root, &args)? {
        // `--others` names a nested repository as its folder, with a `/` after it.
        let path = path.trim_end_matches('/').to_owned();
        if kept_out(&path) {
            continue;
        }
        match opened(root, &path) {
            OnDisk::File(file, runs) => files.push((path, file, runs)),
            OnDisk::SecondName => held.push(Held {
                path,
                why: Why::SecondName,
            }),
            OnDisk::Other => by_git.push(path),
        }
    }
    stage_opened(root, files)?;
    for part in by_git.chunks(UNSTAGED_AT_ONCE) {
        let specs: Vec<String> = part.iter().map(|path| super::literally(path)).collect();
        let mut args = vec!["add", "-A", "--"];
        args.extend(specs.iter().map(String::as_str));
        run_ok(root, &args)?;
    }
    // A row staged from its bytes carries no times of the file on disk, which git would read as
    // a change: recorded now, where the file still holds what was staged. One that changed
    // since stays a change, which [`not_as_staged`] then refuses. Never with paths: handed one,
    // `update-index` stages that file again from its name.
    // The index write that adds git's untracked cache, as a save's `git add` did (FD-11,
    // [`git::UNTRACKED_CACHE`]).
    run_ok(
        root,
        &git::with_untracked_cache(root, &["update-index", "-q", "--refresh"]),
    )?;
    Ok(held)
}

/// **Stages each of `files` from its open descriptor**: the bytes git stores are read from the
/// descriptor (`hash-object --stdin`, filtered as the file at its path would be), and the row
/// is written with them (`update-index --index-info`), so nothing is read again by name. Its
/// mode is the owner's run bit where git trusts the file system's, else the mode it is staged
/// with already.
fn stage_opened(root: &Path, files: Vec<(String, std::fs::File, bool)>) -> Result<(), String> {
    if files.is_empty() {
        return Ok(());
    }
    let trusted = git::run(root, &["config", "--type=bool", "core.fileMode"], git::READ)
        .map_or(true, |run| run.line().trim() != "false");
    let staged: HashMap<String, String> = if trusted {
        HashMap::new()
    } else {
        listed(root, &["ls-files", "-s", "-z"])?
            .iter()
            .filter_map(|row| {
                let (meta, path) = row.split_once('\t')?;
                Some((path.to_owned(), meta.split(' ').next()?.to_owned()))
            })
            .collect()
    };
    let mut rows = Vec::new();
    for (path, file, runs) in files {
        let at = format!("--path={path}");
        let hashed = git::run_reading(
            root,
            &["hash-object", "-w", "--stdin", &at],
            file,
            super::WRITE,
        )
        .map_err(|e| e.to_string())?;
        let sha = String::from_utf8_lossy(&hashed.out).trim().to_owned();
        if hashed.code != Some(0) || !object_id(&sha) {
            return Err(String::from_utf8_lossy(&hashed.err).trim().to_owned());
        }
        let mode = match (trusted, runs) {
            (true, true) => "100755",
            (true, false) => "100644",
            (false, _) if staged.get(&path).is_some_and(|mode| mode == "100755") => "100755",
            (false, _) => "100644",
        };
        rows.extend_from_slice(format!("{mode} {sha}\t{path}\0").as_bytes());
    }
    let written = git::run_with_input(
        root,
        &["update-index", "-z", "--index-info"],
        rows,
        super::WRITE,
    )
    .map_err(|e| e.to_string())?;
    if written.code != Some(0) {
        return Err(String::from_utf8_lossy(&written.err).trim().to_owned());
    }
    Ok(())
}

/// Runs `args` in `root`, or what git said where it failed.
fn run_ok(root: &Path, args: &[&str]) -> Result<(), String> {
    let run = git::run(root, args, super::WRITE).map_err(|e| e.to_string())?;
    if run.ok() {
        Ok(())
    } else {
        Err(super::tail(&run))
    }
}

/// The journal's words for a save [`refused`] held back — matched whole, never searched for.
pub const NOT_AS_STAGED: &str = "a staged file that is not the one file on disk";

#[cfg(test)]
mod tests;
