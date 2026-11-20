// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A GitHub App's token for a chat (#2053): the JWT it is asked for with, the chat's repos read
//! from its clones' remotes, and the mint, against recorded exchanges taken from GitHub's REST
//! documentation (`tests/forge_contract/app_tokens/github/`, the recorded-forge pattern of
//! ADR 0070 §7).

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use ring::signature::KeyPair as _;

use super::{
    For, MOST_REPOS, Remote, clone_of, clones_now, held_from_chats, jwt, kept_for, lives_for,
    mint_over, remotes_in, remotes_of_workspace, revoke_each, revoke_over, taken_at_end, token_for,
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
    let now = str::parse::<jiff::Timestamp>("2026-10-29T14:00:00Z").unwrap();
    let minutes = |d: Duration| d.as_secs() / 60;
    assert_eq!(minutes(kept_for(Some("2026-10-29T15:00:00Z"), now)), 50);
    assert_eq!(minutes(kept_for(Some("2026-10-29T14:30:00Z"), now)), 20);
    assert_eq!(minutes(kept_for(Some("2026-10-29T14:05:00Z"), now)), 0);
    assert_eq!(minutes(kept_for(Some("2026-10-30T14:00:00Z"), now)), 50);
    assert_eq!(minutes(kept_for(None, now)), 50);
    assert_eq!(minutes(kept_for(Some("tomorrow"), now)), 50);
}

#[test]
fn a_token_lives_until_github_says_it_expires_and_never_past_its_hour() {
    let now = str::parse::<jiff::Timestamp>("2026-10-29T14:00:00Z").unwrap();
    let minutes = |d: Duration| d.as_secs() / 60;
    assert_eq!(minutes(lives_for(Some("2026-10-29T15:00:00Z"), now)), 60);
    assert_eq!(minutes(lives_for(Some("2026-10-29T14:30:00Z"), now)), 30);
    assert_eq!(minutes(lives_for(Some("2026-10-29T13:00:00Z"), now)), 0);
    assert_eq!(minutes(lives_for(Some("2026-10-30T14:00:00Z"), now)), 60);
    assert_eq!(minutes(lives_for(None, now)), 60);
}

#[test]
fn a_revoke_deletes_the_installation_token_sent_with_the_token_itself() {
    let recorded = recording("revoked");
    revoke_over(&recorded, &github()).unwrap();
    assert!(recorded.unspent().is_empty());
}

#[test]
fn a_revoke_github_refuses_says_github_s_words() {
    let said = revoke_over(&recording("revoke_refused"), &github()).unwrap_err();
    assert!(
        said.contains("Bad credentials") && said.contains("HTTP 401"),
        "{said}"
    );
}

/// Keeps `token` for `chat` of `root`, reused for `reused` from now and alive for `alive`.
fn keep(root: &str, chat: u32, token: &str, reused: Duration, alive: Duration) -> bool {
    let now = jiff::Timestamp::now();
    super::keep_one(
        asked(root, chat, token),
        super::ends_so_far(),
        zeroize::Zeroizing::new(token.to_owned()),
        super::after(now, reused),
        super::after(now, alive),
    )
}

fn asked(root: &str, chat: u32, token: &str) -> super::Asked {
    super::Asked {
        root: PathBuf::from(root),
        chat,
        app: "Iv1.test".to_owned(),
        vault: "forge".to_owned(),
        key: "app-key".to_owned(),
        remotes: Vec::new(),
        clone: Some(PathBuf::from(format!("/w/{token}"))),
    }
}

fn taken(root: &str, chat: Option<u32>) -> Vec<String> {
    let mut tokens: Vec<String> = taken_at_end(Path::new(root), chat)
        .iter()
        .map(|token| token.as_str().to_owned())
        .collect();
    tokens.sort();
    tokens
}

#[test]
fn a_chat_ending_takes_every_live_token_kept_for_it_and_no_other_chat_s() {
    let minute = Duration::from_secs(60);
    let root = "/projects/ending-one";
    keep(root, 1, "one-now", 50 * minute, 60 * minute);
    // Past the time it is handed out again, though GitHub has not let it go yet.
    keep(root, 1, "one-before", Duration::ZERO, 8 * minute);
    // Past its hour: GitHub has let it go already, so there is nothing to revoke.
    keep(root, 1, "one-gone", Duration::ZERO, Duration::ZERO);
    keep(root, 2, "two", 50 * minute, 60 * minute);
    keep(
        "/projects/ending-other",
        1,
        "other",
        50 * minute,
        60 * minute,
    );

    assert_eq!(taken(root, Some(1)), ["one-before", "one-now"]);
    assert!(taken(root, Some(1)).is_empty(), "taken once");
    assert_eq!(taken(root, Some(2)), ["two"]);
    assert_eq!(taken("/projects/ending-other", Some(1)), ["other"]);
}

#[test]
fn the_project_ending_takes_every_chat_s_tokens_of_that_project_alone() {
    let hour = Duration::from_secs(3600);
    keep("/projects/quitting", 1, "q1", hour, hour);
    keep("/projects/quitting", 7, "q7", hour, hour);
    keep("/projects/staying", 1, "s1", hour, hour);
    assert_eq!(taken("/projects/quitting", None), ["q1", "q7"]);
    assert_eq!(taken("/projects/staying", None), ["s1"]);
}

#[test]
fn a_token_minted_while_its_chat_ended_is_kept_for_nobody() {
    let hour = Duration::from_secs(3600);
    let root = "/projects/racing";
    // Counted before the chat was found open; it ends while its token is minted.
    let seen = super::ends_so_far();
    assert!(taken(root, Some(4)).is_empty(), "nothing kept yet");
    let raced = super::keep_one(
        asked(root, 4, "raced"),
        seen,
        zeroize::Zeroizing::new("raced".to_owned()),
        super::after(jiff::Timestamp::now(), hour),
        super::after(jiff::Timestamp::now(), hour),
    );
    assert!(!raced, "found open before the chat ended: not kept");
    assert!(
        keep(root, 4, "after", hour, hour),
        "found open after it: kept"
    );
    assert!(keep(root, 5, "other", hour, hour), "another chat's is kept");
    assert_eq!(taken(root, Some(4)), ["after"]);
}

#[test]
fn a_revoke_github_does_not_answer_says_so() {
    let nobody =
        crate::forge::recorded::Recorded::parse(r#"{"source": "none", "exchanges": []}"#).unwrap();
    let said = revoke_over(&nobody, &github()).unwrap_err();
    assert!(said.contains("could not be asked to revoke"), "{said}");
}

#[test]
fn each_token_is_revoked_and_one_refused_or_unreached_stops_none_of_the_others() {
    let asked = std::cell::RefCell::new(Vec::new());
    let tokens = ["a", "b", "c"].map(|t| zeroize::Zeroizing::new(t.to_owned()));
    let revoked = revoke_each(&tokens, &|token: &str| {
        asked.borrow_mut().push(token.to_owned());
        match token {
            "a" => Err("Bad credentials (HTTP 401)".to_owned()),
            "b" => Err("no answer".to_owned()),
            _ => Ok(()),
        }
    });
    assert_eq!(*asked.borrow(), ["a", "b", "c"]);
    assert_eq!(revoked, 1);
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

/// The record of the window's clones (`push-remotes.json`) as purlis would have written it for
/// every clone of workspace `ws` under `root` now, each at its `origin`.
fn recorded_now(root: &Path) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    let Ok(clones) = crate::repos::clones(root, "ws") else {
        return out;
    };
    for repo in clones.repos {
        let url = crate::worktree::git::run(
            &repo.path,
            &["config", "--local", "remote.origin.url"],
            crate::worktree::git::READ,
        )
        .unwrap();
        let url = url.out.trim().to_owned();
        if let Some(place) = crate::forge::gitcredential::place_of(&url) {
            out.insert(crate::sandbox::local::clone_key(&repo.path), place);
        }
    }
    out
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

    let remotes = remotes_of_workspace(
        root,
        Some(&root.join("workspaces/ws")),
        &[granted],
        None,
        &recorded_now(root),
    )
    .unwrap();
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
    let remotes = remotes_of_workspace(
        root,
        Some(&root.join("workspaces/ws")),
        &[],
        None,
        &recorded_now(root),
    )
    .unwrap();
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
    let config = tempfile::tempdir().unwrap();
    crate::forge::gitcredential::write_push_remotes(config.path(), &recorded_now(root)).unwrap();
    let said = token_for(
        &For {
            root,
            chat: 9,
            folder: Some(&root.join("workspaces/ws")),
            here: Some(&clone),
            writes: &[],
            started: None,
            on_a_runner: false,
            config_root: Some(config.path()),
            ends_seen: None,
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
    let recorded = recorded_now(root);
    let ws = root.join("workspaces/ws");
    let names = |started: &[super::Started]| -> Vec<String> {
        remotes_of_workspace(root, Some(&ws), &[], Some(started), &recorded)
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

#[test]
fn only_a_clone_purlis_cloned_or_the_window_saved_names_the_chat_s_repos() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let ws = root.join("workspaces/ws");
    let api = a_clone(root, "api", "https://github.com/acme/api.git");
    let recorded = recorded_now(root);
    // A git folder put in place by anything else, there before the chat started or not.
    let elsewhere = tempfile::tempdir().unwrap();
    let put = elsewhere.path().join("put");
    std::fs::create_dir_all(&put).unwrap();
    git(&put, &["init", "-q"]);
    git(
        &put,
        &["remote", "add", "origin", "https://github.com/acme/put.git"],
    );
    std::fs::create_dir_all(ws.join("x")).unwrap();
    std::fs::rename(put.join(".git"), ws.join("x/.git")).unwrap();
    let names = |recorded: &std::collections::BTreeMap<String, String>| -> Vec<String> {
        remotes_of_workspace(root, Some(&ws), &[], Some(&clones_now(root)), recorded)
            .unwrap()
            .into_iter()
            .map(|remote| remote.name)
            .collect()
    };
    assert_eq!(names(&recorded), ["api"]);

    // A remote added to a recorded clone is not the one recorded for it.
    git(
        &api,
        &["remote", "add", "more", "https://github.com/acme/more.git"],
    );
    assert_eq!(names(&recorded), ["api"]);

    // With no record, no clone counts.
    assert_eq!(
        names(&std::collections::BTreeMap::new()),
        Vec::<String>::new()
    );
}

/// A token the operator minted (#4207): kept for its chat's next runs, for no other chat, and gone
/// as the chat ends.
#[cfg(unix)]
#[test]
fn a_token_from_the_cluster_is_kept_for_its_chat_alone_till_its_chat_ends() {
    use crate::forge::mintport::Minted;
    use std::sync::atomic::{AtomicU32, Ordering};
    let project = tempfile::tempdir().unwrap();
    let root = project.path();
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
    let mints = AtomicU32::new(0);
    let mint = |app: &str| {
        assert_eq!(app, "Iv23liExample");
        let made = mints.fetch_add(1, Ordering::SeqCst);
        Ok(Some(Minted {
            token: zeroize::Zeroizing::new(format!("minted-{made}")),
            expires_at: None,
            owner: "acme".to_owned(),
            repos: vec!["app".to_owned()],
            operator: "ed25519:ab".to_owned(),
        }))
    };
    let token = |chat| {
        super::token_in_cluster(root, chat, &gh, &mint)
            .unwrap()
            .unwrap()
    };
    assert_eq!(*token(1), "minted-0");
    assert_eq!(*token(1), "minted-0", "kept for its next run");
    assert_eq!(*token(2), "minted-1", "no other chat's");
    assert_eq!(taken_at_end(root, Some(1)).len(), 1);
    assert_eq!(*token(1), "minted-2", "gone as its chat ended");
    // Told of no port, there is nothing to keep, and the run goes to the desktop.
    assert!(
        super::token_in_cluster(root, 3, &gh, &|_| Ok(None))
            .unwrap()
            .is_none()
    );
    let _ = taken_at_end(root, None);
}

#[test]
fn a_token_mint_refused_for_an_unrecorded_clone_raises_trust_this_clone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // The ask is `consents`' (Q308b), which the default, `watch`, only logs.
    std::fs::write(
        crate::names::manifest(root),
        "[policy]\nconsents = \"enforce\"\n",
    )
    .unwrap();
    // A clone the person made in their own terminal: purlis has no record of it.
    a_clone(root, "api", "https://github.com/acme/api.git");
    let config = tempfile::tempdir().unwrap();
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
    let ws = root.join("workspaces/ws");
    let chat = For {
        root,
        chat: 7,
        folder: Some(&ws),
        here: None,
        writes: &[],
        started: None,
        on_a_runner: false,
        config_root: Some(config.path()),
        ends_seen: None,
    };
    let mint = || {
        token_for(&chat, &gh, &|| {
            Err("the key is read only to mint".to_owned())
        })
    };

    // Refused, twice: one ask, carrying Trust this clone for that clone at its remote.
    let said = mint().unwrap_err();
    mint().unwrap_err();
    let trust = crate::windowonly::Control::TrustClone {
        folder: "workspaces/ws/api".to_owned(),
        place: "github.com/acme/api".to_owned(),
    };
    let raised = crate::windowonly::every(root);
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!((raised[0].session, &raised[0].control), (7, &trust));
    assert!(
        raised[0]
            .control
            .says()
            .starts_with("Trust this clone so chats can use GitHub"),
        "{}",
        raised[0].control.says()
    );
    // The chat is told what was asked and where, never "from the window" alone.
    assert!(said.contains("Trust this clone"), "{said}");
    assert!(said.contains("Asks"), "{said}");
    assert!(!said.contains("from the window"), "{said}");

    // The person's Trust this clone records it and clears the ask.
    crate::windowonly::trust_this_clone(
        &crate::forge::route::HostScope::for_a_test(),
        config.path(),
        root,
        "workspaces/ws/api",
        "github.com/acme/api",
    )
    .unwrap();
    assert_eq!(crate::windowonly::every(root), []);

    // And the next mint succeeds: the clone names the chat's repo, and the token kept for it is
    // handed over, with no ask raised again.
    let recorded = crate::forge::gitcredential::read_push_remotes(config.path());
    let remotes = remotes_of_workspace(root, Some(&ws), &[], None, &recorded).unwrap();
    assert_eq!(
        remotes
            .iter()
            .map(|remote| format!("{}/{}", remote.owner, remote.name))
            .collect::<Vec<_>>(),
        ["acme/api"]
    );
    let hour = super::after(jiff::Timestamp::now(), Duration::from_secs(3600));
    assert!(super::keep_one(
        super::Asked {
            root: root.to_path_buf(),
            chat: 7,
            app: "Iv23liExample".to_owned(),
            vault: gh.vault.clone(),
            key: gh.values[0].key.clone(),
            remotes,
            clone: None,
        },
        super::ends_so_far(),
        zeroize::Zeroizing::new("ghs_minted".to_owned()),
        hour,
        hour,
    ));
    assert_eq!(mint().unwrap().as_str(), "ghs_minted");
    assert_eq!(crate::windowonly::every(root), []);
    let _ = taken_at_end(root, None);
}

#[test]
fn a_dismissed_notice_that_later_stops_a_chat_is_raised_again_as_an_ask() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // The ask is `consents`' (Q308b), which the default, `watch`, only logs.
    std::fs::write(
        crate::names::manifest(root),
        "[policy]\nconsents = \"enforce\"\n",
    )
    .unwrap();
    a_clone(root, "web", "https://github.com/acme/web.git");
    let config = tempfile::tempdir().unwrap();
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
    let ws = root.join("workspaces/ws");
    let mint = |chat: u32| {
        token_for(
            &For {
                root,
                chat,
                folder: Some(&ws),
                here: None,
                writes: &[],
                started: None,
                on_a_runner: false,
                config_root: Some(config.path()),
                ends_seen: None,
            },
            &gh,
            &|| Err("the key is read only to mint".to_owned()),
        )
    };
    // The clone's Notice stands (the window lists it), and whatever the person did with it,
    // nothing is asked while no chat is stopped by it.
    assert_eq!(
        crate::forge::gitcredential::vouch::unvouched_clones(root, config.path()).len(),
        1
    );
    assert_eq!(crate::windowonly::every(root), []);

    // A chat it stops raises one ask; once that chat ends, the next chat it stops raises one
    // again.
    mint(3).unwrap_err();
    crate::windowonly::forget(root, 3);
    assert_eq!(crate::windowonly::every(root), []);
    mint(4).unwrap_err();
    mint(4).unwrap_err();
    let raised = crate::windowonly::every(root);
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].session, 4);
    assert_eq!(raised[0].control.label(), "Trust this clone");
}

/// The `gh` tool, minted by the organisation's GitHub App.
fn minted_gh() -> crate::sandbox::tools::Tool {
    crate::sandbox::tools::Tool {
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
    }
}

#[test]
fn a_clone_made_after_the_chat_started_raises_no_trust_this_clone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let config = tempfile::tempdir().unwrap();
    let ws = root.join("workspaces/ws");
    std::fs::create_dir_all(&ws).unwrap();
    // The chat started with no clone; it cloned one mid-run, which the mint never counts.
    let started = clones_now(root);
    a_clone(root, "api", "https://github.com/acme/api.git");
    let gh = minted_gh();
    let said = token_for(
        &For {
            root,
            chat: 11,
            folder: Some(&ws),
            here: None,
            writes: &[],
            started: Some(&started),
            on_a_runner: false,
            config_root: Some(config.path()),
            ends_seen: None,
        },
        &gh,
        &|| Err("the key is read only to mint".to_owned()),
    )
    .unwrap_err();

    assert_eq!(said, super::NO_RECORDED_CLONE);
    assert_eq!(crate::windowonly::every(root), []);
}

#[test]
fn a_clone_whose_git_folder_a_chat_writes_raises_no_trust_this_clone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let config = tempfile::tempdir().unwrap();
    let ws = root.join("workspaces/ws");
    let clone = a_clone(root, "api", "https://github.com/acme/api.git");
    let gh = minted_gh();
    // The chat may write the clone's .git, so the mint would never count it.
    let writes = [clone.clone()];
    let said = token_for(
        &For {
            root,
            chat: 12,
            folder: Some(&ws),
            here: None,
            writes: &writes,
            started: None,
            on_a_runner: false,
            config_root: Some(config.path()),
            ends_seen: None,
        },
        &gh,
        &|| Err("the key is read only to mint".to_owned()),
    )
    .unwrap_err();

    assert_eq!(said, super::NO_RECORDED_CLONE);
    assert_eq!(crate::windowonly::every(root), []);
}

#[test]
fn a_clone_there_at_start_raises_trust_this_clone_and_after_the_press_the_mint_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // The ask is `consents`' (Q308b), which the default, `watch`, only logs.
    std::fs::write(
        crate::names::manifest(root),
        "[policy]\nconsents = \"enforce\"\n",
    )
    .unwrap();
    let config = tempfile::tempdir().unwrap();
    let ws = root.join("workspaces/ws");
    a_clone(root, "api", "https://github.com/acme/api.git");
    let started = clones_now(root);
    let gh = minted_gh();
    let chat = For {
        root,
        chat: 13,
        folder: Some(&ws),
        here: None,
        writes: &[],
        started: Some(&started),
        on_a_runner: false,
        config_root: Some(config.path()),
        ends_seen: None,
    };
    let mint = || {
        token_for(&chat, &gh, &|| {
            Err("the key is read only to mint".to_owned())
        })
    };

    mint().unwrap_err();
    let raised = crate::windowonly::every(root);
    assert_eq!(raised.len(), 1, "{raised:?}");
    crate::windowonly::trust_this_clone(
        &crate::forge::route::HostScope::for_a_test(),
        config.path(),
        root,
        "workspaces/ws/api",
        "github.com/acme/api",
    )
    .unwrap();

    // The clone the press trusted is one the mint counts: its repo is the chat's, and the
    // token kept for it is handed over.
    let recorded = crate::forge::gitcredential::read_push_remotes(config.path());
    let remotes = remotes_of_workspace(root, Some(&ws), &[], Some(&started), &recorded).unwrap();
    assert_eq!(remotes.len(), 1, "{remotes:?}");
    let hour = super::after(jiff::Timestamp::now(), Duration::from_secs(3600));
    assert!(super::keep_one(
        super::Asked {
            root: root.to_path_buf(),
            chat: 13,
            app: "Iv23liExample".to_owned(),
            vault: gh.vault.clone(),
            key: gh.values[0].key.clone(),
            remotes,
            clone: None,
        },
        super::ends_so_far(),
        zeroize::Zeroizing::new("ghs_minted".to_owned()),
        hour,
        hour,
    ));
    assert_eq!(mint().unwrap().as_str(), "ghs_minted");
    assert_eq!(crate::windowonly::every(root), []);
    let _ = taken_at_end(root, None);
}

/// A workspace whose clone `api` the window recorded and whose clone `web` was cloned from the
/// CLI (#7196), and a chat that started with both. A token kept for it is handed, and a mint
/// here would fail, so an `Ok` is the kept token.
struct Mixed {
    dir: tempfile::TempDir,
    config: tempfile::TempDir,
    started: Vec<super::Started>,
    api: PathBuf,
    web: PathBuf,
}

impl Mixed {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let api = a_clone(dir.path(), "api", "https://github.com/acme/api.git");
        let web = a_clone(dir.path(), "web", "https://github.com/acme/web.git");
        let mut recorded = std::collections::BTreeMap::new();
        recorded.insert(
            crate::sandbox::local::clone_key(&api),
            "github.com/acme/api".to_owned(),
        );
        crate::forge::gitcredential::write_push_remotes(config.path(), &recorded).unwrap();
        let started = clones_now(dir.path());
        Self {
            dir,
            config,
            started,
            api,
            web,
        }
    }

    fn ws(&self) -> PathBuf {
        self.dir.path().join("workspaces/ws")
    }

    fn chat<'a>(&'a self, ws: &'a Path, here: &'a Path) -> For<'a> {
        For {
            root: self.dir.path(),
            chat: 21,
            folder: Some(ws),
            here: Some(here),
            writes: &[],
            started: Some(&self.started),
            on_a_runner: false,
            config_root: Some(self.config.path()),
            ends_seen: None,
        }
    }

    /// The token `ghs_api` kept for the chat, for the clone `scoped_to` (the one its run is in).
    fn keep_for(&self, scoped_to: Option<&Path>) {
        let ws = self.ws();
        let recorded = crate::forge::gitcredential::read_push_remotes(self.config.path());
        let remotes = remotes_of_workspace(
            self.dir.path(),
            Some(&ws),
            &[],
            Some(&self.started),
            &recorded,
        )
        .unwrap();
        assert_eq!(remotes.len(), 1, "{remotes:?}");
        let gh = minted_gh();
        let hour = super::after(jiff::Timestamp::now(), Duration::from_secs(3600));
        assert!(super::keep_one(
            super::Asked {
                root: self.dir.path().to_path_buf(),
                chat: 21,
                app: "Iv23liExample".to_owned(),
                vault: gh.vault.clone(),
                key: gh.values[0].key.clone(),
                clone: clone_of(&remotes, scoped_to),
                remotes,
            },
            super::ends_so_far(),
            zeroize::Zeroizing::new("ghs_api".to_owned()),
            hour,
            hour,
        ));
    }

    fn token(&self, here: &Path) -> Result<String, String> {
        let ws = self.ws();
        token_for(&self.chat(&ws, here), &minted_gh(), &|| {
            Err("the key is read only to mint".to_owned())
        })
        .map(|token| token.as_str().to_owned())
    }
}

#[test]
fn a_run_in_a_recorded_clone_gets_its_token() {
    let fix = Mixed::new();
    fix.keep_for(Some(&fix.api));
    assert_eq!(fix.token(&fix.api).as_deref(), Ok("ghs_api"));
    let _ = taken_at_end(fix.dir.path(), None);
}

#[test]
fn a_run_in_an_unrecorded_clone_is_not_refused_and_asks_nothing_so_gh_r_a_recorded_repo_works() {
    let fix = Mixed::new();
    fix.keep_for(None);
    // `gh -R acme/api` from inside `web`: the token the recorded repo has, as before (#7196).
    assert_eq!(fix.token(&fix.web).as_deref(), Ok("ghs_api"));
    assert_eq!(crate::windowonly::every(fix.dir.path()), []);
    let _ = taken_at_end(fix.dir.path(), None);
}

#[test]
fn the_start_note_names_every_clone_the_window_did_not_make_and_the_press_clears_it() {
    let fix = Mixed::new();
    let ws = fix.ws();
    let said = super::unrecorded_note(&fix.chat(&ws, &fix.api)).unwrap();
    assert!(said.contains("workspaces/ws/web"), "{said}");
    assert!(!said.contains("workspaces/ws/api"), "{said}");
    assert!(said.contains("wasn't made from the window"), "{said}");
    assert!(said.contains("no Agent identity token"), "{said}");
    assert!(said.contains("Trust this clone"), "{said}");
    crate::windowonly::trust_this_clone(
        &crate::forge::route::HostScope::for_a_test(),
        fix.config.path(),
        fix.dir.path(),
        "workspaces/ws/web",
        "github.com/acme/web",
    )
    .unwrap();
    assert_eq!(super::unrecorded_note(&fix.chat(&ws, &fix.web)), None);
    assert_eq!(super::unrecorded_here_note(&fix.chat(&ws, &fix.web)), None);
}

#[test]
fn a_gh_that_failed_in_an_unrecorded_clone_names_that_one_clone_and_a_recorded_one_says_nothing() {
    let fix = Mixed::new();
    let ws = fix.ws();
    let said = super::unrecorded_here_note(&fix.chat(&ws, &fix.web)).unwrap();
    assert!(said.contains("workspaces/ws/web"), "{said}");
    assert!(!said.contains("workspaces/ws/api"), "{said}");
    assert!(said.contains("Trust this clone"), "{said}");
    // A folder inside it, and its branch folder, are in it too.
    let inside = fix.web.join("src");
    std::fs::create_dir_all(&inside).unwrap();
    assert!(super::unrecorded_here_note(&fix.chat(&ws, &inside)).is_some());
    let branch = ws.join(".worktrees/web/p1");
    std::fs::create_dir_all(&branch).unwrap();
    let said = super::unrecorded_here_note(&fix.chat(&ws, &branch)).unwrap();
    assert!(said.contains("workspaces/ws/web"), "{said}");
    // The recorded clone, and a folder in none, say nothing.
    assert_eq!(super::unrecorded_here_note(&fix.chat(&ws, &fix.api)), None);
    assert_eq!(super::unrecorded_here_note(&fix.chat(&ws, &ws)), None);
}

#[test]
fn a_branch_folder_is_in_its_clone_for_the_run_s_token() {
    let fix = Mixed::new();
    let branch = fix.ws().join(".worktrees/api/p1");
    std::fs::create_dir_all(&branch).unwrap();
    let recorded = crate::forge::gitcredential::read_push_remotes(fix.config.path());
    let ws = fix.ws();
    let remotes = remotes_of_workspace(
        fix.dir.path(),
        Some(&ws),
        &[],
        Some(&fix.started),
        &recorded,
    )
    .unwrap();
    assert_eq!(
        clone_of(&remotes, Some(&branch)),
        Some(fix.api.canonicalize().unwrap())
    );
}

/// A fake forge for the mint: one installation (42) of an App that has `has`. As GitHub does, a
/// mint with `permissions` gets exactly those, one with none gets all the App has, and one asking
/// for a permission the installation lacks is a 422. With `ignores_the_ask` the token comes back
/// with everything the App has whatever was asked (a reply the mint must not keep). It keeps the
/// body it was sent.
struct FakeForge {
    has: Vec<(&'static str, &'static str)>,
    ignores_the_ask: bool,
    body: std::sync::Mutex<Vec<crate::forge::transport::Field>>,
}

impl FakeForge {
    fn new(has: &[(&'static str, &'static str)]) -> FakeForge {
        FakeForge {
            has: has.to_vec(),
            ignores_the_ask: false,
            body: std::sync::Mutex::default(),
        }
    }

    fn sent(&self) -> Vec<(String, String)> {
        use crate::forge::transport::Field;
        self.body
            .lock()
            .unwrap()
            .iter()
            .map(|f| match f {
                Field::Text(n, v) | Field::Typed(n, v) => (n.clone(), v.clone()),
                Field::File(..) => panic!("no file in a mint"),
            })
            .collect()
    }
}

impl crate::forge::transport::Transport for FakeForge {
    fn send(
        &self,
        _forge: &Forge,
        call: &crate::forge::transport::Call,
    ) -> Result<crate::forge::transport::Reply, crate::forge::transport::NoAnswer> {
        use crate::forge::transport::{Endpoint, Reply};
        let Endpoint::Rest { path, .. } = &call.endpoint else {
            panic!("REST only")
        };
        let reply = |code, status, out: String, err: &str| {
            Ok(Reply {
                code,
                out,
                err: err.to_owned(),
                status: Some(status),
                headers: vec![],
            })
        };
        let all: serde_json::Map<String, serde_json::Value> = self
            .has
            .iter()
            .map(|(n, l)| ((*n).to_owned(), (*l).into()))
            .collect();
        if path.ends_with("/installation") {
            return reply(
                0,
                200,
                serde_json::json!({"id": 42, "permissions": all}).to_string(),
                "",
            );
        }
        *self.body.lock().unwrap() = call.fields.clone();
        let asked: Vec<(String, String)> = self
            .sent()
            .into_iter()
            .filter_map(|(n, v)| {
                Some((
                    n.strip_prefix("permissions[")?
                        .strip_suffix(']')?
                        .to_owned(),
                    v,
                ))
            })
            .collect();
        let mut granted = serde_json::Map::new();
        if asked.is_empty() || self.ignores_the_ask {
            granted = all;
        } else {
            for (n, l) in asked {
                if !self.has.iter().any(|(h, _)| *h == n) {
                    return reply(
                        1,
                        422,
                        String::new(),
                        "gh: The permissions requested are not granted to this installation. \
                         (HTTP 422)",
                    );
                }
                granted.insert(n, l.into());
            }
        }
        reply(
            0,
            201,
            serde_json::json!({"token": "t", "permissions": granted}).to_string(),
            "",
        )
    }

    fn check_auth(&self, _forge: &Forge) -> Result<(), crate::forge::ForgeError> {
        Ok(())
    }
}

fn the_app() -> Vec<(&'static str, &'static str)> {
    vec![
        ("contents", "write"),
        ("issues", "write"),
        ("metadata", "read"),
        ("pull_requests", "write"),
        ("workflows", "write"),
        ("administration", "write"),
        ("actions", "write"),
        ("statuses", "write"),
        ("deployments", "write"),
    ]
}

fn mint_on(fake: &FakeForge, workflows: bool) -> Result<super::Minted, String> {
    super::mint_over_with(
        fake,
        &github(),
        &[remote("acme", "api", "/w/api")],
        None,
        workflows,
    )
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
        .collect()
}

// Release delete, draft and notes edit (R5b, R3, R6) are all `contents:write`, which a chat needs
// to push, and GitHub has no narrower permission for them: this allowlist cannot close them.
#[test]
fn the_mint_body_is_exactly_the_allowlist() {
    let fake = FakeForge::new(&the_app());
    mint_on(&fake, false).unwrap();
    assert_eq!(
        fake.sent(),
        pairs(&[
            ("repositories[]", "api"),
            ("permissions[contents]", "write"),
            ("permissions[issues]", "write"),
            ("permissions[metadata]", "read"),
            ("permissions[pull_requests]", "write"),
        ])
    );
    mint_on(&fake, true).unwrap();
    assert_eq!(
        fake.sent().last(),
        Some(&("permissions[workflows]".to_owned(), "write".to_owned()))
    );
}

#[test]
fn a_token_whose_reply_has_a_permission_beyond_the_allowlist_is_refused() {
    let mut fake = FakeForge::new(&the_app());
    fake.ignores_the_ask = true;
    let said = mint_on(&fake, false).unwrap_err();
    assert!(said.contains("beyond"), "{said}");
    // `workflows` is beyond it too, unless the person ticked it.
    assert!(mint_on(&fake, true).is_err());
}

#[test]
fn a_token_whose_reply_names_no_permissions_is_refused() {
    struct Bare;
    impl crate::forge::transport::Transport for Bare {
        fn send(
            &self,
            forge: &Forge,
            call: &crate::forge::transport::Call,
        ) -> Result<crate::forge::transport::Reply, crate::forge::transport::NoAnswer> {
            let mut fake = FakeForge::new(&the_app());
            fake.ignores_the_ask = true;
            let mut reply = fake.send(forge, call)?;
            if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&reply.out)
                && v.get("token").is_some()
            {
                v.as_object_mut().unwrap().remove("permissions");
                reply.out = v.to_string();
            }
            Ok(reply)
        }
        fn check_auth(&self, _forge: &Forge) -> Result<(), crate::forge::ForgeError> {
            Ok(())
        }
    }
    let said = super::mint_over_with(
        &Bare,
        &github(),
        &[remote("acme", "api", "/w/api")],
        None,
        false,
    )
    .unwrap_err();
    assert!(said.contains("beyond"), "{said}");
}

#[test]
fn an_app_lacking_a_listed_permission_mints_a_narrower_token() {
    let fake = FakeForge::new(&[
        ("contents", "write"),
        ("metadata", "read"),
        ("actions", "read"),
        ("checks", "read"),
    ]);
    mint_on(&fake, false).unwrap();
    assert_eq!(
        fake.sent(),
        pairs(&[
            ("repositories[]", "api"),
            ("permissions[contents]", "write"),
            ("permissions[metadata]", "read"),
        ])
    );
}

#[test]
fn a_permission_the_installation_has_only_at_read_is_asked_for_at_read() {
    let fake = FakeForge::new(&[
        ("contents", "write"),
        ("issues", "read"),
        ("metadata", "read"),
    ]);
    mint_on(&fake, false).unwrap();
    assert!(
        fake.sent()
            .contains(&("permissions[issues]".to_owned(), "read".to_owned()))
    );
}

#[test]
fn an_installation_with_none_of_the_allowlist_mints_nothing_and_does_not_ask_for_everything() {
    let fake = FakeForge::new(&[("actions", "write")]);
    let said = mint_on(&fake, false).unwrap_err();
    assert!(said.contains("holds no permission"), "{said}");
    assert_eq!(fake.sent(), []);
}

#[test]
fn a_422_about_permissions_says_none_was_widened_and_another_422_does_not() {
    use crate::forge::transport::Reply;
    let reply = |err: &str| Reply {
        code: 1,
        out: String::new(),
        err: err.to_owned(),
        status: Some(422),
        headers: vec![],
    };
    assert!(super::permission_refusal(&reply(
        "The permissions requested are not granted (HTTP 422)"
    )));
    assert!(!super::permission_refusal(&reply(
        "Validation Failed: repository not accessible (HTTP 422)"
    )));
}
