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
//! branch is used in its place ([`Fresh::instead`]); where it is ahead of its remote, it is
//! used as the clone has it. Where it and its remote have each moved on, the remote-tracking
//! branch is used too (#3836): the commits only the clone's branch has are most often a
//! checkout's copies of work the remote has since landed another way (a squash), and a piece
//! cut from them would carry them into its pull request. The note says which, and what brings
//! the clone up.
//!
//! A base may also name a remote-tracking branch, `origin/main` (#3836): the piece is cut from
//! or moved onto it, and recorded as cut from the clone's branch of the same name, which
//! follows it. A sandboxed chat, which cannot make a branch or move one, has that way to the
//! remote whatever the clone's own branch holds.
//!
//! Every git call goes through the hardened runner, under whatever isolation the caller holds.

use std::path::Path;

use super::{Refusal, git, name, porcelain};

/// What bringing a base up did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fresh {
    /// The branch of the clone the base names, recorded as the piece's base: the name asked,
    /// or for a remote-tracking branch (`origin/main`) the clone's branch of the same name
    /// (#3836).
    pub branch: String,
    /// The remote-tracking branch the base was moved up to, as git names it short
    /// (`origin/main`), where it was moved.
    pub moved_to: Option<String>,
    /// The remote-tracking branch to start from in its place (`refs/remotes/origin/main`),
    /// where the base is behind it and could not be moved (checked out in a worktree, whose
    /// files would be left behind its commit), where the two have each moved on, or where the
    /// base named the remote-tracking branch itself (#3836).
    pub instead: Option<String>,
    /// What the person should know: the base is behind its remote and was left where it is, or
    /// it and its remote have each moved on.
    pub notes: Vec<String>,
}

/// What a base names in the clone (#3836): a branch of its own, or a remote-tracking branch
/// and the clone's branch of the same name that is recorded in its place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    /// The branch of the clone, recorded as the piece's base.
    pub branch: String,
    /// The remote and the remote-tracking branch (`refs/remotes/origin/main`) the base named,
    /// where it named one rather than a branch of the clone.
    pub tracking: Option<(String, String)>,
}

/// What `base` names in the clone at `clone`: its own branch of that name where it has one,
/// else a remote-tracking branch (`origin/main`) whose remote it has, recorded as the clone's
/// branch of the same name. Refused where it names neither.
pub fn named(clone: &Path, base: &str) -> Result<Named, Refusal> {
    name::branch_name_ok(base)?;
    if commit_of(clone, &name::as_ref(base))?.is_some() {
        return Ok(Named {
            branch: base.to_owned(),
            tracking: None,
        });
    }
    let listed = git::run(clone, &["remote"], git::READ)?;
    let remotes: Vec<&str> = if listed.ok() {
        listed.out.lines().map(str::trim).collect()
    } else {
        Vec::new()
    };
    let not_there = |why: String| Refusal::Unreadable {
        what: format!("the branch '{base}'"),
        why,
    };
    let Some((remote, branch)) = split_remote(base, &remotes) else {
        return Err(not_there(
            "the clone does not have it, so nothing was cut from it or moved onto it. Name a \
             branch of the clone (`git branch` lists them) or a remote-tracking branch \
             (`origin/main`)"
                .to_owned(),
        ));
    };
    let tracking = format!("refs/remotes/{base}");
    if commit_of(clone, &tracking)?.is_none() {
        return Err(not_there(format!(
            "the clone has no remote-tracking branch {base}, so nothing was cut from it or \
             moved onto it. `purlis sync` fetches {remote}"
        )));
    }
    name::branch_name_ok(branch)?;
    if commit_of(clone, &name::as_ref(branch))?.is_none() {
        return Err(not_there(format!(
            "the clone has no branch {branch} to record as the piece's base in its place, so \
             nothing was cut from it or moved onto it. Name a branch of the clone (`git \
             branch` lists them)"
        )));
    }
    Ok(Named {
        branch: branch.to_owned(),
        tracking: Some((remote.to_owned(), tracking)),
    })
}

/// `base` read as `<remote>/<branch>` for one of `remotes`, the longest that fits, where one
/// does and leaves a branch after it.
pub fn split_remote<'a>(base: &'a str, remotes: &[&str]) -> Option<(&'a str, &'a str)> {
    remotes
        .iter()
        .filter(|remote| !remote.is_empty())
        .filter_map(|remote| {
            let branch = base.strip_prefix(remote)?.strip_prefix('/')?;
            (!branch.is_empty()).then(|| (&base[..remote.len()], branch))
        })
        .max_by_key(|(remote, _)| remote.len())
}

/// Brings the base `base` names in the clone at `clone` up to its remote-tracking branch
/// ([`named`], [`bring_up_named`]). Refused where the clone has no such branch.
pub fn bring_up(clone: &Path, base: &str) -> Result<Fresh, Refusal> {
    bring_up_named(clone, &named(clone, base)?)
}

/// [`bring_up`], for a base already read. A base that named a remote-tracking branch is
/// started from it, the clone's branch brought up to it first where that is a fast-forward
/// nothing has checked out (#3836).
pub fn bring_up_named(clone: &Path, base: &Named) -> Result<Fresh, Refusal> {
    let Some((_, tracking)) = &base.tracking else {
        return bring_up_branch(clone, &base.branch);
    };
    let local_ref = name::as_ref(&base.branch);
    let local = commit_of(clone, &local_ref)?;
    let theirs = commit_of(clone, tracking)?;
    if upstream_of(clone, &base.branch, &local_ref)?.as_deref() == Some(tracking.as_str()) {
        let fresh = bring_up_branch(clone, &base.branch)?;
        if fresh.moved_to.is_some() || fresh.instead.is_some() || local == theirs {
            return Ok(fresh);
        }
    }
    // Ahead of it, or following another: the remote-tracking branch, as asked.
    Ok(Fresh {
        branch: base.branch.clone(),
        moved_to: None,
        instead: Some(tracking.clone()),
        notes: Vec::new(),
    })
}

/// Brings branch `branch` of the clone at `clone` up to its remote-tracking branch, where that
/// is a fast-forward and no worktree has it checked out.
fn bring_up_branch(clone: &Path, branch: &str) -> Result<Fresh, Refusal> {
    let unmoved = || Fresh {
        branch: branch.to_owned(),
        ..Fresh::default()
    };
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
        return Ok(unmoved());
    };
    let Some(theirs) = commit_of(clone, &upstream)? else {
        return Ok(unmoved());
    };
    let short = upstream
        .strip_prefix("refs/remotes/")
        .unwrap_or(&upstream)
        .to_owned();
    if theirs == local {
        return Ok(unmoved());
    }
    let behind = git::run(
        clone,
        &["merge-base", "--is-ancestor", &local, &theirs],
        git::READ,
    )?;
    match behind.code {
        Some(0) => {}
        // Ahead of it: the clone's own commits are the base's. Apart from it: the remote's
        // (#3836).
        Some(1) => {
            let ahead = git::run(
                clone,
                &["merge-base", "--is-ancestor", &theirs, &local],
                git::READ,
            )?;
            if ahead.code != Some(0) {
                return Ok(Fresh {
                    branch: branch.to_owned(),
                    moved_to: None,
                    instead: Some(upstream.clone()),
                    notes: vec![format!(
                        "{branch} and {short} have each moved on, so purlis used {short}: the \
                         commits only the clone's {branch} has are left out. `--base HEAD` \
                         cuts from the clone as it is"
                    )],
                });
            }
            return Ok(unmoved());
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
            branch: branch.to_owned(),
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
            branch: branch.to_owned(),
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
        branch: branch.to_owned(),
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
fn upstream_of(clone: &Path, branch: &str, local_ref: &str) -> Result<Option<String>, Refusal> {
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

#[cfg(test)]
mod tests {
    use super::split_remote;

    #[test]
    fn a_base_is_read_as_a_remote_and_its_branch_by_the_longest_remote_that_fits() {
        // #3836: `origin/main` names origin's main; a branch with a slash keeps it.
        assert_eq!(
            split_remote("origin/main", &["origin"]),
            Some(("origin", "main"))
        );
        assert_eq!(
            split_remote("origin/release/1.2", &["origin"]),
            Some(("origin", "release/1.2"))
        );
        assert_eq!(
            split_remote("team/a/main", &["team", "team/a"]),
            Some(("team/a", "main"))
        );
        // A name no remote starts, a remote's name alone, or a prefix that is not a whole
        // remote name: none.
        assert_eq!(split_remote("main", &["origin"]), None);
        assert_eq!(split_remote("origin", &["origin"]), None);
        assert_eq!(split_remote("origin/", &["origin"]), None);
        assert_eq!(split_remote("originx/main", &["origin"]), None);
        assert_eq!(split_remote("origin/main", &[""]), None);
    }
}
