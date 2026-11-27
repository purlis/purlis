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
//! branch is used in its place ([`Fresh::instead`]), and so it is where the two have each moved
//! on (#3890): the clone's branch then holds commits its remote never took, most often ones
//! upstream squashed into one of its own, and is never moved. Where it is only ahead of its
//! remote, it is used as the clone has it, its own commits the base's. The note says which,
//! and what brings the clone up.
//!
//! A base may also name a remote-tracking branch, `origin/main` (#3836): the piece is cut from
//! or moved onto it, and recorded as cut from the clone's branch of the same name, which
//! follows it. A sandboxed chat, which cannot make a branch or move one, has that way to the
//! remote whatever the clone's own branch holds.
//!
//! Every git call goes through the hardened runner, under whatever isolation the caller holds.

use std::path::Path;
use std::time::Duration;

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
    /// files would be left behind its commit), where the two have diverged (#3890), or where the
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
        // Ahead of it: the clone's own commits are the base's. Apart from it: the remote's.
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
                        "{branch} and {short} have each moved on, so purlis used {short} and \
                         left {branch} where it is. To bring {branch} onto {short}, rebase or \
                         reset it from your own terminal"
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

/// The remote-tracking branch (`refs/remotes/origin/main`) that branch `branch` of the clone at
/// `clone` follows: its upstream, or `origin`'s of the same name. `None` where it follows none.
pub fn followed(clone: &Path, branch: &str) -> Result<Option<String>, Refusal> {
    upstream_of(clone, branch, &name::as_ref(branch))
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

/// How long one ask of a remote for its default branch may take ([`asked_default`]): a single
/// line over the network, so far less than a fetch's deadline.
pub const DEFAULT_ASKED: Duration = Duration::from_secs(20);

/// How long an ask of a remote that failed waits before it is made once more (#7870): long
/// enough for a credential helper or proxy that several cuts asked at once to settle.
pub const ASK_AGAIN_AFTER: Duration = Duration::from_millis(500);

/// How long the fetch of a remote's default branch before a dispatch cuts from it may take
/// ([`dispatch_base`]): one branch, so less than a whole fetch's [`git::NETWORK`].
pub const DEFAULT_FETCHED: Duration = Duration::from_secs(60);

/// The branch the remote `remote` of the clone at `clone` names its default, asked of the
/// remote itself (`git ls-remote --symref <remote> HEAD`), reached as a fetch is
/// ([`crate::repocmd::sync::reach`], `hosts` the chat's internet access where it has one).
/// Never read from the clone, whose refs a chat may write. `None` where the remote does not
/// answer or names none: [`asked_default`] says why.
pub fn remote_default(
    root: &Path,
    clone: &Path,
    remote: &str,
    hosts: Option<&[String]>,
) -> Option<String> {
    asked_default(root, clone, remote, hosts).ok()
}

/// [`remote_default`], or why there is none, in words that finish "purlis could not tell
/// which branch is the default:".
///
/// **Bounded, and asked twice at most** (#7870): each ask has [`DEFAULT_ASKED`], not a whole
/// fetch's [`git::NETWORK`], since it carries one line; an ask that fails is made once more
/// after [`ASK_AGAIN_AFTER`], as several cuts in one turn ask the same remote at once and one
/// of them may meet a busy credential helper or proxy. A remote purlis may not reach at all (an
/// SSH address on a host the project does not manage, a host outside the chat's egress) is not
/// asked, and the reason is [`crate::repocmd::sync::reach`]'s.
pub fn asked_default(
    root: &Path,
    clone: &Path,
    remote: &str,
    hosts: Option<&[String]>,
) -> Result<String, String> {
    asked_default_within(root, clone, remote, hosts, DEFAULT_ASKED, ASK_AGAIN_AFTER)
}

/// [`asked_default`], each ask bounded by `within` and the second made after `backoff`.
pub fn asked_default_within(
    root: &Path,
    clone: &Path,
    remote: &str,
    hosts: Option<&[String]>,
    within: Duration,
    backoff: Duration,
) -> Result<String, String> {
    let (_, helper) = crate::repocmd::sync::reach(root, clone, remote, hosts, false)?;
    let asked = twice(
        || {
            let run = git::run_network_within(
                clone,
                helper.as_deref(),
                &["ls-remote", "--symref", remote, "HEAD"],
                within,
            )
            .map_err(|why| format!("'{remote}' did not answer: {why}"))?;
            if run.ok() {
                Ok(run)
            } else {
                Err(format!(
                    "'{remote}' did not answer (access or network): {}",
                    run.err.lines().next().unwrap_or("").trim()
                ))
            }
        },
        backoff,
    )?;
    let branch = asked
        .out
        .lines()
        .find_map(|line| {
            let (named, at) = line.strip_prefix("ref: ")?.split_once('\t')?;
            (at.trim() == "HEAD")
                .then(|| named.strip_prefix("refs/heads/"))
                .flatten()
                .map(str::to_owned)
        })
        .ok_or_else(|| format!("'{remote}' answered, and named no branch its default"))?;
    name::branch_name_ok(&branch)
        .map_err(|_| format!("'{remote}' named a default branch purlis does not take"))?;
    Ok(branch)
}

/// `ask`, made once more after `backoff` where it failed: the second answer, whatever it is.
pub fn twice<T>(
    mut ask: impl FnMut() -> Result<T, String>,
    backoff: Duration,
) -> Result<T, String> {
    ask().or_else(|_| {
        std::thread::sleep(backoff);
        ask()
    })
}

/// Where a dispatch's worktree is cut from, in a clone that has a remote ([`dispatch_base`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchBase {
    /// The clone's branch of the remote's default: what the piece is cut from, brought up to
    /// the remote-tracking branch first ([`bring_up`]), and recorded as cut from.
    pub branch: String,
    /// What the asking chat should know: the default as last fetched, and how old, where it
    /// could not be fetched now; a branch made in the clone to follow it.
    pub notes: Vec<String>,
}

/// **A dispatch's base where the clone has remote `remote`** (#7870): never the commit the
/// clone happens to be on.
///
/// The remote is asked which branch is its default ([`asked_default`]), and that branch is
/// fetched (bounded by [`DEFAULT_FETCHED`]) into its remote-tracking branch, so the task starts
/// from the remote as it is now. Where the fetch fails, it starts from the branch as last
/// fetched, and a note says how long ago that was. Where the remote cannot be asked, the
/// default the clone last fetched (`refs/remotes/<remote>/HEAD`) is used, and said so with its
/// age. Refused, with the reason, only where there is neither. A clone with no branch of the
/// default's name gets one that follows the remote-tracking branch, as `git checkout` makes
/// one, since a piece is recorded as cut from a branch of the clone.
pub fn dispatch_base(
    root: &Path,
    clone: &Path,
    remote: &str,
    hosts: Option<&[String]>,
) -> Result<DispatchBase, String> {
    let tracking = |branch: &str| format!("refs/remotes/{remote}/{branch}");
    let mut notes = Vec::new();
    let asked = asked_default(root, clone, remote, hosts);
    // The ask is made by each cut at once; the fetch and the branch it may make, one at a time.
    let lock = fetch_lock(clone);
    let _one_at_a_time = lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let branch = match asked {
        Ok(branch) => {
            if let Err(why) = fetch_branch(root, clone, remote, &branch, hosts) {
                let Some(when) = last_fetched(clone, &tracking(&branch)) else {
                    return Err(format!(
                        "'{remote}' names {branch} its default, purlis could not fetch it \
                         ({why}), and the clone has never fetched it"
                    ));
                };
                notes.push(format!(
                    "purlis could not fetch {remote}/{branch} ({why}), so the new branch \
                     starts from it as the clone last fetched it ({when})"
                ));
            }
            branch
        }
        Err(why) => {
            let fetched = tracked_head(clone, remote).and_then(|branch| {
                last_fetched(clone, &tracking(&branch)).map(|when| (branch, when))
            });
            let Some((branch, when)) = fetched else {
                return Err(format!(
                    "{why}, and the clone has fetched no default branch of '{remote}'"
                ));
            };
            notes.push(format!(
                "purlis could not ask '{remote}' which branch is its default ({why}), so the \
                 new branch starts from {remote}/{branch} as the clone last fetched it ({when})"
            ));
            branch
        }
    };
    let local = name::as_ref(&branch);
    if commit_of(clone, &local)
        .map_err(|why| why.in_window())?
        .is_none()
    {
        let made = git::run(
            clone,
            &["branch", "--quiet", "--track", &branch, &tracking(&branch)],
            git::READ,
        )
        .map_err(|why| why.to_string())?;
        if !made.ok() {
            return Err(format!(
                "the clone has no branch {branch}, and git did not make one following \
                 {remote}/{branch}: {}",
                made.err.trim()
            ));
        }
        notes.push(format!(
            "the clone had no branch {branch}, so purlis made one following {remote}/{branch}"
        ));
    }
    Ok(DispatchBase { branch, notes })
}

/// Fetches branch `branch` of remote `remote` into its remote-tracking branch, bounded by
/// [`DEFAULT_FETCHED`], reached as [`crate::repocmd::sync::reach`] reaches it.
fn fetch_branch(
    root: &Path,
    clone: &Path,
    remote: &str,
    branch: &str,
    hosts: Option<&[String]>,
) -> Result<(), String> {
    let (_, helper) = crate::repocmd::sync::reach(root, clone, remote, hosts, false)?;
    let refspec = format!("+refs/heads/{branch}:refs/remotes/{remote}/{branch}");
    let fetched = git::run_network_within(
        clone,
        helper.as_deref(),
        &[
            "fetch",
            "--no-tags",
            "--no-recurse-submodules",
            "--no-write-fetch-head",
            "--no-auto-maintenance",
            remote,
            &refspec,
        ],
        DEFAULT_FETCHED,
    )
    .map_err(|why| why.to_string())?;
    if fetched.ok() {
        Ok(())
    } else {
        Err(format!(
            "access or network: {}",
            fetched.err.lines().next().unwrap_or("").trim()
        ))
    }
}

/// The branch `refs/remotes/<remote>/HEAD` names in the clone: the default the clone last
/// fetched, where it recorded one.
fn tracked_head(clone: &Path, remote: &str) -> Option<String> {
    let head = format!("refs/remotes/{remote}/HEAD");
    let read = git::run(clone, &["symbolic-ref", "--quiet", &head], git::READ).ok()?;
    let prefix = format!("refs/remotes/{remote}/");
    let branch = read
        .ok()
        .then(|| read.line().trim().strip_prefix(&prefix))??;
    name::branch_name_ok(branch).ok()?;
    Some(branch.to_owned())
}

/// How long ago the clone last moved remote-tracking branch `tracking`, as a person reads it,
/// where it has that branch: from its reflog, or said unknown where git kept none.
fn last_fetched(clone: &Path, tracking: &str) -> Option<String> {
    commit_of(clone, tracking).ok()??;
    let logged = git::run(
        clone,
        &[
            "log",
            "--walk-reflogs",
            "-1",
            "--date=unix",
            "--format=%gd",
            tracking,
        ],
        git::READ,
    )
    .ok()
    .filter(git::Run::ok)
    // `refs/remotes/origin/main@{1764240000}`: the time of its last move, in seconds.
    .and_then(|run| {
        let line = run.line().trim().to_owned();
        let (_, at) = line.strip_suffix('}')?.rsplit_once("@{")?;
        at.parse::<u64>().ok()
    });
    Some(match logged {
        Some(at) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_secs());
            crate::cistate::ago(now.saturating_sub(at))
        }
        None => "git did not record when".to_owned(),
    })
}

/// One fetch of a dispatch's base at a time in each clone, in this process: three cuts in one
/// turn would otherwise race git for the same ref lock, and two of them fail.
fn fetch_lock(clone: &Path) -> std::sync::Arc<std::sync::Mutex<()>> {
    use std::collections::HashMap;
    use std::sync::{Arc, LazyLock, Mutex, PoisonError};
    static LOCKS: LazyLock<Mutex<HashMap<std::path::PathBuf, Arc<Mutex<()>>>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    let mut held = LOCKS.lock().unwrap_or_else(PoisonError::into_inner);
    Arc::clone(held.entry(clone.to_path_buf()).or_default())
}

#[cfg(test)]
mod tests {
    use super::{split_remote, twice};
    use std::time::{Duration, Instant};

    #[test]
    fn an_ask_that_fails_is_made_once_more_after_its_backoff_and_no_more() {
        // #7870: several cuts in one turn ask one remote at once, and one may meet a busy
        // credential helper or proxy.
        let backoff = Duration::from_millis(50);
        let mut asked = 0;
        let started = Instant::now();
        let answer = twice(
            || {
                asked += 1;
                if asked == 1 {
                    Err("busy".to_owned())
                } else {
                    Ok("main")
                }
            },
            backoff,
        );
        assert_eq!(answer, Ok("main"));
        assert_eq!(asked, 2);
        assert!(started.elapsed() >= backoff, "the second ask waits first");

        let mut asked = 0;
        let answer: Result<(), String> = twice(
            || {
                asked += 1;
                Err(format!("no answer {asked}"))
            },
            Duration::ZERO,
        );
        assert_eq!(answer, Err("no answer 2".to_owned()), "the second's reason");
        assert_eq!(asked, 2, "twice at most");

        let mut asked = 0;
        let answer = twice(
            || {
                asked += 1;
                Ok::<_, String>(asked)
            },
            Duration::from_secs(60),
        );
        assert_eq!(answer, Ok(1), "an answer is never asked for again");
    }

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
