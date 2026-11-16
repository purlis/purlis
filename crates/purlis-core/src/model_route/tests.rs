// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A run's route and the in-memory checks of each request (T1a-08, #6751): the URL join
//! (R-084), Messages' allowlist (R-087, R-089) and the binding itself (R-077). The same rules
//! through a listener, against the fake provider, are `purlis-host`'s
//! `the_key_proxy_through_a_start_path`.

use super::*;
use crate::model_provider::{self, From, built_ins};

/// A route through a declaration of the person's own whose base is `base`.
fn through(base: &str) -> Route {
    let text = format!(
        "id = \"gateway\"\nname = \"A gateway\"\nkind = \"router\"\n\n[[endpoint]]\n\
         format = \"anthropic-messages\"\nbase = \"{base}\"\nauth = \"x-api-key\"\n"
    );
    let declaration =
        model_provider::parse(&text, &From::Yours, &[], "gateway.toml").expect("a declaration");
    Route::bind("mine", &declaration, Format::AnthropicMessages).expect("a Messages endpoint")
}

/// The Anthropic built-in's Messages route.
fn anthropic() -> Route {
    let declaration = built_ins()
        .iter()
        .find(|declaration| declaration.id == "anthropic")
        .expect("the Anthropic built-in ships");
    Route::bind("claude-anthropic", declaration, Format::AnthropicMessages)
        .expect("a Messages endpoint")
}

#[test]
fn request_targets_join_to_upstream_urls_or_are_refused() {
    let route = through("https://gateway.example/anthropic");
    let table: &[(&str, &str, Result<&str, Refusal>)] = &[
        (
            "POST",
            "/v1/messages",
            Ok("https://gateway.example/anthropic/v1/messages"),
        ),
        (
            "POST",
            "/v1/messages?beta=true",
            Ok("https://gateway.example/anthropic/v1/messages?beta=true"),
        ),
        // An escaped unreserved letter is the letter.
        (
            "POST",
            "/v1/messag%65s",
            Ok("https://gateway.example/anthropic/v1/messages"),
        ),
        (
            "POST",
            "http://elsewhere.example/v1/messages",
            Err(Refusal::NotAPath),
        ),
        (
            "POST",
            "https://gateway.example/anthropic/v1/messages",
            Err(Refusal::NotAPath),
        ),
        ("CONNECT", "elsewhere.example:443", Err(Refusal::Connect)),
        ("CONNECT", "/v1/messages", Err(Refusal::Connect)),
        ("OPTIONS", "*", Err(Refusal::NotAPath)),
        ("POST", "/v1/../x", Err(Refusal::DotSegment)),
        ("POST", "/v1/./messages", Err(Refusal::DotSegment)),
        ("POST", "/v1/..", Err(Refusal::DotSegment)),
        ("POST", "/v1/%2e%2e/x", Err(Refusal::DotSegment)),
        ("POST", "/v1/%2E./x", Err(Refusal::DotSegment)),
        ("POST", "/v1/.%2e", Err(Refusal::DotSegment)),
        ("POST", "/v1%2Fmessages", Err(Refusal::EncodedSlash)),
        ("POST", "/v1%2fmessages", Err(Refusal::EncodedSlash)),
        ("POST", "/v1%5Cmessages", Err(Refusal::EncodedSlash)),
        ("POST", "/v1%5cmessages", Err(Refusal::EncodedSlash)),
        ("POST", "/v1/messages%", Err(Refusal::Malformed)),
        ("POST", "/v1/messages%zz", Err(Refusal::Malformed)),
        ("POST", "/v1\\messages", Err(Refusal::Malformed)),
        ("POST", "/v1/messages#top", Err(Refusal::Malformed)),
    ];
    for (method, target, upstream) in table {
        assert_eq!(
            route.upstream_url(method, target).as_deref(),
            upstream.as_ref().map(|url| *url),
            "{method} {target}"
        );
    }
    assert_eq!(
        anthropic().upstream_url("POST", "/v1/messages?beta=true"),
        Ok("https://api.anthropic.com/v1/messages?beta=true".to_owned()),
        "a bare origin's base adds no path"
    );
}

#[test]
fn only_messages_listed_method_and_path_is_forwarded_with_its_query() {
    let route = anthropic();
    for (target, path_and_query) in [
        ("/v1/messages", "/v1/messages"),
        ("/v1/messages?beta=true", "/v1/messages?beta=true"),
    ] {
        assert_eq!(
            route.judge("POST", target),
            Verdict::Forward {
                path_and_query: path_and_query.to_owned()
            },
            "POST {target}"
        );
    }
    for (method, target) in [
        ("GET", "/v1/messages"),
        ("DELETE", "/v1/messages"),
        ("PUT", "/v1/messages"),
        ("post", "/v1/messages"),
        ("OPTIONS", "*"),
        ("POST", "/v1/files"),
        ("POST", "/v1/messages/batches"),
        ("POST", "/v1/messages/count_tokens"),
        ("POST", "/v1/messages/count_tokens?beta=true"),
        ("GET", "/v1/models?limit=1000"),
        ("GET", "/inference-profiles"),
        ("POST", "/v1/messages/"),
        ("POST", "/v1/messages/x/.."),
        ("POST", "/v1%2Fmessages"),
        ("POST", "/V1/messages"),
        ("GET", "/api/hello"),
        ("POST", "/api/hello"),
        ("POST", "https://api.anthropic.com/v1/messages"),
        ("CONNECT", "api.anthropic.com:443"),
    ] {
        assert!(
            matches!(route.judge(method, target), Verdict::Refused(_)),
            "{method} {target} was let through: {:?}",
            route.judge(method, target)
        );
    }
    assert_eq!(
        route.judge("POST", "/v1/messages/count_tokens"),
        Verdict::Refused(Refusal::NotListed)
    );
}

#[test]
fn claude_codes_hello_is_answered_absent() {
    let route = anthropic();
    assert_eq!(route.judge("HEAD", "/api/hello"), Verdict::Absent);
    assert_eq!(
        route.judge("HEAD", "/api/hello?x=1"),
        Verdict::Absent,
        "a query on it changes nothing"
    );
}

#[test]
fn the_allowlist_is_matched_against_the_joined_upstream_path() {
    // `/v1/messages` joined to a base with a path is not the listed full path (R-086).
    assert_eq!(
        through("https://gateway.example/anthropic").judge("POST", "/v1/messages"),
        Verdict::Refused(Refusal::NotListed)
    );
}

#[test]
fn a_format_with_no_list_yet_sends_nothing() {
    assert!(allowlist(Format::OpenaiChat).is_empty());
    assert_eq!(
        allowlist(Format::AnthropicMessages),
        &[("POST", "/v1/messages")]
    );
}

#[test]
fn a_route_is_the_declaration_as_it_was_bound() {
    let first = through("https://one.example");
    let edited = through("https://two.example");
    assert_ne!(first.digest, edited.digest, "the edit moved the digest");
    assert_eq!(first.base.origin(), "https://one.example");
    assert_eq!(first.profile, "mine");
    assert_eq!(first.declaration, "gateway");
    assert_eq!(first.auth, Auth::XApiKey);
    let no_chat = model_provider::parse(
        "id = \"chat\"\nname = \"Chat only\"\nkind = \"router\"\n\n[[endpoint]]\n\
         format = \"openai-chat\"\nbase = \"https://chat.example\"\n",
        &From::Yours,
        &[],
        "chat.toml",
    )
    .unwrap();
    assert_eq!(
        Route::bind("mine", &no_chat, Format::AnthropicMessages),
        None,
        "no endpoint of the harness's format, no route"
    );
}
