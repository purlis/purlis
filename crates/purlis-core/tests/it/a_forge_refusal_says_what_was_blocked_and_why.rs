// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A forge refusal says what was blocked and why (#7779; ADR 0128, amended 2026-11-27, §10).
//!
//! It is one of two kinds, and says which: **no action**, a request purlis does not carry, which
//! no setting can allow (a floor act also says what it is, and that it is the person's to do on
//! the forge); or **a known act the policy doesn't allow**, which names the act, the repo and why.
//! The path in the text is the route's pattern: numbers as `{n}`, any segment outside the route
//! vocabulary as `{…}`, no query, capped, and inert. Each refusal is counted, by its kind and act,
//! under Settings › Project › Policy, "Would have blocked". Nothing here changes a decision.

use std::sync::{Arc, Mutex};

use purlis_core::enforcement::{self, Family, Mode};
use purlis_core::forge::mergestance::MergeStance;
use purlis_core::forge::proxy::{ChatId, Outcome, Refusal, Refused, RequestHead};

use crate::support::forge_proxy::{Objects, Scene, is_refused};

/// What chat-a is told for `method host target`, refused for `refusal`.
fn said(s: &Scene, method: &str, host: &str, target: &str, refusal: Refusal) -> String {
    let out = s.send(method, host, target, b"{}");
    assert!(is_refused(&out, refusal), "{method} {target}: {out:?}");
    s.proxy
        .refused_said(&s.a, &RequestHead::new(method, host, 443, target), refusal)
        .unwrap_or_else(|| panic!("{method} {target} has no sentence"))
}

#[test]
fn an_unknown_forge_request_says_no_setting_can_allow_it_and_names_the_repo() {
    purlis_core::unsteered!();
    let s = Scene::single();
    let text = said(
        &s,
        "POST",
        "api.github.com",
        "/repos/acme/app/milestones",
        Refusal::NotOnList,
    );
    assert_eq!(
        text,
        "purlis doesn't carry this request (`POST /repos/{owner}/{repo}/milestones` on \
         `github.com/acme/app`), so it was refused. No setting can allow it."
    );
    let gitlab = said(
        &s,
        "POST",
        "gitlab.com",
        "/api/v4/projects/42/milestones",
        Refusal::NotOnList,
    );
    assert_eq!(
        gitlab,
        "purlis doesn't carry this request (`POST /api/v4/projects/{project}/milestones` on \
         `gitlab.com/group/proj`), so it was refused. No setting can allow it."
    );
}

#[test]
fn a_known_forge_act_refused_names_the_act_the_repo_and_why() {
    purlis_core::unsteered!();
    let s = Scene::single();
    let merge = said(
        &s,
        "PUT",
        "api.github.com",
        "/repos/acme/app/pulls/7/merge",
        Refusal::Merge,
    );
    assert_eq!(
        merge,
        "A merge on `github.com/acme/app` isn't allowed by `guards.merge`, so it was refused. \
         It waits for the person's press in purlis's window."
    );
    let release = said(
        &s,
        "POST",
        "api.github.com",
        "/repos/acme/app/releases",
        Refusal::ReleaseOrTag,
    );
    assert_eq!(
        release,
        "A release or tag on `github.com/acme/app` isn't allowed from a chat, so it was refused. \
         It's held for the person's press in purlis's window."
    );
    let fork = said(
        &s,
        "POST",
        "api.github.com",
        "/repos/acme/app/forks",
        Refusal::NotOnList,
    );
    assert_eq!(
        fork,
        "A fork on `github.com/acme/app` isn't allowed from a chat, so it was refused. \
         It's held for the person's press in purlis's window."
    );
}

#[test]
fn a_floor_act_refusal_says_it_is_the_persons_to_do() {
    purlis_core::unsteered!();
    let s = Scene::single();
    let admin = said(
        &s,
        "GET",
        "api.github.com",
        "/repos/acme/app/hooks",
        Refusal::Admin,
    );
    assert_eq!(
        admin,
        "purlis doesn't carry this request (`GET /repos/{owner}/{repo}/hooks` on \
         `github.com/acme/app`), so it was refused. It's the repo's administration, which is \
         the person's to do on the forge. No setting can allow it."
    );
    let rule = said(
        &s,
        "PUT",
        "api.github.com",
        "/repos/acme/app/rulesets/9",
        Refusal::Ruleset,
    );
    assert!(
        rule.contains("`PUT /repos/{owner}/{repo}/rulesets/{n}`")
            && rule
                .contains("It's a rule over the repo, which is the person's to do on the forge.")
            && rule.ends_with("No setting can allow it."),
        "{rule}"
    );
    let approval = said(
        &s,
        "POST",
        "api.github.com",
        "/repos/acme/app/pulls/7/reviews",
        Refusal::Approval,
    );
    assert!(
        approval
            .contains("It's an approval or a review, which is the person's to do on the forge."),
        "{approval}"
    );
}

type Counted = Arc<Mutex<Vec<(String, Refused)>>>;

fn counting() -> (Scene, Counted) {
    let counted: Counted = Arc::default();
    let kept = counted.clone();
    let s = Scene::tuned(Objects::default(), move |proxy| {
        proxy.with_refusals(Box::new(move |chat: &ChatId, refused: &Refused| {
            kept.lock()
                .unwrap()
                .push((chat.as_str().to_owned(), refused.clone()));
        }))
    });
    (s, counted)
}

#[test]
fn a_forge_refusal_is_counted_under_would_have_blocked() {
    purlis_core::unsteered!();
    let (s, counted) = counting();
    let sent = [
        ("POST", "/repos/acme/app/milestones", "forge.no-action"),
        (
            "GET",
            "/repos/acme/app/hooks",
            "forge.no-action.administration",
        ),
        (
            "PUT",
            "/repos/acme/app/pulls/7/merge",
            "forge.not-allowed.merge",
        ),
        (
            "POST",
            "/repos/acme/app/releases",
            "forge.not-allowed.release-or-tag",
        ),
        ("POST", "/repos/acme/app/forks", "forge.not-allowed.fork"),
    ];
    for (method, target, _) in sent {
        let out = s.send(method, "api.github.com", target, b"{}");
        assert!(matches!(out, Outcome::Refused(_)), "{target}: {out:?}");
    }
    // A read the route carries is not a refusal, and counts nothing.
    let read = s.send("GET", "api.github.com", "/repos/acme/app/pulls/7", b"");
    assert!(matches!(read, Outcome::Forward(_)), "{read:?}");
    let counted = counted.lock().unwrap().clone();
    let rules: Vec<&str> = counted.iter().map(|(_, r)| r.rule).collect();
    assert_eq!(rules, sent.map(|(_, _, rule)| rule));
    for (chat, refused) in &counted {
        assert_eq!(chat, "chat-a");
        assert_eq!(refused.host, "api.github.com:443");
        // Each line is one the window puts on the page: a known rule of `guards.forge`, about a
        // `host:port`.
        assert!(
            enforcement::known_rule(Family::GuardsForge, refused.rule),
            "{}",
            refused.rule
        );
        assert!(enforcement::is_host_and_port(&refused.host));
    }
}

#[test]
fn a_forge_refusal_text_holds_no_body_or_number() {
    purlis_core::unsteered!();
    let (s, counted) = counting();
    let target = "/repos/acme/app/milestones/48213?state=SECRETQUERY";
    let body = br#"{"title":"SECRETBODY 77341"}"#;
    let out = s.send("POST", "api.github.com", target, body);
    assert!(is_refused(&out, Refusal::NotOnList), "{out:?}");
    let text = s
        .proxy
        .refused_said(
            &s.a,
            &RequestHead::new("POST", "api.github.com", 443, target),
            Refusal::NotOnList,
        )
        .unwrap();
    assert!(
        text.contains("`POST /repos/{owner}/{repo}/milestones/{n}`"),
        "{text}"
    );
    for leaked in ["48213", "77341", "SECRETBODY", "SECRETQUERY", "state", "?"] {
        assert!(!text.contains(leaked), "{leaked} in {text}");
    }
    let counted = counted.lock().unwrap().clone();
    assert_eq!(counted.len(), 1);
    let (_, refused) = &counted[0];
    assert!(
        !refused.rule.bytes().any(|b| b.is_ascii_digit()),
        "{}",
        refused.rule
    );
}

#[test]
fn an_unmatched_requests_unknown_segments_are_shown_as_a_placeholder_and_its_query_dropped() {
    purlis_core::unsteered!();
    let s = Scene::single();
    let text = said(
        &s,
        "POST",
        "api.github.com",
        "/repos/acme/app/zzwhatever/labels/sub-issues?per_page=3&q=hello",
        Refusal::NotOnList,
    );
    assert!(
        text.contains("`POST /repos/{owner}/{repo}/{…}/labels/{…}` on `github.com/acme/app`"),
        "{text}"
    );
    for leaked in ["zzwhatever", "sub-issues", "per_page", "hello", "?"] {
        assert!(!text.contains(leaked), "{leaked} in {text}");
    }
    // A request on the API host that names no repo is shown on the host alone.
    let outside = said(
        &s,
        "POST",
        "api.github.com",
        "/user/zzsomething",
        Refusal::NotOnList,
    );
    assert_eq!(
        outside,
        "purlis doesn't carry this request (`POST /user/{…}` on `api.github.com`), so it was \
         refused. No setting can allow it."
    );
}

#[test]
fn a_long_unmatched_path_is_capped_and_drawn_inert() {
    purlis_core::unsteered!();
    let s = Scene::single();
    let long = format!("/repos/acme/app/{}", ["labels"; 60].join("/"));
    let text = said(&s, "POST", "api.github.com", &long, Refusal::NotOnList);
    let shown = text
        .split('`')
        .nth(1)
        .unwrap_or_else(|| panic!("no request in {text}"));
    assert!(
        shown.chars().count() <= 120,
        "{} chars: {shown}",
        shown.len()
    );
    assert!(shown.ends_with("/…"), "{shown}");
    // Markup, a link and a backtick of the chat's own are each one segment outside the
    // vocabulary: a placeholder, never drawn.
    let inert = "/repos/acme/app/%5Bx%5D(https:evil.example)/%3Cb%3Ehi/%60code%60";
    let text = said(&s, "POST", "api.github.com", inert, Refusal::NotOnList);
    assert!(
        text.contains("`POST /repos/{owner}/{repo}/{…}/{…}/{…}`"),
        "{text}"
    );
    for drawn in ["evil", "http", "<", ">", "[", "]("] {
        assert!(!text.contains(drawn), "{drawn} in {text}");
    }
    assert_eq!(text.matches('`').count(), 4, "{text}");
}

/// `guards.merge` held only because this machine has not accepted the project's policy.
struct Unaccepted;

impl MergeStance for Unaccepted {
    fn mode(&self) -> Mode {
        Mode::Enforce
    }

    fn forwarded(&self, _: &str, _: &str, _: &str) {}

    fn unaccepted(&self) -> bool {
        true
    }
}

#[test]
fn a_forge_refusal_under_a_broken_acceptance_says_the_policy_isnt_accepted_on_this_machine() {
    purlis_core::unsteered!();
    let s = Scene::tuned(Objects::default(), |proxy| {
        proxy.with_merge_stance(Arc::new(Unaccepted))
    });
    let merge = said(
        &s,
        "PUT",
        "api.github.com",
        "/repos/acme/app/pulls/7/merge",
        Refusal::Merge,
    );
    assert_eq!(
        merge,
        "A merge on `github.com/acme/app` was refused because this project's policy isn't \
         accepted on this machine. Accept it in purlis's window."
    );
    // It names no family to loosen, and carries no note on setting one to log or off.
    assert!(!merge.contains("guards."), "{merge}");
    assert!(
        !merge.contains(purlis_core::toolgate::OVERRIDE_NOTE.trim()),
        "{merge}"
    );
    // An act no policy opens is refused as it always is: the acceptance is not its cause.
    let release = said(
        &s,
        "POST",
        "api.github.com",
        "/repos/acme/app/releases",
        Refusal::ReleaseOrTag,
    );
    assert!(release.contains("isn't allowed from a chat"), "{release}");
}
