// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! OB-15 (#687): every third-party call a feature makes is listed in the local network log, and
//! a no-account run lists no Charter address but the updater's.
//!
//! The forge client (FW-2a) is the feature the acceptance names. Its two transports are both
//! checked: the native one against a recorded forge on loopback, and the CLI one against the
//! stand-in `gh` and `glab` (`support/forge_cli.rs`). Both run in a child of this binary with a
//! `HOME` of its own, so the network log lands in a scratch machine store and never in the
//! operator's.
//!
//! What a line may hold is checked as well as what it must: never a token, a query, a body or
//! the names in a path.

use crate::support;

use std::sync::Arc;

use purlis_core::forge::http::{ApiRoot, TokenSource};
use purlis_core::forge::route::{HostScope, Resolver, SignIn};
use purlis_core::forge::{Account, Caller, ForgeError, Kind, Owner};
use purlis_core::netlog::{self, Feature, Party, Via};
use secrecy::SecretString;
use support::forge_cli::{Scene, in_a_child, in_child};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

const CANARY: &str = "CANARYnetlogToken91fe";

struct Canary;
impl TokenSource for Canary {
    fn token(&self) -> Result<SecretString, ForgeError> {
        Ok(SecretString::from(CANARY))
    }
}

fn account() -> Account {
    Account {
        kind: Kind::GitHub,
        host: "github.com".into(),
        login: "octocat".into(),
    }
}

/// Every line in this run's network log, as text.
fn log_text() -> String {
    let root = purlis_core::machine::config_root().expect("a config root");
    let dir = netlog::dir(&root);
    let mut text = String::new();
    for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        text.push_str(&std::fs::read_to_string(entry.path()).unwrap_or_default());
    }
    text
}

fn entries() -> Vec<netlog::Entry> {
    netlog::entries(&purlis_core::machine::config_root().expect("a config root"))
}

/// A mock forge no other test in this process can share, and the calls listed to it (#1929).
///
/// `MockServer::start` lends a server from wiremock's process-wide pool, and a test that ends
/// hands its server back: the next test to start one can get the same port, and with it the
/// line the first test left under that address. A server from the builder is bound for this
/// test alone, so no other test holds its port while it lives; and the lines already listed
/// under its address when it starts, an earlier server's that the OS gave the same port, are
/// not this test's.
struct OwnServer {
    server: MockServer,
    authority: String,
    before: usize,
}

impl OwnServer {
    fn start(rt: &tokio::runtime::Runtime) -> OwnServer {
        let server = rt.block_on(MockServer::builder().start());
        let authority = server.uri().trim_start_matches("http://").to_string();
        let before = Self::listed_to(&authority).count();
        OwnServer {
            server,
            authority,
            before,
        }
    }

    fn listed_to(authority: &str) -> impl Iterator<Item = netlog::Entry> + '_ {
        entries()
            .into_iter()
            .filter(move |e| e.via == Via::Https && e.host == authority)
    }

    /// The native calls listed to this server since it started, oldest first.
    fn calls(&self) -> Vec<netlog::Entry> {
        Self::listed_to(&self.authority).skip(self.before).collect()
    }
}

/// A git network call to a remote given by its name, in a child and a log of its own (#1022).
#[test]
fn a_git_call_to_a_remote_by_name_is_listed_in_a_child_of_its_own() {
    purlis_core::unsteered!();
    in_a_child(
        "every_third_party_call_is_listed_in_the_network_log::by_name::",
        "bin",
    );
}

mod by_name {
    use super::*;

    #[test]
    fn a_fetch_from_a_remote_by_name_is_listed_with_the_host_its_url_names() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let ran = purlis_core::forklock::output(
                std::process::Command::new("git")
                    .arg("-C")
                    .arg(dir.path())
                    .args(args),
            )
            .expect("git runs");
            assert!(ran.status.success(), "git {args:?}: {ran:?}");
        };
        git(&["init", "-q"]);
        git(&[
            "remote",
            "add",
            "origin",
            "https://netlog-byname.invalid/hidden-owner/hidden-repo.git",
        ]);
        // No host answers `.invalid`: what is checked is the line the call leaves.
        let run = purlis_core::worktree::git::run_network(dir.path(), None, &["fetch", "--prune"])
            .expect("git starts");
        assert!(!run.ok(), "{run:?}");

        // The push beside it runs in this child too: only this remote's lines are its.
        let listed: Vec<_> = entries()
            .into_iter()
            .filter(|e| e.feature == Feature::Git && e.host == "netlog-byname.invalid")
            .collect();
        assert_eq!(listed.len(), 1, "{listed:?}\n{}", log_text());
        assert_eq!(listed[0].method.as_str(), "FETCH");
        let text = log_text();
        for hidden in ["hidden-owner", "hidden-repo"] {
            assert!(
                !text.contains(hidden),
                "the network log holds {hidden}: {text}"
            );
        }
    }

    /// A push goes where the remote's push URL says, which a remote can set apart from the URL
    /// it fetches from: the line names that host, not the fetch's (#1828).
    #[test]
    fn a_push_to_a_remote_by_name_is_listed_with_the_host_it_pushes_to() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let ran = purlis_core::forklock::output(
                std::process::Command::new("git")
                    .arg("-C")
                    .arg(dir.path())
                    .args(args),
            )
            .expect("git runs");
            assert!(ran.status.success(), "git {args:?}: {ran:?}");
        };
        git(&["init", "-q"]);
        git(&[
            "remote",
            "add",
            "origin",
            "https://netlog-fetch.invalid/hidden-owner/hidden-repo.git",
        ]);
        git(&[
            "config",
            "remote.origin.pushurl",
            "https://netlog-push.invalid/hidden-owner/hidden-repo.git",
        ]);
        git(&[
            "-c",
            "user.name=Netlog",
            "-c",
            "user.email=netlog@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "one",
        ]);
        // No host answers `.invalid`: what is checked is the line the call leaves.
        let run = purlis_core::worktree::git::run_network(
            dir.path(),
            None,
            &["push", "origin", "HEAD:refs/heads/pushed"],
        )
        .expect("git starts");
        assert!(!run.ok(), "{run:?}");

        let hosts: Vec<_> = entries()
            .into_iter()
            .filter(|e| e.feature == Feature::Git && e.host.starts_with("netlog-"))
            .filter(|e| e.host != "netlog-byname.invalid" && !e.host.starts_with("netlog-nameless"))
            .map(|e| (e.host, e.method))
            .collect();
        assert_eq!(
            hosts,
            [("netlog-push.invalid".to_owned(), "PUSH".to_owned())],
            "{}",
            log_text()
        );
    }

    /// A push that names no remote goes to the branch's push remote, which the fetch's default
    /// never reads (#1853): the line names that remote's host.
    #[test]
    fn a_push_naming_no_remote_is_listed_with_the_host_of_the_remote_git_pushes_to() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let ran = purlis_core::forklock::output(
                std::process::Command::new("git")
                    .arg("-C")
                    .arg(dir.path())
                    .args(args),
            )
            .expect("git runs");
            assert!(ran.status.success(), "git {args:?}: {ran:?}");
        };
        git(&["init", "-q", "-b", "main"]);
        git(&[
            "remote",
            "add",
            "origin",
            "https://netlog-nameless-fetch.invalid/hidden-owner/hidden-repo.git",
        ]);
        git(&[
            "remote",
            "add",
            "mirror",
            "https://netlog-nameless-push.invalid/hidden-owner/hidden-repo.git",
        ]);
        git(&["config", "branch.main.remote", "origin"]);
        git(&["config", "branch.main.pushRemote", "mirror"]);
        git(&[
            "-c",
            "user.name=Netlog",
            "-c",
            "user.email=netlog@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "one",
        ]);
        // No host answers `.invalid`: what is checked is the line the call leaves.
        let run = purlis_core::worktree::git::run_network(dir.path(), None, &["push"])
            .expect("git starts");
        assert!(!run.ok(), "{run:?}");

        let hosts: Vec<_> = entries()
            .into_iter()
            .filter(|e| e.feature == Feature::Git && e.host.starts_with("netlog-nameless"))
            .map(|e| (e.host, e.method))
            .collect();
        assert_eq!(
            hosts,
            [("netlog-nameless-push.invalid".to_owned(), "PUSH".to_owned())],
            "{}",
            log_text()
        );
    }
}

/// `purlis report`'s calls, in a child and a log of its own: its sign-ins are handed over for the
/// whole process, which no other case here may see (#1873).
#[test]
fn a_reports_calls_are_listed_in_a_child_of_its_own() {
    purlis_core::unsteered!();
    in_a_child(
        "every_third_party_call_is_listed_in_the_network_log::report::",
        "bin",
    );
}

mod report {
    use super::*;

    /// `purlis report`'s calls over the native transport are listed as the report's, not a
    /// forge client's (#1873): the search and the filing, each under `Feature::Report`, through
    /// the backend `report::as_the_reporter` hands the person in the window.
    #[test]
    fn a_reports_native_calls_are_listed_as_the_reports() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        use purlis_core::forge::accounts::{self, Pasted};
        use purlis_core::forge::recorded::Recorded;
        use purlis_core::forge::signin;
        use purlis_core::forge::transport::Transport;
        use wiremock::matchers::{method, path};

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let own = OwnServer::start(&rt);
        let server = &own.server;
        rt.block_on(async {
            Mock::given(method("GET"))
                .and(path("/search/issues"))
                .respond_with(ResponseTemplate::new(200).set_body_string(
                    r#"{"total_count": 0, "incomplete_results": false, "items": []}"#,
                ))
                .mount(server)
                .await;
            Mock::given(method("POST"))
                .and(path("/repos/purlis/purlis/issues"))
                .respond_with(ResponseTemplate::new(201).set_body_string(
                    r#"{"id": 5042, "node_id": "I_kwDOPurlis42", "number": 42,
                        "title": "The picker crashes", "state": "open", "labels": [],
                        "html_url": "https://github.com/purlis/purlis/issues/42"}"#,
                ))
                .mount(server)
                .await;
        });
        let home = tempfile::tempdir().unwrap();
        let check = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/forge_contract/accounts/github/token_check.json"),
        )
        .unwrap();
        let check = Arc::new(Recorded::parse(&check).unwrap());
        let reach = move |_: &purlis_core::forge::Forge,
                          _: Arc<dyn TokenSource>|
              -> Result<Arc<dyn Transport>, ForgeError> {
            Ok(check.clone() as Arc<dyn Transport>)
        };
        let keyring = accounts::keyring_at(home.path());
        accounts::sign_in_with_token(
            &HostScope::for_a_test(),
            home.path(),
            keyring.as_ref(),
            &reach,
            Kind::GitHub,
            "github.com",
            Pasted::new("pasted-for-the-netlog-report".to_string()),
        )
        .expect("signed in");
        signin::hand_over_at_root_for_a_test(
            HostScope::for_a_test(),
            home.path(),
            ApiRoot::at(&server.uri()),
            Arc::new(purlis_core::forge::cli::Cli::default()),
        );
        let caller = Caller::window();
        let draft = purlis_core::report::Draft {
            kind: purlis_core::report::Kind::Bug,
            title: "The picker crashes".into(),
            body: "Open the picker.".into(),
            scrubbed: Vec::new(),
        };
        let tracker = purlis_core::report::as_the_reporter(&caller).expect("signed in");
        purlis_core::report::search_duplicates(tracker.as_ref(), &caller, &draft)
            .expect("searched");
        purlis_core::report::file(tracker.as_ref(), &caller, &draft).expect("filed");
        signin::take_back();

        let native: Vec<_> = own
            .calls()
            .into_iter()
            .map(|e| (e.feature, e.method))
            .collect();
        assert!(
            native.iter().any(|(_, method)| method == "GET")
                && native.iter().any(|(_, method)| method == "POST")
                && native
                    .iter()
                    .all(|(feature, _)| *feature == Feature::Report),
            "{native:?}\n{}",
            log_text()
        );
    }
}

/// A review's fetch of a request's head, in a child and a log of its own (#704): it is a git
/// network call, which the cases above count.
#[test]
fn a_reviews_fetch_is_listed_in_a_child_of_its_own() {
    purlis_core::unsteered!();
    in_a_child(
        "every_third_party_call_is_listed_in_the_network_log::review::",
        "bin",
    );
}

mod review {
    use super::*;

    #[test]
    fn a_requests_head_fetched_for_its_review_is_listed_as_a_git_fetch_naming_no_repo() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let plane = std::fs::canonicalize(dir.path()).unwrap();
        let clone = plane.join("workspaces/alpha/thing");
        std::fs::create_dir_all(&clone).unwrap();
        let ran = purlis_core::forklock::output(
            std::process::Command::new("git")
                .arg("-C")
                .arg(&clone)
                .args(["init", "-q"]),
        )
        .expect("git runs");
        assert!(ran.status.success(), "git init: {ran:?}");
        // No host answers `.invalid`: what is checked is the line the fetch leaves.
        let fetched = purlis_core::files::review_fetch::fetch(
            &plane,
            "alpha",
            "thing",
            "https://netlog-review.invalid/hidden-owner/hidden-repo.git",
            "refs/pull/7/head",
            "pr-7",
            None,
        );
        assert!(fetched.is_err(), "{fetched:?}");

        let git: Vec<_> = entries()
            .into_iter()
            .filter(|e| e.feature == Feature::Git)
            .collect();
        assert_eq!(git.len(), 1, "{git:?}\n{}", log_text());
        let one = &git[0];
        assert_eq!(
            (
                one.host.as_str(),
                one.via,
                one.to,
                one.method.as_str(),
                one.path.as_str()
            ),
            (
                "netlog-review.invalid",
                Via::Https,
                Party::ThirdParty,
                "FETCH",
                ""
            )
        );
        let text = log_text();
        for hidden in [
            "hidden-owner",
            "hidden-repo",
            "refs/pull",
            "purlis/review",
            "pr-7",
        ] {
            assert!(
                !text.contains(hidden),
                "the network log holds {hidden}: {text}"
            );
        }
    }
}

#[test]
fn every_forge_call_is_listed_in_a_child_with_a_store_of_its_own() {
    purlis_core::unsteered!();
    in_a_child(
        "every_third_party_call_is_listed_in_the_network_log::listed::",
        "bin",
    );
}

mod listed {
    use super::*;

    #[test]
    fn a_native_forge_call_is_listed_with_its_host_method_path_and_status() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let own = OwnServer::start(&rt);
        let server = &own.server;
        rt.block_on(
            Mock::given(any())
                .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
                .mount(server),
        );
        let resolver = Resolver::new(Kind::GitHub, "github.com")
            .at_root(ApiRoot::at(&server.uri()))
            .signed_in(
                &HostScope::for_a_test(),
                account(),
                SignIn {
                    tokens: Arc::new(Canary),
                    imported_from_cli: false,
                },
            );
        let backend =
            purlis_core::forge::Forge::default_of(Kind::GitHub).backend_for(Arc::new(resolver));
        let me = Caller::window().as_account(account());
        let _ = backend.owned(&me, &Owner::new("secret-org-name"));

        let native = own.calls();
        assert!(
            !native.is_empty(),
            "the native call is not listed:\n{}",
            log_text()
        );
        let one = &native[0];
        assert_eq!(one.feature, Feature::Forge);
        assert_eq!(one.to, Party::ThirdParty);
        assert_eq!(one.method, "GET");
        assert_eq!(one.status, Some(200));
        assert!(
            !one.path.contains("secret-org-name") && !one.path.contains('?'),
            "the path keeps a name or a query: {}",
            one.path
        );
        let text = log_text();
        assert!(!text.contains(CANARY), "the network log holds the token");
        assert!(
            !text.contains("secret-org-name"),
            "the network log names the owner"
        );
    }

    #[test]
    fn a_native_gitlab_call_is_listed_with_its_nested_group_masked() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let own = OwnServer::start(&rt);
        let server = &own.server;
        rt.block_on(
            Mock::given(any())
                .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
                .mount(server),
        );
        let account = Account {
            kind: Kind::GitLab,
            host: "gitlab.com".into(),
            login: "octocat".into(),
        };
        let resolver = Resolver::new(Kind::GitLab, "gitlab.com")
            .at_root(ApiRoot::at(&server.uri()))
            .signed_in(
                &HostScope::for_a_test(),
                account.clone(),
                SignIn {
                    tokens: Arc::new(Canary),
                    imported_from_cli: false,
                },
            );
        let backend =
            purlis_core::forge::Forge::default_of(Kind::GitLab).backend_for(Arc::new(resolver));
        let me = Caller::window().as_account(account);
        let _ = backend.owned(&me, &Owner::new("secret-group/api"));

        let native = own.calls();
        assert_eq!(native.len(), 1, "{native:?}\n{}", log_text());
        let one = &native[0];
        assert_eq!(
            (one.feature, one.to, one.method.as_str(), one.status),
            (Feature::Forge, Party::ThirdParty, "GET", Some(200))
        );
        assert_eq!(one.path, "groups/{}/projects");
        let text = log_text();
        assert!(!text.contains(CANARY), "the network log holds the token");
        assert!(
            !text.contains("secret-group"),
            "the network log names the group"
        );
    }

    #[test]
    fn a_forge_call_through_the_cli_is_listed_with_its_host() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let scene = Scene::new("netlog-cli.example");
        scene.gh_api("orgs/hidden-team/repos?per_page=100&page=1", 0, "[]", "");
        let _ = scene
            .forge("github")
            .backend()
            .owned(&Caller::command(), &Owner::new("hidden-team"));

        let listed: Vec<_> = entries()
            .into_iter()
            .filter(|e| e.host == "netlog-cli.example")
            .collect();
        assert_eq!(listed.len(), 1, "{listed:?}\n{}", log_text());
        let one = &listed[0];
        assert_eq!(
            (one.feature, one.via, one.to),
            (Feature::Forge, Via::Gh, Party::ThirdParty)
        );
        assert_eq!(one.path, "orgs/{}/repos");
        assert!(!log_text().contains("hidden-team"));
    }

    #[test]
    fn a_git_network_call_is_listed_with_its_host_and_operation_and_nothing_of_the_repo() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        // No host answers `.invalid`, so the call fails without reaching anyone: what is
        // checked is the line it leaves, and what that line leaves out.
        let dir = tempfile::tempdir().unwrap();
        let url =
            "https://someone:pw-under-test@netlog-git.invalid:8443/hidden-owner/hidden-repo.git";
        let run = purlis_core::worktree::git::run_network(
            dir.path(),
            None,
            &["ls-remote", "--heads", url, "refs/heads/hidden-branch"],
        )
        .expect("git starts");
        assert!(!run.ok(), "{run:?}");
        // A call that reaches no host, a folder given as the remote, is no network call.
        let _ = purlis_core::worktree::git::run_network(
            dir.path(),
            None,
            &[
                "ls-remote",
                &dir.path().join("hidden-folder").display().to_string(),
            ],
        );

        let git: Vec<_> = entries()
            .into_iter()
            .filter(|e| e.feature == Feature::Git)
            .collect();
        assert_eq!(git.len(), 1, "{git:?}\n{}", log_text());
        let one = &git[0];
        assert_eq!(
            (one.host.as_str(), one.via, one.to, one.method.as_str()),
            (
                "netlog-git.invalid:8443",
                Via::Https,
                Party::ThirdParty,
                "LS-REMOTE"
            )
        );
        assert_eq!(
            (one.path.as_str(), one.status, one.answered),
            ("", None, false)
        );
        let text = log_text();
        for hidden in [
            "hidden-owner",
            "hidden-repo",
            "hidden-branch",
            "hidden-folder",
            "someone",
            "pw-under-test",
        ] {
            assert!(
                !text.contains(hidden),
                "the network log holds {hidden}: {text}"
            );
        }
    }

    #[test]
    fn a_no_account_run_lists_no_charter_address_but_the_updaters() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        // The forge work a run does with no account: an auth check and a listing, both through
        // the CLI (`gh`), as the two lines below expect. The updater's own check is the app's,
        // and `updates::tests::the_real_check_lists_its_read_in_the_network_log` drives it.
        let scene = Scene::new("netlog-run.example");
        scene.answers(
            "gh",
            &["auth", "status", "--hostname", "netlog-run.example"],
            0,
            "",
            "",
        );
        scene.gh_api("orgs/acme/repos?per_page=100&page=1", 0, "[]", "");
        let forge = scene.forge("github");
        let _ = forge.check_auth();
        let _ = forge
            .backend()
            .owned(&Caller::command(), &Owner::new("acme"));

        let all: Vec<_> = entries()
            .into_iter()
            .filter(|e| e.host == "netlog-run.example")
            .collect();
        assert_eq!(all.len(), 2, "the run is not listed whole: {all:?}");
        // By host, not by path: a template keeps no names, so the host is what says where a
        // call went. Charter's files and tracker are on GitHub's hosts, so any line to one that
        // is not the updater's is a call to Charter this run had no business making.
        for e in entries() {
            assert!(
                !is_githubs(&e.host) || e.feature == Feature::Updater,
                "a run without an account reached a GitHub host: {e:?}"
            );
            assert_ne!(e.to, Party::Charter, "{e:?}");
        }
    }
}

/// GitHub's hosts, where Charter's release files and tracker are.
fn is_githubs(host: &str) -> bool {
    let host = host.split(':').next().unwrap_or_default();
    host == "github.com"
        || host == "api.github.com"
        || host == "githubusercontent.com"
        || host.ends_with(".githubusercontent.com")
        || host.ends_with(".github.com")
}

#[test]
fn githubs_hosts_are_told_from_the_rest() {
    purlis_core::unsteered!();
    for host in [
        "github.com",
        "api.github.com",
        "objects.githubusercontent.com",
        "github.com:443",
    ] {
        assert!(is_githubs(host), "{host}");
    }
    for host in [
        "netlog-run.example",
        "127.0.0.1:8080",
        "notgithub.com",
        "ghe.example.com",
    ] {
        assert!(!is_githubs(host), "{host}");
    }
}
