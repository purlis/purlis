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

/// The press: pins the application the account was signed in through on its host.
fn register(machine: &Machine) {
    registered::pin(
        &scope(),
        machine.home.path(),
        machine.account.kind,
        &machine.account.host,
        machine.account.client_id.as_deref().unwrap(),
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
fn a_self_hosted_gitlab_renews_through_the_application_its_account_signed_in_through() {
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
            .contains(&("client_id".to_string(), "gl-group-app".to_string()))
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

// Tokens of the shape each forge issues: the exact prefix, then the characters and the length.
// Each prefix is split in the source, so a scan of the source for a token finds none.
const GHS_A: &str = concat!("gh", "s_", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
const GHS_B: &str = concat!("gh", "s_", "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB");
const GHS_JOB: &str = concat!("gh", "s_", "JJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJ");
const GHU: &str = concat!("gh", "u_", "UUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUU");
const GHO: &str = concat!("gh", "o_", "OOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOO");
const GHP: &str = concat!("gh", "p_", "PPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPP");
const GHR: &str = concat!("gh", "r_", "RRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRR");
const PAT_FINE: &str = concat!(
    "github",
    "_pat_",
    "fine____________________________________"
);
const PAT_NAMED: &str = concat!(
    "github",
    "_pat_",
    "named___________________________________"
);
const PAT_CLI: &str = concat!(
    "github",
    "_pat_",
    "cli_____________________________________"
);
const PAT_GH: &str = concat!(
    "github",
    "_pat_",
    "gh______________________________________"
);
const PAT_GITHUB: &str = concat!(
    "github",
    "_pat_",
    "github__________________________________"
);
const PAT_ENT: &str = concat!(
    "github",
    "_pat_",
    "ent_____________________________________"
);
const PAT_FILE: &str = concat!(
    "github",
    "_pat_",
    "file____________________________________"
);
const PAT_DIRECT: &str = concat!(
    "github",
    "_pat_",
    "direct__________________________________"
);
const PAT_SECRET: &str = concat!(
    "github",
    "_pat_",
    "do_not_print_me_________________________"
);
const GLPAT_A: &str = concat!("gl", "pat-", "AAAAAAAAAAAAAAAAAAAA");
const GLPAT_X: &str = concat!("gl", "pat-", "XXXXXXXXXXXXXXXXXXXX");
const GLPAT_C: &str = concat!("gl", "pat-", "CCCCCCCCCCCCCCCCCCCC");
const GLCBT: &str = concat!("gl", "cbt-", "0123456789abcdef");
const GITHUB_COM: &str = "6769746875622e636f6d";
const GITLAB_COM: &str = "6769746c61622e636f6d";
const GHE_EXAMPLE_COM: &str = "6768652e6578616d706c652e636f6d";
const GIT_EXAMPLE_COM: &str = "6769742e6578616d706c652e636f6d";

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
    let c = got(&[("GITHUB_TOKEN", GHS_A)], "github.com");
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
    // From the CLI's own variable: an installation token and a fine-grained one.
    for (token, kind, lifetime) in [
        (GHS_A, TokenKind::GitHubInstallation, Lifetime::PerJob),
        (PAT_FINE, TokenKind::GitHubFineGrained, Lifetime::LongLived),
    ] {
        let c = got(&[("GH_TOKEN", token)], "github.com");
        assert_eq!(c.token_kind(), kind, "{token}");
        assert_eq!(c.token_kind().lifetime(), lifetime, "{token}");
    }
    // A user-to-server token only from the variable purlis names for the host.
    let named = format!("PURLIS_FORGE_TOKEN__{GITHUB_COM}");
    for token in [GHU, GHO] {
        let c = got(&[(named.as_str(), token)], "github.com");
        assert_eq!(c.token_kind(), TokenKind::GitHubUser, "{token}");
        assert_eq!(c.token_kind().lifetime(), Lifetime::PerJob, "{token}");
        let why = refused(&[("GH_TOKEN", token)], "github.com").to_string();
        assert!(why.contains("only from the variable it names"), "{why}");
        assert!(!why.contains(token), "{why}");
    }
}

#[test]
fn a_classic_token_and_a_refresh_token_are_refused_from_any_variable_and_the_message_names_fine_grained()
 {
    let named = format!("PURLIS_FORGE_TOKEN__{GITHUB_COM}");
    for (var, token) in [
        ("GH_TOKEN", GHP),
        ("GITHUB_TOKEN", GHP),
        (named.as_str(), GHP),
        (named.as_str(), GHR),
        ("GH_TOKEN", GHR),
    ] {
        let why = refused(&[(var, token)], "github.com");
        assert!(matches!(why, CredentialError::Unusable { .. }), "{var}");
        let said = why.to_string();
        assert!(!said.contains(token), "{said}");
        assert!(
            said.contains("fine-grained") || said.contains("refresh token"),
            "{said}"
        );
    }
    // And a classic token on a GHES host the same.
    let why = refused(
        &[("GH_ENTERPRISE_TOKEN", GHP), ("GH_HOST", "ghe.example.com")],
        "ghe.example.com",
    );
    assert!(why.to_string().contains("fine-grained"), "{why}");
}

#[test]
fn a_token_is_taken_by_its_exact_prefix_and_shape_not_by_how_it_begins() {
    let named = format!("PURLIS_FORGE_TOKEN__{GITHUB_COM}");
    let too_short = "ghs_abc".to_string();
    let wrong_characters = format!("ghs_{}", "-".repeat(36));
    let too_long = format!("ghs_{}", "a".repeat(300));
    let pat_short = "github_pat_short".to_string();
    for bad in [too_short, wrong_characters, too_long, pat_short] {
        let why = refused(&[(named.as_str(), bad.as_str())], "github.com");
        assert!(matches!(why, CredentialError::Unusable { .. }), "{bad}");
        assert!(why.to_string().contains("is not the shape of one"), "{why}");
        assert!(!why.to_string().contains(&bad), "{why}");
    }
    // A prefix that merely looks alike is no known prefix at all.
    for other in ["GHS_aaaa", "ghsx_aaaa", "xghs_aaaa", "gh_s_aaaa"] {
        let why = refused(&[("GH_TOKEN", other)], "github.com");
        assert!(why.to_string().contains("no shape purlis knows"), "{why}");
    }
    // The GitLab prefixes the same.
    let gitlab = [
        ("GITLAB_HOST", "gitlab.com"),
        ("GITLAB_TOKEN", "glpat-short"),
    ];
    assert!(
        refused(&gitlab, "gitlab.com")
            .to_string()
            .contains("is not the shape of one")
    );
}

#[test]
fn an_ambient_variable_takes_only_a_token_of_a_known_shape_and_an_opaque_one_needs_the_operators_opt_in()
 {
    // A CLI's own variable (what a CI job or a shell exports) is not an opt-in: a token of no
    // known prefix is refused.
    for (vars, host) in [
        (vec![("GH_TOKEN", "0123456789abcdef")], "github.com"),
        (
            vec![
                ("GITLAB_HOST", "gitlab.com"),
                ("GITLAB_TOKEN", "0123456789abcdef"),
            ],
            "gitlab.com",
        ),
    ] {
        assert!(
            matches!(refused(&vars, host), CredentialError::Unusable { .. }),
            "{vars:?}"
        );
    }
    // Naming it for the host is not enough on its own: an unknown type is the operator's per-type
    // opt-in (ADR 0128 D9).
    let named = format!("PURLIS_FORGE_TOKEN__{GITHUB_COM}");
    let accept = format!("PURLIS_FORGE_ACCEPT__{GITHUB_COM}");
    let why = refused(&[(named.as_str(), "0123456789abcdef")], "github.com").to_string();
    assert!(
        why.contains(&accept) && why.contains("unrecognised"),
        "{why}"
    );
    let c = got(
        &[
            (named.as_str(), "0123456789abcdef"),
            (accept.as_str(), "unrecognised"),
        ],
        "github.com",
    );
    assert_eq!(c.token_kind(), TokenKind::Unrecognised);
    assert_eq!(c.token_kind().lifetime(), Lifetime::LongLived);
    // The opt-in is for that type and that host only.
    let other = format!("PURLIS_FORGE_ACCEPT__{GITLAB_COM}");
    assert!(matches!(
        refused(
            &[
                (named.as_str(), "0123456789abcdef"),
                (other.as_str(), "unrecognised")
            ],
            "github.com"
        ),
        CredentialError::Unusable { .. }
    ));
    assert!(matches!(
        refused(
            &[
                (named.as_str(), "0123456789abcdef"),
                (accept.as_str(), "ghu")
            ],
            "github.com"
        ),
        CredentialError::Unusable { .. }
    ));
}

#[test]
fn each_gitlab_token_kind_is_read_from_its_shape_and_its_variable() {
    let c = got(
        &[("GITLAB_TOKEN", GLPAT_A), ("GITLAB_HOST", "gitlab.com")],
        "gitlab.com",
    );
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
    let named = format!("PURLIS_FORGE_TOKEN__{GIT_EXAMPLE_COM}");
    let c = got(&[(named.as_str(), GLCBT)], "git.example.com");
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
    let github = [("GITHUB_TOKEN", GHS_A), ("GH_TOKEN", GHS_B)];
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
        ("GH_ENTERPRISE_TOKEN", PAT_ENT),
        ("GH_HOST", "ghe.example.com"),
    ];
    assert_eq!(got(&ghes, "ghe.example.com").expose(), PAT_ENT);
    assert!(refused(&ghes, "github.com").is_none_given());
    assert!(refused(&ghes, "other.example.com").is_none_given());
    let actions = [
        ("GITHUB_TOKEN", GHS_A),
        ("GITHUB_SERVER_URL", "https://ghe.example.com"),
    ];
    assert_eq!(got(&actions, "ghe.example.com").expose(), GHS_A);
    assert!(refused(&actions, "other.example.com").is_none_given());
    // A self-hosted GitLab's `GITLAB_TOKEN` is its `GITLAB_HOST`'s, never gitlab.com's.
    let gitlab = [
        ("GITLAB_TOKEN", GLPAT_X),
        ("GITLAB_HOST", "https://git.example.com/"),
    ];
    assert_eq!(got(&gitlab, "git.example.com").expose(), GLPAT_X);
    assert!(refused(&gitlab, "gitlab.com").is_none_given());
}

#[test]
fn github_token_on_a_ghes_job_is_never_offered_to_github_com() {
    // Actions on GHES: GITHUB_TOKEN (and a GH_TOKEN set from it) is the GHES server's.
    for server in [
        "https://ghe.example.com",
        "ghe.example.com",
        "https://GHE.Example.com/",
    ] {
        let job = [
            ("GITHUB_TOKEN", GHS_A),
            ("GH_TOKEN", GHS_B),
            ("GITHUB_SERVER_URL", server),
        ];
        assert!(refused(&job, "github.com").is_none_given(), "{server}");
        assert_eq!(got(&job, "ghe.example.com").expose(), GHS_A, "{server}");
    }
    // A server URL that is not a host names no host, and the doubt closes the door.
    for server in ["https://", "not a url", "https://ghe.example.com:port/x y"] {
        let job = [("GITHUB_TOKEN", GHS_A), ("GITHUB_SERVER_URL", server)];
        assert!(refused(&job, "github.com").is_none_given(), "{server:?}");
        assert!(
            refused(&job, "ghe.example.com").is_none_given(),
            "{server:?}"
        );
    }
    // github.com's own job, or no server named: github.com.
    for job in [
        vec![
            ("GITHUB_TOKEN", GHS_A),
            ("GITHUB_SERVER_URL", "https://github.com"),
        ],
        vec![("GITHUB_TOKEN", GHS_A)],
        vec![("GITHUB_TOKEN", GHS_A), ("GITHUB_SERVER_URL", "  ")],
    ] {
        assert_eq!(got(&job, "github.com").expose(), GHS_A, "{job:?}");
    }
    // The variable purlis names for github.com is the operator's own choice, whatever the job says.
    let named = format!("PURLIS_FORGE_TOKEN__{GITHUB_COM}");
    let job = [
        (named.as_str(), PAT_NAMED),
        ("GITHUB_TOKEN", GHS_A),
        ("GITHUB_SERVER_URL", "https://ghe.example.com"),
    ];
    assert_eq!(got(&job, "github.com").expose(), PAT_NAMED);
}

#[test]
fn gitlab_tokens_are_offered_only_to_the_host_gitlab_names_and_to_none_when_it_names_none() {
    // No host named: neither is offered, gitlab.com included.
    let bare = [("GITLAB_TOKEN", GLPAT_A), ("CI_JOB_TOKEN", "abcdef012345")];
    for host in ["gitlab.com", "git.example.com"] {
        assert!(refused(&bare, host).is_none_given(), "{host}");
    }
    // GitLab.com's own CI names it.
    let on_gitlab_com = [
        ("GITLAB_TOKEN", GLPAT_A),
        ("CI_JOB_TOKEN", "abcdef012345"),
        ("CI_SERVER_HOST", "gitlab.com"),
    ];
    assert_eq!(got(&on_gitlab_com, "gitlab.com").expose(), GLPAT_A);
    // A self-managed job's token, and its GITLAB_TOKEN, are that server's and nobody else's.
    let on_self_managed = [
        ("GITLAB_TOKEN", GLPAT_A),
        ("CI_JOB_TOKEN", "abcdef012345"),
        ("CI_SERVER_HOST", "git.example.com"),
    ];
    assert!(refused(&on_self_managed, "gitlab.com").is_none_given());
    assert_eq!(got(&on_self_managed, "git.example.com").expose(), GLPAT_A);
    let job_only = [
        ("CI_JOB_TOKEN", "abcdef012345"),
        ("CI_SERVER_HOST", "git.example.com"),
    ];
    assert!(refused(&job_only, "gitlab.com").is_none_given());
    // GITLAB_HOST wins over CI_SERVER_HOST for GITLAB_TOKEN, and does not move the job's token.
    let both = [
        ("GITLAB_TOKEN", GLPAT_A),
        ("GITLAB_HOST", "other.example.com"),
        ("CI_JOB_TOKEN", "abcdef012345"),
        ("CI_SERVER_HOST", "git.example.com"),
    ];
    assert_eq!(got(&both, "other.example.com").expose(), GLPAT_A);
    assert_eq!(got(&both, "git.example.com").expose(), "abcdef012345");
    // A host variable that is not a host names none.
    let bad = [("GITLAB_TOKEN", GLPAT_A), ("GITLAB_HOST", "not a host")];
    assert!(refused(&bad, "gitlab.com").is_none_given());
}

#[test]
fn the_named_variable_for_a_host_wins_over_the_forge_clis_own() {
    let both = [
        ("PURLIS_FORGE_TOKEN__6769746875622e636f6d", PAT_NAMED),
        ("GH_TOKEN", PAT_CLI),
    ];
    let c = got(&both, "github.com");
    assert_eq!(
        (c.expose(), c.from()),
        (PAT_NAMED, "PURLIS_FORGE_TOKEN__6769746875622e636f6d")
    );
    let gh_first = [("GH_TOKEN", PAT_GH), ("GITHUB_TOKEN", PAT_GITHUB)];
    assert_eq!(got(&gh_first, "github.com").expose(), PAT_GH);
    // An explicit GITLAB_TOKEN is a choice; a job's own token is the fallback.
    let ci = [
        ("GITLAB_TOKEN", GLPAT_C),
        ("CI_JOB_TOKEN", "job"),
        ("CI_SERVER_HOST", "gitlab.com"),
    ];
    assert_eq!(got(&ci, "gitlab.com").expose(), GLPAT_C);
}

#[test]
fn a_variable_that_is_set_and_unusable_is_an_error_and_the_next_one_is_not_tried() {
    let named = format!("PURLIS_FORGE_TOKEN__{GITHUB_COM}");
    let vars = [(named.as_str(), "has a space"), ("GH_TOKEN", PAT_FINE)];
    assert!(matches!(
        refused(&vars, "github.com"),
        CredentialError::Unusable { .. }
    ));
}

#[test]
fn a_named_variable_that_is_set_and_empty_is_an_error_never_a_fallback() {
    let named = format!("PURLIS_FORGE_TOKEN__{GITHUB_COM}");
    let file = format!("PURLIS_FORGE_TOKEN_FILE__{GITHUB_COM}");
    for (var, value) in [
        (named.as_str(), ""),
        (named.as_str(), "  "),
        (named.as_str(), "\n"),
        (file.as_str(), ""),
        (file.as_str(), " "),
    ] {
        // The CLI's own variable is set and good, and is not what answers.
        let vars = [
            (var, value),
            ("GH_TOKEN", PAT_FINE),
            ("GITHUB_TOKEN", GHS_A),
        ];
        let why = refused(&vars, "github.com");
        assert!(matches!(why, CredentialError::Unusable { .. }), "{var}");
        assert!(
            why.to_string().contains(&format!("{var} is set but empty")),
            "{why}"
        );
    }
    // An ambient variable exported empty is not a choice, and is skipped.
    let vars = [("GH_TOKEN", ""), ("GITHUB_TOKEN", GHS_A)];
    assert_eq!(got(&vars, "github.com").expose(), GHS_A);
}

#[test]
fn the_token_and_its_file_both_set_is_a_refusal_naming_both() {
    let named = format!("PURLIS_FORGE_TOKEN__{GITHUB_COM}");
    let file = format!("PURLIS_FORGE_TOKEN_FILE__{GITHUB_COM}");
    for (token, path) in [
        (PAT_FINE, "/run/secrets/x"),
        ("", "/run/secrets/x"),
        (PAT_FINE, ""),
    ] {
        let vars = [(named.as_str(), token), (file.as_str(), path)];
        let why = refused(&vars, "github.com");
        assert!(
            matches!(why, CredentialError::Unusable { .. }),
            "{token:?} {path:?}"
        );
        let said = why.to_string();
        assert!(said.contains(&named) && said.contains(&file), "{said}");
        assert!(!said.contains(PAT_FINE), "{said}");
    }
}

#[test]
fn a_token_of_a_host_purlis_cannot_place_needs_its_forge_named() {
    let vars = [(
        "PURLIS_FORGE_TOKEN__6768652e6578616d706c652e636f6d",
        "0123456789abcdef",
    )];
    let why = refused(&vars, "ghe.example.com").to_string();
    assert!(
        why.contains("PURLIS_FORGE_KIND__6768652e6578616d706c652e636f6d"),
        "{why}"
    );
    let hinted = [
        (
            "PURLIS_FORGE_TOKEN__6768652e6578616d706c652e636f6d",
            "0123456789abcdef",
        ),
        (
            "PURLIS_FORGE_KIND__6768652e6578616d706c652e636f6d",
            "github",
        ),
        (
            "PURLIS_FORGE_ACCEPT__6768652e6578616d706c652e636f6d",
            "unrecognised",
        ),
    ];
    assert_eq!(got(&hinted, "ghe.example.com").kind(), Kind::GitHub);
    // A prefix that says GitHub is enough.
    let by_prefix = [(
        "PURLIS_FORGE_TOKEN__6768652e6578616d706c652e636f6d",
        PAT_ENT,
    )];
    assert_eq!(got(&by_prefix, "ghe.example.com").kind(), Kind::GitHub);
    let bad = [
        ("PURLIS_FORGE_TOKEN__6768652e6578616d706c652e636f6d", "abc"),
        (
            "PURLIS_FORGE_KIND__6768652e6578616d706c652e636f6d",
            "forgejo",
        ),
    ];
    assert!(matches!(
        refused(&bad, "ghe.example.com"),
        CredentialError::Unusable { .. }
    ));
}

#[test]
fn a_token_that_disagrees_with_its_variable_its_hint_or_its_host_is_refused_not_guessed_at() {
    for (vars, host) in [
        (vec![("GH_TOKEN", GLPAT_X)], "github.com"),
        (
            vec![("GITLAB_TOKEN", PAT_GITHUB), ("GITLAB_HOST", "gitlab.com")],
            "gitlab.com",
        ),
        (
            vec![("PURLIS_FORGE_TOKEN__6769746875622e636f6d", GLPAT_X)],
            "github.com",
        ),
        (
            vec![("PURLIS_FORGE_TOKEN__6769746c61622e636f6d", PAT_GITHUB)],
            "gitlab.com",
        ),
        (
            vec![
                (
                    "PURLIS_FORGE_TOKEN__6769742e6578616d706c652e636f6d",
                    GLPAT_X,
                ),
                (
                    "PURLIS_FORGE_KIND__6769742e6578616d706c652e636f6d",
                    "github",
                ),
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
    let var = "PURLIS_FORGE_TOKEN_EXPIRES_AT__6769746875622e636f6d";
    let in_days = |days: u64| (NOW + days * 86_400).to_string();
    let with = |expires: &str| {
        connection(&[("GH_TOKEN", PAT_FINE), (var, expires)]).credential_at("github.com", NOW)
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
    assert!(why.to_string().contains("GH_TOKEN") && !why.to_string().contains(PAT_FINE));
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
    let var = "PURLIS_FORGE_TOKEN_EXPIRES_AT__6769746875622e636f6d";
    let c = got(
        &[("GITHUB_TOKEN", GHS_JOB), (var, &(NOW + 60).to_string())],
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
    std::fs::write(&file, format!("{PAT_FILE}\n")).unwrap();
    let var = "PURLIS_FORGE_TOKEN_FILE__6769746875622e636f6d";
    let path = file.to_str().unwrap();
    let c = got(&[(var, path)], "github.com");
    assert_eq!(
        (c.expose(), c.token_kind()),
        (PAT_FILE, TokenKind::GitHubFineGrained)
    );
    assert_eq!(c.from(), var);
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
            .contains("PURLIS_FORGE_TOKEN__6768652e6578616d706c652e636f6d"),
        "{why}"
    );
    assert!(matches!(
        refused(&[], "not a host"),
        CredentialError::Unusable { .. }
    ));
}

#[test]
fn two_different_hosts_never_share_a_variable_name_in_any_family() {
    let hosts = [
        "github.com",
        "git-hub.com",
        "git.hub.com",
        "git--hub.com",
        "a.b",
        "a-b",
        "a--b",
        "a-b.c",
        "a.b-c",
        "a.b:8443",
        "a:8443",
        "a.8443",
        "a-8443",
        "x",
        "x.file",
        "x.expires.at",
        "x.expires.at.file",
        "x-file",
        "ghe.example.com",
        "ghe-example.com",
        "ghe-example-com",
        "ghe.example.com:443",
    ];
    let mut seen: HashMap<String, String> = HashMap::new();
    for host in hosts.into_iter().filter(|h| crate::forge::host_ok(h)) {
        let key = host_key(host);
        for name in [
            variable_for(host),
            file_variable_for(host),
            format!("PURLIS_FORGE_TOKEN_EXPIRES_AT__{key}"),
            format!("PURLIS_FORGE_KIND__{key}"),
            format!("PURLIS_FORGE_ACCEPT__{key}"),
        ] {
            let was = seen.insert(name.clone(), host.to_string());
            assert!(
                was.is_none(),
                "{host} and {} are both {name}",
                was.unwrap_or_default()
            );
        }
    }
    // One spelling per host: the lower-case hex of its bytes, ADR 0128 D9.2.
    assert_eq!(host_key("github.com"), "6769746875622e636f6d");
    assert_eq!(
        variable_for("GitHub.com"),
        "PURLIS_FORGE_TOKEN__6769746875622e636f6d"
    );
    assert_eq!(
        file_variable_for("ghe.example.com:8443"),
        format!(
            "PURLIS_FORGE_TOKEN_FILE__{}",
            host_key("ghe.example.com:8443")
        )
    );
    // No spelling has an underscore, so a family's prefix is the whole of what is not the host.
    assert!(hosts.iter().all(|h| !host_key(h).contains('_')));
}

#[test]
fn a_credential_shows_no_token_in_debug_and_the_token_is_a_secret() {
    let c = got(&[("GH_TOKEN", PAT_SECRET)], "github.com");
    let shown = format!("{c:?}");
    assert!(!shown.contains("do_not_print"), "{shown}");
    assert!(shown.contains("github.com"));
    assert_eq!(secrecy::ExposeSecret::expose_secret(c.token()), PAT_SECRET);
    assert!(crate::applog::holds_a_value_kept_out(PAT_SECRET));
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
                    TokenKind::GitHubFineGrained,
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
            .contains("PURLIS_FORGE_TOKEN__6768652e6578616d706c652e636f6d"),
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
    let token: Arc<dyn CredentialSource> = Arc::new(connection(&[("GITHUB_TOKEN", GHS_JOB)]));
    let _ = now_ok;
    let headless = Credentials::new(Mode::Headless, Some(account.clone()), Some(token.clone()));
    let interactive = Credentials::new(Mode::Interactive, Some(account), Some(token));
    // Real time: the account's token was granted just now and has hours left.
    let in_ci = headless.credential_for("github.com").unwrap();
    assert_eq!(
        (in_ci.expose(), in_ci.source()),
        (GHS_JOB, Source::TokenConnection)
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

// ---------------------------------------------------------------------------------------
// More than one account on a host.

/// A forge that says whose token it is: `login`'s.
struct Who(&'static str);

impl crate::forge::transport::Transport for Who {
    fn send(
        &self,
        _forge: &crate::forge::Forge,
        _call: &crate::forge::transport::Call,
    ) -> Result<crate::forge::transport::Reply, crate::forge::transport::NoAnswer> {
        Ok(crate::forge::transport::Reply {
            code: 0,
            out: format!(r#"{{"login": "{0}", "username": "{0}"}}"#, self.0),
            err: String::new(),
            status: Some(200),
            headers: Vec::new(),
        })
    }

    fn check_auth(&self, _forge: &crate::forge::Forge) -> Result<(), crate::forge::ForgeError> {
        Ok(())
    }
}

fn pasted_as(
    home: &std::path::Path,
    host: &str,
    login: &'static str,
    token: &str,
) -> accounts::ForgeAccount {
    let keyring = accounts::keyring_at(home);
    let reach = move |_: &crate::forge::Forge,
                      _: Arc<dyn crate::forge::http::TokenSource>|
          -> Result<
        Arc<dyn crate::forge::transport::Transport>,
        crate::forge::ForgeError,
    > { Ok(Arc::new(Who(login))) };
    accounts::sign_in_with_token(
        &scope(),
        home,
        keyring.as_ref(),
        &reach,
        Kind::GitHub,
        host,
        accounts::Pasted::new(token.to_string()),
    )
    .unwrap()
}

#[test]
fn with_two_accounts_on_a_host_and_none_chosen_nothing_is_picked() {
    let home = tempfile::tempdir().unwrap();
    pasted_as(
        home.path(),
        "ghe.example.com",
        "alice",
        "github_pat_alice_token",
    );
    pasted_as(
        home.path(),
        "ghe.example.com",
        "alice-emu",
        "github_pat_emu_token",
    );
    let source = SignedIn::new(&scope(), home.path());
    let why = source.credential_at("ghe.example.com", 1_000).unwrap_err();
    assert!(matches!(why, CredentialError::Unusable { .. }), "{why:?}");
    let said = why.to_string();
    assert!(said.contains("Choose an account"), "{said}");
    assert!(
        said.contains("alice") && said.contains("alice-emu"),
        "{said}"
    );
    assert!(!said.contains("github_pat_"), "{said}");
}

#[test]
fn an_account_the_project_is_bound_to_is_the_only_one_that_answers() {
    let home = tempfile::tempdir().unwrap();
    let alice = pasted_as(
        home.path(),
        "ghe.example.com",
        "alice",
        "github_pat_alice_token",
    );
    let emu = pasted_as(
        home.path(),
        "ghe.example.com",
        "alice-emu",
        "github_pat_emu_token",
    );
    let source = SignedIn::new(&scope(), home.path());
    assert_eq!(
        source
            .credential_of("ghe.example.com", &emu.id, 1_000)
            .unwrap()
            .expose(),
        "github_pat_emu_token"
    );
    assert_eq!(
        source
            .credential_of("GHE.example.com", &alice.id, 1_000)
            .unwrap()
            .login(),
        Some("alice")
    );
    // An account of another host, or one that is not listed, is never answered with.
    let elsewhere = pasted_as(
        home.path(),
        "other.example.com",
        "alice",
        "github_pat_other_token",
    );
    for id in [elsewhere.id.as_str(), "01NOSUCHACCOUNT"] {
        let why = source
            .credential_of("ghe.example.com", id, 1_000)
            .unwrap_err();
        assert!(
            matches!(why, CredentialError::Unusable { .. }),
            "{id}: {why:?}"
        );
        assert!(why.to_string().contains("Choose an account"), "{why}");
    }
}

#[test]
fn a_sign_in_the_forge_ended_is_never_silently_replaced_by_another_account_on_the_host() {
    let home = tempfile::tempdir().unwrap();
    let ended = pasted_as(
        home.path(),
        "ghe.example.com",
        "alice",
        "github_pat_alice_token",
    );
    let other = pasted_as(
        home.path(),
        "ghe.example.com",
        "alice-emu",
        "github_pat_emu_token",
    );
    let keyring = accounts::keyring_at(home.path());
    accounts::signed_out_by_the_forge(&scope(), home.path(), keyring.as_ref(), &ended.id).unwrap();
    let source = SignedIn::new(&scope(), home.path());
    // Asked without a choice: neither the ended account's token nor the other's.
    let why = source.credential_at("ghe.example.com", 1_000).unwrap_err();
    assert!(!why.to_string().contains("github_pat_"), "{why}");
    assert!(
        why.is_none_given() || matches!(why, CredentialError::Unusable { .. }),
        "{why:?}"
    );
    assert!(why.to_string().contains("Choose an account"), "{why}");
    // Asked for the ended one: it says to sign in again, and does not answer with the other.
    let why = source
        .credential_of("ghe.example.com", &ended.id, 1_000)
        .unwrap_err();
    assert!(why.is_sign_in_again(), "{why:?}");
    // The other still answers for itself.
    assert_eq!(
        source
            .credential_of("ghe.example.com", &other.id, 1_000)
            .unwrap()
            .expose(),
        "github_pat_emu_token"
    );
}
