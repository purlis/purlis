// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The repos the person can get a project from** (#1818, spec #1810): what each of their
//! signed-in forge accounts reaches, purlis projects first.
//!
//! **Asked as each account, over purlis's own sign-in.** The list is read only where the host
//! handed its sign-ins over ([`signin::accounts`]): in the window's process, as the person in the
//! window, naming the account ([`Caller::window`] + [`Caller::as_account`]), so every request
//! takes the native transport with that account's token (ADR 0070 §4, #1816). Nothing here asks
//! as the forge's CLI: a host with no account is not listed, and says what to do instead.
//!
//! **A project is found by its manifest, cheaply.** Each repo's top level is read once
//! ([`Repos::top_level`], one request on either forge for any repo whose top level fits a page)
//! and a project is one that holds `purlis.toml` under any of its names. Nothing is cloned. The
//! probes are the account's background work, so they stay inside its request budget (FW-4,
//! [`crate::forge::budget`]): at most [`PROBES_PER_ACCOUNT`] per listing, admitted only while the
//! account's hour is not spent, and a probe held back, refused or failed leaves its repo
//! [`Manifest::Unknown`], listed and unmarked, never called "no project". On GitHub a repeat
//! probe is a conditional request, which the forge does not count (`forge/etag.rs`).
//!
//! **What the person sees**: one row per repo, projects first and then by path; a repo two
//! accounts on one host reach is listed once, as the first. And one [`Reached`] per account and
//! per default host with no account, each saying how many it listed or what to do: sign in, or
//! paste a URL.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::forge::accounts::{ForgeAccount, State};
use crate::forge::signin;
use crate::forge::{
    Account, Caller, Failure, Forge, ForgeBackend, ForgeError, Kind, Owner, RepoRecord,
};
use crate::names::PLANE_MANIFEST;

/// How many repos one listing probes for a manifest, per account: the list's share of the
/// account's hour. GitLab counts every request (`forge/budget.rs`), so a person with hundreds of
/// repos would otherwise spend a large part of purlis's allowance each time the page opens.
pub const PROBES_PER_ACCOUNT: usize = 50;

/// How many probes are in flight at once: enough that a full share answers in about the time
/// of a few requests, few enough that a forge never sees a burst from one window.
const PROBES_AT_ONCE: usize = 6;

/// Whether a repo holds a purlis project's manifest at its top level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Manifest {
    /// It does: a purlis project.
    Found,
    /// Its top level was read and holds none.
    Absent,
    /// Not known: not probed (past the share, or the hour spent), or the probe failed (an empty
    /// repo has no tree).
    Unknown,
}

/// One repo the person can get, as the list draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub kind: Kind,
    pub host: String,
    /// The account that reaches it.
    pub login: String,
    /// `owner/name`, the full namespace path on GitLab.
    pub path: String,
    /// Everything before the name: a user, an organisation, a group or a subgroup's path.
    pub owner: String,
    pub name: String,
    pub description: String,
    /// What the clone is given: the repo's HTTPS address, else its SSH one.
    pub url: String,
    pub manifest: Manifest,
}

/// One account the list was asked as, or one default host with no account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reached {
    pub kind: Kind,
    pub host: String,
    /// The account, or `None` for a host the person has no account on.
    pub login: Option<String>,
    /// How many repos it listed, before a repo another account already listed was dropped.
    pub listed: usize,
    /// What to say instead of repos: why the listing failed, or what to do with no account.
    pub trouble: Option<String>,
}

/// The whole list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Listing {
    pub repos: Vec<Listed>,
    pub reaches: Vec<Reached>,
}

/// The repos every signed-in account reaches, projects first, as the person in the window.
/// Where the host handed no sign-ins over, nothing is asked and every default host says to sign
/// in.
pub fn list() -> Listing {
    let accounts = signin::accounts().unwrap_or_default();
    list_as(&accounts, &|forge| forge.backend())
}

/// [`list`], for `accounts`, each forge's backend built by `backend_of`.
fn list_as(
    accounts: &[ForgeAccount],
    backend_of: &dyn Fn(&Forge) -> Box<dyn ForgeBackend>,
) -> Listing {
    let mut listing = Listing::default();
    let signed_in: Vec<&ForgeAccount> = accounts
        .iter()
        .filter(|a| a.state == State::SignedIn)
        .collect();
    for account in &signed_in {
        let forge = Forge {
            kind: account.kind,
            host: account.host.clone(),
        };
        let backend = backend_of(&forge);
        let caller = Caller::window().as_account(Account {
            kind: account.kind,
            host: account.host.clone(),
            login: account.login.clone(),
        });
        match backend.reachable(&caller, &Owner::every()) {
            Ok(repos) => {
                let listed = repos.len();
                let fresh: Vec<RepoRecord> = repos
                    .into_iter()
                    .filter(|repo| !listing.repos.iter().any(|seen| same(seen, account, repo)))
                    .collect();
                let manifests = probe(backend.as_ref(), &caller, &fresh);
                listing.repos.extend(
                    fresh
                        .iter()
                        .zip(manifests)
                        .map(|(repo, manifest)| listed_as(account, repo, manifest)),
                );
                listing.reaches.push(reached(account, listed, None));
            }
            Err(why) => {
                listing
                    .reaches
                    .push(reached(account, 0, Some(could_not_list(account, &why))))
            }
        }
    }
    for kind in [Kind::GitHub, Kind::GitLab] {
        let host = kind.default_host();
        if !signed_in
            .iter()
            .any(|a| a.kind == kind && a.host.eq_ignore_ascii_case(host))
        {
            let gone = accounts
                .iter()
                .find(|a| a.kind == kind && a.host.eq_ignore_ascii_case(host));
            listing.reaches.push(Reached {
                kind,
                host: host.to_string(),
                login: None,
                listed: 0,
                trouble: Some(no_account(kind, host, gone)),
            });
        }
    }
    order(&mut listing.repos);
    listing
}

/// Whether `seen` is `repo`, reached by another account on the same host.
fn same(seen: &Listed, account: &ForgeAccount, repo: &RepoRecord) -> bool {
    seen.kind == account.kind
        && seen.host.eq_ignore_ascii_case(&account.host)
        && seen.path.eq_ignore_ascii_case(&repo.path_with_namespace)
}

/// Projects first, then by path, then by host: the order the person scans and searches.
pub fn order(repos: &mut [Listed]) {
    repos.sort_by_cached_key(|r| {
        (
            r.manifest != Manifest::Found,
            r.path.to_lowercase(),
            r.host.to_lowercase(),
        )
    });
}

/// Each repo's manifest, in order: the first [`PROBES_PER_ACCOUNT`] probed as the account's
/// background work, a few at a time; the rest [`Manifest::Unknown`].
fn probe(backend: &dyn ForgeBackend, caller: &Caller, repos: &[RepoRecord]) -> Vec<Manifest> {
    let share = repos.len().min(PROBES_PER_ACCOUNT);
    let mut found = vec![Manifest::Unknown; repos.len()];
    let next = AtomicUsize::new(0);
    let background = caller.clone().background();
    let answers: Vec<(usize, Manifest)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..PROBES_AT_ONCE.min(share))
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let at = next.fetch_add(1, Ordering::SeqCst);
                        if at >= share {
                            break mine;
                        }
                        mine.push((at, manifest_of(backend, &background, &repos[at])));
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap_or_default())
            .collect()
    });
    for (at, manifest) in answers {
        found[at] = manifest;
    }
    found
}

/// Whether `repo`'s top level holds a project's manifest, under any name it has had.
fn manifest_of(backend: &dyn ForgeBackend, caller: &Caller, repo: &RepoRecord) -> Manifest {
    match backend.top_level(caller, repo, None) {
        Ok(names) if names.iter().any(|n| PLANE_MANIFEST.recognises(n)) => Manifest::Found,
        Ok(_) => Manifest::Absent,
        Err(_) => Manifest::Unknown,
    }
}

fn listed_as(account: &ForgeAccount, repo: &RepoRecord, manifest: Manifest) -> Listed {
    let path = repo.path_with_namespace.clone();
    let (owner, name) = match path.rsplit_once('/') {
        Some((owner, name)) => (owner.to_string(), name.to_string()),
        None => (String::new(), path.clone()),
    };
    Listed {
        kind: account.kind,
        host: account.host.clone(),
        login: account.login.clone(),
        owner,
        name: if repo.name.is_empty() {
            name
        } else {
            repo.name.clone()
        },
        path,
        description: repo.description.clone(),
        url: clone_url(repo),
        manifest,
    }
}

/// The address a picked repo is cloned from: its HTTPS page, which both forges clone from and
/// purlis's own credential helper signs in to (#752); else its SSH address.
fn clone_url(repo: &RepoRecord) -> String {
    if repo.web_url.starts_with("https://") || repo.ssh_url.is_empty() {
        repo.web_url.clone()
    } else {
        repo.ssh_url.clone()
    }
}

fn reached(account: &ForgeAccount, listed: usize, trouble: Option<String>) -> Reached {
    Reached {
        kind: account.kind,
        host: account.host.clone(),
        login: Some(account.login.clone()),
        listed,
        trouble,
    }
}

/// What a default host with no signed-in account says: sign in, or paste a URL.
fn no_account(kind: Kind, host: &str, gone: Option<&ForgeAccount>) -> String {
    match gone {
        Some(account) => format!(
            "Your sign-in to {host} as {} has ended. Sign in again to list your {} repos, or \
             paste a project's URL above.",
            account.login,
            kind.display()
        ),
        None => format!(
            "Not signed in to {host}. Sign in to list your {} repos, or paste a project's URL \
             above.",
            kind.display()
        ),
    }
}

/// Why an account's listing failed, and what to do about it.
fn could_not_list(account: &ForgeAccount, why: &ForgeError) -> String {
    let (host, login) = (&account.host, &account.login);
    match why.failure() {
        Failure::Auth => format!(
            "{host} refused purlis's sign-in as {login}. Sign in again in {}, or paste a \
             project's URL above.",
            signin::SIGN_IN_AT
        ),
        Failure::RateLimited { .. } => format!(
            "{host} says {login} has made too many requests for now. Try again later, or paste \
             a project's URL above."
        ),
        Failure::Forbidden => format!(
            "{host} would not list the repos {login} reaches: the sign-in lacks the right. Sign \
             in again with the scopes listed, or paste a project's URL above."
        ),
        _ => format!(
            "purlis could not list the repos {login} reaches on {host}: {}. Try again, or paste \
             a project's URL above.",
            why.said().trim_end_matches('.')
        ),
    }
}

#[cfg(test)]
mod tests;
