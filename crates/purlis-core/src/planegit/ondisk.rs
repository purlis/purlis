// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A save commits a file only as the one name it has on disk** (the plane's save and a
//! repo's): each staged path that is not a deletion is looked at once the `git add` is done,
//! and one the readers would not read is taken out of the index again and not committed.
//!
//! The readers ([`crate::held`], [`crate::files`]) already refuse a file with a second name, a
//! hard link, since its bytes can be another file's. A save holds itself to the same rule, and
//! to one more: what is staged is the file on disk now, so a file swapped or removed after the
//! add is not committed on the strength of bytes nobody looks at again.
//!
//! **Asked after the add, never before it.** The index is what the commit carries, and it can
//! hold rows staged before the save ran, which a listing of the working tree made ahead of the
//! add would not see; and a check before the add leaves the add itself as a gap in which the
//! file can change. A blob the add wrote for a refused row stays a loose object no commit
//! names: it is never pushed, and git's own housekeeping prunes it.

use std::collections::HashSet;
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

/// **What a save of the repository at `root` says where [`not_as_staged`] answered `found`**,
/// or `None` where it holds nothing back. Each path held back is taken out of the index again
/// first ([`unstage`]); where that fails, it is said.
pub(crate) fn refused(root: &Path, found: &Result<Vec<Held>, String>) -> Option<Refusal> {
    let held = match found {
        Ok(held) if held.is_empty() => return None,
        Ok(held) => held,
        Err(said) => {
            return Some(Refusal {
                named: vec![format!(
                    "purlis could not tell whether what is staged is the file on disk: {said}"
                )],
                then: "Nothing was committed.".into(),
            });
        }
    };
    let named = held
        .iter()
        .map(|one| {
            format!(
                "{} {}",
                shown::readable(&one.path, shown::DISPLAY_LIMIT),
                one.why.words()
            )
        })
        .collect();
    let then = match unstage(root, held) {
        Ok(()) => "Nothing was committed, and those files are no longer staged.".to_owned(),
        Err(said) => format!(
            "Nothing was committed, and git could not take those files out of the index again \
             ({said}). Unstage them with `git reset -- <path>` before committing."
        ),
    };
    Some(Refusal { named, then })
}

/// The journal's words for a save [`refused`] held back — matched whole, never searched for.
pub const NOT_AS_STAGED: &str = "a staged file that is not the one file on disk";

#[cfg(test)]
mod tests;
