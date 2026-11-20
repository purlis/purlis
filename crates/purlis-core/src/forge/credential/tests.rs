// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use super::renewal::{LEAD, LONGEST_SLEEP, Outcome, RETRY, Renewal};
use super::*;
use crate::forge::accounts::{self, State};
use crate::forge::device::tests::StandIn;
use crate::forge::oauth::Endpoints;
use crate::forge::refresh::tests::{Machine, WhoAmI, machine};
use crate::forge::refresh::{AHEAD, Refresher};
use crate::forge::registered;
use crate::forge::registration;
use crate::forge::route::HostScope;

const RENEWED: &str = r#"{"access_token": "second-access-value", "scope": "repo,read:org",
    "expires_in": 28800, "refresh_token": "second-refresh-value"}"#;

fn scope() -> HostScope {
    HostScope::for_a_test()
}

/// The refresher of `machine`, renewing through `forge` at the endpoints of whatever
/// registration its account's host resolves to.
fn refresher(machine: &Machine, forge: &Arc<StandIn>) -> Refresher {
    let registration = registration::resolve(
        machine.home.path(),
        machine.account.kind,
        &machine.account.host,
    )
    .unwrap()
    .expect("a registration for the account's host");
    Refresher::through(
        &scope(),
        machine.home.path(),
        forge.clone(),
        Endpoints::of(registration),
    )
}

fn signed_in(machine: &Machine, forge: &Arc<StandIn>) -> SignedIn {
    SignedIn::through(refresher(machine, forge), machine.home.path())
}

fn renewal(machine: &Machine, forge: &Arc<StandIn>) -> Renewal {
    Renewal::through(refresher(machine, forge), machine.home.path())
}

fn register(machine: &Machine) {
    let client_id = match machine.account.kind {
        Kind::GitHub => "Iv1.ghes-client",
        Kind::GitLab => "gl-user-owned-app",
    };
    registered::register(
        &scope(),
        machine.home.path(),
        machine.account.kind,
        &machine.account.host,
        client_id,
    )
    .unwrap();
}

// ---------------------------------------------------------------------------------------
// The interactive sign-in.

#[test]
fn a_sign_in_answers_for_its_own_host_with_its_own_token_and_for_no_other() {
    let machine = machine(Kind::GitHub, "github.com", accounts::Method::Device);
    let source = signed_in(&machine, &Arc::new(StandIn::answering(&[])));
    let now = machine.expires_at - LEAD.as_secs() - 1;

    let got = source.credential_at("GitHub.com", now).unwrap();
    assert_eq!(got.expose(), "first-access-value");
    assert_eq!(got.kind(), Kind::GitHub);
    assert_eq!(got.host(), "github.com");
    assert_eq!(got.source(), Source::SignedIn);
    assert_eq!(got.token_kind(), TokenKind::SignIn);
    assert_eq!(got.login(), Some("octocat"));
    assert_eq!(got.expiry(now), Expiry::Renewed);
    assert_eq!(got.expires_at(), Some(machine.expires_at));

    for other in ["ghe.example.com", "gitlab.com", "github.com.evil.net"] {
        let why = source.credential_at(other, now).unwrap_err();
        assert!(why.is_none_given(), "{other}: {why}");
    }
}

#[test]
fn gitlab_com_answers_with_its_own_grant() {
    let machine = machine(Kind::GitLab, "gitlab.com", accounts::Method::Pkce);
    let source = signed_in(&machine, &Arc::new(StandIn::answering(&[])));
    let got = source.credential_at("gitlab.com", machine.expires_at - LEAD.as_secs() - 1);
    let got = got.unwrap();
    assert_eq!(
        (got.kind(), got.token_kind()),
        (Kind::GitLab, TokenKind::SignIn)
    );
}

#[test]
fn ghes_renews_with_the_client_id_registered_for_that_host_and_at_that_host() {
    let machine = machine(Kind::GitHub, "ghe.example.com", accounts::Method::Device);
    register(&machine);
    let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
    let source = signed_in(&machine, &forge);
    let got = source
        .credential_at("ghe.example.com", machine.expires_at - AHEAD.as_secs())
        .unwrap();
    assert_eq!(got.expose(), "second-access-value");
    let posted = forge.posted.lock().unwrap().clone();
    assert_eq!(posted.len(), 1);
    assert_eq!(
        posted[0].0,
        "https://ghe.example.com/login/oauth/access_token"
    );
    assert!(
        posted[0]
            .1
            .contains(&("client_id".to_string(), "Iv1.ghes-client".to_string()))
    );
    // Never a client secret: the registration is a public client.
    assert!(posted[0].1.iter().all(|(name, _)| name != "client_secret"));
}

#[test]
fn a_self_hosted_gitlab_renews_through_the_users_own_oauth_application() {
    let machine = machine(Kind::GitLab, "git.example.com", accounts::Method::Pkce);
    register(&machine);
    let forge = Arc::new(StandIn::answering(&[Ok((
        200,
        r#"{"access_token": "gl-second", "scope": "api", "expires_in": 7200,
            "refresh_token": "gl-second-refresh"}"#,
    ))]));
    let source = signed_in(&machine, &forge);
    let got = source
        .credential_at("git.example.com", machine.expires_at - AHEAD.as_secs())
        .unwrap();
    assert_eq!(got.expose(), "gl-second");
    let posted = forge.posted.lock().unwrap().clone();
    assert_eq!(posted[0].0, "https://git.example.com/oauth/token");
    assert!(
        posted[0]
            .1
            .contains(&("client_id".to_string(), "gl-user-owned-app".to_string()))
    );
}

#[test]
fn a_pasted_token_is_the_fallback_sign_in_and_is_long_lived() {
    let home = tempfile::tempdir().unwrap();
    let keyring = accounts::keyring_at(home.path());
    let reach = |_: &crate::forge::Forge,
                 _: Arc<dyn crate::forge::http::TokenSource>|
     -> Result<
        Arc<dyn crate::forge::transport::Transport>,
        crate::forge::ForgeError,
    > { Ok(Arc::new(WhoAmI)) };
    accounts::sign_in_with_token(
        &scope(),
        home.path(),
        keyring.as_ref(),
        &reach,
        Kind::GitHub,
        "ghe.example.com",
        accounts::Pasted::new("github_pat_pasted_value".to_string()),
    )
    .unwrap();
    let source = SignedIn::new(&scope(), home.path());
    let got = source.credential_at("ghe.example.com", 1_000).unwrap();
    assert_eq!(got.expose(), "github_pat_pasted_value");
    assert_eq!(got.token_kind(), TokenKind::Pasted);
    assert_eq!(got.expiry(1_000), Expiry::Unknown);
}

// ---------------------------------------------------------------------------------------
// Renewal, before the expiry and without the person.

#[test]
fn the_sweep_renews_a_token_with_less_than_the_lead_left_before_any_request_needs_it() {
    assert!(LEAD > AHEAD, "a sweep renews earlier than a request does");
    let machine = machine(Kind::GitHub, "github.com", accounts::Method::Device);
    let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
    let renewal = renewal(&machine, &forge);
    let now = machine.expires_at - LEAD.as_secs();

    let swept = renewal.sweep(now);
    assert_eq!(swept.len(), 1);
    assert_eq!(
        (swept[0].host.as_str(), swept[0].outcome),
        ("github.com", Outcome::Renewed)
    );
    assert_eq!(forge.posted.lock().unwrap().len(), 1);

    // The next request finds the renewed token and asks nothing.
    let source = signed_in(&machine, &forge);
    let got = source.credential_at("github.com", now + 5).unwrap();
    assert_eq!(got.expose(), "second-access-value");
    assert_eq!(forge.posted.lock().unwrap().len(), 1);
    assert_eq!(got.expires_at(), Some(now + 28800));
}

#[test]
fn the_sweep_asks_nothing_of_a_token_with_more_than_the_lead_left() {
    let machine = machine(Kind::GitHub, "github.com", accounts::Method::Device);
    let forge = Arc::new(StandIn::answering(&[]));
    let swept = renewal(&machine, &forge).sweep(machine.expires_at - LEAD.as_secs() - 1);
    assert_eq!(swept[0].outcome, Outcome::NotDue);
    assert!(forge.posted.lock().unwrap().is_empty());
}

/// The hosts the window was told about, as the app's hook is: by the host a test made unique.
static TOLD: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[test]
fn a_refused_renewal_gives_one_notice_and_no_credential_and_nothing_stands_in() {
    crate::forge::refresh::tell_when_ended(&scope(), |account| {
        TOLD.lock().unwrap().push(account.host.clone());
    });
    let host = "notice.example.com";
    let machine = machine(Kind::GitLab, host, accounts::Method::Pkce);
    register(&machine);
    let forge = Arc::new(StandIn::answering(&[Ok((
        400,
        r#"{"error": "invalid_grant"}"#,
    ))]));
    let renewal = renewal(&machine, &forge);
    let now = machine.expires_at - LEAD.as_secs();

    assert_eq!(renewal.sweep(now)[0].outcome, Outcome::Ended);

    // The sign-in is over: marked signed out, its token gone from the keyring.
    let listed = accounts::list(
        machine.home.path(),
        accounts::keyring_at(machine.home.path()).as_ref(),
    )
    .unwrap();
    assert_eq!(listed[0].state, State::SignedOut);
    assert!(machine.kept().is_none());

    // No credential, and the error says which host to sign in to again.
    let source = signed_in(&machine, &forge);
    let why = source.credential_at(host, now).unwrap_err();
    assert!(why.is_sign_in_again(), "{why:?}");
    assert_eq!(why.host(), host);
    assert!(why.to_string().contains(host) && why.to_string().contains("Sign in again"));
    assert!(!format!("{why:?}").contains("first-access-value"));
    assert!(!format!("{why:?}").contains("first-refresh-value"));

    // Later sweeps find it signed out: the forge is not asked, and the Notice is not raised again.
    assert!(renewal.sweep(now + 60).is_empty());
    assert!(renewal.sweep(now + 120).is_empty());
    assert_eq!(forge.posted.lock().unwrap().len(), 1);
    let told = TOLD.lock().unwrap().iter().filter(|h| *h == host).count();
    assert_eq!(told, 1, "exactly one Notice");
}

#[test]
fn a_forge_that_cannot_be_reached_ends_nothing_and_keeps_the_token_while_it_has_time() {
    let machine = machine(Kind::GitHub, "github.com", accounts::Method::Device);
    let forge = Arc::new(StandIn::answering(&[Err("offline")]));
    let renewal = renewal(&machine, &forge);
    let now = machine.expires_at - LEAD.as_secs();
    assert_eq!(renewal.sweep(now)[0].outcome, Outcome::Unreachable);
    let listed = accounts::list(
        machine.home.path(),
        accounts::keyring_at(machine.home.path()).as_ref(),
    )
    .unwrap();
    assert_eq!(listed[0].state, State::SignedIn);
    // And the token, still valid, is what a request gets meanwhile.
    let source = signed_in(&machine, &Arc::new(StandIn::answering(&[Err("offline")])));
    assert_eq!(
        source.credential_at("github.com", now).unwrap().expose(),
        "first-access-value"
    );
}

#[test]
fn the_next_wake_is_the_lead_before_the_earliest_end_within_the_limits() {
    let machine = machine(Kind::GitHub, "github.com", accounts::Method::Device);
    let renewal = renewal(&machine, &Arc::new(StandIn::answering(&[])));
    // Hours away: the longest sleep, so an account signed in meanwhile is found.
    assert_eq!(
        renewal.next_wake(machine.expires_at - 6 * 3600),
        LONGEST_SLEEP
    );
    // Closer than the longest sleep: exactly when the lead begins.
    let at = machine.expires_at - LEAD.as_secs() - 300;
    assert_eq!(
        renewal.next_wake(at),
        std::time::Duration::from_secs(300).max(RETRY)
    );
    // Due already: not a spin.
    assert_eq!(renewal.next_wake(machine.expires_at), RETRY);
    // Nothing to renew at all.
    let empty = tempfile::tempdir().unwrap();
    let none = Renewal::new(&scope(), empty.path());
    assert_eq!(none.next_wake(1_000), LONGEST_SLEEP);
    assert!(none.sweep(1_000).is_empty());
}

// ---------------------------------------------------------------------------------------
// The token connection.

fn connection(vars: &[(&str, &str)]) -> TokenConnection {
    let vars: HashMap<String, String> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    TokenConnection::reading(move |name| vars.get(name).cloned())
}

const NOW: u64 = 1_800_000_000;

fn got(vars: &[(&str, &str)], host: &str) -> Credential {
    connection(vars)
        .credential_at(host, NOW)
        .unwrap_or_else(|e| panic!("{host}: {e}"))
}

fn refused(vars: &[(&str, &str)], host: &str) -> CredentialError {
    connection(vars)
        .credential_at(host, NOW)
        .expect_err("a refusal")
}

#[test]
fn actions_github_token_is_an_installation_token_per_job_and_never_warned_about() {
    let c = got(&[("GITHUB_TOKEN", "ghs_installation")], "github.com");
    assert_eq!(
        (c.kind(), c.token_kind()),
        (Kind::GitHub, TokenKind::GitHubInstallation)
    );
    assert_eq!(c.source(), Source::TokenConnection);
    assert_eq!(c.from(), "GITHUB_TOKEN");
    assert_eq!(c.expiry(NOW), Expiry::PerJob);
    assert_eq!(c.login(), None);
}

#[test]
fn each_github_token_kind_is_read_from_its_shape() {
    for (token, kind, lifetime) in [
        ("ghp_classic", TokenKind::GitHubClassic, Lifetime::LongLived),
        (
            "github_pat_fine",
            TokenKind::GitHubFineGrained,
            Lifetime::LongLived,
        ),
        (
            "ghs_install",
            TokenKind::GitHubInstallation,
            Lifetime::PerJob,
        ),
        ("ghu_user", TokenKind::GitHubUser, Lifetime::PerJob),
        ("gho_oauth", TokenKind::GitHubUser, Lifetime::PerJob),
    ] {
        let c = got(&[("GH_TOKEN", token)], "github.com");
        assert_eq!(c.token_kind(), kind, "{token}");
        assert_eq!(c.token_kind().lifetime(), lifetime, "{token}");
    }
}

#[test]
fn an_ambient_variable_takes_only_a_token_of_a_known_shape_and_an_opaque_one_must_be_named() {
    // A CLI's own variable (what a CI job or a shell exports) is not an opt-in: a token of no
    // known prefix is refused. Naming it for the host is the opt-in.
    for vars in [
        vec![("GH_TOKEN", "0123456789abcdef")],
        vec![("GITLAB_TOKEN", "0123456789abcdef")],
    ] {
        let host = if vars[0].0 == "GH_TOKEN" {
            "github.com"
        } else {
            "gitlab.com"
        };
        assert!(
            matches!(refused(&vars, host), CredentialError::Unusable { .. }),
            "{vars:?}"
        );
    }
    let named = [("PURLIS_FORGE_TOKEN_GITHUB_COM", "0123456789abcdef")];
    let c = got(&named, "github.com");
    assert_eq!(c.token_kind(), TokenKind::Unrecognised);
    assert_eq!(c.token_kind().lifetime(), Lifetime::LongLived);
}

#[test]
fn each_gitlab_token_kind_is_read_from_its_shape_and_its_variable() {
    let c = got(&[("GITLAB_TOKEN", "glpat-abc")], "gitlab.com");
    assert_eq!(
        (c.kind(), c.token_kind()),
        (Kind::GitLab, TokenKind::GitLabAccess)
    );
    assert_eq!(c.expiry(NOW), Expiry::Unknown);

    // A CI job's token, whatever it looks like, on the host CI says it is.
    let job = [
        ("CI_JOB_TOKEN", "abcdef012345"),
        ("CI_SERVER_HOST", "git.example.com"),
    ];
    let c = got(&job, "git.example.com");
    assert_eq!(
        (c.kind(), c.token_kind()),
        (Kind::GitLab, TokenKind::GitLabJob)
    );
    assert_eq!(c.expiry(NOW), Expiry::PerJob);
    // And a `glcbt-` token is a job's by its prefix.
    let c = got(
        &[("PURLIS_FORGE_TOKEN_GIT_EXAMPLE_COM", "glcbt-job")],
        "git.example.com",
    );
    assert_eq!(c.token_kind(), TokenKind::GitLabJob);
}

#[test]
fn a_job_token_is_offered_only_to_the_host_ci_names() {
    let job = [
        ("CI_JOB_TOKEN", "abc"),
        ("CI_SERVER_HOST", "git.example.com"),
    ];
    assert!(refused(&job, "gitlab.com").is_none_given());
    assert!(refused(&job, "other.example.com").is_none_given());
}

#[test]
fn a_token_is_offered_only_to_the_host_its_variable_names() {
    let github = [("GITHUB_TOKEN", "ghs_a"), ("GH_TOKEN", "ghp_b")];
    for host in [
        "ghe.example.com",
        "gitlab.com",
        "github.com.evil.net",
        "api.github.com",
    ] {
        assert!(refused(&github, host).is_none_given(), "{host}");
    }
    // GHES: the enterprise variable, on the host `GH_HOST` names, and Actions' on its own server.
    let ghes = [
        ("GH_ENTERPRISE_TOKEN", "ghp_ent"),
        ("GH_HOST", "ghe.example.com"),
    ];
    assert_eq!(got(&ghes, "ghe.example.com").expose(), "ghp_ent");
    assert!(refused(&ghes, "github.com").is_none_given());
    assert!(refused(&ghes, "other.example.com").is_none_given());
    let actions = [
        ("GITHUB_TOKEN", "ghs_a"),
        ("GITHUB_SERVER_URL", "https://ghe.example.com"),
    ];
    assert_eq!(got(&actions, "ghe.example.com").expose(), "ghs_a");
    assert!(refused(&actions, "other.example.com").is_none_given());
    // A self-hosted GitLab's `GITLAB_TOKEN` is its `GITLAB_HOST`'s, never gitlab.com's.
    let gitlab = [
        ("GITLAB_TOKEN", "glpat-x"),
        ("GITLAB_HOST", "https://git.example.com/"),
    ];
    assert_eq!(got(&gitlab, "git.example.com").expose(), "glpat-x");
    assert!(refused(&gitlab, "gitlab.com").is_none_given());
}

#[test]
fn the_named_variable_for_a_host_wins_over_the_forge_clis_own() {
    let both = [
        ("PURLIS_FORGE_TOKEN_GITHUB_COM", "ghp_named"),
        ("GH_TOKEN", "ghp_cli"),
    ];
    let c = got(&both, "github.com");
    assert_eq!(
        (c.expose(), c.from()),
        ("ghp_named", "PURLIS_FORGE_TOKEN_GITHUB_COM")
    );
    let gh_first = [("GH_TOKEN", "ghp_gh"), ("GITHUB_TOKEN", "ghp_github")];
    assert_eq!(got(&gh_first, "github.com").expose(), "ghp_gh");
    // An explicit GITLAB_TOKEN is a choice; a job's own token is the fallback.
    let ci = [
        ("GITLAB_TOKEN", "glpat-chosen"),
        ("CI_JOB_TOKEN", "job"),
        ("CI_SERVER_HOST", "gitlab.com"),
    ];
    assert_eq!(got(&ci, "gitlab.com").expose(), "glpat-chosen");
}

#[test]
fn a_variable_that_is_set_and_unusable_is_an_error_and_the_next_one_is_not_tried() {
    let vars = [
        ("PURLIS_FORGE_TOKEN_GITHUB_COM", "has a space"),
        ("GH_TOKEN", "ghp_fine"),
    ];
    assert!(matches!(
        refused(&vars, "github.com"),
        CredentialError::Unusable { .. }
    ));
    // An empty variable is not set.
    let vars = [
        ("PURLIS_FORGE_TOKEN_GITHUB_COM", "  "),
        ("GH_TOKEN", "ghp_fine"),
    ];
    assert_eq!(got(&vars, "github.com").expose(), "ghp_fine");
}

#[test]
fn a_token_of_a_host_purlis_cannot_place_needs_its_forge_named() {
    let vars = [("PURLIS_FORGE_TOKEN_GHE_EXAMPLE_COM", "0123456789abcdef")];
    let why = refused(&vars, "ghe.example.com").to_string();
    assert!(why.contains("PURLIS_FORGE_KIND_GHE_EXAMPLE_COM"), "{why}");
    let hinted = [
        ("PURLIS_FORGE_TOKEN_GHE_EXAMPLE_COM", "0123456789abcdef"),
        ("PURLIS_FORGE_KIND_GHE_EXAMPLE_COM", "github"),
    ];
    assert_eq!(got(&hinted, "ghe.example.com").kind(), Kind::GitHub);
    // A prefix that says GitHub is enough.
    let by_prefix = [("PURLIS_FORGE_TOKEN_GHE_EXAMPLE_COM", "ghp_enterprise")];
    assert_eq!(got(&by_prefix, "ghe.example.com").kind(), Kind::GitHub);
    let bad = [
        ("PURLIS_FORGE_TOKEN_GHE_EXAMPLE_COM", "abc"),
        ("PURLIS_FORGE_KIND_GHE_EXAMPLE_COM", "forgejo"),
    ];
    assert!(matches!(
        refused(&bad, "ghe.example.com"),
        CredentialError::Unusable { .. }
    ));
}

#[test]
fn a_token_that_disagrees_with_its_variable_its_hint_or_its_host_is_refused_not_guessed_at() {
    for (vars, host) in [
        (vec![("GH_TOKEN", "glpat-gitlab")], "github.com"),
        (vec![("GITLAB_TOKEN", "ghp_github")], "gitlab.com"),
        (
            vec![("PURLIS_FORGE_TOKEN_GITHUB_COM", "glpat-gitlab")],
            "github.com",
        ),
        (
            vec![("PURLIS_FORGE_TOKEN_GITLAB_COM", "ghp_github")],
            "gitlab.com",
        ),
        (
            vec![
                ("PURLIS_FORGE_TOKEN_GIT_EXAMPLE_COM", "glpat-x"),
                ("PURLIS_FORGE_KIND_GIT_EXAMPLE_COM", "github"),
            ],
            "git.example.com",
        ),
    ] {
        let why = refused(&vars, host);
        assert!(
            matches!(why, CredentialError::Unusable { .. }),
            "{vars:?}: {why:?}"
        );
    }
}

#[test]
fn a_long_lived_token_with_a_known_expiry_is_warned_about_before_it_ends_and_refused_after() {
    let var = "PURLIS_FORGE_TOKEN_GITHUB_COM_EXPIRES_AT";
    let in_days = |days: u64| (NOW + days * 86_400).to_string();
    let with = |expires: &str| {
        connection(&[("GH_TOKEN", "github_pat_fine"), (var, expires)])
            .credential_at("github.com", NOW)
    };
    assert_eq!(with(&in_days(60)).unwrap().expiry(NOW), Expiry::Fine);
    match with(&in_days(3)).unwrap().expiry(NOW) {
        Expiry::Soon { left } => assert_eq!(left.as_secs(), 3 * 86_400),
        other => panic!("{other:?}"),
    }
    // The boundary is inclusive: exactly two weeks left is already a warning.
    assert!(matches!(
        with(&in_days(14)).unwrap().expiry(NOW),
        Expiry::Soon { .. }
    ));
    assert_eq!(with(&in_days(15)).unwrap().expiry(NOW), Expiry::Fine);
    // Ended: no credential, naming the variable that gave the token and not the token.
    let why = with(&(NOW - 1).to_string()).unwrap_err();
    assert!(matches!(why, CredentialError::Expired { .. }), "{why:?}");
    assert!(why.to_string().contains("GH_TOKEN") && !why.to_string().contains("github_pat_fine"));
    // RFC 3339 is read too, and a time that is no time is an error.
    let at = jiff::Timestamp::from_second(i64::try_from(NOW).unwrap() + 86_400).unwrap();
    assert!(matches!(
        with(&at.to_string()).unwrap().expiry(NOW),
        Expiry::Soon { .. }
    ));
    assert!(matches!(
        with("next tuesday"),
        Err(CredentialError::Unusable { .. })
    ));
}

#[test]
fn a_per_job_token_is_never_warned_about_whatever_its_expiry_says() {
    let var = "PURLIS_FORGE_TOKEN_GITHUB_COM_EXPIRES_AT";
    let c = got(
        &[("GITHUB_TOKEN", "ghs_job"), (var, &(NOW + 60).to_string())],
        "github.com",
    );
    assert_eq!(c.expiry(NOW), Expiry::PerJob);
}

#[test]
fn something_that_is_not_a_token_is_refused_without_repeating_it() {
    for bad in [
        "two words",
        "line\nbreak",
        &"x".repeat(5000),
        "tab\there",
        "nul\0byte",
        "é-accented",
    ] {
        let why = refused(&[("GH_TOKEN", bad)], "github.com");
        assert!(matches!(why, CredentialError::Unusable { .. }), "{bad:?}");
        let said = format!("{why} {why:?}");
        assert!(
            !said.contains(bad.trim()) || bad.trim().is_empty(),
            "{said}"
        );
    }
}

#[test]
fn a_token_in_a_mounted_file_is_read_trimmed_and_a_bad_file_says_so_without_its_content() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("token");
    std::fs::write(&file, "ghp_from_a_file\n").unwrap();
    let var = "PURLIS_FORGE_TOKEN_GITHUB_COM_FILE";
    let path = file.to_str().unwrap();
    let c = got(&[(var, path)], "github.com");
    assert_eq!(
        (c.expose(), c.token_kind()),
        ("ghp_from_a_file", TokenKind::GitHubClassic)
    );
    assert_eq!(c.from(), var);
    // The token itself, if both are set, wins over the file.
    let both = [("PURLIS_FORGE_TOKEN_GITHUB_COM", "ghp_direct"), (var, path)];
    assert_eq!(got(&both, "github.com").expose(), "ghp_direct");
    // A path with nothing at it, a directory, and a file too large for a token.
    let missing = dir.path().join("none");
    let large = dir.path().join("large");
    std::fs::write(&large, "x".repeat(20_000)).unwrap();
    for bad in [missing.as_path(), dir.path(), large.as_path()] {
        let why = refused(&[(var, bad.to_str().unwrap())], "github.com");
        assert!(matches!(why, CredentialError::Unusable { .. }), "{bad:?}");
        assert!(why.to_string().contains(var));
    }
    std::fs::write(&file, "a token\nwith two lines\n").unwrap();
    let why = refused(&[(var, path)], "github.com").to_string();
    assert!(!why.contains("two lines"), "{why}");
}

#[test]
fn no_variable_gives_the_name_of_the_one_to_set() {
    let why = refused(&[], "ghe.example.com");
    assert!(why.is_none_given());
    assert!(
        why.to_string()
            .contains("PURLIS_FORGE_TOKEN_GHE_EXAMPLE_COM"),
        "{why}"
    );
    assert!(matches!(
        refused(&[], "not a host"),
        CredentialError::Unusable { .. }
    ));
}

#[test]
fn two_different_hosts_never_share_a_variable_name() {
    let hosts = [
        "github.com",
        "git-hub.com",
        "git.hub.com",
        "git--hub.com",
        "git-.hub.com".trim_start_matches("git-.hub.com"), // not a host: skipped below
        "a.b",
        "a-b",
        "a-b.c",
        "a.b-c",
        "a.b:8443",
        "a:8443",
        "a.8443",
        "a-8443",
        "ghe.example.com",
        "ghe-example.com",
        "ghe-example-com",
        "ghe.example.com:443",
    ];
    let mut seen: HashMap<String, &str> = HashMap::new();
    for host in hosts.into_iter().filter(|h| crate::forge::host_ok(h)) {
        let key = host_key(host);
        assert!(
            seen.insert(key.clone(), host)
                .is_none_or(|before| before == host),
            "{host} and {} are both {key}",
            seen[&key]
        );
    }
    assert_eq!(host_key("ghe.example.com"), "GHE_EXAMPLE_COM");
    assert_eq!(host_key("git-example.com"), "GIT__EXAMPLE_COM");
    assert_eq!(host_key("ghe.example.com:8443"), "GHE_EXAMPLE_COM___8443");
}

#[test]
fn a_credential_shows_no_token_in_debug_and_the_token_is_a_secret() {
    let c = got(&[("GH_TOKEN", "ghp_do_not_print_me")], "github.com");
    let shown = format!("{c:?}");
    assert!(!shown.contains("do_not_print"), "{shown}");
    assert!(shown.contains("github.com"));
    assert_eq!(
        secrecy::ExposeSecret::expose_secret(c.token()),
        "ghp_do_not_print_me"
    );
    assert!(crate::applog::holds_a_value_kept_out("ghp_do_not_print_me"));
}

// ---------------------------------------------------------------------------------------
// The precedence rule.

type Answering = Box<dyn Fn(&str) -> Result<Credential, CredentialError> + Send + Sync>;

/// A source that answers as scripted and counts how often it was asked.
struct Scripted {
    answer: Answering,
    asked: AtomicUsize,
}

impl Scripted {
    fn giving(source: Source, token: &'static str) -> Arc<Scripted> {
        Arc::new(Scripted {
            answer: Box::new(move |host| {
                Ok(Credential::new(
                    Kind::GitHub,
                    host,
                    secrecy::SecretString::from(token.to_string()),
                    source,
                    TokenKind::GitHubClassic,
                    "scripted".to_string(),
                ))
            }),
            asked: AtomicUsize::new(0),
        })
    }

    fn failing(error: CredentialError) -> Arc<Scripted> {
        Arc::new(Scripted {
            answer: Box::new(move |_| Err(error.clone())),
            asked: AtomicUsize::new(0),
        })
    }

    fn asked(&self) -> usize {
        self.asked.load(Ordering::SeqCst)
    }
}

impl CredentialSource for Scripted {
    fn credential_for(&self, host: &str) -> Result<Credential, CredentialError> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        (self.answer)(host)
    }
}

fn none_given() -> CredentialError {
    CredentialError::None {
        host: "github.com".into(),
        how: "…".into(),
    }
}

fn ended() -> CredentialError {
    CredentialError::SignInAgain {
        host: "github.com".into(),
        said: "Sign in again.".into(),
    }
}

fn unusable() -> CredentialError {
    CredentialError::Unusable {
        host: "github.com".into(),
        said: "Not a token.".into(),
    }
}

fn credentials(mode: Mode, signed_in: &Arc<Scripted>, connection: &Arc<Scripted>) -> Credentials {
    Credentials::new(mode, Some(signed_in.clone()), Some(connection.clone()))
}

#[test]
fn headless_a_token_connection_for_the_host_wins_over_a_sign_in() {
    let account = Scripted::giving(Source::SignedIn, "account");
    let token = Scripted::giving(Source::TokenConnection, "token");
    let got = credentials(Mode::Headless, &account, &token)
        .credential_for("github.com")
        .unwrap();
    assert_eq!(
        (got.expose(), got.source()),
        ("token", Source::TokenConnection)
    );
    assert_eq!(account.asked(), 0);
}

#[test]
fn interactive_the_sign_in_wins_and_a_token_connection_is_never_read() {
    let account = Scripted::giving(Source::SignedIn, "account");
    let token = Scripted::giving(Source::TokenConnection, "token");
    let got = credentials(Mode::Interactive, &account, &token)
        .credential_for("github.com")
        .unwrap();
    assert_eq!((got.expose(), got.source()), ("account", Source::SignedIn));
    assert_eq!(token.asked(), 0);
}

#[test]
fn interactive_with_no_sign_in_a_token_in_the_environment_is_not_used() {
    let account = Scripted::failing(none_given());
    let token = Scripted::giving(Source::TokenConnection, "token");
    let why = credentials(Mode::Interactive, &account, &token)
        .credential_for("github.com")
        .unwrap_err();
    assert!(why.is_none_given());
    assert_eq!(token.asked(), 0);
}

#[test]
fn headless_with_no_token_for_the_host_the_machines_sign_in_answers() {
    let account = Scripted::giving(Source::SignedIn, "account");
    let token = Scripted::failing(none_given());
    let got = credentials(Mode::Headless, &account, &token)
        .credential_for("github.com")
        .unwrap();
    assert_eq!(got.source(), Source::SignedIn);
    assert_eq!((account.asked(), token.asked()), (1, 1));
}

#[test]
fn headless_a_token_that_is_given_and_unusable_or_expired_is_the_answer_and_nothing_stands_in() {
    for failure in [
        unusable(),
        CredentialError::Expired {
            host: "github.com".into(),
            variable: "GH_TOKEN".into(),
        },
    ] {
        let account = Scripted::giving(Source::SignedIn, "account");
        let token = Scripted::failing(failure.clone());
        let why = credentials(Mode::Headless, &account, &token)
            .credential_for("github.com")
            .unwrap_err();
        assert_eq!(why, failure);
        assert_eq!(account.asked(), 0, "the sign-in is not tried in its place");
    }
}

#[test]
fn a_sign_in_the_forge_ended_is_the_answer_in_either_mode_and_no_token_connection_replaces_it() {
    // Headless, with a token connection that has nothing for the host.
    let account = Scripted::failing(ended());
    let token = Scripted::failing(none_given());
    let why = credentials(Mode::Headless, &account, &token)
        .credential_for("github.com")
        .unwrap_err();
    assert!(why.is_sign_in_again());
    // Interactive, with a token sitting in the environment: still not used.
    let account = Scripted::failing(ended());
    let token = Scripted::giving(Source::TokenConnection, "token");
    let why = credentials(Mode::Interactive, &account, &token)
        .credential_for("github.com")
        .unwrap_err();
    assert!(why.is_sign_in_again());
    assert_eq!(token.asked(), 0);
}

#[test]
fn a_machine_with_no_keyring_and_no_token_says_how_to_give_one() {
    let headless = Credentials::new(Mode::Headless, None, Some(Arc::new(connection(&[]))));
    let why = headless.credential_for("ghe.example.com").unwrap_err();
    assert!(why.is_none_given());
    assert!(
        why.to_string()
            .contains("PURLIS_FORGE_TOKEN_GHE_EXAMPLE_COM"),
        "{why}"
    );
    let window = Credentials::new(Mode::Interactive, None, None);
    let why = window.credential_for("ghe.example.com").unwrap_err();
    assert!(
        why.to_string().contains("Sign in to ghe.example.com"),
        "{why}"
    );
}

#[test]
fn the_real_sources_follow_the_rule_with_both_a_token_and_a_sign_in_on_one_host() {
    let machine = machine(Kind::GitHub, "github.com", accounts::Method::Device);
    let now_ok = machine.expires_at - LEAD.as_secs() - 1;
    let account: Arc<dyn CredentialSource> =
        Arc::new(signed_in(&machine, &Arc::new(StandIn::answering(&[]))));
    let token: Arc<dyn CredentialSource> =
        Arc::new(connection(&[("GITHUB_TOKEN", "ghs_the_jobs_own")]));
    let _ = now_ok;
    let headless = Credentials::new(Mode::Headless, Some(account.clone()), Some(token.clone()));
    let interactive = Credentials::new(Mode::Interactive, Some(account), Some(token));
    // Real time: the account's token was granted just now and has hours left.
    let in_ci = headless.credential_for("github.com").unwrap();
    assert_eq!(
        (in_ci.expose(), in_ci.source()),
        ("ghs_the_jobs_own", Source::TokenConnection)
    );
    let in_the_window = interactive.credential_for("github.com").unwrap();
    assert_eq!(
        (in_the_window.expose(), in_the_window.source()),
        ("first-access-value", Source::SignedIn)
    );
    // On a host the token connection has nothing for, headless uses the sign-in only if it has
    // one there: it has none on ghe.example.com.
    assert!(
        headless
            .credential_for("ghe.example.com")
            .unwrap_err()
            .is_none_given()
    );
}
