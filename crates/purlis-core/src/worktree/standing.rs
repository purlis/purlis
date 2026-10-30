// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a piece holds that removing it would lose, and whether its branch has landed in the
//! branch it was cut from: the two questions asked before a piece is taken away on someone's
//! behalf (#1453).
//!
//! [`super::remove`] asks the first one itself and answers it as a refusal. A window that asks
//! the person **before** it discards a piece's folder needs the same answer without the
//! removal, so it can put exactly what would go in front of them: [`at_risk`] is that read. It
//! also names what `remove` never counts and still deletes: the files git ignores, and the
//! git state git keeps for that folder alone ([`OwnGit`], #1472).
//!
//! [`landed`] is the question no other verb asks: whether every commit of a piece's branch is
//! already in the branch it was cut from, or every file it changed reads there as it has it (a
//! squash merge). It is what lets purlis take a finished piece's folder away without being
//! told to, and it is deliberately narrower than "git would let me delete it": a branch that
//! was only pushed somewhere is not merged, and is kept.

use std::path::Path;

use super::{
    Base, DETACHED_PREFIX, Refusal, base_key, clone_dir, commits_alone, git, head_of, name,
    path_for, relocation_refusal, unique_commits, within_workspace,
};

/// What removing a piece would lose.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AtRisk {
    /// The branch the piece is on; `None` for a detached HEAD.
    pub branch: Option<String>,
    /// The uncommitted paths, as `git status --porcelain` prints them (`?? new.txt`,
    /// ` M a.rs`). `None` where git would not say, which is never read as none.
    pub changes: Option<Vec<String>>,
    /// The paths git ignores there, a folder ignored whole as one entry (`target/`): build
    /// output, local settings, and what purlis itself keeps hidden in a chat's folder.
    /// **Removing the piece deletes them, and git's own guard does not count them.** `None`
    /// where git would not say.
    pub ignored: Option<Vec<String>>,
    /// How many commits no other branch and no remote reaches. `None` where git would not say.
    pub unmerged: Option<u32>,
    /// Those commits, as `<short sha> <subject>`, newest first, at most [`super::NAMED`] and
    /// one more.
    pub commits: Vec<String>,
    /// The folder's own git state, which goes with it and nothing else holds (#1472). `None`
    /// where git would not say, which is never read as none.
    pub own: Option<OwnGit>,
}

impl AtRisk {
    /// Whether git answered every question and found nothing: the piece can go and nothing
    /// goes with it.
    pub fn nothing(&self) -> bool {
        self.changes.as_ref().is_some_and(Vec::is_empty)
            && self.unmerged == Some(0)
            && self.own.as_ref().is_some_and(OwnGit::is_empty)
    }
}

/// **What git keeps for one linked worktree alone** (#1472), in the clone's
/// `.git/worktrees/<name>/`, which `git worktree remove` deletes with the folder. Neither the
/// branch the piece is on nor git's own guard counts any of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnGit {
    /// The refs only this folder has, by their whole name: `refs/worktree/*`, `refs/bisect/*`
    /// (a bisect in progress) and `refs/rewritten/*` (a rebase in progress). Sorted.
    pub refs: Vec<String>,
    /// Whether it has settings of its own (`config.worktree`).
    pub config: bool,
    /// The commits only its HEAD's reflog holds, as `<short sha> <subject>`, newest first and
    /// at most [`super::NAMED`] and one more: made in the folder and then amended, reset or
    /// rebased away. No branch, tag, remote or other worktree reaches them.
    pub reflog_only: Vec<String>,
}

impl OwnGit {
    /// Whether there is none.
    pub fn is_empty(&self) -> bool {
        self.refs.is_empty() && !self.config && self.reflog_only.is_empty()
    }

    /// Each piece of it, in words, one line each: what a question before a removal names.
    pub fn said(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .refs
            .iter()
            .map(|name| format!("{name}, a ref only this folder has"))
            .collect();
        if self.config {
            out.push("config.worktree, settings of this folder alone".to_owned());
        }
        out.extend(
            self.reflog_only.iter().map(|commit| {
                format!("{commit}, a commit only this folder's history of HEAD holds")
            }),
        );
        out
    }
}

/// The ref namespaces git keeps per worktree that hold something a person made.
const OWN_REFS: [&str; 3] = ["refs/worktree/", "refs/bisect/", "refs/rewritten/"];

/// The most entries of a folder's HEAD reflog read. A reflog longer than that is not read to
/// its end, and is said as not answered: never as nothing.
const REFLOG_READ: usize = 2000;

/// What git answers in `tree` to `args`, or `None` where it would not say.
fn answered(tree: &Path, args: &[&str]) -> Option<String> {
    git::run(tree, args, git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|run| run.out)
}

/// **The git state only the linked worktree at `tree` keeps**, without its HEAD's history: the
/// refs only it has and its own settings, or `None` where git would not say. What
/// [`super::remove`] and a workspace's removal refuse over (#1472). A commit that only the
/// folder's HEAD reflog reaches is not counted there: it was amended, reset or rebased away,
/// which a person does every day, and a removal that refused until it left the reflog would
/// teach `--force`.
pub(crate) fn own_state(tree: &Path) -> Option<OwnGit> {
    let answered = |args: &[&str]| answered(tree, args);
    let mut args = vec!["for-each-ref", "--format=%(refname)"];
    args.extend(OWN_REFS);
    let mut refs: Vec<String> = answered(&args)?
        .lines()
        .map(str::trim)
        .filter(|name| OWN_REFS.iter().any(|own| name.starts_with(own)))
        .map(str::to_owned)
        .collect();
    refs.sort();
    let admin = answered(&["rev-parse", "--absolute-git-dir"])?;
    let admin = Path::new(admin.trim());
    if !admin.is_absolute() {
        return None;
    }
    // Looked at, never read or followed: whatever is there is the folder's own.
    let config = match std::fs::symlink_metadata(admin.join("config.worktree")) {
        Ok(meta) => !(meta.is_file() && meta.len() == 0),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => return None,
    };
    Some(OwnGit {
        refs,
        config,
        reflog_only: Vec::new(),
    })
}

/// [`own_state`] and the commits only the folder's HEAD reflog reaches, or `None` where git
/// would not say: what [`at_risk`] names, for Tidy and Discard.
fn own_git(tree: &Path) -> Option<OwnGit> {
    let mut own = own_state(tree)?;
    let answered = |args: &[&str]| answered(tree, args);
    let most = format!("--max-count={REFLOG_READ}");
    // A HEAD with no reflog at all (`core.logAllRefUpdates` off) has nothing in it; one there
    // that git will not read is not answered.
    let kept = git::run(tree, &["reflog", "exists", "HEAD"], git::READ).ok()?;
    let entries = if kept.ok() {
        answered(&["log", "--walk-reflogs", "--format=%H", &most, "HEAD"])?
    } else {
        String::new()
    };
    let mut seen: Vec<&str> = Vec::new();
    for sha in entries.lines().map(str::trim).filter(|sha| !sha.is_empty()) {
        if !seen.contains(&sha) {
            seen.push(sha);
        }
    }
    if entries.lines().count() >= REFLOG_READ {
        return None;
    }
    let reflog_only = if seen.is_empty() {
        Vec::new()
    } else {
        let named = format!("--max-count={}", super::NAMED + 1);
        let mut args = vec!["log", "--format=%h %s", &named];
        args.extend(seen.iter().copied());
        // `--all` is every ref, this HEAD and every other worktree's.
        args.extend(["--not", "--all", "--"]);
        answered(&args)?.lines().map(str::to_owned).collect()
    };
    own.reflog_only = reflog_only;
    Some(own)
}

/// What removing `piece` would lose, read and nothing else. `Ok(None)` for a piece whose
/// folder is not there.
pub fn at_risk(plane: &Path, ws: &str, repo: &str, piece: &str) -> Result<Option<AtRisk>, Refusal> {
    relocation_refusal(plane)?;
    let path = path_for(plane, ws, repo, piece)?;
    let path = within_workspace(plane, ws, &path)?;
    clone_dir(plane, ws, repo)?;
    if path.symlink_metadata().is_err() {
        return Ok(None);
    }
    let branch = match head_of(&path) {
        Ok(Base::Branch(branch)) => Some(branch),
        _ => None,
    };
    let unmerged = unique_commits(&path, branch.as_deref())?;
    let (changes, ignored) = match uncommitted(&path) {
        Some((changes, ignored)) => (Some(changes), Some(ignored)),
        None => (None, None),
    };
    Ok(Some(AtRisk {
        changes,
        ignored,
        commits: match unmerged {
            Some(0) => Vec::new(),
            _ => commits_alone(&path, branch.as_deref()),
        },
        unmerged,
        branch,
        own: own_git(&path),
    }))
}

/// What `git status` reports in `tree`: the uncommitted paths as it prints them, **each
/// untracked file by its own path** (`--untracked-files=all`; a new folder is not one line),
/// and apart from them the ignored ones (its `!! ` lines, without the mark). An ignored path is
/// listed as the rule that ignores it matches it (`--ignored=matching`): a folder ignored whole
/// is one entry (`target/`), and a file ignored by name is itself. `None` when git could not
/// say.
fn uncommitted(tree: &Path) -> Option<(Vec<String>, Vec<String>)> {
    let seen = git::run(
        tree,
        &[
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--ignored=matching",
        ],
        git::READ,
    )
    .ok()?;
    if !seen.ok() {
        return None;
    }
    let (ignored, changes): (Vec<&str>, Vec<&str>) = seen
        .out
        .lines()
        .filter(|line| !line.trim().is_empty())
        .partition(|line| line.starts_with("!! "));
    Some((
        changes.into_iter().map(str::to_owned).collect(),
        ignored
            .into_iter()
            .map(|line| line["!! ".len()..].to_owned())
            .collect(),
    ))
}

/// Deletes `branch` in `repo`'s clone **where git finds it merged, and only then**
/// (`git branch -d`: merged into the clone's HEAD, or into its upstream). Whether it went.
///
/// For a branch purlis cut whose folder has just been removed. A branch holding a commit that
/// is nowhere else stays, as ADR 0072 §4 says of every branch: deleting one that holds work is
/// git's and the forge's, never purlis's.
pub fn drop_if_merged(plane: &Path, ws: &str, repo: &str, branch: &str) -> bool {
    if name::branch_name_ok(branch).is_err() {
        return false;
    }
    let Ok(clone) = clone_dir(plane, ws, repo) else {
        return false;
    };
    git::run(&clone, &["branch", "-d", "--", branch], git::READ).is_ok_and(|seen| seen.ok())
}

/// Whether a piece's branch has landed in the branch it was cut from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Landed {
    /// Every commit of `branch` is in `base`.
    Yes { branch: String, base: String },
    /// Not its commits, but every file `branch` changed reads in `base` as the branch has it:
    /// squashed, rebased or picked in (#1472). git does not find it merged, so the branch
    /// itself is kept wherever it is taken away: it is the one place those commits are.
    Carried { branch: String, base: String },
    /// It holds commits `base` does not.
    No,
    /// purlis cannot say: the folder is gone, the piece is on another branch than `branch` or
    /// on none, the branch it was cut from was not recorded (or was a detached commit), or git
    /// did not answer. Never read as landed.
    Unknown,
}

/// Whether `branch`, the branch `piece` was cut on, has landed in the branch it was cut from
/// (`branch.<branch>.charterBase`, which [`super::add`] records).
///
/// **Asked of the branch purlis cut, never of whatever the piece is on now**: a piece whose
/// chat switched it to another branch is [`Landed::Unknown`], and is left alone.
pub fn landed(plane: &Path, ws: &str, repo: &str, piece: &str, branch: &str) -> Landed {
    let Ok(()) = relocation_refusal(plane) else {
        return Landed::Unknown;
    };
    let Ok(path) = path_for(plane, ws, repo, piece)
        .and_then(|path| within_workspace(plane, ws, &path).map_err(Refusal::from))
    else {
        return Landed::Unknown;
    };
    let Ok(clone) = clone_dir(plane, ws, repo) else {
        return Landed::Unknown;
    };
    if path.symlink_metadata().is_err() || name::branch_name_ok(branch).is_err() {
        return Landed::Unknown;
    }
    match head_of(&path) {
        Ok(Base::Branch(on)) if on == branch => {}
        _ => return Landed::Unknown,
    }
    // `--get-all`, for `merge`'s reason: a key holding two values is somebody else choosing.
    let Ok(recorded) = git::run(
        &clone,
        &["config", "--get-all", &base_key(branch)],
        git::READ,
    ) else {
        return Landed::Unknown;
    };
    let values: Vec<&str> = recorded
        .out
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    let [base] = values.as_slice() else {
        return Landed::Unknown;
    };
    if base.starts_with(DETACHED_PREFIX) || name::branch_name_ok(base).is_err() {
        return Landed::Unknown;
    }
    let asked = git::run(
        &clone,
        &[
            "merge-base",
            "--is-ancestor",
            &name::as_ref(branch),
            &name::as_ref(base),
        ],
        git::READ,
    );
    match asked {
        // Exit 0 is "is an ancestor" and 1 is "is not"; anything else is git not answering.
        Ok(seen) if seen.ok() => Landed::Yes {
            branch: branch.to_owned(),
            base: (*base).to_owned(),
        },
        // Not by its commits: squashed, rebased or picked into the base, which git cannot
        // see by ancestry. Asked of the files instead (#1472).
        Ok(seen) if seen.code == Some(1) => {
            let carried = || Landed::Carried {
                branch: branch.to_owned(),
                base: (*base).to_owned(),
            };
            if carried_in(&clone, branch, base) == Some(true) {
                return carried();
            }
            // Squashed into the base's remote branch, which the clone has fetched while its
            // own base was left behind (#3879): a squash merge on the forge, its branch
            // deleted there and pruned here, is carried as much as one in the base itself.
            match super::fresh::upstream_of(&clone, base, &name::as_ref(base)) {
                Ok(Some(upstream))
                    if carried_in_refs(&clone, &name::as_ref(branch), &upstream) == Some(true) =>
                {
                    carried()
                }
                _ => Landed::No,
            }
        }
        _ => Landed::Unknown,
    }
}

/// **Whether `base` holds, file by file, everything `branch` changed**, though not its commits:
/// what a squash merge, a rebase or a picked commit leaves. `None` where git did not answer.
///
/// Every path the branch changed since it parted from the base must read the same in the base
/// as at the branch's tip ([`carried`]). Two reads of names, and no file is read through a
/// filter or an external diff. A base that changed one of those files again since is not
/// found carried, so the branch is kept: never a false "merged".
///
/// **This lets a folder go, never a commit** ([`Landed::Carried`]): the folder is taken away
/// and the branch stays, an ordinary branch of the repo, since git does not find it merged.
pub(crate) fn carried_in(clone: &Path, branch: &str, base: &str) -> Option<bool> {
    carried_in_refs(clone, &name::as_ref(branch), &name::as_ref(base))
}

/// [`carried_in`], for two full refs: the base may be a remote-tracking branch (#3879).
fn carried_in_refs(clone: &Path, branch: &str, base: &str) -> Option<bool> {
    let (branch, base) = (branch.to_owned(), base.to_owned());
    let parted = git::run(clone, &["merge-base", &branch, &base], git::READ)
        .ok()
        .filter(git::Run::ok)?;
    let parted = parted.line().to_owned();
    if parted.is_empty() {
        return None;
    }
    let names = |from: &str, to: &str| -> Option<Vec<String>> {
        let seen = git::run(
            clone,
            &[
                "diff",
                "--name-only",
                "-z",
                "--no-renames",
                "--no-ext-diff",
                "--no-textconv",
                // A repo's own settings may hide a change of a submodule's commit (#1472).
                "--ignore-submodules=none",
                from,
                to,
                "--",
            ],
            git::READ,
        )
        .ok()
        .filter(git::Run::ok)?;
        Some(
            seen.out
                .split('\0')
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .collect(),
        )
    };
    let changed = names(&parted, &branch)?;
    let differs = names(&branch, &base)?;
    Some(carried(&changed, &differs))
}

/// Whether a branch that changed the paths `changed` is carried by a base that differs from
/// its tip only in the paths `differs`: none of the paths it changed reads otherwise there.
fn carried(changed: &[String], differs: &[String]) -> bool {
    let differs: std::collections::HashSet<&str> = differs.iter().map(String::as_str).collect();
    !changed.iter().any(|path| differs.contains(path.as_str()))
}

#[cfg(test)]
mod tests {
    use super::carried;

    fn paths(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn a_base_that_reads_as_the_branch_in_every_file_it_changed_carries_it() {
        // Squashed: the base differs from the branch's tip only where others worked.
        assert!(carried(&paths(&["src/a.rs", "b.md"]), &paths(&["c.txt"])));
        // Nothing differs at all: the base is the branch's tree.
        assert!(carried(&paths(&["src/a.rs"]), &[]));
    }

    #[test]
    fn one_file_the_branch_changed_that_reads_otherwise_in_the_base_is_not_carried() {
        // Not merged, or merged and changed again since: kept either way.
        assert!(!carried(
            &paths(&["src/a.rs", "b.md"]),
            &paths(&["c.txt", "b.md"])
        ));
    }
}
