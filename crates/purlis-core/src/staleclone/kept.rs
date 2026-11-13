// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A look at a clone that cannot have changed runs no git** (felt speed round 3, H8).
//!
//! [`super::found`] asked git four to six questions of every clone on every look, and the window
//! looks every time the project's git standing moves: every five minutes at least, as auto-save
//! fetches. What [`super::judged`] decides from is the clone's branch, its remote's default
//! branch (`origin/HEAD`) and `origin`'s addresses in its config, and each of those is a small
//! file of the clone's git directory. So the answer is kept with what those files said, and used
//! again while each still says it:
//!
//! - `HEAD`, and the ref it names: a switch, and a branch born under an unchanged `HEAD`;
//! - `refs/remotes/origin/HEAD`: the remote's default branch;
//! - `config` and `config.worktree`: `origin`, its push URLs and where the forge said it moved;
//! - `packed-refs`, which may hold either ref: by its identity (inode, size, time), since git
//!   only ever replaces it whole, and not trusted while it is younger than [`SETTLED`];
//! - the git config outside the clone (system, global, the command line's), as git lists it,
//!   asked once per look of the whole project.
//!
//! The small files are compared by what they say, never by time, so an edit that keeps a file's
//! time and size is still seen. **Unsure is a full read**: a config that includes another file
//! (the included one is not compared), a `.git` that is not a plain directory, refs kept in a
//! reftable, any file purlis could not read. Nothing about what counts as stale changes: a kept
//! answer is one a full read would give again.
//!
//! In memory and in this process only: nothing is written, so there is no store to give a tier
//! (ADR 0069).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use super::Seen;
use crate::worktree::git;

/// How old `packed-refs` must be for its time to be trusted: a file rewritten twice within one
/// tick of the file system's clock, to the same size, would otherwise look unchanged.
const SETTLED: Duration = Duration::from_secs(2);

/// What a clone's git files said when its answer was read.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    outside: Arc<str>,
    /// `HEAD`, the ref it names, `refs/remotes/origin/HEAD`, `config`, `config.worktree`:
    /// each one's bytes, or `None` where there is no such file.
    files: Vec<Option<Vec<u8>>>,
    packed: Option<(u64, u64, SystemTime)>,
}

/// Each clone's kept answer, by its folder.
static KEPT: LazyLock<Mutex<HashMap<PathBuf, (Stamp, Seen)>>> = LazyLock::new(Default::default);

/// The git config outside any clone, as git lists it from `plane`: what every clone's answer is
/// kept with. `None` where git would not list it, or where it includes another file, whose
/// contents the listing does not show: then nothing is kept.
pub(super) fn outside(plane: &Path) -> Option<Arc<str>> {
    let run = git::run(
        plane,
        &["config", "--list", "--no-includes", "--show-origin"],
        git::READ,
    )
    .ok()
    .filter(git::Run::ok)?;
    let includes = run.out.lines().any(|line| {
        let key = line.split_once('\t').map_or(line, |(_, key)| key);
        key.starts_with("include.") || key.starts_with("includeif.")
    });
    (!includes).then(|| Arc::from(run.out))
}

/// The answer for the clone at `clone`: the kept one where its git files say what they said when
/// it was read, else `read`'s, kept for the next look where they can be stamped.
///
/// What is kept is what [`super::judged`] reads: whether the tree is clean, and what only its
/// branch holds, are a fix's to read again, and are not kept.
pub(super) fn look(outside: Option<&Arc<str>>, clone: &Path, read: impl FnOnce() -> Seen) -> Seen {
    // Stamped before the read: a change made while it reads is a stamp the next look differs from.
    let stamp = outside.and_then(|outside| stamp(outside, clone));
    if let Some(stamp) = &stamp {
        let kept = KEPT.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, seen)) = kept.get(clone).filter(|(was, _)| was == stamp) {
            return seen.clone();
        }
    }
    let seen = Seen {
        clean: None,
        only_here: None,
        ..read()
    };
    let mut kept = KEPT.lock().unwrap_or_else(PoisonError::into_inner);
    match stamp {
        Some(stamp) => kept.insert(clone.to_path_buf(), (stamp, seen.clone())),
        None => kept.remove(clone),
    };
    seen
}

/// Lets go of the answers kept for clones under `plane` that are not among `clones`: a clone
/// removed is not held for ever.
pub(super) fn only(plane: &Path, clones: &[&Path]) {
    KEPT.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|at, _| !at.starts_with(plane) || clones.contains(&at.as_path()));
}

/// What the git files of the clone at `clone` say now, or `None` where purlis is unsure what
/// they would make git answer.
fn stamp(outside: &Arc<str>, clone: &Path) -> Option<Stamp> {
    let dir = clone.join(".git");
    // A `.git` file or link names a git directory elsewhere, and a reftable keeps refs in files
    // these are not: both are read in full.
    if !std::fs::symlink_metadata(&dir).ok()?.is_dir() || dir.join("reftable").exists() {
        return None;
    }
    let head = small(&dir.join("HEAD"))??;
    let named = match std::str::from_utf8(&head).ok()?.strip_prefix("ref: ") {
        Some(name) => {
            let name = name.trim_end();
            let plain = name.starts_with("refs/heads/")
                && name.split('/').all(|part| !part.is_empty() && part != "..");
            small(&dir.join(plain.then_some(name)?))?
        }
        // Detached: it names no branch, whatever any ref says.
        None => None,
    };
    let origin_head = small(&dir.join("refs/remotes/origin/HEAD"))?;
    let config = small(&dir.join("config"))?;
    let worktree = small(&dir.join("config.worktree"))?;
    if includes(config.as_deref()) || includes(worktree.as_deref()) {
        return None;
    }
    let packed = match std::fs::metadata(dir.join("packed-refs")) {
        Ok(meta) => {
            let modified = meta.modified().ok()?;
            let age = SystemTime::now().duration_since(modified).ok()?;
            if age < SETTLED {
                return None;
            }
            Some((inode(&meta), meta.len(), modified))
        }
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return None,
    };
    Some(Stamp {
        outside: Arc::clone(outside),
        files: vec![Some(head), named, origin_head, config, worktree],
        packed,
    })
}

/// Whether a config file's `text` has an `[include]` or `[includeIf …]` section, which names
/// another file git reads with it. A word in a comment is not one; a header purlis cannot read
/// as text counts as one.
fn includes(text: Option<&[u8]>) -> bool {
    let Some(text) = text else {
        return false;
    };
    let Ok(text) = std::str::from_utf8(text) else {
        return true;
    };
    text.lines().any(|line| {
        line.trim_start().strip_prefix('[').is_some_and(|header| {
            header
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("include")
        })
    })
}

/// A small git file's bytes: `Some(None)` where there is none, `None` where it could not be read.
fn small(path: &Path) -> Option<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => Some(Some(bytes)),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Some(None),
        Err(_) => None,
    }
}

#[cfg(unix)]
fn inode(meta: &std::fs::Metadata) -> u64 {
    std::os::unix::fs::MetadataExt::ino(meta)
}

#[cfg(not(unix))]
fn inode(_: &std::fs::Metadata) -> u64 {
    0
}
