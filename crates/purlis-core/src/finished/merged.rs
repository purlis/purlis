// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A tip inside a merged request's head** (#5439): a branch whose commits are neither on its
//! base nor carried there, because its request got more commits after this tip before it
//! merged, still lost nothing. Its commits are part of what merged.
//!
//! A request that closed without merging holds them too where its head is on a branch of
//! origin (#4500): a request superseded by another branch's leaves its own on origin.
//!
//! Asked of the forge, never assumed: the forge says the newest request whose head is the
//! branch merged or closed, and the tip is an ancestor of that request's head
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
    /// It closed without merging: its number, the commit its head was at, and the ref the
    /// forge keeps that head under.
    Closed {
        number: u64,
        head: String,
        head_ref: String,
    },
    /// It is open, or there is none.
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
                    head_ref: head_ref(found.forge.kind, request.number),
                    head: request.head,
                },
                State::Closed => Request::Closed {
                    number: request.number,
                    head_ref: head_ref(found.forge.kind, request.number),
                    head: request.head,
                },
                State::Open => Request::NotMerged,
            },
            Ok(None) => Request::NotMerged,
            Err(why) => Request::Unasked(why.to_string()),
        }
    }
}

/// The ref a forge of `kind` keeps the head of request `number` under, whatever became of it.
fn head_ref(kind: crate::forge::Kind, number: u64) -> String {
    match kind {
        crate::forge::Kind::GitHub => format!("refs/pull/{number}/head"),
        crate::forge::Kind::GitLab => format!("refs/merge-requests/{number}/head"),
    }
}

/// What the forge's newest request from a branch says of the branch's tip (#5439, #4500).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    /// The tip is inside the head of request `n`, which merged: by a merge, a squash or the
    /// merge queue, its commits are part of what merged.
    Merged(u64),
    /// The tip is inside the head of request `n`, which closed without merging, and that head
    /// is on a branch origin has now: nothing is lost with the folder.
    ClosedOnOrigin(u64),
    /// The tip is inside the head of request `n`, which closed without merging, and that head
    /// is on no branch of origin: its commits may be nowhere else.
    ClosedNotOnOrigin(u64),
    /// No request the forge names holds the tip.
    Nothing,
}

/// **What the newest request from `branch` of `repo` says of its tip** ([`Found`]): `Err`
/// where that could not be told, saying why.
///
/// Where the clone does not have the request's head, it is fetched by the request's own ref.
/// A closed request's head is "on origin" where a branch of origin holds it **as origin answers
/// now**: the clone's remote-tracking branches name which to ask, and each is asked of origin
/// (`git ls-remote`), since one deleted there outlives it in the clone until a fetch prunes it.
pub fn holding(
    plane: &Path,
    ws: &str,
    repo: &str,
    branch: &str,
    forge: Forge<'_>,
) -> Result<Found, String> {
    if name::branch_name_ok(branch).is_err() {
        return Ok(Found::Nothing);
    }
    let (number, head, head_ref, merged) = match forge.ask(repo, branch) {
        Request::Merged {
            number,
            head,
            head_ref,
        } => (number, head, head_ref, true),
        Request::Closed {
            number,
            head,
            head_ref,
        } => (number, head, head_ref, false),
        Request::NotMerged => return Ok(Found::Nothing),
        Request::Unasked(why) => return Err(why),
    };
    // A head is a whole commit id, SHA-1 or SHA-256, and nothing else is passed to git as one:
    // a shorter one would be looked up as a prefix.
    if !whole_commit(&head) {
        return Err(format!(
            "the forge named '{}' as the head",
            crate::shown::short(&head)
        ));
    }
    let clone = worktree::clone_dir(plane, ws, repo).map_err(|why| why.to_string())?;
    if !has(&clone, &head) {
        crate::repocmd::sync::fetch_ref(plane, &clone, "origin", &head_ref, None)?;
        if !has(&clone, &head) {
            return Err(format!(
                "the head of request {number} ({}) is not in the clone after fetching {head_ref}",
                crate::shown::short(&head)
            ));
        }
    }
    let inside = |tip: &str| -> Result<bool, String> {
        let run = git::run(
            &clone,
            &["merge-base", "--is-ancestor", tip, &head],
            git::READ,
        )
        .map_err(|why| why.to_string())?;
        match run.code {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(format!(
                "git did not say whether {branch} is inside request {number}: {}",
                run.err.lines().next().unwrap_or("").trim()
            )),
        }
    };
    if !inside(&name::as_ref(branch))? {
        return Ok(Found::Nothing);
    }
    if merged {
        return Ok(Found::Merged(number));
    }
    Ok(if on_origin(plane, &clone, branch, &head)? {
        Found::ClosedOnOrigin(number)
    } else {
        Found::ClosedNotOnOrigin(number)
    })
}

/// The most remote-tracking branches holding a head that are asked of origin.
const MOST_ASKED: usize = 5;

/// Whether a branch origin has now holds `head`: the request's own `branch` first, then each
/// of the clone's remote-tracking branches of origin that holds it, asked of origin, until one
/// origin still has at a commit that holds it. A branch origin has moved on is fetched to be
/// read.
fn on_origin(plane: &Path, clone: &Path, branch: &str, head: &str) -> Result<bool, String> {
    let listed = git::run(
        clone,
        &[
            "for-each-ref",
            &format!("--count={}", MOST_ASKED + 1),
            "--format=%(refname)",
            "--contains",
            head,
            "refs/remotes/origin/",
        ],
        git::READ,
    )
    .map_err(|why| why.to_string())?;
    if !listed.ok() {
        return Err(format!(
            "git did not say which branches of origin hold {}: {}",
            crate::shown::short(head),
            listed.err.lines().next().unwrap_or("").trim()
        ));
    }
    let mut names: Vec<&str> = vec![branch];
    names.extend(
        listed
            .out
            .lines()
            .filter_map(|line| line.trim().strip_prefix("refs/remotes/origin/"))
            .filter(|name| *name != "HEAD" && *name != branch && name::branch_name_ok(name).is_ok())
            .take(MOST_ASKED),
    );
    for name in names {
        let Some(at) = crate::repocmd::sync::remote_branch(plane, clone, "origin", name)? else {
            continue;
        };
        if !whole_commit(&at) {
            continue;
        }
        if !has(clone, &at) {
            crate::repocmd::sync::fetch_ref(
                plane,
                clone,
                "origin",
                &format!("refs/heads/{name}"),
                None,
            )?;
        }
        let holds = git::run(
            clone,
            &["merge-base", "--is-ancestor", head, &at],
            git::READ,
        )
        .is_ok_and(|run| run.ok());
        if holds {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether `commit` is a whole commit id, SHA-1 or SHA-256.
fn whole_commit(commit: &str) -> bool {
    matches!(commit.len(), 40 | 64) && commit.chars().all(|c| c.is_ascii_hexdigit())
}

/// Whether the clone at `clone` has the commit `commit`.
fn has(clone: &Path, commit: &str) -> bool {
    git::run(
        clone,
        &["cat-file", "-e", &format!("{commit}^{{commit}}")],
        git::READ,
    )
    .is_ok_and(|run| run.ok())
}
