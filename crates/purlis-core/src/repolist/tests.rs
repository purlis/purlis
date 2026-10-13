// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::forge::accounts::Method;

fn account(kind: Kind, host: &str, login: &str) -> ForgeAccount {
    ForgeAccount {
        id: "01ACCOUNT".into(),
        kind,
        host: host.into(),
        login: login.into(),
        method: Method::Pat,
        state: State::SignedIn,
        scopes: None,
    }
}

fn record(path: &str, web: &str, ssh: &str) -> RepoRecord {
    RepoRecord {
        id: None,
        name: path.rsplit('/').next().unwrap().into(),
        path_with_namespace: path.into(),
        default_branch: Some("main".into()),
        description: String::new(),
        web_url: web.into(),
        ssh_url: ssh.into(),
        topics: Vec::new(),
        forge: Kind::GitLab,
    }
}

fn row(path: &str, host: &str, manifest: Manifest) -> Listed {
    listed_as(
        &account(Kind::GitHub, host, "me"),
        &record(path, &format!("https://{host}/{path}"), ""),
        manifest,
    )
}

#[test]
fn projects_come_first_then_paths_in_order_whatever_their_case() {
    let mut repos = vec![
        row("zed/a", "github.com", Manifest::Absent),
        row("Acme/b", "github.com", Manifest::Unknown),
        row("me/plane", "github.com", Manifest::Found),
        row("acme/a", "github.com", Manifest::Absent),
        row("acme/a", "git.example.com", Manifest::Absent),
        row("Big/project", "github.com", Manifest::Found),
    ];
    order(&mut repos);
    let shown: Vec<(&str, &str)> = repos
        .iter()
        .map(|r| (r.path.as_str(), r.host.as_str()))
        .collect();
    assert_eq!(
        shown,
        [
            ("Big/project", "github.com"),
            ("me/plane", "github.com"),
            ("acme/a", "git.example.com"),
            ("acme/a", "github.com"),
            ("Acme/b", "github.com"),
            ("zed/a", "github.com"),
        ]
    );
}

#[test]
fn a_subgroups_repo_names_its_whole_group_as_owner() {
    let listed = row("grp/sub/handbook", "gitlab.com", Manifest::Found);
    assert_eq!(listed.owner, "grp/sub");
    assert_eq!(listed.name, "handbook");
    assert_eq!(listed.url, "https://gitlab.com/grp/sub/handbook");
}

#[test]
fn a_repo_whose_page_is_not_https_is_cloned_over_ssh() {
    let plain = record(
        "team/api",
        "http://git.example.com/team/api",
        "git@git.example.com:team/api.git",
    );
    assert_eq!(clone_url(&plain), "git@git.example.com:team/api.git");
    let https = record(
        "team/api",
        "https://git.example.com/team/api",
        "git@git.example.com:team/api.git",
    );
    assert_eq!(clone_url(&https), "https://git.example.com/team/api");
    // With no SSH address there is nothing better: the clone says why the page will not do.
    assert_eq!(
        clone_url(&record("team/api", "http://git.example.com/team/api", "")),
        "http://git.example.com/team/api"
    );
}

#[test]
fn every_listing_failure_names_its_host_and_offers_the_url_box() {
    let me = account(Kind::GitHub, "github.com", "octocat");
    for failure in [
        Failure::Auth,
        Failure::RateLimited { reset: None },
        Failure::Forbidden,
        Failure::Transport,
        Failure::Unrecognised,
    ] {
        let said = could_not_list(&me, &ForgeError::of(failure.clone(), "the forge's words."));
        assert!(said.contains("github.com"), "{failure:?}: {said}");
        assert!(said.contains("octocat"), "{failure:?}: {said}");
        assert!(
            said.contains("paste a project's URL"),
            "{failure:?}: {said}"
        );
        assert!(!said.contains(".."), "{failure:?}: {said}");
    }
    assert!(could_not_list(&me, &ForgeError::of(Failure::Auth, "x")).contains("Sign in again"));
}

#[test]
fn a_host_with_no_account_says_sign_in_and_one_whose_sign_in_ended_says_so() {
    let never = no_account(Kind::GitLab, "gitlab.com", None);
    assert!(
        never.starts_with("Not signed in to gitlab.com. Sign in"),
        "{never}"
    );
    assert!(never.contains("GitLab repos"), "{never}");
    let mut ended = account(Kind::GitLab, "gitlab.com", "jdoe");
    ended.state = State::SignedOut;
    let said = no_account(Kind::GitLab, "gitlab.com", Some(&ended));
    assert!(said.contains("as jdoe has ended. Sign in again"), "{said}");
}

#[test]
fn with_no_account_the_list_asks_nothing() {
    let listing = list_as(&[], &|_| panic!("no backend is built with no account"));
    assert!(listing.repos.is_empty());
    let hosts: Vec<&str> = listing.reaches.iter().map(|r| r.host.as_str()).collect();
    assert_eq!(hosts, ["github.com", "gitlab.com"]);
}
