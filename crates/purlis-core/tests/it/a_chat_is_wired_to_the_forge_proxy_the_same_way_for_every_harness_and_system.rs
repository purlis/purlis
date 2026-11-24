// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's forge wiring is the one design, for every harness and both systems** (ADR 0128 D3b,
//! D3c, slice B): for each harness, the chat's environment is composed as a start composes it
//! (a profile's pairs, the forge lock, the wiring, then the window's composition with its hooks),
//! from an app whose own environment holds a person's forge login. What comes out is exactly the
//! variables and git pairs the design names, in an order that keeps purlis's helper from being
//! emptied, and nothing in it is the person's.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use purlis_core::chatenv::{self, Starting};
use purlis_core::forgelock::{self, PERSON};
use purlis_core::forgewire::{
    self, Advice, DOWNLOAD_ONLY, Fix, ForgeKind, Os, PROVEN, Repo, Route, SecretString, Tool,
    Wiring,
};
use purlis_core::harness::Harness;

const SENTINEL: &str = "SENTINEL-4f9a0c1e7b2d4a6c8e0f1a3b5c7d9e0f";
const HELPER: &str = "/opt/purlis/purlis";

fn repos() -> Vec<Repo> {
    vec![
        Repo::new("github.com", "acme", "app", ForgeKind::GitHub).unwrap(),
        Repo::new("git.example.org", "grp", "lib", ForgeKind::GitLab).unwrap(),
    ]
}

fn wiring(scratch: &Path) -> Wiring {
    // The chat's own copy of the authority is made from it, so the file is there.
    std::fs::write(scratch.join("ca.pem"), "-----BEGIN CERTIFICATE-----\n").expect("a CA");
    forgewire::compose(
        &Route {
            egress_url: "http://127.0.0.1:44123".to_owned(),
            gh_socket: scratch.join("s/gh.sock"),
            ca_cert: scratch.join("ca.pem"),
            sentinel: SecretString::from(SENTINEL),
            helper_exe: PathBuf::from(HELPER),
            repos: repos(),
        },
        scratch,
    )
    .expect("the wiring")
}

/// A chat of `harness`'s environment as its start composes it, from an app environment that holds
/// a person's login and a CA bundle, a token and a proxy of their own.
fn chat_env(harness: Harness, scratch: &Path) -> Vec<(String, String)> {
    let profile = vec![
        ("GIT_CONFIG_COUNT".to_owned(), "1".to_owned()),
        ("GIT_CONFIG_KEY_0".to_owned(), "user.name".to_owned()),
        ("GIT_CONFIG_VALUE_0".to_owned(), "a profile".to_owned()),
    ];
    let home = scratch.join("home");
    std::fs::create_dir_all(&home).unwrap();
    // The sandbox's own order: the lock, then the wiring (which takes the lock's marker out).
    let locked = forgelock::apply_to_line(
        profile,
        Some(scratch),
        Some(&home),
        harness == Harness::ClaudeCode,
    )
    .expect("the lock");
    let line = forgewire::apply(locked, &wiring(scratch));
    let hooks = purlis_core::githooks::GitHooks::at(scratch.join("hooks"));
    let starting = Starting {
        harness: Some(harness),
        operator: &[],
        strip: &[],
        set: &line,
        git_hooks: Some(&hooks),
        declared: &[],
    };
    let app: Vec<(OsString, OsString)> = vec![
        ("HOME".into(), home.into()),
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("GH_TOKEN".into(), format!("{PERSON}-gh-token").into()),
        ("GITLAB_TOKEN".into(), format!("{PERSON}-glab-token").into()),
        ("GIT_SSL_CAINFO".into(), "/the/persons/bundle.pem".into()),
    ];
    chatenv::compose(app, &starting)
        .into_iter()
        .map(|(name, value)| {
            (
                name.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect()
}

fn get<'a>(env: &'a [(String, String)], name: &str) -> Option<&'a str> {
    env.iter()
        .rev()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

/// The `GIT_CONFIG_*` pairs of `env`, in the order git reads them.
fn pairs(env: &[(String, String)]) -> Vec<(String, String)> {
    let count: usize = get(env, "GIT_CONFIG_COUNT")
        .and_then(|n| n.parse().ok())
        .expect("a count");
    (0..count)
        .map(|n| {
            (
                get(env, &format!("GIT_CONFIG_KEY_{n}"))
                    .unwrap_or_else(|| panic!("key {n}"))
                    .to_owned(),
                get(env, &format!("GIT_CONFIG_VALUE_{n}"))
                    .unwrap_or_else(|| panic!("value {n}"))
                    .to_owned(),
            )
        })
        .collect()
}

#[test]
fn every_harness_is_given_exactly_the_wiring_the_design_names() {
    purlis_core::unsteered!();
    for harness in Harness::ALL {
        let scratch = tempfile::tempdir().unwrap();
        let env = chat_env(harness, scratch.path());
        let s = scratch.path();
        let at = |name: &str| get(&env, name).unwrap_or_else(|| panic!("{harness:?}: no {name}"));
        // gh.
        assert_eq!(at("GH_HOST"), "github.localhost", "{harness:?}");
        // Written where no chat writes, never in the chat's own scratch folder.
        let gh_dir = PathBuf::from(at("GH_CONFIG_DIR"));
        assert!(gh_dir.starts_with(forgewire::wire_folder(s)), "{harness:?}");
        assert!(gh_dir.ends_with("gh"), "{harness:?}");
        assert_eq!(
            at("GH_TOKEN"),
            SENTINEL,
            "{harness:?}: the person's token is replaced"
        );
        assert_eq!(at("GH_ENTERPRISE_TOKEN"), SENTINEL);
        // glab.
        assert_eq!(at("GITLAB_TOKEN"), SENTINEL);
        // The chat reads its own copy of the authority, made in its wire folder (K2): never the
        // held route folder's.
        let copy = PathBuf::from(at("GLAB_CA_CERT"));
        assert!(copy.starts_with(forgewire::wire_folder(s)), "{harness:?}");
        assert!(copy.ends_with("ca.pem"), "{harness:?}");
        assert_eq!(
            std::fs::read_to_string(&copy).unwrap(),
            std::fs::read_to_string(s.join("ca.pem")).unwrap(),
            "{harness:?}"
        );
        assert_eq!(get(&env, "GITLAB_HOST"), None);
        // git and everything else that goes through the chat's egress port.
        assert_eq!(at("HTTPS_PROXY"), "http://127.0.0.1:44123");
        assert_eq!(at("https_proxy"), "http://127.0.0.1:44123");
        // cargo.
        assert_eq!(at("CARGO_NET_GIT_FETCH_WITH_CLI"), "true");
        // The helper's one input.
        assert_eq!(at("PURLIS_FORGE_SENTINEL"), SENTINEL);
        // Never chat-wide trust, and nothing for npm or pip.
        for never in [
            "GIT_SSL_CAINFO",
            "GIT_SSL_CAPATH",
            "NODE_EXTRA_CA_CERTS",
            "PIP_CERT",
            "SSL_CERT_FILE",
            "SSL_CERT_DIR",
            "REQUESTS_CA_BUNDLE",
            "CURL_CA_BUNDLE",
            forgelock::MARKER,
        ] {
            assert_eq!(get(&env, never), None, "{harness:?}: {never} is set");
        }
        // The person's login is nowhere in what the chat is started with.
        for (name, value) in &env {
            assert!(!value.contains(PERSON), "{harness:?}: {name}={value}");
        }
    }
}

#[test]
fn every_harnesss_git_pairs_reset_the_helper_list_before_purlis_s_helper_and_nothing_empties_it_after()
 {
    purlis_core::unsteered!();
    for harness in Harness::ALL {
        let scratch = tempfile::tempdir().unwrap();
        let env = chat_env(harness, scratch.path());
        // Each harness's git trusts the chat's own copy of the authority.
        let ca = get(&env, "GLAB_CA_CERT").expect("a CA copy").to_owned();
        let got = pairs(&env);
        // The profile's own pair first; then the window's composition's (hooks), wherever it put
        // them; then purlis's wiring last of all.
        assert_eq!(got[0], ("user.name".to_owned(), "a profile".to_owned()));
        // The window's composition adds its no-gc pairs (#7531) and its hooks pair after every
        // pair there is, so purlis's block is found by its reset, and only those follow it:
        // none of them is a credential key, so none can empty the helper list.
        let at = got
            .iter()
            .position(|(k, v)| k == "credential.helper" && v.is_empty())
            .unwrap_or_else(|| panic!("{harness:?}: no reset in {got:?}"));
        let tail = &got[at..at + 15];
        assert!(
            got[at + 15..].iter().all(|(k, _)| matches!(
                k.as_str(),
                "gc.auto" | "maintenance.auto" | "core.hooksPath"
            )),
            "{harness:?}: {got:?}"
        );
        let helper =
            format!("!'{HELPER}' purlis-git-sentinel --host git.example.org --host github.com");
        assert_eq!(
            tail,
            [
                ("credential.helper".to_owned(), String::new()),
                (
                    "credential.https://git.example.org.helper".to_owned(),
                    helper.clone()
                ),
                ("credential.https://github.com.helper".to_owned(), helper),
                (
                    "http.https://git.example.org/.sslCAInfo".to_owned(),
                    ca.clone()
                ),
                ("http.https://github.com/.sslCAInfo".to_owned(), ca),
            ]
            .into_iter()
            .chain(
                [
                    "api.github.com",
                    "git.example.org",
                    "github.com",
                    "githubusercontent.com",
                    "gitlab.com",
                ]
                .into_iter()
                .flat_map(|host| [host.to_owned(), format!("*.{host}")])
                .map(|url| {
                    (
                        format!("http.https://{url}/.proxy"),
                        "http://127.0.0.1:44123".to_owned(),
                    )
                })
            )
            .collect::<Vec<_>>(),
            "{harness:?}"
        );
        // After the reset no pair resets again, and no helper but purlis's is named at all.
        let reset = got
            .iter()
            .rposition(|(k, v)| k == "credential.helper" && v.is_empty());
        let first_helper = got.iter().position(|(k, _)| {
            k.starts_with("credential.") && k.ends_with(".helper") && k != "credential.helper"
        });
        assert!(reset < first_helper, "{harness:?}: {got:?}");
        assert!(
            got.iter()
                .skip(reset.unwrap() + 1)
                .all(|(k, v)| k != "credential.helper" || !v.is_empty()),
            "{harness:?}: a pair empties the list after the reset"
        );
        for (k, v) in &got {
            if k.ends_with(".helper") && !v.is_empty() {
                assert!(
                    v.contains("purlis-git-sentinel"),
                    "{harness:?}: a helper that is not purlis's: {k}={v}"
                );
            }
            assert!(
                k != "http.sslCAInfo" && k != "http.extraHeader",
                "{harness:?}: {k}"
            );
        }
    }
}

#[test]
fn only_the_forge_lock_s_files_name_the_chat_s_config_and_the_wiring_s_gh_folder_wins() {
    purlis_core::unsteered!();
    let scratch = tempfile::tempdir().unwrap();
    let env = chat_env(Harness::Codex, scratch.path());
    // The lock's empty gh folder is replaced by the chat's own, which holds the socket setting.
    let gh = PathBuf::from(get(&env, "GH_CONFIG_DIR").unwrap());
    assert!(gh.starts_with(forgewire::wire_folder(scratch.path())));
    assert!(!gh.starts_with(scratch.path().join("home")));
    assert!(gh.join("config.yml").is_file());
    assert!(!gh.join("hosts.yml").exists());
    // The lock's global git file and empty system config still stand, so no helper of the
    // person's is read from a file at all.
    assert_eq!(get(&env, "GIT_CONFIG_SYSTEM"), Some("/dev/null"));
    assert!(get(&env, "GIT_CONFIG_GLOBAL").is_some_and(|g| g.ends_with("forge-lock/gitconfig")));
    assert_eq!(get(&env, "GIT_TERMINAL_PROMPT"), Some("0"));
}

#[test]
fn each_system_names_the_versions_it_ran_and_advises_on_any_other() {
    purlis_core::unsteered!();
    for os in [Os::Linux, Os::MacOs] {
        for tool in [Tool::Gh, Tool::Glab, Tool::Git] {
            let ran = forgewire::proven(tool, os).expect("a version");
            assert_eq!(
                forgewire::check_version(tool, os, Some(ran)),
                None,
                "{os:?} {tool:?}"
            );
            let other = forgewire::check_version(tool, os, Some("0.0.1")).expect("advice");
            assert_eq!(other.fix, Fix::Install { tool, version: ran });
            assert!(other.sentence.contains(ran) && other.sentence.contains("0.0.1"));
            // Never a refusal: advice, which a person dismisses.
            assert!(other.sentence.ends_with("or Dismiss to carry on."));
        }
    }
    // Recorded for exactly these two systems and these three tools, once each.
    assert_eq!(PROVEN.len(), 6);
    let Advice { cause, .. } = forgewire::check_version(Tool::Gh, Os::Linux, None).unwrap();
    assert_eq!(cause, "forge-route-unproven-gh");
}

#[test]
fn the_download_only_hosts_are_never_named_by_the_chat_s_trust_or_denied_as_a_tunnel() {
    purlis_core::unsteered!();
    let scratch = tempfile::tempdir().unwrap();
    let wiring = wiring(scratch.path());
    let denied = forgewire::denied_hosts(&repos());
    for host in DOWNLOAD_ONLY {
        assert!(!denied.iter().any(|d| d == host), "{host} is denied");
        // Tunnelled with its real certificate: no per-host CA, no helper.
        assert!(
            !wiring.git_pairs.iter().any(|(k, _)| k.contains(host)),
            "{host} is named in the git pairs"
        );
    }
    assert!(denied.iter().any(|d| d == "github.com") && denied.iter().any(|d| d == "gitlab.com"));
}
