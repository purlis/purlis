// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A workspace's repo left where new work trips on it, and the one fix for each (#1985).
//!
//! Two ways a repo a workspace holds (its clone, `workspaces/<ws>/<repo>`, never a branch folder
//! under `.worktrees`) goes stale:
//!
//! - **On a chat's branch.** Its checked-out branch is not the remote's default branch, and is a
//!   branch purlis cut for a chat or a task: one with a base recorded under it
//!   (`branch.<branch>.charterBase`, [`crate::worktree::cut_branches`]) or one the piece log
//!   names. A chat that starts in the repo itself works on that branch. The fix switches the
//!   repo back to its default branch, and only where nothing is lost: the tree holds no change,
//!   no operation is stopped in it, and every commit of the branch is on another branch or on
//!   the remote. The branch itself is kept.
//! - **On a moved remote.** The forge answered a fetch purlis ran with a redirect: the repo now
//!   lives at another address on the same host (a rename, or a move to another owner). purlis
//!   keeps what the forge said in the repo's own config (`remote.origin.purlisMovedTo`), and
//!   the fix sets `origin` to that address and records it as the remote the window's saves push
//!   to.
//!
//! **Read from what purlis can read without running anything a chat wrote**: git through the
//! hardened runner ([`crate::worktree::git`]), which runs no hook, filter or fsmonitor, and the
//! plane's piece log. Nothing here asks the network.
//!
//! [`judged`] and [`step`] are the decisions, over a [`Seen`] that says what was read; [`read`]
//! and [`fix`] do the reading and the one git command a fix runs. The fix is the person's, from
//! the window (the command is on `WINDOW_ONLY`), and it reads the repo again before it acts.

use std::collections::BTreeSet;
use std::path::Path;

use crate::forge;
use crate::repos::{self, Head};
use crate::worktree::{self, git};

/// The repo's config key that keeps where the forge said `origin` moved to.
pub const MOVED_KEY: &str = "remote.origin.purlisMovedTo";

/// What purlis read about one repo of a workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Seen {
    pub workspace: String,
    pub repo: String,
    /// The branch it has checked out; `None` on a detached HEAD or where git would not say.
    pub branch: Option<String>,
    /// The remote's default branch as the repo knows it (`origin/HEAD`), with no fetch.
    pub default: Option<String>,
    /// The branches purlis cut for a chat or a task.
    pub chat_branches: BTreeSet<String>,
    /// `origin`'s address as the repo's config records it.
    pub origin: Option<String>,
    /// Where the forge said `origin` now lives ([`MOVED_KEY`]).
    pub moved_to: Option<String>,
    /// No change in the tree and no operation stopped in it; `None` where git would not say.
    pub clean: Option<bool>,
    /// The commits of HEAD no other branch and no remote-tracking branch reaches; `None` where
    /// git would not say.
    pub only_here: Option<u32>,
}

/// One way a repo is stale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stale {
    /// On `branch`, which purlis cut for a chat or a task, where `default` is the remote's.
    OnAChatsBranch { branch: String, default: String },
    /// `origin` is `origin`, and the forge said the repo now lives at `now`.
    RemoteMoved { origin: String, now: String },
}

/// Which fix the person pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fix {
    Branch,
    Remote,
}

/// What a fix does: the one git command it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Switch { to: String },
    SetUrl { to: String },
}

/// Why a fix does nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Blocked {
    #[error("Nothing to fix: the repo is no longer stale this way.")]
    Settled,
    #[error("Not switched: the repo has changes. Commit or move them, then fix it again.")]
    Changes,
    #[error("Not switched: git could not say whether the repo holds work nothing else has.")]
    Unread,
    #[error(
        "Not switched: the branch has {} only it holds. Push it or keep it in a branch folder, \
         then fix it again.",
        commits(*.0)
    )]
    OnlyHere(u32),
}

fn commits(count: u32) -> String {
    if count == 1 {
        "1 commit".to_owned()
    } else {
        format!("{count} commits")
    }
}

/// How `seen` is stale, if it is: on a chat's branch first, then on a moved remote.
pub fn judged(seen: &Seen) -> Vec<Stale> {
    let mut out = Vec::new();
    if let (Some(branch), Some(default)) = (&seen.branch, &seen.default)
        && branch != default
        && seen.chat_branches.contains(branch)
    {
        out.push(Stale::OnAChatsBranch {
            branch: branch.clone(),
            default: default.clone(),
        });
    }
    if let (Some(origin), Some(now)) = (&seen.origin, &seen.moved_to)
        && moved_ok(origin, now)
    {
        out.push(Stale::RemoteMoved {
            origin: origin.clone(),
            now: now.clone(),
        });
    }
    out
}

/// Whether purlis would set `origin` to `now` in its place: an HTTPS address with no user and
/// no blank or control character, on the host `origin` is on, naming another repository there.
fn moved_ok(origin: &str, now: &str) -> bool {
    let Some(rest) = now.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split('/').next().unwrap_or_default();
    if authority.is_empty()
        || authority.contains('@')
        || now.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return false;
    }
    let host = forge::host_of(now);
    if host.is_empty() || host != forge::host_of(origin) {
        return false;
    }
    match (
        forge::gitcredential::place_of(origin),
        forge::gitcredential::place_of(now),
    ) {
        (Some(was), Some(is)) => was != is,
        _ => false,
    }
}

/// What the fix `fix` does to `seen`, or why it does nothing.
///
/// The branch fix switches only a repo whose tree holds no change and whose branch holds no
/// commit that is not on another branch or on the remote: a switch then loses nothing, and the
/// branch stays. Where git would not say, it does nothing.
pub fn step(seen: &Seen, fix: Fix) -> Result<Step, Blocked> {
    let stale = judged(seen);
    match fix {
        Fix::Branch => {
            let default = stale
                .into_iter()
                .find_map(|one| match one {
                    Stale::OnAChatsBranch { default, .. } => Some(default),
                    Stale::RemoteMoved { .. } => None,
                })
                .ok_or(Blocked::Settled)?;
            match seen.clean {
                None => return Err(Blocked::Unread),
                Some(false) => return Err(Blocked::Changes),
                Some(true) => {}
            }
            match seen.only_here {
                None => Err(Blocked::Unread),
                Some(0) => Ok(Step::Switch { to: default }),
                Some(count) => Err(Blocked::OnlyHere(count)),
            }
        }
        Fix::Remote => stale
            .into_iter()
            .find_map(|one| match one {
                Stale::RemoteMoved { now, .. } => Some(Step::SetUrl { to: now }),
                Stale::OnAChatsBranch { .. } => None,
            })
            .ok_or(Blocked::Settled),
    }
}

/// The address git says a fetch was redirected to, from its own warning line: the forge's
/// answer that the repository lives there now.
pub fn redirected(said: &str) -> Option<String> {
    said.lines()
        .find_map(|line| line.strip_prefix("warning: redirecting to "))
        .map(|url| url.trim().trim_end_matches('/').to_owned())
        .filter(|url| !url.is_empty())
}

/// `url` as `<host>/<owner>/<repo>`, with no user, for the window to draw.
pub fn place(url: &str) -> String {
    forge::gitcredential::place_of(url).unwrap_or_else(|| "an address purlis cannot read".into())
}

/// One stale repo of the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub workspace: String,
    pub repo: String,
    pub stale: Vec<Stale>,
}

/// Every stale repo of the plane at `plane`, workspace by workspace.
pub fn found(plane: &Path) -> Vec<Found> {
    let names = crate::workspaces::Plane::open(plane)
        .workspaces()
        .unwrap_or_default();
    let mut out = Vec::new();
    for ws in names {
        let Ok(clones) = repos::clones(plane, &ws) else {
            continue;
        };
        for repo in clones.repos {
            let stale = judged(&read(plane, &ws, &repo));
            if !stale.is_empty() {
                out.push(Found {
                    workspace: ws.clone(),
                    repo: repo.name,
                    stale,
                });
            }
        }
    }
    out
}

/// What git and the piece log say about `repo` of workspace `ws`.
pub fn read(plane: &Path, ws: &str, repo: &repos::Repo) -> Seen {
    let at = &repo.path;
    let state = repos::state_of(at).ok();
    let branch = state.as_ref().and_then(|state| match &state.head {
        Head::Branch(branch) => Some(branch.clone()),
        Head::Unborn(_) | Head::Detached(_) => None,
    });
    let default = asked(
        at,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )
    .and_then(|named| named.strip_prefix("origin/").map(str::to_owned))
    .filter(|named| worktree::name::branch_name_ok(named).is_ok());
    let mut chat_branches = worktree::cut_branches(plane, ws, &repo.name).unwrap_or_default();
    chat_branches.extend(
        crate::pieces::claims(plane, ws)
            .into_keys()
            .filter(|(of, _)| *of == repo.name)
            .map(|(_, piece)| piece),
    );
    // The two git questions a fix needs, asked only of a repo on a chat's branch.
    let judged_on = branch
        .as_ref()
        .filter(|branch| default.as_ref() != Some(*branch) && chat_branches.contains(*branch));
    let clean = judged_on
        .and(state.as_ref())
        .map(|state| state.clean() && crate::gitstate::stopped(at).is_none());
    let only_here = judged_on.and_then(|branch| worktree::unique_commits_of(at, Some(branch)));
    Seen {
        workspace: ws.to_owned(),
        repo: repo.name.clone(),
        branch,
        default,
        chat_branches,
        // As the config records it, never through a rewrite (`url.<base>.insteadOf`).
        origin: asked(at, &["config", "--get", "remote.origin.url"]),
        moved_to: asked(at, &["config", "--get", MOVED_KEY]),
        clean,
        only_here,
    }
}

/// git's one-line answer to `args` in `at`, or `None` where it gave none.
fn asked(at: &Path, args: &[&str]) -> Option<String> {
    git::run(at, args, git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|run| run.line().trim().to_owned())
        .filter(|line| !line.is_empty())
}

/// What a fix did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixed {
    /// One sentence for the window.
    pub said: String,
    /// For the remote fix: the repo, and its remote before and after, as places.
    pub moved: Option<Moved>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    pub clone: std::path::PathBuf,
    pub was: String,
    pub now: String,
}

/// The person's fix on `repo` of workspace `ws`: the repo is read again, and the fix runs its
/// one git command or answers what blocks it. Never a force, a stash or a reset.
pub fn fix(plane: &Path, ws: &str, name: &str, fix: Fix) -> Result<Fixed, String> {
    let repo = repos::clone_at(plane, ws, name)
        .ok_or_else(|| format!("Workspace {ws} holds no repo named {name}."))?;
    let seen = read(plane, ws, &repo);
    match step(&seen, fix).map_err(|blocked| blocked.to_string())? {
        Step::Switch { to } => {
            // Untimed: a switch checks out a tree, and a killed one leaves it half-written. An
            // ignored file the default branch tracks is not overwritten.
            let ran = git::run_untimed(&repo.path, &["switch", "--no-overwrite-ignore", &to])
                .map_err(|why| format!("Not switched: purlis could not start git ({why})."))?;
            if !ran.ok() {
                let said = ran.err.lines().find(|line| !line.trim().is_empty());
                return Err(format!(
                    "Not switched: {}.",
                    said.map_or("git refused", str::trim).trim_end_matches('.')
                ));
            }
            Ok(Fixed {
                said: format!("{name} is on {to}."),
                moved: None,
            })
        }
        Step::SetUrl { to } => {
            let was = seen.origin.clone().unwrap_or_default();
            let set = git::run(&repo.path, &["remote", "set-url", "origin", &to], git::READ)
                .map_err(|why| format!("Not changed: purlis could not start git ({why})."))?;
            if !set.ok() {
                return Err(format!(
                    "Not changed: git would not set the address ({}).",
                    set.err.trim()
                ));
            }
            // What the forge said is done with once origin is there.
            let _ = git::run(&repo.path, &["config", "--unset-all", MOVED_KEY], git::READ);
            Ok(Fixed {
                said: format!("{name} fetches from {}.", place(&to)),
                moved: Some(Moved {
                    clone: repo.path.clone(),
                    was: place(&was),
                    now: place(&to),
                }),
            })
        }
    }
}

/// What a fetch from `origin` in the repo at `at` said, `said` (git's standard error): where the
/// forge redirected it to another address on the same host, that address is kept
/// ([`MOVED_KEY`]); where it did not, a kept one is let go. Run only for a fetch that worked.
pub fn heard(at: &Path, said: &str) {
    let origin = asked(at, &["config", "--get", "remote.origin.url"]);
    match (redirected(said), origin) {
        (Some(now), Some(origin)) if moved_ok(&origin, &now) => {
            let _ = git::run(at, &["config", MOVED_KEY, &now], git::READ);
        }
        _ => {
            if asked(at, &["config", "--get", MOVED_KEY]).is_some() {
                let _ = git::run(at, &["config", "--unset-all", MOVED_KEY], git::READ);
            }
        }
    }
}

#[cfg(test)]
mod tests;
