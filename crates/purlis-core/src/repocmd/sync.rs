// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter sync`: fetch and fast-forward every clone in a workspace. Python's `cmd_sync`.
//!
//! # It never loses work
//!
//! A clone is moved only when all of these hold, and each one that does not is a sentence
//! and a skip — never a force, never a stash, never a reset:
//!
//! - git could read the tree (**unreadable is not clean**: an empty `status` is what would
//!   otherwise authorise the merge);
//! - nothing is changed or untracked in it;
//! - no rebase, merge, cherry-pick, revert or bisect is in progress — Python did not ask,
//!   and a rebase stopped on a clean tree sits on a detached HEAD it then moved;
//! - HEAD is on a branch — Python fast-forwarded a detached HEAD to `origin/HEAD`;
//! - `origin` is not an SSH remote for a host charter does not manage;
//! - the fetch worked;
//! - the merge is a fast-forward (`--ff-only`) **that overwrites no ignored file**
//!   (`--no-overwrite-ignore`). git treats an ignored file as expendable by default, so an
//!   upstream commit that starts tracking `config.local` replaced the operator's own copy
//!   of it under Python's sync, with a tick printed over it.

use std::path::Path;

use super::{Say, Sink, banner, submodules};
use crate::forge;
use crate::gitstate;
use crate::repos::{self, Head};
use crate::workspaces::Plane;
use crate::worktree::git;

/// Which workspaces to sync.
pub enum Scope<'a> {
    One(&'a str),
    All,
}

/// Run `sync`. Always exits 0, as Python's does: a skipped clone is a sentence, not a failure.
pub fn sync(root: &Path, scope: Scope, say: Sink) -> u8 {
    let (names, label) = match scope {
        Scope::One(ws) => {
            banner(ws, say);
            if !crate::contain::workspace_name_ok(ws) {
                say(Say::Fail(format!("no workspace '{ws}'").into()));
                return 1;
            }
            (vec![ws.to_string()], format!("workspace '{ws}'"))
        }
        Scope::All => {
            let names = Plane::open(root).workspaces().unwrap_or_default();
            let label = format!("all workspaces ({})", names.len());
            (names, label)
        }
    };
    let mut targets: Vec<(String, repos::Repo)> = Vec::new();
    for ws in &names {
        let Ok(found) = repos::clones(root, ws) else {
            continue;
        };
        for (name, why) in found.refused {
            say(Say::Warn(format!("{ws}/{name}: skipping — {why}").into()));
        }
        targets.extend(found.repos.into_iter().map(|r| (ws.clone(), r)));
    }
    if targets.is_empty() {
        say(Say::Warn(
            format!("No cloned repos to sync in {label}.").into(),
        ));
        return 0;
    }
    for (ws, repo) in &targets {
        sync_one(root, ws, repo, say);
    }
    0
}

/// Where remote `remote` of the clone at `clone`, in the project at `root`, is, and the one
/// credential helper git may ask for it, as [`sync`] reaches a remote (qw387): over HTTPS, with
/// the forge's own credential for a host the project manages and none for another, never over
/// SSH to a host it does not manage, and where `hosts` is given (a sandboxed chat's internet
/// access) only on a host in it. `pushing` asks for the helper a push gets. Why not, where not.
pub fn reach(
    root: &Path,
    clone: &Path,
    remote: &str,
    hosts: Option<&[String]>,
    pushing: bool,
) -> Result<(String, Option<String>), String> {
    if remote.is_empty() || remote.starts_with('-') || remote.chars().any(char::is_control) {
        return Err(format!(
            "'{}' cannot name a remote",
            crate::shown::short(remote)
        ));
    }
    // The address the clone records, as written: what it names is what the project manages
    // and the chat's sandbox allows, whatever a rewrite in git's config makes of it.
    let key = format!("remote.{remote}.url");
    let url = git::run(clone, &["config", "--get", &key], git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|run| run.line().trim().to_owned())
        .filter(|url| !url.is_empty())
        .ok_or_else(|| format!("the clone has no remote '{remote}'"))?;
    let managed = forge::resolve_host(&url, root);
    if managed.is_none() && is_ssh(&url) {
        return Err(format!(
            "remote '{remote}' is an SSH address on a host this project does not manage, and \
             purlis reaches a remote over HTTPS with a forge's token only (docs/git-policy.md)"
        ));
    }
    // What git dials too, as its own config rewrites the address, to fetch and to push.
    let dialled: Vec<String> = [
        &["remote", "get-url", remote][..],
        &["remote", "get-url", "--push", remote],
    ]
    .iter()
    .filter_map(|args| {
        git::run(clone, args, git::READ)
            .ok()
            .filter(git::Run::ok)
            .map(|run| run.line().trim().to_owned())
    })
    .collect();
    if let Some(hosts) = hosts {
        for other in &dialled {
            let host = forge::host_of(other);
            if !host.is_empty() && !crate::sandbox::egress::allows(hosts, &host) {
                return Err(format!(
                    "remote '{remote}' reaches '{}', which this chat's sandbox may not reach. The \
                     project's `[sandbox] egress` in {} says which hosts a chat reaches",
                    crate::shown::escaped(&host),
                    crate::plane::MANIFEST
                ));
            }
        }
        let host = forge::host_of(&url);
        if !crate::sandbox::egress::allows(hosts, &host) {
            return Err(format!(
                "remote '{remote}' is on '{}', which this chat's sandbox may not reach. The \
                 project's `[sandbox] egress` in {} says which hosts a chat reaches",
                crate::shown::escaped(&host),
                crate::plane::MANIFEST
            ));
        }
    }
    let helper = managed
        .as_ref()
        .map(|forge| {
            if pushing {
                forge::gitcredential::helper_for_push(forge, clone, &url)
            } else {
                forge::gitcredential::helper_for_fetch(forge, clone)
            }
        })
        .transpose()?;
    Ok((url, helper))
}

/// Fetches remote `remote` of the clone at `clone`, reached as [`reach`] reaches it.
pub fn fetch_remote(
    root: &Path,
    clone: &Path,
    remote: &str,
    hosts: Option<&[String]>,
) -> Result<(), String> {
    let (_, helper) = reach(root, clone, remote, hosts, false)?;
    let fetched = git::run_network(
        clone,
        helper.as_deref(),
        &["fetch", "--prune", "--no-recurse-submodules", remote],
    )
    .map_err(|e| e.to_string())?;
    if fetched.ok() {
        // The forge's word that the repository moved, kept for the window's Notice (#1985).
        if remote == "origin" {
            crate::staleclone::heard(clone, &fetched.err);
        }
        Ok(())
    } else {
        Err(format!(
            "git could not fetch from '{remote}' (access or network): {}",
            fetched.err.lines().next().unwrap_or("").trim()
        ))
    }
}

/// Whether a remote URL goes over SSH: `ssh://…`, or scp-style `host:path`.
fn is_ssh(url: &str) -> bool {
    if url.starts_with("ssh://") || url.starts_with("git+ssh://") || url.starts_with("ssh+git://") {
        return true;
    }
    !url.contains("://") && !forge::host_of(url).is_empty()
}

/// Whether a plain `git fetch` on `branch` fetches from `origin`: the branch names no remote of
/// its own, or names `origin`.
fn fetches_from_origin(d: &Path, branch: &str) -> bool {
    let key = format!("branch.{branch}.remote");
    match git::run(d, &["config", "--get", &key], git::READ) {
        Ok(run) if run.ok() => run.line().trim() == "origin",
        _ => true,
    }
}

fn sync_one(root: &Path, ws: &str, repo: &repos::Repo, say: Sink) {
    let label = format!("{ws}/{}", repo.name);
    let d = &repo.path;
    let state = match repos::state_of(d) {
        Ok(state) => state,
        Err(unreadable) => {
            say(Say::Warn(
                format!("{label}: skipping — {unreadable}.").into(),
            ));
            return;
        }
    };
    if !state.clean() {
        say(Say::Warn(
            format!("{label}: uncommitted changes — skipping (your work is left untouched).")
                .into(),
        ));
        // A submodule left behind the commit the branch records IS an unstaged change to the
        // gitlink, so the explanation of this skip may be one `git submodule status` away.
        submodules::report(root, d, &label, None, None, say);
        return;
    }
    // The same check a save asks (#433). The tree is clean by now, so what stopped is an
    // operation rather than unmerged files.
    if let Some(stopped) = gitstate::stopped(d) {
        let what = stopped.operation.map_or("merge", gitstate::Operation::word);
        say(Say::Warn(
            format!(
                "{label}: a {what} is in progress — skipping (finish or abort it first; nothing \
             was fetched or moved)."
            )
            .into(),
        ));
        return;
    }
    let branch = match &state.head {
        Head::Branch(b) | Head::Unborn(b) => b.clone(),
        Head::Detached(at) => {
            let at = if at.is_empty() {
                String::new()
            } else {
                format!(" at {at}")
            };
            say(Say::Warn(
                format!(
                    "{label}: HEAD is detached{at} — not on a branch, so there is nothing to \
                 fast-forward; left as-is."
                )
                .into(),
            ));
            return;
        }
    };
    let origin = git::run(d, &["remote", "get-url", "origin"], git::READ)
        .ok()
        .filter(|r| r.ok())
        .map(|r| r.line().trim().to_string())
        .unwrap_or_default();
    let managed = forge::resolve_host(&origin, root);
    if managed.is_none() && is_ssh(&origin) {
        say(Say::Warn(
            format!(
                "{label}: origin is an SSH remote on a host this plane does not manage — purlis \
             fetches over HTTPS with a forge CLI's token only (docs/git-policy.md); skipping."
            )
            .into(),
        ));
        return;
    }
    // The forge's own helper for a host the plane manages, and NO credential for one it does
    // not: a token for one forge is never offered to another. The window's own sign-in goes
    // only to the remote recorded for the clone, as a save's push does (#1851).
    let helper = match managed
        .as_ref()
        .map(|forge| forge::gitcredential::helper_for_fetch(forge, d))
        .transpose()
    {
        Ok(helper) => helper,
        Err(why) => {
            say(Say::Fail(format!("{label}: {why} — skipping.").into()));
            return;
        }
    };
    let fetched = git::run_network(
        d,
        helper.as_deref(),
        &["fetch", "--prune", "--no-recurse-submodules"],
    );
    let fetched = match fetched {
        Ok(run) if run.ok() => run,
        _ => {
            say(Say::Fail(
                format!("{label}: fetch failed (access or network) — skipping.").into(),
            ));
            return;
        }
    };
    // The forge's word that the repository moved, kept for the window's Notice (#1985): only
    // for a fetch from `origin`, which is the remote a branch with no other names.
    if fetches_from_origin(d, &branch) {
        crate::staleclone::heard(d, &fetched.err);
    }
    let upstream = format!("origin/{branch}");
    // Untimed: a merge checks out a tree, and a killed one leaves it half-written.
    let merged = git::run_untimed(
        d,
        &["merge", "--ff-only", "--no-overwrite-ignore", &upstream],
    );
    match merged {
        Ok(run) if run.ok() => {}
        Ok(run) if run.err.contains("would be overwritten") => {
            say(Say::Warn(
                format!(
                    "{label}: {branch} was not fast-forwarded — it would overwrite files in the \
                 tree that git does not track or that are ignored; left as-is."
                )
                .into(),
            ));
            return;
        }
        _ => {
            say(Say::Warn(
                format!(
                    "{label}: {branch} won't fast-forward (diverged/local commits) — left as-is."
                )
                .into(),
            ));
            return;
        }
    }
    // The tick is not printed over a tree the fast-forward left behind (charter #817): a
    // fast-forward moves the gitlink and never the submodule's own checkout.
    if submodules::report(
        root,
        d,
        &label,
        Some(&branch),
        Some(&format!("{branch} is up to date, its submodules are not")),
        say,
    ) {
        return;
    }
    say(Say::Done(format!("{label}: up to date on {branch}").into()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ssh_remote_is_told_apart_from_an_https_one_and_a_local_path() {
        assert!(is_ssh("git@github.com:acme/x.git"));
        assert!(is_ssh("ssh://git@host/acme/x.git"));
        assert!(!is_ssh("https://github.com/acme/x.git"));
        assert!(!is_ssh("file:///srv/x.git"));
        assert!(!is_ssh("/srv/x.git"));
    }
}
