// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The held acts' context and the push gate (one login K3): which repo a push is of, who may be
//! acted for, and how many pushes are read at once.

use std::path::PathBuf;
use std::sync::Arc;

use purlis_core::forge::Kind;
use purlis_core::forge::confirmed::ConfirmedRepo;
use purlis_core::forge::proxy::listener::{PushGate, PushRead, PushUpdate, PushVerdict};
use purlis_core::forge::proxy::{
    ChatId, Credential, CredentialError, CredentialRequest, CredentialSource, ForgeProxy, NoObjects,
};
use purlis_core::heldact::HeldContext;
use purlis_core::mergehold::Repo;

use super::*;

fn entry(host: &str, path: &str) -> (ConfirmedRepo, PathBuf) {
    (
        ConfirmedRepo {
            clone_key: "clone".to_owned(),
            kind: if host.contains("gitlab") {
                Kind::GitLab
            } else {
                Kind::GitHub
            },
            host: host.to_owned(),
            path: path.to_owned(),
            repo_id: 7,
            account: "a1".to_owned(),
            client_id: String::new(),
        },
        PathBuf::from("/clone"),
    )
}

#[test]
fn a_path_confirmed_on_one_host_is_the_repo_there_and_not_at_another_hosts_address() {
    let (repo, _) = pick(vec![entry("github.com", "acme/app")], "acme/app").expect("one repo");

    assert!(is_at(&repo, "github.com", 443));
    assert!(
        !is_at(&repo, "gitlab.com", 443),
        "a push to GitLab is not the push of the GitHub repo of the same path"
    );
    assert!(
        !is_at(&repo, "github.com", 8443),
        "a port is part of the address"
    );
}

#[test]
fn a_path_confirmed_on_two_forges_is_refused_for_a_path_cannot_tell_them_apart() {
    let both = vec![
        entry("github.com", "acme/app"),
        entry("gitlab.com", "acme/app"),
    ];

    let refused = pick(both, "acme/app").expect_err("refused");

    assert!(refused.contains("more than one forge"), "{refused}");
}

#[test]
fn a_path_confirmed_for_two_clones_of_one_host_is_one_repo() {
    let mut second = entry("github.com", "Acme/App");
    second.1 = PathBuf::from("/other");

    let picked = pick(vec![entry("github.com", "acme/app"), second], "acme/app");

    assert!(picked.is_ok());
}

#[test]
fn a_path_nothing_confirms_is_refused() {
    let refused = pick(vec![entry("github.com", "acme/app")], "acme/other").expect_err("refused");

    assert!(refused.contains("not a repo confirmed"), "{refused}");
}

#[test]
fn no_more_than_four_quarantine_fetches_run_at_once_and_a_finished_one_gives_its_place_back() {
    let quarantines = Quarantines::default();

    let held: Vec<_> = (0..MOST_FETCHING)
        .map(|_| quarantines.begin_fetching().expect("a place"))
        .collect();

    assert!(quarantines.begin_fetching().is_none(), "the fifth waits");
    drop(held);
    assert!(
        quarantines.begin_fetching().is_some(),
        "places are given back"
    );
}

/// A set of sessions: the chat named `chat` is session 7, and it is open where `open` says.
struct Sessions {
    open: bool,
}

impl SessionOfRun for Sessions {
    fn session(&self, run: &str) -> Option<u32> {
        (self.open && run == "chat").then_some(7)
    }
    fn is_open(&self, session: u32) -> bool {
        self.open && session == 7
    }
}

struct NoCredentials;

impl CredentialSource for NoCredentials {
    fn credential_for(&self, _: &CredentialRequest) -> Result<Credential, CredentialError> {
        Err(CredentialError::Unavailable)
    }
}

fn context(open: bool, config_root: &std::path::Path) -> Arc<ProjectContext> {
    Arc::new(ProjectContext {
        project: PathBuf::from("/no/such/project"),
        config_root: config_root.to_path_buf(),
        sessions: Arc::new(Sessions { open }),
        quarantines: Arc::new(Quarantines::default()),
        held: std::sync::Weak::new(),
    })
}

fn gate(context: Arc<ProjectContext>) -> CiPushGate {
    CiPushGate {
        context,
        proxy: Arc::new(ForgeProxy::new(
            Box::new(NoCredentials),
            Box::new(NoObjects),
        )),
        quarantines: Arc::new(Quarantines::default()),
        parent: PathBuf::from("/no/such/quarantine"),
    }
}

fn push_of(host: &str, repo: &str) -> PushRead {
    PushRead {
        host: host.to_owned(),
        port: 443,
        repo: repo.to_owned(),
        updates: vec![PushUpdate {
            old: "0".repeat(40),
            new: "1".repeat(40),
            name: "refs/heads/work".to_owned(),
        }],
        pack: Vec::new(),
    }
}

fn said(verdict: PushVerdict) -> String {
    match verdict {
        PushVerdict::Refuse(why) => why,
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn a_press_acts_for_a_chat_only_while_its_session_is_open() {
    let home = tempfile::tempdir().unwrap();
    let repo = Repo::try_from("acme/app").unwrap();

    let closed = context(false, home.path()).world(7, &repo).err();

    assert_eq!(closed.as_deref(), Some("this chat is not open"));
}

#[test]
fn a_push_of_a_chat_that_is_not_open_is_not_sent() {
    let home = tempfile::tempdir().unwrap();
    let gate = gate(context(false, home.path()));

    let why = said(gate.check(&ChatId::new("chat"), &push_of("github.com", "acme/app")));

    assert!(why.contains("not open"), "{why}");
}

#[test]
fn a_push_to_a_repo_nothing_confirms_is_not_sent_whatever_host_it_names() {
    let home = tempfile::tempdir().unwrap();
    let gate = gate(context(true, home.path()));

    for host in ["github.com", "gitlab.com"] {
        let why = said(gate.check(&ChatId::new("chat"), &push_of(host, "acme/app")));

        assert!(why.contains("not a repo confirmed"), "{host}: {why}");
    }
}
