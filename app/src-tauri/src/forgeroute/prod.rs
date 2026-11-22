// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The seams of the forge route, filled** (one login, slice K; ADR 0128): what the app runs
//! where [`super::Seams::stubs`] has stubs, for the ones that need no window.
//!
//! - [`CoreListener`] is slice A2's listener ([`purlis_core::forge::proxy::listener::RunRoute`]):
//!   the run's unix socket, its `CONNECT` door and its authority's certificate, in the run's
//!   folder.
//! - [`ConfirmedRecorded`] is slice G's store as the route's [`Recorded`]: a project's repos are
//!   the ones the person confirmed for its clones, each read from the forge as the person at the
//!   run's start (its default branch and ids), and its account is the one they chose.
//! - [`ChatGate`] is slice G's credential source
//!   ([`purlis_core::forge::credential::ChatRoute`]) as the route's [`Gate`]: the proxy gives the
//!   person's token only for a repo confirmed by its numeric id, for one of the run's clones.
//!
//! **Tier** (ADR 0069): nothing is stored here; what is read is memory, for a run.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use purlis_core::forge::accounts;
use purlis_core::forge::confirmed::{self, Admission, AsThePerson, ConfirmedRepo, Serving};
use purlis_core::forge::credential::chatroute::ChatRoute;
use purlis_core::forge::proxy::listener::PushGate;
use purlis_core::forge::proxy::{
    AllowedRepo, ChatId, Credential, CredentialError, CredentialRequest, CredentialSource,
    ForgeProxy, Kind,
};

use super::{Bound, Gate, Listener, Recorded, forge_host};

/// The clones of the project at `project`: the checkouts the person's chats work in.
pub fn clones_of(project: &Path) -> Vec<PathBuf> {
    purlis_core::forge::apptoken::clones_now(project)
        .into_iter()
        .map(|started| started.clone)
        .collect()
}

/// **Slice A2's listener** for each run: a socket for `gh` and a `CONNECT` door for `git` and
/// `glab`, in the run's own folder. Where there is no unix socket (Windows) there is none.
pub struct CoreListener;

impl Listener for CoreListener {
    #[cfg(unix)]
    fn bind(
        &self,
        run: &str,
        dir: &Path,
        proxy: Arc<ForgeProxy>,
        push: Arc<dyn PushGate>,
    ) -> io::Result<Bound> {
        use purlis_core::forge::proxy::listener::{RunRoute, Wiring};
        // This run's hand-off secret: the door answers only the chat's own egress proxy, which is
        // handed the secret with the door's address, and the proxy splices only an answer that
        // proves the door.
        let handoff = purlis_core::sandbox::egress::handoff::Secret::random()?;
        let wiring = Wiring::new(ChatId::new(run), proxy, push, handoff.clone());
        let route = RunRoute::open(wiring, dir)?;
        Ok(Bound {
            socket: route.socket.clone(),
            egress_url: format!("http://{}", route.door),
            ca: route.ca_cert.clone(),
            door: route.door,
            handoff,
            hosts: route.hosts.clone(),
            guard: Box::new(route),
        })
    }

    #[cfg(not(unix))]
    fn bind(
        &self,
        _: &str,
        _: &Path,
        _: Arc<ForgeProxy>,
        _: Arc<dyn PushGate>,
    ) -> io::Result<Bound> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the forge listener needs a unix socket",
        ))
    }

    fn ready(&self) -> bool {
        cfg!(unix)
    }
}

/// **Slice G's confirmed repos** as the route's [`Recorded`].
pub struct ConfirmedRecorded {
    config_root: PathBuf,
}

impl ConfirmedRecorded {
    pub fn new(config_root: PathBuf) -> Self {
        Self { config_root }
    }

    /// Every repo confirmed for a clone of `project`, with its clone.
    pub fn confirmed_for(&self, project: &Path) -> Vec<(ConfirmedRepo, PathBuf)> {
        clones_of(project)
            .into_iter()
            .flat_map(|clone| {
                confirmed::repos_of_clone(&self.config_root, &clone)
                    .unwrap_or_default()
                    .into_iter()
                    .map(move |repo| (repo, clone.clone()))
            })
            .collect()
    }
}

impl Recorded for ConfirmedRecorded {
    /// The repos a chat of `project` may reach: each confirmed repo of its clones whose account is
    /// chosen and whose forge answers the person's read of it now (its default branch, which a
    /// push is never allowed to move, and its ids). A repo the forge cannot be asked about is
    /// left out, so the chat has no route to it.
    fn repos(&self, project: &Path) -> Vec<AllowedRepo> {
        let keyring = accounts::keyring_at(&self.config_root);
        let lookup = AsThePerson {
            keyring: keyring.as_ref(),
            reach: &crate::forgeaccounts::token_check,
        };
        let mut out: Vec<AllowedRepo> = Vec::new();
        let listed = accounts::listed(&self.config_root).unwrap_or_default();
        for (repo, clone) in self.confirmed_for(project) {
            // As a chat's own route serves a repo: the chosen account signed in, allowed and
            // through the host's pinned application ([`confirmed::account_for`]).
            let Serving::Account(account) =
                confirmed::account_for(&self.config_root, &repo, &listed)
            else {
                continue;
            };
            let read = match lookup.read(&account, &repo.path) {
                Ok(read) => read,
                Err(why) => {
                    tracing::info!(
                        "purlis: {} was not given to a chat's route: {why}",
                        repo.path
                    );
                    continue;
                }
            };
            // The id read now is the id confirmed: a repo renamed away and replaced under its
            // old name is another repo, and has no route.
            if read.id != repo.repo_id || read.default_branch.is_empty() {
                continue;
            }
            // And the clone trusted, with the repo confirmed by that id ([`confirmed::serves`]).
            if !matches!(
                confirmed::serves(
                    &self.config_root,
                    &clone,
                    repo.kind,
                    &repo.host,
                    &repo.path,
                    read.id
                ),
                Ok(Admission::Confirmed(_))
            ) {
                continue;
            }
            let (host, port) = confirmed::address_of(&repo.host);
            let kind = match repo.kind {
                purlis_core::forge::Kind::GitHub => Kind::GitHub,
                purlis_core::forge::Kind::GitLab => Kind::GitLab,
            };
            let Ok(allowed) = AllowedRepo::new(kind, &host, port, &repo.path, &read.default_branch)
            else {
                continue;
            };
            let mut allowed = allowed.with_forge_id(repo.forge_id());
            if let Some(node) = &read.node_id {
                allowed = allowed.with_node_id(node);
            }
            if !out
                .iter()
                .any(|have| have.host() == allowed.host() && have.path() == allowed.path())
            {
                out.push(allowed);
            }
        }
        out
    }

    /// The branches of the branch folders (`purlis worktree`) the chat `chat` holds, in the
    /// clones that are `repo`: [`purlis_core::piececlaim::branches_held_by`], read from the piece
    /// log and git's own list of the clone's worktrees.
    fn own_branches(&self, project: &Path, chat: &str, repo: &AllowedRepo) -> Vec<String> {
        let clones: Vec<PathBuf> = self
            .confirmed_for(project)
            .into_iter()
            .filter(|(confirmed, _)| {
                let (host, port) = confirmed::address_of(&confirmed.host);
                host == repo.host() && port == repo.port() && confirmed.path == repo.path()
            })
            .map(|(_, clone)| clone)
            .collect();
        let Ok(workspaces) =
            purlis_core::workspaces::Plane::open(project.to_path_buf()).workspaces()
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for ws in workspaces {
            let Ok(found) = purlis_core::repos::clones(project, &ws) else {
                continue;
            };
            for held in found.repos {
                let here = std::fs::canonicalize(&held.path).unwrap_or(held.path.clone());
                if !clones.contains(&here) {
                    continue;
                }
                let Ok(pieces) = purlis_core::worktree::list(project, &ws, &held.name) else {
                    continue;
                };
                let pieces: Vec<(String, Option<String>)> =
                    pieces.into_iter().map(|p| (p.piece, p.branch)).collect();
                out.extend(purlis_core::piececlaim::branches_held_by(
                    project, &ws, &held.name, &pieces, chat,
                ));
            }
        }
        out
    }

    /// The account the person chose for the project's repos on `host`: one account, or none.
    fn account(&self, project: &Path, host: &str) -> Option<String> {
        let mut found: Option<String> = None;
        for (repo, _) in self.confirmed_for(project) {
            let (bare, _) = confirmed::address_of(&repo.host);
            if forge_host(&bare) != host || repo.account.is_empty() {
                continue;
            }
            match &found {
                Some(other) if *other != repo.account => return None,
                _ => found = Some(repo.account),
            }
        }
        found
    }
}

/// **Slice G's credential source** as the route's [`Gate`].
pub struct ChatGate {
    route: ChatRoute,
}

impl ChatGate {
    pub fn new(route: ChatRoute) -> Self {
        Self { route }
    }
}

impl Gate for ChatGate {
    fn enrol(&self, run: &str, project: &Path) {
        self.route.enrol(&ChatId::new(run), clones_of(project));
    }

    fn leave(&self, run: &str) {
        self.route.leave(&ChatId::new(run));
    }

    fn credential(&self, request: &CredentialRequest) -> Result<Credential, CredentialError> {
        self.route.credential_for(request)
    }
}
