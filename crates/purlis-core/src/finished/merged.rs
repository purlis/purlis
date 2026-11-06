// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A tip inside a merged request's head** (#5439): a branch whose commits are neither on its
//! base nor carried there, because its request got more commits after this tip before it
//! merged, still lost nothing. Its commits are part of what merged.
//!
//! Asked of the forge, never assumed: the forge says the newest request whose head is the
//! branch merged, and the tip is an ancestor of that request's head
//! (`git merge-base --is-ancestor <tip> <head>`). Where the clone does not have the head, it is
//! fetched by the request's own ref (`refs/pull/<n>/head` on GitHub,
//! `refs/merge-requests/<n>/head` on GitLab). Offline, or with no forge, nothing can be told
//! and the folder is not finished.

use std::path::Path;

use crate::forge::Caller;
use crate::forge::pr::{Repo, State};
use crate::worktree::{self, git, name};

/// What the forge says of the newest request whose head is a branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// It merged: its number, the commit its head was at, and the ref the forge keeps that
    /// head under.
    Merged {
        number: u64,
        head: String,
        head_ref: String,
    },
    /// It is open or closed without merging, or there is none.
    NotMerged,
    /// The forge could not be asked, and why: offline, or no forge for the clone.
    Unasked(String),
}

/// How the forge is asked of a repo's branch: [`Forge::of_the_plane`]'s answer for a real
/// plane, or a test's own. Each folder of a prune is read on a thread of its own, so it is
/// shared between them.
#[derive(Clone, Copy)]
pub struct Forge<'a>(&'a (dyn Fn(&str, &str) -> Request + Sync));

impl std::fmt::Debug for Forge<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Forge")
    }
}

impl<'a> Forge<'a> {
    /// The forge `ask` answers, given a repo of the workspace and a branch of it.
    pub fn asking(ask: &'a (dyn Fn(&str, &str) -> Request + Sync)) -> Self {
        Self(ask)
    }

    /// No forge: every ask is [`Request::Unasked`], so no branch is read inside a merged
    /// request. For a reading that has no forge to ask, a test's above all.
    pub fn none() -> Forge<'static> {
        static NONE: fn(&str, &str) -> Request =
            |_, _| Request::Unasked("no forge was asked".to_owned());
        Forge(&NONE)
    }

    fn ask(&self, repo: &str, branch: &str) -> Request {
        (self.0)(repo, branch)
    }
}

/// The forge of each clone of `ws`, as its `origin` names it and the plane declares it,
/// asked as `caller`.
pub fn of_the_plane<'a>(
    plane: &'a Path,
    ws: &'a str,
    caller: &'a Caller,
) -> impl Fn(&str, &str) -> Request + Sync + 'a {
    move |repo, branch| {
        let clone = match worktree::clone_dir(plane, ws, repo) {
            Ok(clone) => clone,
            Err(why) => return Request::Unasked(why.to_string()),
        };
        let found = match Repo::of_clone(plane, &clone) {
            Ok(found) => found,
            Err(why) => return Request::Unasked(why),
        };
        match found.forge.backend().by_head(caller, &found.path, branch) {
            Ok(Some(request)) => match request.state {
                State::Merged { .. } => Request::Merged {
                    number: request.number,
                    head_ref: match found.forge.kind {
                        crate::forge::Kind::GitHub => format!("refs/pull/{}/head", request.number),
                        crate::forge::Kind::GitLab => {
                            format!("refs/merge-requests/{}/head", request.number)
                        }
                    },
                    head: request.head,
                },
                State::Open | State::Closed => Request::NotMerged,
            },
            Ok(None) => Request::NotMerged,
            Err(why) => Request::Unasked(why.to_string()),
        }
    }
}

/// **The merged request whose head holds `branch` of `repo`**, by its number: `Ok(None)` where
/// the forge names none that merged or its head does not hold the branch's tip, `Err` where
/// that could not be told, saying why.
pub fn holding(
    plane: &Path,
    ws: &str,
    repo: &str,
    branch: &str,
    forge: Forge<'_>,
) -> Result<Option<u64>, String> {
    if name::branch_name_ok(branch).is_err() {
        return Ok(None);
    }
    let (number, head, head_ref) = match forge.ask(repo, branch) {
        Request::Merged {
            number,
            head,
            head_ref,
        } => (number, head, head_ref),
        Request::NotMerged => return Ok(None),
        Request::Unasked(why) => return Err(why),
    };
    // A head is a whole commit id, SHA-1 or SHA-256, and nothing else is passed to git as one:
    // a shorter one would be looked up as a prefix.
    if !matches!(head.len(), 40 | 64) || !head.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "the forge named '{}' as the head",
            crate::shown::short(&head)
        ));
    }
    let clone = worktree::clone_dir(plane, ws, repo).map_err(|why| why.to_string())?;
    let has = |commit: &str| {
        git::run(
            &clone,
            &["cat-file", "-e", &format!("{commit}^{{commit}}")],
            git::READ,
        )
        .is_ok_and(|run| run.ok())
    };
    if !has(&head) {
        crate::repocmd::sync::fetch_ref(plane, &clone, "origin", &head_ref, None)?;
        if !has(&head) {
            return Err(format!(
                "the head of request {number} ({}) is not in the clone after fetching {head_ref}",
                crate::shown::short(&head)
            ));
        }
    }
    let within = git::run(
        &clone,
        &["merge-base", "--is-ancestor", &name::as_ref(branch), &head],
        git::READ,
    )
    .map_err(|why| why.to_string())?;
    match within.code {
        Some(0) => Ok(Some(number)),
        Some(1) => Ok(None),
        _ => Err(format!(
            "git did not say whether {branch} is inside request {number}: {}",
            within.err.lines().next().unwrap_or("").trim()
        )),
    }
}
