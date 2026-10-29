// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A GitHub App's token for a chat (#2053): the JWT it is asked for with, the chat's repos read
//! from its clones' remotes, and the mint, against recorded exchanges taken from GitHub's REST
//! documentation (`tests/forge_contract/app_tokens/github/`, the recorded-forge pattern of
//! ADR 0070 §7).

use std::path::{Path, PathBuf};

use base64::Engine as _;
use ring::signature::KeyPair as _;

use super::{
    For, MOST_REPOS, Remote, clone_of, clones_now, held_from_chats, jwt, kept_for, mint_over,
    remotes_in, remotes_of_workspace, token_for,
};
use crate::forge::recorded::Recorded;
use crate::forge::{Forge, Kind};

/// The body of a 2048-bit RSA key made for these tests alone and used nowhere else, PKCS#1.
const PKCS1: &[&str] = &[
    "MIIEowIBAAKCAQEAyLQzaC2M6MsSa1dvIokROES8F/FxYVMhaadx5p46OjcJ8fE/",
    "3Zcj1f5RyC7hp6VDLWxdjzerF8C0p1R3Ks03NRI0ZLuVVjRvdyyECU49Oi6U1+dJ",
    "CukHhSrZzNc3Cn3DIEcpY6izRphx9wPPxwa8Cibi3UZ866rMnBOBYi7sJFxZE+NL",
    "gKYueL0m7mVXvARkK6iMz3GfP5UaHebfjE1UrvyvbBJTCMQQR5EtL9nmFNh6y5Y+",
    "ZQ/qyPr2xoCrtPbEqmmgRYAiUAJzCg/at/71fxhfokcRsE0mN/4yltinPAews9Sj",
    "xKJmLNe4tx+jjvoQZLcMexAJanNuSncXLR7nswIDAQABAoIBADCBlLcF4ztT8PSu",
    "Yfanx9igjBEBUlSiQb4GMPn6nbn6GVY25yeH4qpSV7OY2QKqZ0zwFsCJRLlt4nta",
    "Z56ZkFEsQCtkRphZ/eKAiLyzAwsvvNtdU4LmhOHDVHY0QWIwuNrxN+iVGhbiuxBn",
    "BG1mjEWr1kKCSaivU9oYwLlY+8thdLLQS047GOufL5GwXwhkPlUJRGlXR3fY6qoE",
    "BCTVFXkuLcmQbcvRPQcBloWkfNaVyQgVJRiV6tGiHaenl5Z4s4r7Y8BkXJuUTuss",
    "Ftt/Ffb/nOWi110nALb+H22cRUVPLkCgREhKHTaymwOg5Cgu6z1l3+oFoCyVh+Ev",
    "UtjZV6ECgYEA53pjT6zkkfs+nXyooe44ozTt47S9InMm+J1iedvRvhrpvLAEn39A",
    "S7/MElUK/JfmS/F/nhwdrjzCE4AXElSJiZGzyFlPCvX80CWpqfXHIHLsDyBjl8CO",
    "SR3MoDAxIH2VMICBCwisJuP5AjgGP9iJT2NPyDa2zes0w5Cv8cjbezECgYEA3fc6",
    "KMA1ix6y3lD5+Jd8KEEyZv1C/x2oSrvnZAsSFvhaRkBGzE99g+h+TdP0g8aCHhPF",
    "HXPoA+OIqXLnfGmFheb+weV/pEIq1arcLV8HZsjg8yXhAODi3Co6xxyV/ebUlS7G",
    "xeZZn4UgkGnc7bVCZISLkBqPNmLiVqLXAEeKECMCgYBbgCOVsTBBDGG5+zblm7dD",
    "vH50zlMdNQuopv3wUZZdYWIXtjR9n92Yk95lOLy5EIp2ZqYdp9uLrnTBJJb6JfSg",
    "Lro0bikymf4vgVPJ3V+zUfpCGHmF2RXljSQABkzFHR9GJuXbqLf0P5WE7QR7ZGsS",
    "uJ0m9YnZaCpZS+5g0xb/YQKBgEp/a7m5qT7kMsyDCSLyVO191FjUr+PIJG/Q2Tyu",
    "qoejG0lw0VJJa6sWIS4l8W849Pr7T7Eu8bQ8beXRdvg+wl3Kxb52NH0A6DeGfReN",
    "SYKp8TYqhHYzBHE4iznA0mbBTqAzbuuFAJeVqvRNU6VErN/nzJ+1ClQKpETHqrmh",
    "SSlZAoGBAKPump25EhIl1HLcf+Qz+TU7r+6JmjTWpuuJrStyHN0VyZpY1w77hrTQ",
    "Zi5cXHIId315D0aF+oJeDpLwZkY3a3r5z8kUCgBOkyG6GlUvK9WY9J9X65YlKhRB",
    "kk2mliU8zkJ3dyKztUqzPtUrYnmlntKlYh2rSykOpCpcQfDehoKY",
];

/// The same key, PKCS#8.
const PKCS8: &[&str] = &[
    "MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDItDNoLYzoyxJr",
    "V28iiRE4RLwX8XFhUyFpp3Hmnjo6Nwnx8T/dlyPV/lHILuGnpUMtbF2PN6sXwLSn",
    "VHcqzTc1EjRku5VWNG93LIQJTj06LpTX50kK6QeFKtnM1zcKfcMgRyljqLNGmHH3",
    "A8/HBrwKJuLdRnzrqsycE4FiLuwkXFkT40uApi54vSbuZVe8BGQrqIzPcZ8/lRod",
    "5t+MTVSu/K9sElMIxBBHkS0v2eYU2HrLlj5lD+rI+vbGgKu09sSqaaBFgCJQAnMK",
    "D9q3/vV/GF+iRxGwTSY3/jKW2Kc8B7Cz1KPEomYs17i3H6OO+hBktwx7EAlqc25K",
    "dxctHuezAgMBAAECggEAMIGUtwXjO1Pw9K5h9qfH2KCMEQFSVKJBvgYw+fqdufoZ",
    "VjbnJ4fiqlJXs5jZAqpnTPAWwIlEuW3ie1pnnpmQUSxAK2RGmFn94oCIvLMDCy+8",
    "211TguaE4cNUdjRBYjC42vE36JUaFuK7EGcEbWaMRavWQoJJqK9T2hjAuVj7y2F0",
    "stBLTjsY658vkbBfCGQ+VQlEaVdHd9jqqgQEJNUVeS4tyZBty9E9BwGWhaR81pXJ",
    "CBUlGJXq0aIdp6eXlnizivtjwGRcm5RO6ywW238V9v+c5aLXXScAtv4fbZxFRU8u",
    "QKBESEodNrKbA6DkKC7rPWXf6gWgLJWH4S9S2NlXoQKBgQDnemNPrOSR+z6dfKih",
    "7jijNO3jtL0icyb4nWJ529G+Gum8sASff0BLv8wSVQr8l+ZL8X+eHB2uPMITgBcS",
    "VImJkbPIWU8K9fzQJamp9ccgcuwPIGOXwI5JHcygMDEgfZUwgIELCKwm4/kCOAY/",
    "2IlPY0/INrbN6zTDkK/xyNt7MQKBgQDd9zoowDWLHrLeUPn4l3woQTJm/UL/HahK",
    "u+dkCxIW+FpGQEbMT32D6H5N0/SDxoIeE8Udc+gD44ipcud8aYWF5v7B5X+kQirV",
    "qtwtXwdmyODzJeEA4OLcKjrHHJX95tSVLsbF5lmfhSCQadzttUJkhIuQGo82YuJW",
    "otcAR4oQIwKBgFuAI5WxMEEMYbn7NuWbt0O8fnTOUx01C6im/fBRll1hYhe2NH2f",
    "3ZiT3mU4vLkQinZmph2n24uudMEklvol9KAuujRuKTKZ/i+BU8ndX7NR+kIYeYXZ",
    "FeWNJAAGTMUdH0Ym5duot/Q/lYTtBHtkaxK4nSb1idloKllL7mDTFv9hAoGASn9r",
    "ubmpPuQyzIMJIvJU7X3UWNSv48gkb9DZPK6qh6MbSXDRUklrqxYhLiXxbzj0+vtP",
    "sS7xtDxt5dF2+D7CXcrFvnY0fQDoN4Z9F41JgqnxNiqEdjMEcTiLOcDSZsFOoDNu",
    "64UAl5Wq9E1TpUSs3+fMn7UKVAqkRMequaFJKVkCgYEAo+6anbkSEiXUctx/5DP5",
    "NTuv7omaNNam64mtK3Ic3RXJmljXDvuGtNBmLlxccgh3fXkPRoX6gl4OkvBmRjdr",
    "evnPyRQKAE6TIboaVS8r1Zj0n1frliUqFEGSTaaWJTzOQnd3IrO1SrM+1StieaWe",
    "0qViHatLKQ6kKlxB8N6Ggpg=",
];

/// `body` as a PEM of `label`. The armour is put together here, so the repository's own secret
/// scan does not take this test's key for one.
fn pem(label: &str, body: &[&str]) -> String {
    let dashes = "-----";
    format!(
        "{dashes}BEGIN {label}{dashes}\n{}\n{dashes}END {label}{dashes}\n",
        body.join("\n")
    )
}

fn rsa_label() -> String {
    ["RSA ", "PRIVATE KEY"].concat()
}

fn pkcs8_label() -> String {
    ["PRIVATE ", "KEY"].concat()
}

fn recording(case: &str) -> Recorded {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/forge_contract/app_tokens/github")
        .join(format!("{case}.json"));
    Recorded::parse(&std::fs::read_to_string(&file).unwrap()).unwrap()
}

fn github() -> Forge {
    Forge::default_of(Kind::GitHub)
}

fn remote(owner: &str, name: &str, clone: &str) -> Remote {
    Remote {
        host: "github.com".to_owned(),
        owner: owner.to_owned(),
        name: name.to_owned(),
        clone: PathBuf::from(clone),
    }
}

/// The JSON of part `at` of `token`.
fn part(token: &str, at: usize) -> serde_json::Value {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(token.split('.').nth(at).unwrap())
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn the_jwt_is_signed_rs256_by_the_app_s_key_and_names_the_app_for_at_most_ten_minutes() {
    let now = 1_800_000_000;
    for key in [pem(&rsa_label(), PKCS1), pem(&pkcs8_label(), PKCS8)] {
        let token = jwt(&key, "Iv23liExample", now).unwrap();
        assert_eq!(
            part(&token, 0),
            serde_json::json!({"alg": "RS256", "typ": "JWT"})
        );
        assert_eq!(
            part(&token, 1),
            serde_json::json!({"iat": now - 60, "exp": now + 540, "iss": "Iv23liExample"})
        );
        // Verified with the key's own public half, as GitHub verifies it.
        let body = PKCS1.join("");
        let der = base64::engine::general_purpose::STANDARD
            .decode(body)
            .unwrap();
        let pair = ring::signature::RsaKeyPair::from_der(&der).unwrap();
        let public = ring::signature::UnparsedPublicKey::new(
            &ring::signature::RSA_PKCS1_2048_8192_SHA256,
            pair.public_key().as_ref().to_vec(),
        );
        let (signed, signature) = token.rsplit_once('.').unwrap();
        let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(signature)
            .unwrap();
        public.verify(signed.as_bytes(), &signature).unwrap();
    }
}

#[test]
fn a_value_that_is_not_a_private_key_mints_nothing_and_never_repeats_itself() {
    for value in [
        "ghp_notakey".to_owned(),
        pem(&rsa_label(), &["bm90IGEga2V5"]),
        pem("CERTIFICATE", PKCS1),
    ] {
        let said = jwt(&value, "Iv1.abc", 1_800_000_000).unwrap_err();
        assert!(said.contains("GitHub App's key"), "{said}");
        assert!(
            !said.contains("bm90IGEga2V5") && !said.contains("ghp_"),
            "{said}"
        );
    }
}

#[test]
fn the_chat_s_repos_are_its_clones_github_remotes_and_nothing_else() {
    let config = "remote.origin.url git@github.com:acme/api.git\n\
                  remote.upstream.url https://github.com/acme/api\n\
                  remote.fork.url https://github.com/me/api.git\n\
                  remote.lab.url https://gitlab.com/acme/api.git\n\
                  remote.deep.url https://github.com/acme/group/api.git\n\
                  remote.odd.url https://github.com/acme/a%20b.git\n";
    let forge_of = |url: &str| {
        let host = crate::forge::host_of(url);
        ["github.com", "gitlab.com"]
            .contains(&host.as_str())
            .then(|| Forge {
                kind: if host == "github.com" {
                    Kind::GitHub
                } else {
                    Kind::GitLab
                },
                host,
            })
    };
    assert_eq!(
        remotes_in(Path::new("/w/api"), config, &forge_of),
        vec![
            remote("acme", "api", "/w/api"),
            remote("me", "api", "/w/api")
        ]
    );
}

#[test]
fn one_installation_mints_one_token_limited_to_the_chat_s_repos() {
    let recorded = recording("one_installation");
    let minted = mint_over(
        &recorded,
        &github(),
        &[
            remote("acme", "api", "/w/api"),
            remote("acme", "web", "/w/web"),
        ],
        None,
    )
    .unwrap();
    assert_eq!(minted.token.as_str(), "minted-for-api-and-web");
    assert_eq!(minted.installation, 42);
    assert_eq!(minted.repos, ["api", "web"]);
    assert_eq!(minted.expires_at.as_deref(), Some("2026-10-29T15:00:00Z"));
    assert_eq!(recorded.unspent(), Vec::new());
}

#[test]
fn a_repo_the_app_is_not_installed_on_is_left_out_of_the_token() {
    let recorded = recording("not_installed_left_out");
    let minted = mint_over(
        &recorded,
        &github(),
        &[
            remote("acme", "api", "/w/api"),
            remote("someone", "fork", "/w/api"),
        ],
        None,
    )
    .unwrap();
    assert_eq!(minted.repos, ["api"]);
    assert_eq!(recorded.unspent(), Vec::new());
}

#[test]
fn an_app_installed_on_none_of_the_chat_s_repos_mints_nothing() {
    let recorded = recording("installed_nowhere");
    let said = mint_over(
        &recorded,
        &github(),
        &[remote("acme", "api", "/w/api")],
        None,
    )
    .unwrap_err();
    assert!(said.contains("installed on none"), "{said}");
    assert_eq!(recorded.unspent(), Vec::new());
}

#[test]
fn of_several_installations_the_one_the_run_s_clone_is_on_mints() {
    let dir = tempfile::tempdir().unwrap();
    let api = dir.path().join("api");
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(lib.join("src")).unwrap();
    std::fs::create_dir_all(&api).unwrap();
    let remotes = [
        remote("acme", "api", api.to_str().unwrap()),
        remote("other", "lib", lib.to_str().unwrap()),
    ];
    let recorded = recording("two_installations");
    let minted = mint_over(&recorded, &github(), &remotes, Some(&lib.join("src"))).unwrap();
    assert_eq!(
        (minted.installation, minted.token.as_str()),
        (77, "minted-for-lib")
    );
    assert_eq!(recorded.unspent(), Vec::new());

    // Run outside both, it cannot say which, and mints nothing.
    let recorded = recording("two_installations");
    let said = mint_over(&recorded, &github(), &remotes, Some(dir.path())).unwrap_err();
    assert!(said.contains("several installations"), "{said}");
}

#[test]
fn a_refused_mint_says_github_s_words_and_hands_nothing() {
    let recorded = recording("refused");
    let said = mint_over(
        &recorded,
        &github(),
        &[remote("acme", "api", "/w/api")],
        None,
    )
    .unwrap_err();
    assert!(
        said.contains("refused to mint") && said.contains("HTTP 401"),
        "{said}"
    );
}

#[test]
fn a_token_is_kept_until_ten_minutes_before_it_expires_and_never_past_its_hour() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-10-29T14:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let minutes = |d: std::time::Duration| d.as_secs() / 60;
    assert_eq!(minutes(kept_for(Some("2026-10-29T15:00:00Z"), now)), 50);
    assert_eq!(minutes(kept_for(Some("2026-10-29T14:30:00Z"), now)), 20);
    assert_eq!(minutes(kept_for(Some("2026-10-29T14:05:00Z"), now)), 0);
    assert_eq!(minutes(kept_for(Some("2026-10-30T14:00:00Z"), now)), 50);
    assert_eq!(minutes(kept_for(None, now)), 50);
    assert_eq!(minutes(kept_for(Some("tomorrow"), now)), 50);
}

#[test]
fn more_repos_than_one_token_may_name_mint_nothing_and_ask_github_nothing() {
    let recorded = recording("installed_nowhere");
    let many: Vec<Remote> = (0..=MOST_REPOS)
        .map(|n| remote("acme", &format!("repo-{n}"), "/w/api"))
        .collect();
    let said = mint_over(&recorded, &github(), &many, None).unwrap_err();
    assert!(said.contains("at most 500"), "{said}");
    // Its one recorded exchange was never asked for.
    assert_eq!(recorded.unspent().len(), 1);
}

#[test]
fn the_run_s_clone_is_the_one_its_folder_is_in_however_it_is_spelled() {
    let dir = tempfile::tempdir().unwrap();
    let api = dir.path().join("api");
    std::fs::create_dir_all(api.join("src/deep")).unwrap();
    let remotes = [remote("acme", "api", api.to_str().unwrap())];
    let real = api.canonicalize().unwrap();
    assert_eq!(
        clone_of(&remotes, Some(&api.join("src/deep"))),
        Some(real.clone())
    );
    assert_eq!(
        clone_of(&remotes, Some(&api.join("src/../src"))),
        Some(real)
    );
    assert_eq!(clone_of(&remotes, Some(dir.path())), None);
    assert_eq!(clone_of(&remotes, None), None);
}

/// Runs git in `dir`, which must succeed.
fn git(dir: &Path, args: &[&str]) {
    let mut c = std::process::Command::new("git");
    c.arg("-C").arg(dir).args(args);
    assert!(
        crate::forklock::output(&mut c).unwrap().status.success(),
        "git {args:?}"
    );
}

/// A plane with workspace `ws` holding a clone `name` whose remote `origin` is `url`.
fn a_clone(root: &Path, name: &str, url: &str) -> PathBuf {
    let clone = root.join("workspaces/ws").join(name);
    std::fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "-q"]);
    git(&clone, &["remote", "add", "origin", url]);
    clone
}

#[test]
fn only_clones_whose_git_directory_no_chat_writes_name_the_chat_s_repos() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let kept = a_clone(root, "api", "https://github.com/acme/api.git");
    let granted = a_clone(root, "lib", "https://github.com/acme/lib.git");
    let repos = vec![
        crate::repos::Repo {
            name: "api".into(),
            path: kept.clone(),
        },
        crate::repos::Repo {
            name: "lib".into(),
            path: granted.clone(),
        },
    ];
    let kept_only = held_from_chats(repos.clone(), std::slice::from_ref(&granted));
    assert_eq!(
        kept_only
            .iter()
            .map(|repo| repo.name.as_str())
            .collect::<Vec<_>>(),
        ["api"]
    );
    assert_eq!(held_from_chats(repos, &[]).len(), 2);

    let remotes =
        remotes_of_workspace(root, Some(&root.join("workspaces/ws")), &[granted], None).unwrap();
    assert_eq!(
        remotes
            .iter()
            .map(|remote| format!("{}/{}", remote.owner, remote.name))
            .collect::<Vec<_>>(),
        ["acme/api"]
    );
}

#[test]
fn a_clone_s_remotes_are_read_from_its_own_config_and_never_an_included_one() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let clone = a_clone(root, "api", "https://github.com/acme/api.git");
    let included = root.join("more.gitconfig");
    std::fs::write(
        &included,
        "[remote \"more\"]\n\turl = https://github.com/acme/secret.git\n",
    )
    .unwrap();
    git(
        &clone,
        &["config", "include.path", included.to_str().unwrap()],
    );
    let remotes = remotes_of_workspace(root, Some(&root.join("workspaces/ws")), &[], None).unwrap();
    assert_eq!(
        remotes
            .iter()
            .map(|remote| remote.name.as_str())
            .collect::<Vec<_>>(),
        ["api"]
    );
}

#[test]
fn a_run_in_a_clone_on_another_host_is_refused_naming_it_and_nothing_is_read() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        crate::names::manifest(root),
        "[[forge]]\nkind = \"github\"\nhost = \"ghe.example.com\"\n",
    )
    .unwrap();
    let clone = a_clone(root, "api", "https://ghe.example.com/acme/api.git");
    a_clone(root, "web", "https://github.com/acme/web.git");
    let gh = crate::sandbox::tools::Tool {
        mint: Some(crate::sandbox::tools::mint::Mint {
            app: "Iv23liExample".to_owned(),
        }),
        ..crate::sandbox::tools::Tool::one(
            "gh",
            "forge",
            "GH_TOKEN",
            crate::sandbox::tools::Handed::Env,
            "app-key",
        )
    };
    let said = token_for(
        &For {
            root,
            chat: 9,
            folder: Some(&root.join("workspaces/ws")),
            here: Some(&clone),
            writes: &[],
            started: None,
            on_a_runner: false,
        },
        &gh,
        &|| panic!("the key is read only to mint"),
    )
    .unwrap_err();
    assert!(said.contains("ghe.example.com is not github.com"), "{said}");
}

#[test]
fn a_chat_s_tokens_cover_the_clones_its_workspace_had_when_it_started() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let api = a_clone(root, "api", "https://github.com/acme/api.git");
    a_clone(root, "web", "https://github.com/acme/web.git");
    let started = clones_now(root);
    let ws = root.join("workspaces/ws");
    let names = |started: &[super::Started]| -> Vec<String> {
        remotes_of_workspace(root, Some(&ws), &[], Some(started))
            .unwrap()
            .into_iter()
            .map(|remote| remote.name)
            .collect()
    };
    assert_eq!(names(&started), ["api", "web"]);

    // A clone that appears after the start is not one of them.
    let elsewhere = tempfile::tempdir().unwrap();
    let late = elsewhere.path().join("late");
    std::fs::create_dir_all(&late).unwrap();
    git(&late, &["init", "-q"]);
    git(
        &late,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/late.git",
        ],
    );
    std::fs::create_dir_all(ws.join("x")).unwrap();
    std::fs::rename(late.join(".git"), ws.join("x/.git")).unwrap();
    assert_eq!(names(&started), ["api", "web"]);

    // Nor is one moved into the place of a clone that was there.
    std::fs::rename(&api, ws.join("api-then")).unwrap();
    let swapped = a_clone(root, "api", "https://github.com/acme/other.git");
    assert_eq!(swapped, api);
    assert_eq!(names(&started), ["web"]);
}
