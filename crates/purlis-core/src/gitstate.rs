// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The third answer: charter asked git and git could not say. A port of the part of
//! `charter/gitstate.py` that `save` needs — and the one place charter asks whether git has
//! stopped part-way through something ([`stopped`]).
//!
//! A `git add` that failed stages nothing, so the `git diff --cached --quiet` after it says
//! there is no difference. The value that means *charter could not tell* is byte for byte the
//! value that means *there is nothing there* — and charter printed the second (charter #917:
//! `charter save` said "Nothing to save — the control-plane working tree is clean" over
//! thirteen modified files, because a zero-byte `.git/index.lock` twenty-three hours old made
//! every `add` exit 128).
//!
//! **charter reports a lock. It never clears one.** Nothing inside one command can tell a held
//! lock from an abandoned one: the holder may be a `git commit` with an editor open, or another
//! machine's view of a network mount. So charter states the three facts the person deciding
//! needs — the path, the size and the age — and leaves the `rm` to them, `ps` first.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::counts::counted;
use crate::worktree::git::Run;

/// The name git gives the file. `$GIT_DIR/index.lock` in every git there has ever been.
pub const LOCK_NAME: &str = "index.lock";

/// How long a zero-byte lock must have sat there before charter will call it a crash rather
/// than contention. Fifteen minutes; which side it errs on matters more than the number.
pub const STALE_AFTER: f64 = 15.0 * 60.0;

/// A coarse age — `4s`, `12m`, `23h`, `3d`. Context for a human deciding whether to run `rm`,
/// never an input to a decision charter makes.
pub fn age_phrase(seconds: f64) -> String {
    // `as` from a float saturates: a negative age (a clock that moved back) and NaN are both
    // 0. So no guard for them here: one said `< 0.0` and could not be told from `<= 0.0` or
    // `== 0.0`, since the cast already answered every value it caught.
    let secs = seconds as u64;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

/// A `$GIT_DIR/index.lock` charter found, and the three facts about it.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexLock {
    /// The lock itself, resolved, so the sentence charter prints can be pasted into an `rm`.
    pub path: PathBuf,
    /// Bytes. Zero means git created the file and never wrote the index into it.
    pub size: u64,
    /// Seconds since its mtime.
    pub age: f64,
}

impl IndexLock {
    /// Zero bytes and older than [`STALE_AFTER`] — a crash, not contention. The conjunction
    /// is the claim: a big lock is a git that got as far as writing an index, and a young one
    /// may still be running.
    pub fn crashed(&self) -> bool {
        self.size == 0 && self.age >= STALE_AFTER
    }

    /// One line naming the lock, its size and its age — the sentence #917 needed.
    pub fn describe(&self) -> String {
        let what = if self.crashed() {
            "a git process crashed here and left it behind"
        } else {
            "something may still be holding it"
        };
        format!(
            "{} — {}, {} old; {what}.",
            self.path.display(),
            counted(self.size, "byte", "bytes"),
            age_phrase(self.age)
        )
    }

    /// What the operator does next, in the order they should do it. The `rm` is second and
    /// labelled, because checking for a holder first is what makes removing the file safe.
    pub fn remedy(&self) -> [String; 2] {
        [
            "who holds it:  ps -eo pid,lstart,command | grep '[g]it'".to_string(),
            format!(
                "nobody does:   rm -f {}   (purlis never removes a lock — a held one is real)",
                self.path.display()
            ),
        ]
    }
}

/// The index lock inside `git_dir`, or `None` when there is not one — and `None` too when the
/// path cannot be stat-ed at all, because turning an unreadable git directory into a
/// lock-shaped answer would be the same conflation one layer down.
pub fn find(git_dir: &Path) -> Option<IndexLock> {
    let path = git_dir.join(LOCK_NAME);
    let meta = std::fs::metadata(&path).ok()?;
    let age = meta
        .modified()
        .ok()
        .and_then(|mtime| {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
            let then = mtime.duration_since(UNIX_EPOCH).ok()?;
            Some(now.as_secs_f64() - then.as_secs_f64())
        })
        .unwrap_or(0.0);
    Some(IndexLock {
        // Resolved, because the whole point of printing it is that it can be pasted: macOS
        // hands out temp and home paths through symlinks (`/var` → `/private/var`).
        path: path.canonicalize().unwrap_or(path),
        size: meta.len(),
        age,
    })
}

/// An operation git stopped in the middle of, by the marker it leaves in the git directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Merge,
    Rebase,
    /// `git am`, which stops in the same `rebase-apply` directory a rebase does.
    Am,
    CherryPick,
    Revert,
    Bisect,
}

impl Operation {
    /// The markers, in the order they are asked for. One list for every caller (#433): the
    /// save, the pull and `charter sync` each kept their own, and they had drifted apart.
    const MARKERS: [(&'static str, Self); 7] = [
        ("rebase-merge", Self::Rebase),
        ("rebase-apply", Self::Rebase),
        ("REBASE_HEAD", Self::Rebase),
        ("MERGE_HEAD", Self::Merge),
        ("CHERRY_PICK_HEAD", Self::CherryPick),
        ("REVERT_HEAD", Self::Revert),
        ("BISECT_LOG", Self::Bisect),
    ];

    /// The operation stopped in `git_dir`, or `None`. A marker counts whatever it is — a
    /// file, a directory, a dangling link — because git's own test is that the name exists.
    fn in_progress(git_dir: &Path) -> Option<Self> {
        Self::MARKERS
            .into_iter()
            .find(|(marker, _)| std::fs::symlink_metadata(git_dir.join(marker)).is_ok())
            .map(|(marker, op)| {
                // `git am` leaves `rebase-apply/applying`; a rebase never does.
                if marker == "rebase-apply" && git_dir.join(marker).join("applying").exists() {
                    Self::Am
                } else {
                    op
                }
            })
    }

    /// Its kind as one plain word, for a reader that decides by it: `merge`, `rebase`, `am`,
    /// `cherry-pick`, `revert` or `bisect`.
    pub fn kind(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Rebase => "rebase",
            Self::Am => "am",
            Self::CherryPick => "cherry-pick",
            Self::Revert => "revert",
            Self::Bisect => "bisect",
        }
    }

    /// The word for it: "a merge is stopped part-way".
    pub fn word(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Rebase => "rebase",
            Self::Am => "`git am`",
            Self::CherryPick => "cherry-pick",
            Self::Revert => "revert",
            Self::Bisect => "bisect",
        }
    }

    /// How to finish it, or back out of it, in git's own commands.
    fn way_out(self) -> &'static str {
        match self {
            Self::Merge => "`git commit` to finish the merge, or `git merge --abort`",
            Self::Rebase => "`git rebase --continue`, or `git rebase --abort`",
            Self::Am => "`git am --continue`, or `git am --abort`",
            Self::CherryPick => "`git cherry-pick --continue`, or `git cherry-pick --abort`",
            Self::Revert => "`git revert --continue`, or `git revert --abort`",
            Self::Bisect => "`git bisect reset` once you are done bisecting",
        }
    }
}

/// A tree git has stopped part-way through something in: an [`Operation`] left unfinished,
/// files it still calls unmerged, or both. **A save refuses while there is one** (#433):
/// `git add -A` would stage the conflict markers as if they were the resolution, and the
/// commit would land them — or land a half-replayed rebase on a detached HEAD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stopped {
    /// `None` when only unmerged files are left — a `git stash pop` that conflicted.
    pub operation: Option<Operation>,
    /// The files git calls unmerged, sorted.
    pub unmerged: Vec<String>,
}

impl Stopped {
    /// What is wrong and how to get out of it, as one clause — the Blocked line, the
    /// journal's detail and the refusal all say this.
    pub fn why(&self) -> String {
        let files = (!self.unmerged.is_empty())
            .then(|| format!(" with conflicts in {}", self.unmerged.join(", ")));
        let settle = if self.unmerged.is_empty() {
            ""
        } else {
            "settle the conflicts and `git add` the files, then "
        };
        match self.operation {
            Some(op) => format!(
                "a {} is stopped part-way{}; {settle}{}",
                op.word(),
                files.unwrap_or_default(),
                op.way_out()
            ),
            None => format!(
                "the tree has conflicts to settle first in {}; settle them and `git add` the \
                 files",
                self.unmerged.join(", ")
            ),
        }
    }
}

/// Whether git has stopped part-way through something in the work tree at `tree` — the one
/// check every save, the pull and `charter sync` ask (#433). `None` when it has not, and when
/// git cannot say: a caller that cannot read the tree refuses on that, in its own words.
pub fn stopped(tree: &Path) -> Option<Stopped> {
    // Asked of git, not joined onto `.git`: a linked worktree keeps its merge and rebase
    // markers in `<main>/.git/worktrees/<name>`.
    let git_dir = crate::worktree::git::run(
        tree,
        &["rev-parse", "--absolute-git-dir"],
        crate::worktree::git::READ,
    )
    .ok()
    .filter(Run::ok)
    .map_or_else(|| tree.join(".git"), |r| PathBuf::from(r.line().trim()));
    let operation = Operation::in_progress(&git_dir);
    let unmerged = unmerged(tree);
    (operation.is_some() || !unmerged.is_empty()).then_some(Stopped {
        operation,
        unmerged,
    })
}

/// The files git calls unmerged in `tree`, sorted.
pub fn unmerged(tree: &Path) -> Vec<String> {
    crate::worktree::git::run(
        tree,
        // `diff-files`, the plumbing, never porcelain `diff`: the title bar asks this every ten
        // seconds, and `diff` refreshes the index — taking `index.lock` from under a save's
        // `git add -A` — whenever a tracked file's stat data no longer matches it, which
        // `--no-optional-locks` does not stop (measured on git 2.50: it holds back `status`'s
        // refresh and not `diff`'s). `diff-files` compares against the index as it is and
        // writes nothing, and lists an unmerged path the same way.
        &[
            "--no-optional-locks",
            "diff-files",
            "--name-only",
            "--diff-filter=U",
            "-z",
        ],
        crate::worktree::git::READ,
    )
    .map(|r| {
        let mut out: Vec<String> = r
            .out
            .split('\0')
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string)
            .collect();
        out.sort();
        out.dedup();
        out
    })
    .unwrap_or_default()
}

/// The first line git actually wrote, or `""`.
///
/// Kept and printed rather than replaced with charter's own guess at what went wrong: "a
/// command that failed for a stated reason arrives as a command that failed" is how a full
/// disk read as a broken sweep for a day. git's stderr under a held lock is four lines of
/// which the first names the file.
pub fn said(run: &Run) -> String {
    run.err
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests;
