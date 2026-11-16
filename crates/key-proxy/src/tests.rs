// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a request must carry and what is put in its place, without a socket: the run's token
//! (R-063), the key swapped in for it (R-042), the credentials stripped and `Host` set (R-097,
//! R-100), and how the upstream is reached: through which outbound proxy, with which `CONNECT`,
//! and what a failure is named (R-056). A listener's own tests, which bind `127.0.0.1`, are
//! `purlis-host`'s `the_key_proxy_through_a_start_path`.

use std::collections::HashSet;
use std::io;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

use hyper::HeaderMap;
use hyper::header::HeaderValue;

use purlis_core::model_provider::{Format, built_ins};
use purlis_core::model_route;
use purlis_core::upstream::{Destination, HostNotFound, Outbound, trust};

use super::{
    CREDENTIAL_HEADERS, Decision, Key, Matcher, NO_TOKEN, NOT_A_PATH, NOT_LISTED, NOT_ROUTED,
    Origin, Reach, Route, Token, Unreached, carries, connect, connect_request, decide, destination,
    swap, unreached,
};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn token() -> Token {
    Token(TOKEN.to_owned())
}

fn route(key: Key) -> Route {
    Route {
        origin: Origin::https("api.anthropic.com", 443),
        bound: bound(),
        key,
    }
}

/// The Anthropic built-in's Messages route for Claude Code.
fn bound() -> model_route::Route {
    let declaration = built_ins()
        .iter()
        .find(|declaration| declaration.id == "anthropic")
        .expect("the Anthropic built-in ships");
    model_route::Route::bind("claude-anthropic", declaration, Format::AnthropicMessages)
        .expect("a Messages endpoint")
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
    swap(&mut sent, &route(Key::XApiKey("sk-stand-in".to_owned()))).unwrap();
    assert_eq!(sent.get_all("x-api-key").iter().count(), 1);
    assert_eq!(sent["x-api-key"], "sk-stand-in");
    assert!(sent.get("authorization").is_none(), "the token went on");
    assert_eq!(sent["anthropic-version"], "2023-06-01");

    let mut sent = headers(&[("authorization", &format!("Bearer {TOKEN}"))]);
    swap(&mut sent, &route(Key::Bearer("sk-stand-in".to_owned()))).unwrap();
    assert_eq!(sent.get_all("authorization").iter().count(), 1);
    assert_eq!(sent["authorization"], "Bearer sk-stand-in");
}

#[test]
fn a_key_is_never_written_out_by_debug() {
    let shown = format!("{:?}", Key::XApiKey("sk-stand-in".to_owned()));
    assert!(!shown.contains("sk-stand-in"), "{shown}");
}

#[test]
fn every_credential_header_is_stripped_and_only_the_declarations_carries_the_key() {
    for (key, declared, carried) in [
        (
            Key::XApiKey("sk-stand-in".to_owned()),
            "x-api-key",
            "sk-stand-in",
        ),
        (
            Key::Bearer("sk-stand-in".to_owned()),
            "authorization",
            "Bearer sk-stand-in",
        ),
    ] {
        let mut sent = headers(&[
            ("authorization", &format!("Bearer {TOKEN}")),
            ("Proxy-Authorization", "Basic aGFybmVzczpvd24="),
            ("X-Api-Key", "the harness's own"),
            ("API-KEY", "the harness's own"),
            ("x-goog-api-key", "the harness's own"),
            ("Cookie", "session=the harness's own"),
            ("anthropic-version", "2023-06-01"),
        ]);
        swap(&mut sent, &route(key)).unwrap();
        for name in CREDENTIAL_HEADERS {
            if name == declared {
                continue;
            }
            assert!(
                sent.get(name).is_none(),
                "{name} went on beside {declared}: {sent:?}"
            );
        }
        assert_eq!(sent.get_all(declared).iter().count(), 1, "{sent:?}");
        assert_eq!(sent[declared], carried);
        assert_eq!(sent["anthropic-version"], "2023-06-01");
    }
}

#[test]
fn host_is_the_upstream_origins_whatever_the_harness_sent() {
    let key = || Key::XApiKey("sk-stand-in".to_owned());
    for (origin, host) in [
        (Origin::https("api.anthropic.com", 443), "api.anthropic.com"),
        (
            Origin::https("api.anthropic.com", 8443),
            "api.anthropic.com:8443",
        ),
        (Origin::http("127.0.0.1", 8080), "127.0.0.1:8080"),
    ] {
        let mut sent = headers(&[("Host", "evil.example")]);
        swap(
            &mut sent,
            &Route {
                origin,
                bound: bound(),
                key: key(),
            },
        )
        .unwrap();
        assert_eq!(sent.get_all("host").iter().count(), 1, "{sent:?}");
        assert_eq!(sent["host"], host);
    }
}

#[test]
fn a_key_that_is_no_header_value_is_refused_never_sent_empty() {
    for key in [
        Key::XApiKey("sk-stand-in\r\nx-injected: yes".to_owned()),
        Key::Bearer("sk-\u{0}".to_owned()),
    ] {
        let mut sent = headers(&[("authorization", &format!("Bearer {TOKEN}"))]);
        assert!(
            swap(&mut sent, &route(key)).is_err(),
            "a key that can't be a header value was sent: {sent:?}"
        );
        for name in CREDENTIAL_HEADERS {
            assert!(sent.get(name).is_none(), "{name} left in: {sent:?}");
        }
    }
}

#[test]
fn the_system_proxy_is_asked_for_the_upstream_and_no_proxy_exempts_it() {
    let origin = Origin::https("api.anthropic.com", 443);
    let proxied = Matcher::builder().https("http://127.0.0.1:3128").build();
    assert_eq!(
        destination(&origin, &proxied).unwrap().0,
        Destination::through("api.anthropic.com", 443, "127.0.0.1:3128")
    );

    let exempt = Matcher::builder()
        .https("http://127.0.0.1:3128")
        .no("api.anthropic.com")
        .build();
    assert_eq!(
        destination(&origin, &exempt).unwrap().0,
        Destination::direct("api.anthropic.com", 443)
    );

    let unset = Matcher::builder().build();
    assert_eq!(
        destination(&origin, &unset).unwrap().0,
        Destination::direct("api.anthropic.com", 443)
    );

    let everything = Matcher::builder().all("http://127.0.0.1:3128").build();
    for host in ["127.0.0.1", "::1", "localhost"] {
        let loopback = Origin::http(host, 8080);
        assert_eq!(
            destination(&loopback, &everything).unwrap().0,
            Destination::direct(host, 8080),
            "a loopback upstream went through the outbound proxy"
        );
    }
}

#[test]
fn the_connect_carries_no_proxy_credential_it_was_not_given() {
    let origin = Origin::https("api.anthropic.com", 443);
    let uri = "https://api.anthropic.com/".parse().unwrap();

    let bare = Matcher::builder().https("http://127.0.0.1:3128").build();
    let request = connect_request(&origin, &bare.intercept(&uri).unwrap()).unwrap();
    assert_eq!(request.method(), hyper::Method::CONNECT);
    assert_eq!(request.uri().to_string(), "api.anthropic.com:443");
    assert_eq!(request.headers()["host"], "api.anthropic.com:443");
    assert!(
        request.headers().get("proxy-authorization").is_none(),
        "a proxy credential nobody gave: {:?}",
        request.headers()
    );

    let given = Matcher::builder()
        .https("http://someone:secret@127.0.0.1:3128")
        .build();
    let request = connect_request(&origin, &given.intercept(&uri).unwrap()).unwrap();
    assert_eq!(
        request
            .headers()
            .get("proxy-authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Basic c29tZW9uZTpzZWNyZXQ="),
        "the credential the proxy's address carried was not sent"
    );
}

#[test]
fn each_failure_to_reach_the_upstream_is_named_by_its_cause() {
    let untrusted = std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer),
    );
    assert_eq!(
        Unreached::of_tls(&untrusted).message("api.anthropic.com"),
        "purlis: api.anthropic.com's certificate isn't trusted."
    );
    let broken = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "reset");
    assert_eq!(
        Unreached::of_tls(&broken).message("api.anthropic.com"),
        "purlis: purlis couldn't connect to api.anthropic.com."
    );
    assert_eq!(
        Unreached::ProxyRefused.message("api.anthropic.com"),
        "purlis: The outbound proxy refused the connection."
    );
}

#[test]
fn only_a_listed_request_carrying_the_token_is_forwarded_at_the_joined_path() {
    let route = route(Key::XApiKey("sk-stand-in".to_owned()));
    let token = token();
    let with = headers(&[("authorization", &format!("Bearer {TOKEN}"))]);
    assert_eq!(
        decide(
            Some(&route),
            &token,
            "POST",
            "/v1/messages?beta=true",
            &with
        ),
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
    let route = route(Key::XApiKey("sk-stand-in".to_owned()));
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
    let route = route(Key::XApiKey("sk-stand-in".to_owned()));
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

/// An outbound that records every destination it is asked for and connects to none of them,
/// failing each with `why`.
struct Recorded {
    asked: Arc<Mutex<Vec<Destination>>>,
    why: fn(&Destination) -> io::Error,
}

impl Outbound for Recorded {
    fn connect(&self, to: &Destination) -> io::Result<TcpStream> {
        self.asked.lock().unwrap().push(to.clone());
        Err((self.why)(to))
    }
}

/// A reach through `proxies` whose outbound fails every connect with `why`, and what it was
/// asked for.
fn recorded(
    proxies: Matcher,
    why: fn(&Destination) -> io::Error,
) -> (Reach, Arc<Mutex<Vec<Destination>>>) {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let reach = Reach {
        outbound: Arc::new(Recorded {
            asked: Arc::clone(&asked),
            why,
        }),
        tls: Arc::new(trust::client_config().unwrap()),
        proxies,
    };
    (reach, asked)
}

fn refused(_: &Destination) -> io::Error {
    io::Error::new(io::ErrorKind::ConnectionRefused, "refused")
}

/// Why `connect` to `origin` through `reach` failed.
async fn why_unreached(origin: &Origin, reach: &Reach) -> Unreached {
    match connect(origin, reach).await {
        Err(why) => why,
        Ok(_) => panic!("{origin:?} was reached"),
    }
}

/// With the system's outbound proxy at `proxy`, the call to the upstream is refused naming
/// `scheme`, and nothing is connected to: not the proxy, so no credential reaches it.
async fn a_proxy_of_this_scheme_is_refused_naming_it(proxy: &str, scheme: &str) {
    let origin = Origin::https("api.anthropic.com", 443);
    let (reach, asked) = recorded(Matcher::builder().https(proxy).build(), refused);
    let why = why_unreached(&origin, &reach).await;
    assert_eq!(why, Unreached::UnsupportedProxy(scheme.to_owned()));
    assert_eq!(
        why.message(&origin.host),
        format!("purlis: purlis can't use the outbound proxy's {scheme} scheme.")
    );
    let asked = asked.lock().unwrap();
    assert!(
        asked.is_empty(),
        "the {scheme} proxy was connected to: {asked:?}"
    );
}

#[tokio::test]
async fn a_socks5_proxy_is_refused_naming_its_scheme() {
    a_proxy_of_this_scheme_is_refused_naming_it("socks5://127.0.0.1:1080", "socks5").await;
}

#[tokio::test]
async fn a_socks5h_proxy_is_refused_naming_its_scheme() {
    a_proxy_of_this_scheme_is_refused_naming_it("socks5h://127.0.0.1:1080", "socks5h").await;
}

#[tokio::test]
async fn an_https_proxy_is_refused_naming_its_scheme_and_gets_no_credential() {
    a_proxy_of_this_scheme_is_refused_naming_it("https://someone:secret@127.0.0.1:3128", "https")
        .await;
}

#[tokio::test]
async fn an_http_proxy_is_still_asked_for_a_tunnel() {
    let origin = Origin::https("api.anthropic.com", 443);
    let (reach, asked) = recorded(
        Matcher::builder().https("http://127.0.0.1:3128").build(),
        refused,
    );
    assert_eq!(
        why_unreached(&origin, &reach).await,
        Unreached::ProxyRefused
    );
    assert_eq!(
        *asked.lock().unwrap(),
        vec![Destination::through(
            "api.anthropic.com",
            443,
            "127.0.0.1:3128"
        )]
    );
}

#[tokio::test]
async fn an_upstream_the_resolver_cannot_find_is_named_not_found() {
    let origin = Origin::https("api.anthropic.com", 443);
    let (reach, _) = recorded(Matcher::builder().build(), |to| {
        HostNotFound::error(&to.host)
    });
    let why = why_unreached(&origin, &reach).await;
    assert_eq!(why, Unreached::HostNotFound);
    assert_eq!(
        why.message(&origin.host),
        "purlis: purlis couldn't find api.anthropic.com."
    );

    let (reach, _) = recorded(Matcher::builder().build(), refused);
    assert_eq!(
        why_unreached(&origin, &reach).await,
        Unreached::NoConnection
    );
}

#[test]
fn a_502_for_an_unreached_upstream_leaves_the_harness_free_to_retry() {
    let origin = Origin::https("api.anthropic.com", 443);
    for why in [
        Unreached::HostNotFound,
        Unreached::NoConnection,
        Unreached::Untrusted,
        Unreached::ProxyRefused,
        Unreached::Reset,
        Unreached::UnsupportedProxy("socks5".to_owned()),
    ] {
        let answer = unreached(why.clone(), &origin);
        assert_eq!(answer.status(), hyper::StatusCode::BAD_GATEWAY);
        assert!(
            answer.headers().get("x-should-retry").is_none(),
            "{why:?} said never retry: {:?}",
            answer.headers()
        );
    }
}
