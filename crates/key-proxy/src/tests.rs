// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a request must carry and what is put in its place, without a socket: the run's token
//! (R-063) and the key swapped in for it (R-042). A listener's own tests, which bind
//! `127.0.0.1`, are `purlis-host`'s `the_key_proxy_through_a_start_path`.

use std::collections::HashSet;

use hyper::HeaderMap;
use hyper::header::HeaderValue;

use purlis_core::model_provider::{Format, built_ins};
use purlis_core::model_route;

use super::{
    Decision, Key, NO_TOKEN, NOT_A_PATH, NOT_LISTED, NOT_ROUTED, Route, Token, carries, decide,
    swap,
};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn token() -> Token {
    Token(TOKEN.to_owned())
}

fn headers(lines: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in lines {
        map.append(
            hyper::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            HeaderValue::from_str(value).unwrap(),
        );
    }
    map
}

#[test]
fn minted_tokens_are_32_random_bytes_as_lowercase_hex_and_never_repeat() {
    let mut seen = HashSet::new();
    for _ in 0..1000 {
        let token = Token::mint().unwrap();
        let hex = token.reveal();
        assert_eq!(hex.len(), 64, "32 bytes as hex: {hex}");
        assert!(
            hex.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "lowercase hex only, so it never takes a Claude-plan or JWT shape: {hex}"
        );
        assert!(seen.insert(hex.to_owned()), "a token repeated: {hex}");
    }
}

#[test]
fn only_the_runs_token_as_a_bearer_credential_is_carried() {
    let token = token();
    assert!(carries(
        &token,
        &headers(&[("authorization", &format!("Bearer {TOKEN}"))])
    ));
    assert!(
        !carries(&token, &headers(&[])),
        "a request with no credential header"
    );
    assert!(
        !carries(&token, &headers(&[("x-api-key", TOKEN)])),
        "the token in another header than the one ANTHROPIC_AUTH_TOKEN sets"
    );
    assert!(
        !carries(
            &token,
            &headers(&[("authorization", &format!("Bearer {}", TOKEN.to_uppercase()))])
        ),
        "the token in other letters"
    );
    let mut other = TOKEN.to_owned();
    other.replace_range(63.., "0");
    assert!(
        !carries(
            &token,
            &headers(&[("authorization", &format!("Bearer {other}"))])
        ),
        "another chat's token"
    );
    assert!(
        !carries(
            &token,
            &headers(&[("authorization", &format!("Bearer {}", &TOKEN[..63]))])
        ),
        "a prefix of the token"
    );
    assert!(
        !carries(
            &token,
            &headers(&[
                ("authorization", &format!("Bearer {TOKEN}")),
                ("authorization", "Bearer someone-else"),
            ])
        ),
        "two credential headers, one of them the token"
    );
}

#[test]
fn the_key_goes_in_the_declarations_header_and_the_token_goes_nowhere() {
    let mut sent = headers(&[
        ("authorization", &format!("Bearer {TOKEN}")),
        ("anthropic-version", "2023-06-01"),
        ("x-api-key", "the harness's own"),
    ]);
    swap(&mut sent, &Key::XApiKey("sk-stand-in".to_owned()));
    assert_eq!(sent.get_all("x-api-key").iter().count(), 1);
    assert_eq!(sent["x-api-key"], "sk-stand-in");
    assert!(sent.get("authorization").is_none(), "the token went on");
    assert_eq!(sent["anthropic-version"], "2023-06-01");

    let mut sent = headers(&[("authorization", &format!("Bearer {TOKEN}"))]);
    swap(&mut sent, &Key::Bearer("sk-stand-in".to_owned()));
    assert_eq!(sent.get_all("authorization").iter().count(), 1);
    assert_eq!(sent["authorization"], "Bearer sk-stand-in");
}

#[test]
fn a_key_is_never_written_out_by_debug() {
    let shown = format!("{:?}", Key::XApiKey("sk-stand-in".to_owned()));
    assert!(!shown.contains("sk-stand-in"), "{shown}");
}

/// A listener's route through the Anthropic built-in, to an upstream it never reaches here.
fn anthropic() -> Route {
    let declaration = built_ins()
        .iter()
        .find(|declaration| declaration.id == "anthropic")
        .expect("the Anthropic built-in ships");
    Route {
        upstream: ([127, 0, 0, 1], 9).into(),
        bound: model_route::Route::bind("claude-anthropic", declaration, Format::AnthropicMessages)
            .expect("a Messages endpoint"),
        key: Key::XApiKey("sk-stand-in".to_owned()),
    }
}

#[test]
fn only_a_listed_request_carrying_the_token_is_forwarded_at_the_joined_path() {
    let route = anthropic();
    let token = token();
    let with = headers(&[("authorization", &format!("Bearer {TOKEN}"))]);
    assert_eq!(
        decide(Some(&route), &token, "POST", "/v1/messages?beta=true", &with),
        Decision::Forward("/v1/messages?beta=true".parse().unwrap())
    );
    assert_eq!(
        decide(Some(&route), &token, "POST", "/v1/messages", &headers(&[])),
        Decision::Refuse(NO_TOKEN),
        "a listed request without the token"
    );
    assert_eq!(
        decide(None, &token, "POST", "/v1/messages", &with),
        Decision::Refuse(NOT_ROUTED),
        "a listener with no route"
    );
}

#[test]
fn a_request_the_route_refuses_is_refused_whatever_it_carries() {
    let route = anthropic();
    let token = token();
    let with = headers(&[("authorization", &format!("Bearer {TOKEN}"))]);
    for (method, target, why) in [
        ("GET", "/v1/messages", NOT_LISTED),
        ("POST", "/v1/files", NOT_LISTED),
        ("DELETE", "/v1/messages", NOT_LISTED),
        ("OPTIONS", "*", NOT_A_PATH),
        ("POST", "/v1/messages/count_tokens", NOT_LISTED),
        ("GET", "/v1/models?limit=1000", NOT_LISTED),
        ("POST", "http://elsewhere.example/v1/messages", NOT_A_PATH),
        ("CONNECT", "elsewhere.example:443", NOT_A_PATH),
        ("POST", "/v1/../v1/messages", NOT_A_PATH),
        ("POST", "/v1%2Fmessages", NOT_A_PATH),
    ] {
        for carried in [&with, &headers(&[])] {
            assert_eq!(
                decide(Some(&route), &token, method, target, carried),
                Decision::Refuse(why),
                "{method} {target}"
            );
        }
    }
}

#[test]
fn claude_codes_hello_is_absent_with_or_without_the_token() {
    let route = anthropic();
    let token = token();
    for carried in [
        headers(&[]),
        headers(&[("authorization", &format!("Bearer {TOKEN}"))]),
    ] {
        assert_eq!(
            decide(Some(&route), &token, "HEAD", "/api/hello", &carried),
            Decision::Absent
        );
    }
}
