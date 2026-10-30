// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A piece's base, brought up to its remote** (qw387): what `purlis worktree add --base` cuts
//! from and `purlis worktree update` rebases onto.
//!
//! The base is a branch of the clone, by name, and stays the branch the piece is recorded as
//! cut from: what a merge lands it into and what its Changes are counted against. A clone's
//! local branch is often behind its remote, since nothing but a checkout of it is ever pulled.
//! So before a piece is cut from it or moved onto it, the branch is brought up to the
//! remote-tracking branch it follows (its upstream, or `origin`'s of the same name), **as
//! `git fetch <remote> <branch>:<branch>` would**: only a fast-forward, and only where no
//! worktree has the branch checked out, since moving a checked-out branch would leave that
//! tree's files behind its commit. Where it is checked out and behind, the remote-tracking
//! branch is used in its place ([`Fresh::instead`]); where it has moved on from its remote, it
//! is used as the clone has it. The note says which, and what brings the clone up.
//!
//! Every git call goes through the hardened runner, under whatever isolation the caller holds.

use std::path::Path;

use super::{Refusal, git, name, porcelain};

/// What bringing a base up did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fresh {
    /// The remote-tracking branch the base was moved up to, as git names it short
    /// (`origin/main`), where it was moved.
    pub moved_to: Option<String>,
    /// The remote-tracking branch to start from in its place (`refs/remotes/origin/main`),
    /// where the base is behind it and could not be moved: checked out in a worktree, whose
    /// files would be left behind its commit.
    pub instead: Option<String>,
    /// What the person should know: the base is behind its remote and was left where it is.
    pub notes: Vec<String>,
}

/// Brings branch `branch` of the clone at `clone` up to its remote-tracking branch, where that
/// is a fast-forward and no worktree has it checked out. Refused where the clone has no such
/// branch.
pub fn bring_up(clone: &Path, branch: &str) -> Result<Fresh, Refusal> {
    name::branch_name_ok(branch)?;
    let local_ref = name::as_ref(branch);
    let Some(local) = commit_of(clone, &local_ref)? else {
        return Err(Refusal::Unreadable {
            what: format!("the branch '{branch}'"),
            why: "the clone does not have it, so nothing was cut from it or moved onto it. Name \
                  a branch of the clone (`git branch` lists them)"
                .to_owned(),
        });
    };
    let Some(upstream) = upstream_of(clone, branch, &local_ref)? else {
        return Ok(Fresh::default());
    };
    let Some(theirs) = commit_of(clone, &upstream)? else {
        return Ok(Fresh::default());
    };
    let short = upstream
        .strip_prefix("refs/remotes/")
        .unwrap_or(&upstream)
        .to_owned();
    if theirs == local {
        return Ok(Fresh::default());
    }
    let behind = git::run(
        clone,
        &["merge-base", "--is-ancestor", &local, &theirs],
        git::READ,
    )?;
    match behind.code {
        Some(0) => {}
        // Ahead of it, or apart from it: the clone's own commits are the base's.
        Some(1) => {
            let ahead = git::run(
                clone,
                &["merge-base", "--is-ancestor", &theirs, &local],
                git::READ,
            )?;
            if ahead.code != Some(0) {
                return Ok(Fresh {
                    moved_to: None,
                    instead: None,
                    notes: vec![format!(
                        "{branch} and {short} have each moved on, so {branch} is used as the \
                         clone has it"
                    )],
                });
            }
            return Ok(Fresh::default());
        }
        _ => {
            return Err(Refusal::Unreadable {
                what: format!("the branch '{branch}'"),
                why: format!(
                    "git could not compare it with {short}: {}",
                    behind.err.trim()
                ),
            });
        }
    }
    if let Some(tree) = checked_out_in(clone, branch)? {
        return Ok(Fresh {
            moved_to: None,
            instead: Some(upstream.clone()),
            notes: vec![format!(
                "{branch} is checked out in {} and behind {short}, so purlis left it where it \
                 is and used {short}. Until the clone is brought up (`purlis sync`), the \
                 piece's Changes count {short}'s newer commits too",
                tree.display()
            )],
        });
    }
    // Only from the commit it was read at: a branch moved since is not moved again.
    let moved = git::run(
        clone,
        &[
            "update-ref",
            "-m",
            &format!("purlis: brought up to {short}"),
            &local_ref,
            &theirs,
            &local,
        ],
        git::READ,
    )?;
    if !moved.ok() {
        return Ok(Fresh {
            moved_to: None,
            instead: None,
            notes: vec![format!(
                "{branch} is behind {short} and git did not move it ({}), so it is used as the \
                 clone has it",
                moved.err.trim()
            )],
        });
    }
    Ok(Fresh {
        moved_to: Some(short),
        instead: None,
        notes: Vec::new(),
    })
}

/// The commit `name` names in the clone, or `None` where it names none.
fn commit_of(clone: &Path, name: &str) -> Result<Option<String>, Refusal> {
    let spec = format!("{name}^{{commit}}");
    let found = git::run(
        clone,
        &["rev-parse", "--verify", "--quiet", &spec],
        git::READ,
    )?;
    Ok((found.ok() && !found.line().is_empty()).then(|| found.line().to_owned()))
}

/// The remote-tracking branch `branch` follows: its upstream where git has one, else
/// `origin`'s branch of the same name where the clone has that. Never a local branch.
pub(super) fn upstream_of(
    clone: &Path,
    branch: &str,
    local_ref: &str,
) -> Result<Option<String>, Refusal> {
    let asked = git::run(
        clone,
        &["for-each-ref", "--format=%(upstream)", local_ref],
        git::READ,
    )?;
    let named = asked.line().trim();
    if asked.ok() && named.starts_with("refs/remotes/") {
        return Ok(Some(named.to_owned()));
    }
    let origins = format!("refs/remotes/origin/{branch}");
    let there = git::run(
        clone,
        &["show-ref", "--verify", "--quiet", &origins],
        git::READ,
    )?;
    Ok(there.ok().then_some(origins))
}

/// The worktree of the clone that has `branch` checked out, if one does.
pub fn checked_out_in(clone: &Path, branch: &str) -> Result<Option<std::path::PathBuf>, Refusal> {
    let listed = git::run(clone, &["worktree", "list", "--porcelain"], git::READ)?;
    if !listed.ok() {
        return Err(Refusal::Unreadable {
            what: clone.display().to_string(),
            why: format!("git could not list its worktrees: {}", listed.err.trim()),
        });
    }
    Ok(porcelain::parse(&listed.out)
        .into_iter()
        .find(|row| row.prunable.is_none() && row.branch.as_deref() == Some(branch))
        .map(|row| row.path))
}

/// The branch the remote `remote` of the clone at `clone` names its default, asked of the
/// remote itself (`git ls-remote --symref <remote> HEAD`), reached as a fetch is
/// ([`crate::repocmd::sync::reach`], `hosts` the chat's internet access where it has one).
/// Never read from the clone, whose refs a chat may write. `None` where the remote does not
/// answer or names none.
pub fn remote_default(
    root: &Path,
    clone: &Path,
    remote: &str,
    hosts: Option<&[String]>,
) -> Option<String> {
    let (_, helper) = crate::repocmd::sync::reach(root, clone, remote, hosts, false).ok()?;
    let asked = git::run_network(
        clone,
        helper.as_deref(),
        &["ls-remote", "--symref", remote, "HEAD"],
    )
    .ok()
    .filter(git::Run::ok)?;
    let branch = asked.out.lines().find_map(|line| {
        let (named, at) = line.strip_prefix("ref: ")?.split_once('\t')?;
        (at.trim() == "HEAD")
            .then(|| named.strip_prefix("refs/heads/"))
            .flatten()
            .map(str::to_owned)
    })?;
    name::branch_name_ok(&branch).ok()?;
    Some(branch)
}
