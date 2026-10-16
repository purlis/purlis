// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A workspace's repo left where new work trips on it, and the one fix for each (#1985).
//!
//! Two ways a repo a workspace holds (its clone, `workspaces/<ws>/<repo>`, never a branch folder
//! under `.worktrees`) goes stale:
//!
//! - **Off its default branch.** Its checked-out branch is not the remote's default branch
//!   (`origin/HEAD`). purlis cannot tell whose branch a plain checkout is, so any other branch of
//!   the repo itself counts; branch folders are never judged. A chat that starts in the repo
//!   works on that branch. The fix switches the repo back to its default branch, and only where
//!   nothing is lost: the tree holds no change, no operation is stopped in it, and every commit
//!   of the branch is on another branch or on the remote. The branch itself is kept.
//! - **On a moved remote.** The forge answered a fetch purlis ran with a redirect: the repo now
//!   lives at another address on the same host (a rename, or a move to another owner). purlis
//!   keeps that address in the repo's own config (`remote.origin.purlisMovedTo`), rebuilt from
//!   its checked parts, and the fix sets `origin` to exactly that address. A push URL of
//!   `origin`'s own goes with it where every one is `origin`'s old address, read the same way
//!   (#2117): the same scheme, the same user, the same host and the same path. It is dropped, as
//!   `forge::gitcredential::remote::keep` drops one, so pushes go where `origin` now fetches
//!   from. Where any is another (another place, or another transport or identity for the same
//!   place), every push URL is left, and the window says so.
//!
//! **Read from what purlis can read without running anything a chat wrote**: git through the
//! hardened runner ([`crate::worktree::git`]), which runs no hook, filter or fsmonitor. Nothing
//! here asks the network.
//!
//! [`judged`] and [`step`] are the decisions, over a [`Seen`] that says what was read; [`read`]
//! and [`fix`] do the reading and the one git command a fix runs. The fix is the person's, from
//! the window (the command is on `WINDOW_ONLY`), and it reads the repo again before it acts.

use std::path::Path;

use crate::forge;
use crate::repos::{self, Head};
use crate::worktree::{self, git};

/// The repo's config key that keeps where the forge said `origin` moved to.
pub const MOVED_KEY: &str = "remote.origin.purlisMovedTo";

/// What purlis read about one repo of a workspace. Its `Debug` names each address as a place:
/// a recorded address can carry a token.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Seen {
    pub workspace: String,
    pub repo: String,
    /// The branch it has checked out; `None` on a detached HEAD or where git would not say.
    pub branch: Option<String>,
    /// The remote's default branch as the repo knows it (`origin/HEAD`), with no fetch.
    pub default: Option<String>,
    /// `origin`'s address as the repo's config records it.
    pub origin: Option<String>,
    /// Where the forge said `origin` now lives ([`MOVED_KEY`]).
    pub moved_to: Option<String>,
    /// `origin`'s push URLs as the repo's config records them (`remote.origin.pushurl`).
    pub push_urls: Vec<String>,
    /// No change in the tree and no operation stopped in it; `None` where git would not say.
    pub clean: Option<bool>,
    /// The commits of HEAD no other branch and no remote-tracking branch reaches; `None` where
    /// git would not say.
    pub only_here: Option<u32>,
}

impl std::fmt::Debug for Seen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Seen")
            .field("workspace", &self.workspace)
            .field("repo", &self.repo)
            .field("branch", &self.branch)
            .field("default", &self.default)
            .field("origin", &self.origin.as_deref().map(place))
            .field("moved_to", &self.moved_to.as_deref().map(place))
            .field("push_urls", &places(&self.push_urls))
            .field("clean", &self.clean)
            .field("only_here", &self.only_here)
            .finish()
    }
}

/// One way a repo is stale. Its `Debug` names each address as a place: a recorded address can
/// carry a token.
#[derive(Clone, PartialEq, Eq)]
pub enum Stale {
    /// On `branch`, where `default` is the remote's default branch.
    OffDefault { branch: String, default: String },
    /// `origin` is `origin`, and the forge said the repo now lives at `now`: the address the fix
    /// sets, as [`rebuilt`] made it. `pushes` says what the fix does with `origin`'s push URLs.
    RemoteMoved {
        origin: String,
        now: String,
        pushes: Pushes,
    },
}

impl std::fmt::Debug for Stale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OffDefault { branch, default } => f
                .debug_struct("OffDefault")
                .field("branch", branch)
                .field("default", default)
                .finish(),
            Self::RemoteMoved {
                origin,
                now,
                pushes,
            } => f
                .debug_struct("RemoteMoved")
                .field("origin", &place(origin))
                .field("now", &place(now))
                .field("pushes", pushes)
                .finish(),
        }
    }
}

/// What the remote fix does with `origin`'s push URLs (#2117).
#[derive(Clone, PartialEq, Eq)]
pub enum Pushes {
    /// It has none of its own: pushes go where it fetches from.
    Origin,
    /// Every one is `origin`'s old address, read the same way: the fix drops them, and pushes go
    /// where `origin` now fetches from.
    Follow,
    /// These are left as they are: any one names another place, or the same place over another
    /// transport or as another user.
    Kept(Vec<String>),
}

impl std::fmt::Debug for Pushes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Origin => f.write_str("Origin"),
            Self::Follow => f.write_str("Follow"),
            Self::Kept(urls) => f.debug_tuple("Kept").field(&places(urls)).finish(),
        }
    }
}

/// `urls` as places, for a `Debug` that never prints an address whole.
fn places(urls: &[String]) -> Vec<String> {
    urls.iter().map(|url| place(url)).collect()
}

/// What the fix does with `push_urls` when `origin` moves.
fn pushes_of(origin: &str, push_urls: &[String]) -> Pushes {
    if push_urls.is_empty() {
        return Pushes::Origin;
    }
    let was = address(origin);
    if was.is_some() && push_urls.iter().all(|url| address(url) == was) {
        Pushes::Follow
    } else {
        Pushes::Kept(push_urls.to_vec())
    }
}

/// A remote's address as the parts that say where and how git reaches it: the transport
/// (`git@host:path` is SSH), the user as written, the host (and port) lowercased, and the path
/// as written, less a trailing `/` and `.git`. `None` for a folder or what does not read as an
/// address.
fn address(url: &str) -> Option<(String, String, String, String)> {
    let (scheme, authority, path) = match url.split_once("://") {
        Some((scheme, rest)) => {
            let (authority, path) = rest.split_once('/')?;
            let scheme = scheme.to_ascii_lowercase();
            let scheme = match scheme.as_str() {
                "git+ssh" | "ssh+git" => "ssh".to_owned(),
                _ => scheme,
            };
            (scheme, authority, path)
        }
        None => {
            let (authority, path) = url.split_once(':')?;
            if authority.contains('/') {
                return None;
            }
            ("ssh".to_owned(), authority, path)
        }
    };
    let (user, host) = authority.rsplit_once('@').unwrap_or(("", authority));
    let path = path.trim_start_matches('/').trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    (!host.is_empty() && !path.is_empty()).then(|| {
        (
            scheme,
            user.to_owned(),
            host.to_ascii_lowercase(),
            path.to_owned(),
        )
    })
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
    Switch {
        to: String,
    },
    /// Sets `origin` to `to`, and with `drop_push` drops its push URLs ([`Pushes::Follow`]).
    SetUrl {
        to: String,
        drop_push: bool,
    },
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

/// How `seen` is stale, if it is: off its default branch first, then on a moved remote.
pub fn judged(seen: &Seen) -> Vec<Stale> {
    let mut out = Vec::new();
    if let (Some(branch), Some(default)) = (&seen.branch, &seen.default)
        && branch != default
    {
        out.push(Stale::OffDefault {
            branch: branch.clone(),
            default: default.clone(),
        });
    }
    if let (Some(origin), Some(said)) = (&seen.origin, &seen.moved_to)
        && let Some(now) = rebuilt(origin, said)
    {
        out.push(Stale::RemoteMoved {
            origin: origin.clone(),
            now,
            pushes: pushes_of(origin, &seen.push_urls),
        });
    }
    out
}

/// The address purlis would set `origin` to on the forge's word that the repository is at
/// `said`, or `None`: `https://<host>[:<port>]/<owner>/<repo>.git`, built from `said`'s parts once
/// each is checked, so what the window shows is what is set. The host and the port are
/// `origin`'s own (an SSH `origin` has no HTTPS port); each path segment is letters, digits,
/// `.`, `_` and `-`, and not `.` or `..`, with at least an owner and a name; nothing else (a
/// user, a query, an escape) is taken. And it names another repository than `origin` does.
pub fn rebuilt(origin: &str, said: &str) -> Option<String> {
    let rest = said.strip_prefix("https://")?;
    let (authority, path) = rest.split_once('/')?;
    let (host, port) = host_and_port(authority)?;
    if (host.clone(), port.clone()) != origin_host_and_port(origin)? {
        return None;
    }
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let segments: Vec<&str> = path.split('/').collect();
    let segment_ok = |one: &&str| {
        !one.is_empty()
            && *one != "."
            && *one != ".."
            && one
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    };
    if segments.len() < 2 || !segments.iter().all(segment_ok) {
        return None;
    }
    let port = port.map(|port| format!(":{port}")).unwrap_or_default();
    let url = format!("https://{host}{port}/{}.git", segments.join("/"));
    let was = forge::gitcredential::place_of(origin)?;
    (forge::gitcredential::place_of(&url)? != was).then_some(url)
}

/// An HTTPS authority's host, lowercased, and its port: `None` for a user, an empty or
/// unusual host, or a port that is not digits.
fn host_and_port(authority: &str) -> Option<(String, Option<String>)> {
    if authority.contains('@') {
        return None;
    }
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    let host_ok = !host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'));
    let port_ok =
        port.is_none_or(|port| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()));
    (host_ok && port_ok).then(|| (host.to_ascii_lowercase(), port.map(str::to_owned)))
}

/// The host and the HTTPS port `origin` is reached on: an HTTPS address's own (its user
/// dropped), else the host of an SSH address with no HTTPS port.
fn origin_host_and_port(origin: &str) -> Option<(String, Option<String>)> {
    match origin.strip_prefix("https://") {
        Some(rest) => {
            let authority = rest.split('/').next()?;
            let authority = authority
                .rsplit_once('@')
                .map_or(authority, |(_, host)| host);
            host_and_port(authority)
        }
        None => {
            let host = forge::host_of(origin);
            (!host.is_empty()).then_some((host, None))
        }
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
                    Stale::OffDefault { default, .. } => Some(default),
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
                Stale::RemoteMoved { now, pushes, .. } => Some(Step::SetUrl {
                    to: now,
                    drop_push: pushes == Pushes::Follow,
                }),
                Stale::OffDefault { .. } => None,
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

/// What [`found`] read, and what it could not. Its `Debug` prints no address whole ([`Stale`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Findings {
    pub found: Vec<Found>,
    /// A sentence for each workspace or repo it could not read, which is not judged stale or not.
    pub unread: Vec<String>,
}

/// One stale repo of the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub workspace: String,
    pub repo: String,
    pub stale: Vec<Stale>,
}

/// Every stale repo of the plane at `plane`, workspace by workspace, and what could not be read.
pub fn found(plane: &Path) -> Findings {
    let mut out = Findings::default();
    let names = match crate::workspaces::Plane::open(plane).workspaces() {
        Ok(names) => names,
        Err(why) => {
            out.unread.push(format!(
                "purlis could not list the project's workspaces ({why})."
            ));
            return out;
        }
    };
    for ws in names {
        let clones = match repos::clones(plane, &ws) {
            Ok(clones) => clones,
            Err(why) => {
                out.unread.push(format!("{why}."));
                continue;
            }
        };
        for (name, why) in clones.refused {
            out.unread.push(format!("{ws}/{name}: {why}."));
        }
        for repo in clones.repos {
            let stale = judged(&read(&ws, &repo));
            if !stale.is_empty() {
                out.found.push(Found {
                    workspace: ws.clone(),
                    repo: repo.name,
                    stale,
                });
            }
        }
    }
    out
}

/// What git says about `repo` of workspace `ws`.
pub fn read(ws: &str, repo: &repos::Repo) -> Seen {
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
    // The two git questions a fix needs, asked only of a repo off its default branch.
    let judged_on = branch
        .as_ref()
        .filter(|branch| default.is_some() && default.as_ref() != Some(*branch));
    let clean = judged_on
        .and(state.as_ref())
        .map(|state| state.clean() && crate::gitstate::stopped(at).is_none());
    let only_here = judged_on.and_then(|branch| worktree::unique_commits_of(at, Some(branch)));
    Seen {
        workspace: ws.to_owned(),
        repo: repo.name.clone(),
        branch,
        default,
        // As the config records it, never through a rewrite (`url.<base>.insteadOf`).
        origin: asked(at, &["config", "--get", "remote.origin.url"]),
        moved_to: asked(at, &["config", "--get", MOVED_KEY]),
        push_urls: git::run(
            at,
            &["config", "--get-all", "remote.origin.pushurl"],
            git::READ,
        )
        .ok()
        .filter(git::Run::ok)
        .map(|run| {
            run.out
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default(),
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
/// one git command or answers what blocks it. Never a force, a stash or a reset. `shown` is the
/// address the window named for the remote fix: nothing else is set.
pub fn fix(
    plane: &Path,
    ws: &str,
    name: &str,
    fix: Fix,
    shown: Option<&str>,
) -> Result<Fixed, String> {
    let repo = repos::clone_at(plane, ws, name)
        .ok_or_else(|| format!("Workspace {ws} holds no repo named {name}."))?;
    let seen = read(ws, &repo);
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
        Step::SetUrl { to, drop_push } => {
            if shown != Some(to.as_str()) {
                return Err(
                    "Not changed: the forge's address changed since the window showed it. Look \
                     at it again."
                        .to_owned(),
                );
            }
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
            // Its push URLs named the old place: dropped, pushes follow origin.
            if drop_push {
                let dropped = git::run(
                    &repo.path,
                    &["remote", "set-url", "--push", "--delete", "origin", "."],
                    git::READ,
                )
                .map_err(|why| format!("purlis could not start git ({why})"))
                .and_then(|run| {
                    if run.ok() {
                        Ok(())
                    } else {
                        Err(run.err.trim().to_owned())
                    }
                });
                if let Err(why) = dropped {
                    return Err(format!(
                        "origin fetches from {}, but pushes still go to {}: its push address \
                         could not be dropped ({why}).",
                        place(&to),
                        place(&was)
                    ));
                }
            }
            let fetches = if drop_push {
                "fetches from and pushes to"
            } else {
                "fetches from"
            };
            Ok(Fixed {
                said: format!("{name} {fetches} {}.", place(&to)),
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
/// forge redirected it to another address on the same host, that address is kept as
/// [`rebuilt`] makes it ([`MOVED_KEY`]); where it did not, a kept one is let go. Run only for a
/// fetch that worked.
pub fn heard(at: &Path, said: &str) {
    let origin = asked(at, &["config", "--get", "remote.origin.url"]);
    let now = redirected(said)
        .zip(origin)
        .and_then(|(said, origin)| rebuilt(&origin, &said));
    match now {
        Some(now) => {
            let _ = git::run(at, &["config", MOVED_KEY, &now], git::READ);
        }
        None => {
            if asked(at, &["config", "--get", MOVED_KEY]).is_some() {
                let _ = git::run(at, &["config", "--unset-all", MOVED_KEY], git::READ);
            }
        }
    }
}

#[cfg(test)]
mod tests;
