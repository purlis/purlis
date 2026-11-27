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
use purlis_core::forge::createdbranches::CreatedBranch;
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
            // The default branch is read here, once, when the route is built (#7526): a repo whose
            // default is renamed mid-run keeps the old name until the next route. The ownership gate
            // on update-branch and rebase does not rest on it, so a stale name only widens what
            // `protects` names. `with_protected_branches` has no caller: the forge read carries no
            // branch-protection rules, and the gate refuses every branch the chat has no tip on.
            let Ok(allowed) = AllowedRepo::new(kind, &host, port, &repo.path, &read.default_branch)
            else {
                continue;
            };
            // The project's protected branches (`[risk] protected_branches`, read from `HEAD` and
            // never fewer than the person accepted), beside the route's built-in ones: no push of
            // a chat reaches them, a lineage push included.
            let committed = purlis_core::askrisk::committed_protected_branches(project);
            let committed: Vec<&str> = committed.iter().map(String::as_str).collect();
            let mut allowed = allowed
                .with_forge_id(repo.forge_id())
                .with_protected_branches(&committed);
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

    /// The tip `chat` last put on `branch`: the app's own record under the machine config root
    /// ([`purlis_core::forge::createdbranches`]), read now, keyed by the project's stable id and
    /// the forge's id of the repo. The id is the one the session host holds the project by, so a plane with no stored `[project] id` owns its chats' branches too.
    fn own_tip(
        &self,
        project: &Path,
        chat: &str,
        host: &str,
        repo_id: u64,
        branch: &str,
    ) -> Option<purlis_core::forge::createdbranches::OwnedTip> {
        let project_id = purlis_core::scaffold::planefile::id_to_hold_by(project).ok()?;
        // The branch purlis cut for an ended direct child of the same workspace and persona is its
        // parent's to follow up too (ADR 0128, amended 2026-11-24), from the app's own dispatch
        // records and `[policy] lineage` (an allowance, denied by `deny`, a lock or a value that
        // does not read; read only when the branch is another chat's), never a file a chat writes.
        purlis_core::forge::createdbranches::owned_tip_for(
            &self.config_root,
            &project_id,
            chat,
            host,
            repo_id,
            branch,
            &|owner| purlis_core::dispatchrecord::lineage_allows(project, chat, owner, branch),
        )
    }

    fn branch_owner(&self, host: &str, repo_id: u64, branch: &str) -> Option<String> {
        purlis_core::forge::createdbranches::owner_of(&self.config_root, host, repo_id, branch)
            .map(|(_, chat, _)| chat)
    }

    fn own_branches(&self, project: &Path, chat: &str, host: &str, repo_id: u64) -> Vec<String> {
        let Ok(project_id) = purlis_core::scaffold::planefile::id_to_hold_by(project) else {
            return Vec::new();
        };
        purlis_core::forge::createdbranches::created_by(
            &self.config_root,
            &project_id,
            chat,
            host,
            repo_id,
        )
    }

    fn record_created(&self, project: &Path, chat: &str, created: &[CreatedBranch]) {
        let Ok(project_id) = purlis_core::scaffold::planefile::id_to_hold_by(project) else {
            tracing::warn!("purlis: a branch a chat created was not recorded: no project id");
            return;
        };
        if let Err(why) = purlis_core::forge::createdbranches::record_via(
            &self.config_root,
            &project_id,
            chat,
            created,
            Some(purlis_core::forge::createdbranches::Via::Route),
        ) {
            // Unrecorded, the route refuses every later update to the branch: safe, and said here.
            tracing::warn!("purlis: a branch a chat created was not recorded ({why})");
        }
    }

    fn forget_created(&self, gone: &[CreatedBranch]) {
        if let Err(why) = purlis_core::forge::createdbranches::forget(&self.config_root, gone) {
            // Still recorded, but the in-memory ownership is already dropped for this run.
            tracing::warn!("purlis: a branch gone from the forge was not forgotten ({why})");
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The route's record of a branch a chat made, for a project that stores no `[project] id`
    /// (a plane): the same id keys the write and every read.
    #[test]
    fn a_project_with_no_stored_id_owns_the_branches_its_chats_make() {
        let config = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        assert!(purlis_core::scaffold::planefile::project_id(project.path()).is_none());
        let recorded = ConfirmedRecorded::new(config.path().to_owned());
        let tip = "1".repeat(40);
        recorded.record_created(
            project.path(),
            "chat-1",
            &[CreatedBranch {
                host: "github.com".into(),
                repo: "acme/app".into(),
                repo_id: Some(7),
                branch: "mine".into(),
                tip: tip.clone(),
            }],
        );

        let own = |chat: &str, project: &Path| {
            recorded
                .own_tip(project, chat, "github.com", 7, "mine")
                .map(|owned| owned.tip)
        };
        assert_eq!(own("chat-1", project.path()), Some(tip));
        assert_eq!(
            recorded.own_branches(project.path(), "chat-1", "github.com", 7),
            ["mine"]
        );
        // Another chat, and another project, own nothing.
        assert_eq!(own("chat-2", project.path()), None);
        let other = tempfile::tempdir().unwrap();
        assert_eq!(own("chat-1", other.path()), None);
    }

    /// The route's own closure (`own_tip`, what `with_tips` calls at every update): a parent takes
    /// up the branch purlis cut for its ended direct child (ADR 0128, amended 2026-11-24), and
    /// nothing wider, through the production wiring and the app's own dispatch records.
    #[test]
    fn a_parent_takes_up_its_ended_childs_cut_branch_through_the_routes_own_tip() {
        use purlis_core::dispatchrecord::{
            self, Asker, ChatRef, Ending, Mode, Opening, Place, Worker, Worktree,
        };
        use purlis_core::forge::createdbranches::{self, Via};

        let config = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let project = std::fs::canonicalize(dir.path()).unwrap();
        let recorded = ConfirmedRecorded::new(config.path().to_owned());
        let chat = |id: &str| ChatRef {
            chat: 1,
            id: Some(id.to_owned()),
            name: "a chat".into(),
            persona: Some("builder".into()),
        };
        let now = "2026-11-24T10:00:00Z".parse().unwrap();
        let task = dispatchrecord::open(
            &project,
            Opening {
                mode: Mode::Task,
                asker: Asker {
                    chat: chat("parent"),
                    workspace: Some("alpha".into()),
                    ..Asker::default()
                },
                persona: Some("builder".into()),
                worker: Worker {
                    chat: chat("child"),
                    ..Worker::default()
                },
                task: None,
                place: Place {
                    workspace: Some("alpha".into()),
                    folder: None,
                    worktree: Some(Worktree {
                        repo: "app".into(),
                        piece: "work".into(),
                        branch: Some("task/work".into()),
                        removed: None,
                    }),
                },
                brief: "do it".into(),
                report_owed: true,
            },
            now,
        )
        .unwrap();
        let made = |branch: &str| CreatedBranch {
            host: "github.com".into(),
            repo: "acme/app".into(),
            repo_id: Some(7),
            branch: branch.into(),
            tip: "1".repeat(40),
        };
        // The child's confirmed pushes, as `record_created` writes them (the route's).
        recorded.record_created(&project, "child", &[made("task/work"), made("task/other")]);
        let tip = |chat: &str, branch: &str| {
            recorded
                .own_tip(&project, chat, "github.com", 7, branch)
                .map(|owned| owned.tip)
        };

        // A running child: nothing.
        assert_eq!(tip("parent", "task/work"), None);
        assert!(dispatchrecord::close(&project, &task.id, Ending::default(), now).unwrap());
        // Ended: the parent follows up the branch purlis cut, and no other the child made.
        assert_eq!(tip("parent", "task/work"), Some("1".repeat(40)));
        assert_eq!(tip("parent", "task/other"), None);
        // Not a stranger; the child keeps its own line.
        assert_eq!(tip("stranger", "task/work"), None);
        assert_eq!(
            tip("child", "task/work"),
            Some("1".repeat(40)),
            "its own line"
        );

        // The person's Allow on the branch is never lineage: the parent has no way to chain it.
        let project_id = purlis_core::scaffold::planefile::id_to_hold_by(&project).unwrap();
        createdbranches::record_via(
            config.path(),
            &project_id,
            "child",
            &[made("task/work")],
            Some(Via::Allow),
        )
        .unwrap();
        assert_eq!(tip("parent", "task/work"), None);
    }
}
