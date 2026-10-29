// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Get a project from your forge, from a list** (#1818, spec #1810): the repos each signed-in
//! account reaches, purlis projects on top, against the `forge_contract` recordings
//! (`tests/forge_contract/repolist/`).
//!
//! Each case signs in with a token through the account store, hands the sign-ins over as the app
//! does at start, and lists what the window lists. The native route is the recording, by forge;
//! the CLI route stands for a machine with neither `gh` nor `glab` and counts every ask, so a
//! case that reached for the CLI fails. Every recorded exchange must be asked for, and nothing
//! else: a probe too many is an unrecorded request, and fails the count.
//!
//! What is asserted is what the person sees: which repos, in which order, which are marked as
//! purlis projects, what each host says when it has no account or would not list, and that the
//! manifest probes stay inside the account's request budget.
//!
//! The sign-ins are this process's, so the cases take turns.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use purlis_core::forge::accounts::{self, Pasted};
use purlis_core::forge::http::TokenSource;
use purlis_core::forge::recorded::Recorded;
use purlis_core::forge::route::HostScope;
use purlis_core::forge::signin;
use purlis_core::forge::transport::{Call, NoAnswer, Reply, Transport};
use purlis_core::forge::{Forge, ForgeError, Kind};
use purlis_core::repolist::{self, Listing, Manifest};

/// The token every case pastes. Not shaped like any forge's token: the commit hook scans for
/// those.
const TOKEN: &str = "pasted-for-the-recorded-forge";

/// Every test here hands this process's sign-ins over, which is the whole process's state: each
/// runs in a process of its own ([`crate::alone`], #3613), and this orders them where nextest
/// or a re-run puts several in one.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn recording(path: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/forge_contract")
            .join(path),
    )
    .unwrap()
}

/// The CLI route of a machine with neither CLI: every ask refused, and counted.
#[derive(Default)]
struct NoCli(AtomicUsize);
impl Transport for NoCli {
    fn send(&self, _forge: &Forge, _call: &Call) -> Result<Reply, NoAnswer> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(NoAnswer::Missing("purlis could not find gh on PATH".into()))
    }
    fn check_auth(&self, _forge: &Forge) -> Result<(), ForgeError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(ForgeError::transport("purlis could not find gh on PATH"))
    }
}

/// The native route: each forge's recording, answering only a request sent with the signed-in
/// token, and counting each request it was sent.
struct Native {
    tokens: Arc<dyn TokenSource>,
    github: Arc<Recorded>,
    gitlab: Arc<Recorded>,
    sent: Arc<AtomicUsize>,
}
impl Transport for Native {
    fn send(&self, forge: &Forge, call: &Call) -> Result<Reply, NoAnswer> {
        let token = self
            .tokens
            .token()
            .map_err(|e| NoAnswer::Refused(e.to_string()))?;
        assert_eq!(
            secrecy::ExposeSecret::expose_secret(&token),
            TOKEN,
            "the request's token"
        );
        self.sent.fetch_add(1, Ordering::SeqCst);
        match forge.kind {
            Kind::GitHub => self.github.send(forge, call),
            Kind::GitLab => self.gitlab.send(forge, call),
        }
    }
    fn check_auth(&self, forge: &Forge) -> Result<(), ForgeError> {
        self.github.check_auth(forge)
    }
}

struct Machine {
    _turn: MutexGuard<'static, ()>,
    home: tempfile::TempDir,
    github: Arc<Recorded>,
    gitlab: Arc<Recorded>,
    cli: Arc<NoCli>,
    sent: Arc<AtomicUsize>,
}

/// Signs in to `kind` on its default host with the token, the forge answering `check`.
fn sign_in(home: &Path, kind: Kind, check: &str) {
    let check = Arc::new(Recorded::parse(&recording(check)).unwrap());
    let reach =
        move |_: &Forge, _: Arc<dyn TokenSource>| -> Result<Arc<dyn Transport>, ForgeError> {
            Ok(check.clone() as Arc<dyn Transport>)
        };
    let keyring = accounts::keyring_at(home);
    accounts::sign_in_with_token(
        &HostScope::for_a_test(),
        home,
        keyring.as_ref(),
        &reach,
        kind,
        kind.default_host(),
        Pasted::new(TOKEN.to_string()),
    )
    .expect("signed in");
}

/// The recordings by their text: an empty one answers nothing.
fn parsed(text: &str) -> Arc<Recorded> {
    Arc::new(Recorded::parse(text).unwrap())
}

const NOTHING: &str = r#"{"source": "nothing is asked of this forge", "exchanges": []}"#;

impl Machine {
    /// A machine signed in as `logins` (each a token-check recording), whose forges answer
    /// `github` and `gitlab` (recordings' text), its sign-ins handed over as the app hands them.
    fn new(logins: &[(Kind, &str)], github: &str, gitlab: &str) -> Machine {
        let turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let home = tempfile::tempdir().unwrap();
        for (kind, check) in logins {
            sign_in(home.path(), *kind, check);
        }
        let (github, gitlab) = (parsed(github), parsed(gitlab));
        let cli = Arc::new(NoCli::default());
        let sent = Arc::new(AtomicUsize::new(0));
        let (gh, gl, counted) = (github.clone(), gitlab.clone(), sent.clone());
        signin::hand_over_for_a_test(
            HostScope::for_a_test(),
            home.path(),
            Arc::new(move |tokens| {
                Arc::new(Native {
                    tokens,
                    github: gh.clone(),
                    gitlab: gl.clone(),
                    sent: counted.clone(),
                }) as Arc<dyn Transport>
            }),
            cli.clone(),
        );
        Machine {
            _turn: turn,
            home,
            github,
            gitlab,
            cli,
            sent,
        }
    }

    /// Every recorded exchange was asked for, over the native route, and the CLI never was.
    fn all_native(&self) {
        assert!(
            self.github.unspent().is_empty(),
            "{:?}",
            self.github.unspent()
        );
        assert!(
            self.gitlab.unspent().is_empty(),
            "{:?}",
            self.gitlab.unspent()
        );
        assert_eq!(self.cli.0.load(Ordering::SeqCst), 0, "the CLI was asked");
    }

    fn sent(&self) -> usize {
        self.sent.load(Ordering::SeqCst)
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        signin::take_back();
    }
}

const GITHUB: (Kind, &str) = (Kind::GitHub, "accounts/github/token_check.json");
const GITLAB: (Kind, &str) = (Kind::GitLab, "accounts/gitlab/token_check.json");

/// The listing's rows as `path [project]`, in order.
fn rows(listing: &Listing) -> Vec<String> {
    listing
        .repos
        .iter()
        .map(|r| match r.manifest {
            Manifest::Found => format!("{} [project]", r.path),
            Manifest::Absent => r.path.clone(),
            Manifest::Unknown => format!("{} [?]", r.path),
        })
        .collect()
}

#[test]
fn every_signed_in_account_lists_what_it_reaches_with_projects_on_top_and_no_cli() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new(
        &[GITHUB, GITLAB],
        &recording("repolist/github/reachable.json"),
        &recording("repolist/gitlab/reachable.json"),
    );
    let listing = repolist::list();
    assert_eq!(
        rows(&listing),
        [
            // The manifest under either name marks a project; the projects come first.
            "acme/team-project [project]",
            "grp/sub/handbook [project]",
            "octocat/dots [project]",
            "acme/api",
            "grp/api",
            // An empty repo has no tree to probe: it is listed, unmarked, and not called absent.
            "octocat/empty [?]",
        ]
    );
    machine.all_native();

    let handbook = &listing.repos[1];
    assert_eq!(handbook.owner, "grp/sub");
    assert_eq!(handbook.name, "handbook");
    assert_eq!(handbook.host, "gitlab.com");
    assert_eq!(handbook.login, "jdoe");
    assert_eq!(handbook.url, "https://gitlab.com/grp/sub/handbook");
    assert_eq!(listing.repos[0].kind, Kind::GitHub);
    assert_eq!(listing.repos[0].login, "octocat");

    // One row per account, each saying how many it listed; no host is missing a sign-in.
    let reaches: Vec<(String, Option<String>, usize, Option<String>)> = listing
        .reaches
        .iter()
        .map(|r| (r.host.clone(), r.login.clone(), r.listed, r.trouble.clone()))
        .collect();
    assert_eq!(
        reaches,
        [
            ("github.com".into(), Some("octocat".into()), 4, None),
            ("gitlab.com".into(), Some("jdoe".into()), 2, None),
        ]
    );
}

#[test]
fn with_no_account_nothing_is_asked_and_each_forge_says_sign_in_or_paste_a_url() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new(&[], NOTHING, NOTHING);
    let listing = repolist::list();
    assert!(listing.repos.is_empty());
    assert_eq!(machine.sent(), 0);
    machine.all_native();
    let hosts: Vec<(&str, bool)> = listing
        .reaches
        .iter()
        .map(|r| (r.host.as_str(), r.login.is_none()))
        .collect();
    assert_eq!(hosts, [("github.com", true), ("gitlab.com", true)]);
    for reach in &listing.reaches {
        let said = reach.trouble.as_deref().unwrap();
        assert!(said.contains("Sign in"), "{said}");
        assert!(said.contains("paste"), "{said}");
    }
}

#[test]
fn a_forge_with_no_account_says_so_beside_the_one_that_lists() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new(
        &[GITHUB],
        &recording("repolist/github/reachable.json"),
        NOTHING,
    );
    let listing = repolist::list();
    assert_eq!(listing.repos.len(), 4);
    machine.all_native();
    let gitlab = listing
        .reaches
        .iter()
        .find(|r| r.host == "gitlab.com")
        .unwrap();
    assert_eq!(gitlab.login, None);
    assert!(gitlab.trouble.as_deref().unwrap().contains("gitlab.com"));
}

#[test]
fn a_listing_the_forge_refuses_names_the_fix_and_the_other_forge_still_lists() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new(
        &[GITHUB, GITLAB],
        &recording("repolist/github/reachable.refused.json"),
        &recording("repolist/gitlab/reachable.json"),
    );
    let listing = repolist::list();
    assert_eq!(rows(&listing), ["grp/sub/handbook [project]", "grp/api"]);
    machine.all_native();
    let github = &listing.reaches[0];
    assert_eq!(github.listed, 0);
    let said = github.trouble.as_deref().unwrap();
    assert!(said.contains("github.com"), "{said}");
    assert!(said.contains("Sign in again"), "{said}");
    assert!(!said.contains(TOKEN), "{said}");
}

#[test]
fn two_accounts_on_one_host_each_list_as_themselves_and_a_shared_repo_is_listed_once() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new(
        &[
            GITHUB,
            (Kind::GitHub, "repolist/github/token_check.hubot.json"),
        ],
        &recording("repolist/github/two_accounts.json"),
        NOTHING,
    );
    let listing = repolist::list();
    assert_eq!(
        rows(&listing),
        ["acme/team-project [project]", "acme/api", "hubot/bot"]
    );
    machine.all_native();
    let logins: Vec<&str> = listing.repos.iter().map(|r| r.login.as_str()).collect();
    assert_eq!(logins, ["octocat", "octocat", "hubot"]);
    let listed: Vec<(Option<&str>, usize)> = listing
        .reaches
        .iter()
        .filter(|r| r.host == "github.com")
        .map(|r| (r.login.as_deref(), r.listed))
        .collect();
    assert_eq!(listed, [(Some("octocat"), 2), (Some("hubot"), 2)]);
}

/// A GitHub listing of `count` repos, `acme/r0`.., and a tree probe for each of the first
/// `probed`, none of them a project.
fn many(count: usize, probed: usize) -> String {
    let repos: Vec<serde_json::Value> = (0..count)
        .map(|i| {
            serde_json::json!({
                "id": i, "name": format!("r{i}"), "full_name": format!("acme/r{i}"),
                "default_branch": "main", "description": null,
                "html_url": format!("https://github.com/acme/r{i}"),
                "ssh_url": format!("git@github.com:acme/r{i}.git"), "topics": []
            })
        })
        .collect();
    let exchange = |path: String, out: String| {
        serde_json::json!({
            "call": {"endpoint": {"rest": {"method": null, "path": path}}, "fields": []},
            "reply": {"code": 0, "out": out, "err": "", "status": 200}
        })
    };
    let mut exchanges = vec![exchange(
        "user/repos?affiliation=owner,collaborator,organization_member&per_page=100&page=1".into(),
        serde_json::to_string(&repos).unwrap(),
    )];
    for i in 0..probed {
        exchanges.push(exchange(
            format!("repos/acme/r{i}/git/trees/main"),
            r#"{"sha": "e5bd3914e2e596debea16f433f57875b5b90bcd6", "tree": [{"path": "README.md", "type": "blob"}], "truncated": false}"#.into(),
        ));
    }
    serde_json::json!({"source": "generated: a long GitHub listing", "exchanges": exchanges})
        .to_string()
}

#[test]
fn a_long_list_probes_at_most_its_share_of_repos_and_lists_every_one() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let count = repolist::PROBES_PER_ACCOUNT + 10;
    let machine = Machine::new(
        &[GITHUB],
        &many(count, repolist::PROBES_PER_ACCOUNT),
        NOTHING,
    );
    let listing = repolist::list();
    assert_eq!(listing.repos.len(), count);
    // One listing request, and one probe for each repo of the share: never one per repo.
    assert_eq!(machine.sent(), 1 + repolist::PROBES_PER_ACCOUNT);
    machine.all_native();
    let unknown = listing
        .repos
        .iter()
        .filter(|r| r.manifest == Manifest::Unknown)
        .count();
    assert_eq!(unknown, 10, "the repos past the share are not probed");
}

#[test]
fn once_the_accounts_hour_is_spent_the_list_still_comes_and_no_probe_is_sent() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let machine = Machine::new(&[GITHUB], &many(3, 0), NOTHING);
    // The account's hour, spent: what purlis allows itself for background work is used up.
    let account = purlis_core::forge::Account {
        kind: Kind::GitHub,
        host: "github.com".into(),
        login: "octocat".into(),
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let budget = purlis_core::forge::budget::root(machine.home.path());
    std::fs::create_dir_all(&budget).unwrap();
    std::fs::write(
        budget.join(format!("{}.json", account.key())),
        serde_json::json!({
            // The meter names the account itself as it reads the file.
            "account": "",
            "since": now,
            "sent": purlis_core::forge::budget::HOURLY_ALLOWANCE,
            "counted": purlis_core::forge::budget::HOURLY_ALLOWANCE,
            "not_modified": 0,
            "held_back": 0
        })
        .to_string(),
    )
    .unwrap();
    let listing = repolist::list();
    // The person is waiting on the list, so it is sent; the probes are not.
    assert_eq!(listing.repos.len(), 3);
    assert_eq!(machine.sent(), 1);
    assert!(
        listing
            .repos
            .iter()
            .all(|r| r.manifest == Manifest::Unknown)
    );
    machine.all_native();
}
