// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::sync::Arc;

use super::*;
use crate::forge::accounts::Method;

fn scope() -> HostScope {
    HostScope::for_a_test()
}

fn account(id: &str, kind: Kind, host: &str, login: &str) -> ForgeAccount {
    ForgeAccount {
        id: id.to_string(),
        kind,
        host: host.to_string(),
        login: login.to_string(),
        method: Method::Pat,
        state: State::SignedIn,
        scopes: Some(vec!["repo".to_string()]),
        client_id: None,
    }
}

fn personal() -> ForgeAccount {
    account("01PERSONAL", Kind::GitHub, "github.com", "octocat")
}

/// An enterprise-managed account on the same host.
fn emu() -> ForgeAccount {
    account("01EMU", Kind::GitHub, "github.com", "octocat_acme")
}

/// What the forge says each repo's number is.
struct Numbers(HashMap<&'static str, u64>);

impl RepoLookup for Numbers {
    fn repo_id(&self, _account: &ForgeAccount, path: &str) -> Result<u64, String> {
        self.0
            .get(path)
            .copied()
            .ok_or_else(|| format!("{path} was not found on the forge as that account"))
    }
}

fn forge() -> Numbers {
    Numbers(HashMap::from([
        ("octocat/hello", 1001),
        ("acme/platform", 2002),
        ("acme/tools", 3003),
    ]))
}

fn record(
    home: &Path,
    clone: &Path,
    url: &str,
    who: &ForgeAccount,
) -> Result<ConfirmedRepo, String> {
    let client_id = application_of(home, who)?;
    record_clone(&scope(), home, clone, url, &forge(), who, &client_id)
}

#[test]
fn a_clone_from_the_window_records_the_host_the_path_the_numeric_id_the_account_and_the_client() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    let repo = record(
        home.path(),
        clone.path(),
        "https://github.com/OctoCat/Hello.git",
        &personal(),
    )
    .unwrap();
    assert_eq!(repo.host, "github.com");
    assert_eq!(repo.path, "octocat/hello");
    assert_eq!(repo.repo_id, 1001);
    assert_eq!(repo.account, "01PERSONAL");
    // A pasted token was granted by no application.
    assert_eq!(repo.client_id, "");
    assert_eq!(repos_of_clone(home.path(), clone.path()).unwrap(), [repo]);
}

#[test]
fn a_clone_nobody_confirmed_has_no_repo_in_the_list() {
    let home = tempfile::tempdir().unwrap();
    let from_the_cli = tempfile::tempdir().unwrap();
    assert!(
        repos_of_clone(home.path(), from_the_cli.path())
            .unwrap()
            .is_empty()
    );
    let other = tempfile::tempdir().unwrap();
    record(
        home.path(),
        other.path(),
        "git@github.com:octocat/hello.git",
        &personal(),
    )
    .unwrap();
    // The same repo cloned somewhere else is not served until it is trusted too.
    assert!(
        repos_of_clone(home.path(), from_the_cli.path())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn the_repo_is_the_one_the_forge_numbers_so_a_rename_is_the_same_repo_and_a_lookalike_is_not() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    record(
        home.path(),
        clone.path(),
        "https://github.com/acme/platform",
        &emu(),
    )
    .unwrap();
    let ask = |path: &str, id: u64| {
        admission(
            home.path(),
            clone.path(),
            Kind::GitHub,
            "github.com",
            path,
            id,
        )
        .unwrap()
    };
    assert!(matches!(
        ask("acme/platform", 2002),
        Admission::Confirmed(r) if r.repo_id == 2002
    ));
    // Renamed or transferred: another path, the same number.
    assert!(matches!(
        ask("acme-new/platform", 2002),
        Admission::Alias(r) if r.path == "acme/platform"
    ));
    // The recorded path, now another repo's: a lookalike.
    assert_eq!(ask("acme/platform", 9999), Admission::NotConfirmed);
    assert_eq!(ask("acme/other", 4004), Admission::NotConfirmed);
    assert_eq!(ask("../etc", 2002), Admission::NotConfirmed);
    // Another host's repo with the same number is not this one.
    assert_eq!(
        admission(
            home.path(),
            clone.path(),
            Kind::GitHub,
            "ghe.example.com",
            "acme/platform",
            2002
        )
        .unwrap(),
        Admission::NotConfirmed
    );
}

#[test]
fn a_personal_and_an_enterprise_account_on_one_host_serve_different_projects() {
    let home = tempfile::tempdir().unwrap();
    let hello = tempfile::tempdir().unwrap();
    let platform = tempfile::tempdir().unwrap();
    let both = vec![personal(), emu()];
    let a = record(
        home.path(),
        hello.path(),
        "https://github.com/octocat/hello",
        &personal(),
    )
    .unwrap();
    let b = record(
        home.path(),
        platform.path(),
        "https://github.com/acme/platform",
        &emu(),
    )
    .unwrap();
    assert_eq!(
        account_for(home.path(), &a, &both),
        Serving::Account(personal())
    );
    assert_eq!(account_for(home.path(), &b, &both), Serving::Account(emu()));
    // A press changes which serves a project, and only that project.
    let changed = choose_account(
        &scope(),
        home.path(),
        &key_of_clone(platform.path()),
        Kind::GitHub,
        "github.com",
        2002,
        &personal(),
    )
    .unwrap();
    assert_eq!(
        account_for(home.path(), &changed, &both),
        Serving::Account(personal())
    );
    let still = repos_of_clone(home.path(), hello.path()).unwrap();
    assert_eq!(
        account_for(home.path(), &still[0], &both),
        Serving::Account(personal())
    );
}

#[test]
fn a_chosen_account_that_is_signed_out_is_never_replaced_by_the_other_one() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    let repo = record(
        home.path(),
        clone.path(),
        "https://github.com/acme/platform",
        &emu(),
    )
    .unwrap();
    let mut signed_out = emu();
    signed_out.state = State::SignedOut;
    assert_eq!(
        account_for(home.path(), &repo, &[personal(), signed_out]),
        Serving::ChosenIsSignedOut {
            account_id: "01EMU".into()
        }
    );
    assert_eq!(
        account_for(home.path(), &repo, &[personal()]),
        Serving::ChosenIsSignedOut {
            account_id: "01EMU".into()
        }
    );
}

#[test]
fn one_account_on_a_host_is_the_default_and_asks_nothing_and_two_ask_the_person() {
    assert_eq!(default_account(vec![]), Pick::SignIn);
    assert_eq!(default_account(vec![personal()]), Pick::Only(personal()));
    assert_eq!(
        default_account(vec![personal(), emu()]),
        Pick::Choose(vec![personal(), emu()])
    );
}

#[test]
fn an_account_of_another_host_or_forge_or_a_signed_out_one_cannot_serve_a_repo() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    let gitlab = account("01GL", Kind::GitLab, "gitlab.com", "octocat");
    let ghes = account("01GHES", Kind::GitHub, "ghe.example.com", "octocat");
    for who in [&gitlab, &ghes] {
        let why = record(
            home.path(),
            clone.path(),
            "https://github.com/octocat/hello",
            who,
        )
        .unwrap_err();
        assert!(
            why.contains("signed in to") || why.contains("is on"),
            "{why}"
        );
    }
    let mut out = personal();
    out.state = State::SignedOut;
    assert!(
        record(
            home.path(),
            clone.path(),
            "https://github.com/octocat/hello",
            &out
        )
        .unwrap_err()
        .contains("signed out")
    );
    assert!(!path(home.path()).exists(), "nothing was recorded");
    record(
        home.path(),
        clone.path(),
        "https://github.com/octocat/hello",
        &personal(),
    )
    .unwrap();
    assert!(
        choose_account(
            &scope(),
            home.path(),
            &key_of_clone(clone.path()),
            Kind::GitHub,
            "github.com",
            1001,
            &ghes
        )
        .is_err()
    );
    assert!(
        choose_account(
            &scope(),
            home.path(),
            &key_of_clone(clone.path()),
            Kind::GitHub,
            "github.com",
            7,
            &personal()
        )
        .is_err()
    );
}

#[test]
fn a_remote_that_names_no_repo_or_one_the_forge_does_not_know_records_nothing() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    for url in [
        "https://github.com/octocat",
        "https://github.com/a/b/c",
        "https://github.com/../hello",
        "",
    ] {
        assert!(
            record(home.path(), clone.path(), url, &personal()).is_err(),
            "{url:?}"
        );
    }
    let why = record(
        home.path(),
        clone.path(),
        "https://github.com/octocat/missing",
        &personal(),
    )
    .unwrap_err();
    assert!(why.contains("not found"), "{why}");
    assert!(!path(home.path()).exists());
}

#[test]
fn a_ghes_host_with_a_port_is_matched_whole() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    let ghes = account("01GHES", Kind::GitHub, "ghe.example.com:8443", "octocat");
    let ok = record_clone(
        &scope(),
        home.path(),
        clone.path(),
        "https://ghe.example.com:8443/acme/platform.git",
        &forge(),
        &ghes,
        "Iv1.pinned",
    )
    .unwrap();
    assert_eq!(ok.host, "ghe.example.com:8443");
    // The same server on its default port is another host.
    assert!(
        record_clone(
            &scope(),
            home.path(),
            clone.path(),
            "https://ghe.example.com/acme/tools",
            &forge(),
            &ghes,
            "Iv1.pinned",
        )
        .is_err()
    );
}

#[test]
fn a_file_that_cannot_be_read_is_an_error_never_an_empty_list() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    let at = path(home.path());
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(&at, "not json").unwrap();
    assert!(repos_of_clone(home.path(), clone.path()).is_err());
    assert!(
        record(
            home.path(),
            clone.path(),
            "https://github.com/octocat/hello",
            &personal()
        )
        .is_err()
    );
    assert_eq!(std::fs::read_to_string(&at).unwrap(), "not json");
}

#[test]
fn an_entry_this_purlis_cannot_use_is_skipped_and_kept_when_the_file_is_written() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    let at = path(home.path());
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(
        &at,
        r#"{"version":1,"repos":[{"clone":"/x","kind":"forgejo","host":"h.example.com",
            "path":"a/b","repo_id":1,"account":"A","client_id":"c"}],"note":"kept"}"#,
    )
    .unwrap();
    assert!(
        repos_of_clone(home.path(), Path::new("/x"))
            .unwrap()
            .is_empty()
    );
    record(
        home.path(),
        clone.path(),
        "https://github.com/octocat/hello",
        &personal(),
    )
    .unwrap();
    let text = std::fs::read_to_string(&at).unwrap();
    assert!(
        text.contains("forgejo") && text.contains("\"note\""),
        "{text}"
    );
}

#[test]
fn forgetting_a_clone_takes_its_repos_away_and_no_other() {
    let home = tempfile::tempdir().unwrap();
    let one = tempfile::tempdir().unwrap();
    let two = tempfile::tempdir().unwrap();
    record(
        home.path(),
        one.path(),
        "https://github.com/octocat/hello",
        &personal(),
    )
    .unwrap();
    record(
        home.path(),
        two.path(),
        "https://github.com/acme/tools",
        &emu(),
    )
    .unwrap();
    forget_clone(&scope(), home.path(), one.path()).unwrap();
    assert!(repos_of_clone(home.path(), one.path()).unwrap().is_empty());
    assert_eq!(repos_of_clone(home.path(), two.path()).unwrap().len(), 1);
}

fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| (*v).to_string())
    }
}

#[test]
fn a_headless_runner_serves_the_repos_its_token_connection_names() {
    let got = headless_repos(
        &scope(),
        Kind::GitHub,
        &env(&[(
            "PURLIS_FORGE_REPOS",
            "github.com/Acme/Platform, ghe.example.com/acme/tools",
        )]),
        Some("https://github.com/ignored/when-listed"),
    )
    .unwrap();
    assert_eq!(
        got,
        [
            ConfiguredRepo {
                host: "github.com".into(),
                path: "acme/platform".into(),
                repo_id: None,
            },
            ConfiguredRepo {
                host: "ghe.example.com".into(),
                path: "acme/tools".into(),
                repo_id: None,
            },
        ]
    );
}

#[test]
fn a_headless_runner_with_no_list_serves_the_one_repo_of_its_checkout() {
    let got = headless_repos(
        &scope(),
        Kind::GitLab,
        &env(&[]),
        Some("git@git.example.com:group/sub/proj.git"),
    )
    .unwrap();
    assert_eq!(
        got,
        [ConfiguredRepo {
            host: "git.example.com".into(),
            path: "group/sub/proj".into(),
            repo_id: None,
        }]
    );
}

#[test]
fn a_headless_runner_fails_closed_on_a_list_or_a_checkout_it_cannot_read() {
    for list in [
        "github.com/acme",
        "github.com/acme/a/b",
        "acme/platform",
        "github.com/../x,",
        ",",
    ] {
        assert!(
            headless_repos(
                &scope(),
                Kind::GitHub,
                &env(&[("PURLIS_FORGE_REPOS", list)]),
                None
            )
            .is_err(),
            "{list:?}"
        );
    }
    assert!(headless_repos(&scope(), Kind::GitHub, &env(&[]), None).is_err());
    assert!(headless_repos(&scope(), Kind::GitHub, &env(&[]), Some("")).is_err());
    assert!(
        headless_repos(
            &scope(),
            Kind::GitHub,
            &env(&[]),
            Some("https://github.com/only-owner")
        )
        .is_err()
    );
    // A blank variable is no list: the checkout answers.
    assert!(
        headless_repos(
            &scope(),
            Kind::GitHub,
            &env(&[("PURLIS_FORGE_REPOS", "  ")]),
            Some("https://github.com/acme/platform")
        )
        .is_ok()
    );
}

#[test]
fn what_a_press_confirmed_and_what_a_runner_names_are_reconciled_to_their_intersection() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    let confirmed = vec![
        record(
            home.path(),
            clone.path(),
            "https://github.com/acme/platform",
            &emu(),
        )
        .unwrap(),
        record(
            home.path(),
            clone.path(),
            "https://github.com/acme/tools",
            &emu(),
        )
        .unwrap(),
    ];
    let configured = headless_repos(
        &scope(),
        Kind::GitHub,
        &env(&[(
            "PURLIS_FORGE_REPOS",
            "github.com/acme/platform,github.com/acme/never-confirmed",
        )]),
        None,
    )
    .unwrap();
    let served = reconcile(&confirmed, &configured);
    assert_eq!(served.len(), 1);
    assert_eq!(served[0].path, "acme/platform");
}

#[test]
fn a_path_is_lower_cased_stripped_of_dot_git_and_never_climbs() {
    assert_eq!(
        normalised_path(Kind::GitHub, "/Acme/Platform.git/").as_deref(),
        Some("acme/platform")
    );
    assert_eq!(
        normalised_path(Kind::GitLab, "G/Sub/Proj").as_deref(),
        Some("g/sub/proj")
    );
    for bad in ["a", "a/b/c", "a/..", "a/b c", "a//b", ""] {
        assert_eq!(normalised_path(Kind::GitHub, bad), None, "{bad:?}");
    }
    assert_eq!(normalised_path(Kind::GitLab, "a"), None);
}

#[test]
fn a_repo_is_served_only_when_the_clone_is_trusted_and_its_id_is_confirmed() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    let ask = || {
        serves(
            home.path(),
            clone.path(),
            Kind::GitHub,
            "github.com",
            "acme/platform",
            2002,
        )
        .unwrap()
    };
    // Trusted, with no id read from the forge yet: not served.
    crate::forge::gitcredential::remote::record(
        home.path(),
        clone.path(),
        "github.com/acme/platform".to_string(),
    )
    .unwrap();
    assert_eq!(ask(), Admission::NotConfirmed);
    // Confirmed too: served.
    record(
        home.path(),
        clone.path(),
        "https://github.com/acme/platform",
        &emu(),
    )
    .unwrap();
    assert!(matches!(ask(), Admission::Confirmed(_)));
    // Another folder with no trust of its own is not served, whatever this one holds.
    let elsewhere = tempfile::tempdir().unwrap();
    assert_eq!(
        serves(
            home.path(),
            elsewhere.path(),
            Kind::GitHub,
            "github.com",
            "acme/platform",
            2002
        )
        .unwrap(),
        Admission::NotConfirmed
    );
}

#[test]
fn a_confirmed_id_on_a_clone_nobody_trusts_is_not_served() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    record(
        home.path(),
        clone.path(),
        "https://github.com/acme/platform",
        &emu(),
    )
    .unwrap();
    assert_eq!(
        serves(
            home.path(),
            clone.path(),
            Kind::GitHub,
            "github.com",
            "acme/platform",
            2002
        )
        .unwrap(),
        Admission::NotConfirmed
    );
}

/// A forge that answers each request from a table of path to answer, and keeps the paths asked.
struct Answers {
    table: HashMap<String, (i32, String)>,
    asked: std::sync::Mutex<Vec<String>>,
    tokens: std::sync::Mutex<Vec<String>>,
    token: Arc<dyn TokenSource>,
}

impl crate::forge::transport::Transport for Answers {
    fn send(
        &self,
        _forge: &Forge,
        call: &Call,
    ) -> Result<crate::forge::transport::Reply, crate::forge::transport::NoAnswer> {
        use secrecy::ExposeSecret;
        self.asked.lock().unwrap().push(call.path().to_string());
        self.tokens
            .lock()
            .unwrap()
            .push(self.token.token().unwrap().expose_secret().to_string());
        let (code, out) = self
            .table
            .get(call.path())
            .cloned()
            .unwrap_or((1, String::new()));
        Ok(crate::forge::transport::Reply::of(code, out, String::new()))
    }

    fn check_auth(&self, _forge: &Forge) -> Result<(), ForgeError> {
        Ok(())
    }
}

#[test]
fn the_window_reads_a_repos_number_from_the_forge_as_the_account_with_its_own_token() {
    let home = tempfile::tempdir().unwrap();
    let keyring = accounts::keyring_at(home.path());
    let who = personal();
    keyring
        .set(&accounts::service(&who), "token", "canaryPERSONALtoken")
        .unwrap();
    let seen = Arc::new(std::sync::Mutex::new(None::<Arc<Answers>>));
    let kept = seen.clone();
    let reach = move |_: &Forge, tokens: Arc<dyn TokenSource>| {
        let answers = Arc::new(Answers {
            table: HashMap::from([
                (
                    "repos/octocat/hello".to_string(),
                    (0, r#"{"id": 1001}"#.to_string()),
                ),
                ("repos/octocat/gone".to_string(), (1, String::new())),
                (
                    "repos/octocat/odd".to_string(),
                    (0, r#"{"id": "x"}"#.to_string()),
                ),
            ]),
            asked: Default::default(),
            tokens: Default::default(),
            token: tokens,
        });
        *kept.lock().unwrap() = Some(answers.clone());
        Ok(answers as Arc<dyn crate::forge::transport::Transport>)
    };
    let lookup = AsThePerson {
        keyring: keyring.as_ref(),
        reach: &reach,
    };
    assert_eq!(lookup.repo_id(&who, "octocat/hello"), Ok(1001));
    let answers = seen.lock().unwrap().clone().unwrap();
    assert_eq!(*answers.asked.lock().unwrap(), ["repos/octocat/hello"]);
    // Asked as the account, with the account's own token and no other.
    assert_eq!(*answers.tokens.lock().unwrap(), ["canaryPERSONALtoken"]);
    let gone = lookup.repo_id(&who, "octocat/gone").unwrap_err();
    assert!(
        gone.contains("did not show octocat/gone to octocat"),
        "{gone}"
    );
    assert!(!gone.contains("canary"), "{gone}");
    assert!(lookup.repo_id(&who, "octocat/odd").is_err());
}

#[test]
fn a_gitlab_project_is_asked_by_its_encoded_path_and_a_signed_out_account_asks_nothing() {
    let home = tempfile::tempdir().unwrap();
    let keyring = accounts::keyring_at(home.path());
    let who = account("01GL", Kind::GitLab, "git.example.com", "octocat");
    let asked = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let log = asked.clone();
    let reach = move |_: &Forge, tokens: Arc<dyn TokenSource>| {
        let answers = Arc::new(Answers {
            table: HashMap::from([(
                "projects/group%2Fsub%2Fproj".to_string(),
                (0, r#"{"id": 77}"#.to_string()),
            )]),
            asked: Default::default(),
            tokens: Default::default(),
            token: tokens,
        });
        log.lock().unwrap().push("reached".into());
        Ok(answers as Arc<dyn crate::forge::transport::Transport>)
    };
    let lookup = AsThePerson {
        keyring: keyring.as_ref(),
        reach: &reach,
    };
    // No token in the keyring: signed out, and the forge is never reached.
    assert!(
        lookup
            .repo_id(&who, "group/sub/proj")
            .unwrap_err()
            .contains("signed out")
    );
    assert!(asked.lock().unwrap().is_empty());
    keyring
        .set(&accounts::service(&who), "token", "glpat-canary")
        .unwrap();
    assert_eq!(lookup.repo_id(&who, "group/sub/proj"), Ok(77));
}

#[test]
fn a_trusted_clone_is_confirmed_from_the_address_the_trust_list_holds() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    // Not trusted: nothing to read.
    assert!(
        confirm_trusted(&scope(), home.path(), clone.path(), &forge(), &emu(), "")
            .unwrap_err()
            .contains("not trusted yet")
    );
    crate::forge::gitcredential::remote::record(
        home.path(),
        clone.path(),
        "github.com/Acme/Platform".to_string(),
    )
    .unwrap();
    let repo = confirm_trusted(&scope(), home.path(), clone.path(), &forge(), &emu(), "").unwrap();
    assert_eq!((repo.path.as_str(), repo.repo_id), ("acme/platform", 2002));
    assert_eq!(all(home.path()).unwrap(), [repo]);
}

#[test]
fn a_runner_that_says_the_number_of_its_repo_is_served_only_a_repo_with_that_number() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    let confirmed = vec![
        record(
            home.path(),
            clone.path(),
            "https://github.com/acme/platform",
            &emu(),
        )
        .unwrap(),
    ];
    let said = |id: &str| {
        headless_repos(
            &scope(),
            Kind::GitHub,
            &env(&[
                ("PURLIS_FORGE_REPOS", "github.com/acme/platform"),
                ("GITHUB_REPOSITORY", "Acme/Platform"),
                ("GITHUB_REPOSITORY_ID", id),
            ]),
            None,
        )
        .unwrap()
    };
    let same = said("2002");
    assert_eq!(same[0].repo_id, Some(2002));
    assert_eq!(reconcile(&confirmed, &same)[0].repo_id, 2002);
    // The same path with another number is a lookalike: not served.
    assert!(reconcile(&confirmed, &said("999")).is_empty());
}

#[test]
fn choosing_an_account_names_the_repo_by_its_forge_and_host_too() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    record(
        home.path(),
        clone.path(),
        "https://github.com/octocat/hello",
        &personal(),
    )
    .unwrap();
    let key = key_of_clone(clone.path());
    let choose = |kind, host| choose_account(&scope(), home.path(), &key, kind, host, 1001, &emu());
    assert!(choose(Kind::GitLab, "github.com").is_err());
    assert!(choose(Kind::GitHub, "ghe.example.com").is_err());
    assert!(choose(Kind::GitHub, "GitHub.com").is_ok());
}

#[test]
fn a_repo_recorded_with_no_account_is_listed_and_served_by_none() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    let repo = record_unchosen(
        &scope(),
        home.path(),
        clone.path(),
        &Candidate {
            kind: Kind::GitHub,
            host: "github.com".into(),
            path: "octocat/hello".into(),
            repo_id: 1001,
        },
    )
    .unwrap();
    assert_eq!(all(home.path()).unwrap(), std::slice::from_ref(&repo));
    assert_eq!(
        account_for(home.path(), &repo, &[personal(), emu()]),
        Serving::Unchosen
    );
}
