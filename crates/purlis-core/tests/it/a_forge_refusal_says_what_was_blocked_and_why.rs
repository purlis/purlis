// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A forge refusal says what was blocked and why (#7779; ADR 0128, amended 2026-11-27, §10).
//!
//! It is one of two kinds, and says which: **no action**, a request purlis does not carry, which
//! no setting can allow (a floor act also says what it is, and that it is the person's to do on
//! the forge); or **a known act the policy doesn't allow**, read from the act's own route, which
//! names the act, the repo and why. The path in the text is the route's pattern: numbers as
//! `{n}`, any segment outside the route vocabulary as `{…}`, no query, capped, and inert. Each
//! refusal is counted, by its kind and act, under Settings › Project › Policy, "Would have
//! blocked", once a minute for each chat, rule and host. Nothing here changes a decision.
//!
//! **The route here holds acts as the app's does**: a held sink is installed, so an exact merge,
//! release, tag or fork is held for the person's press and never reaches these sentences. What
//! does reach them raised no ask, and says so. A caller that did not prove itself is told nothing
//! of where its request went.

use std::sync::{Arc, Mutex};

use purlis_core::enforcement::{self, AcceptedOn, Family, Mode};
use purlis_core::forge::mergestance::MergeStance;
use purlis_core::forge::proxy::{ChatId, HeldSink, Outcome, Refusal, Refused, RequestHead};
use purlis_core::heldact::Requested;

use crate::support::forge_proxy::{Auth, Objects, Scene, headers, is_refused};

/// What a refusal says when nobody was asked.
const NOBODY_ASKED: &str = "Nobody was asked, so it will not happen.";

/// The held sink the app installs, standing in: each ask is kept, and the chat told it was raised.
#[derive(Default, Clone)]
struct Asks(Arc<Mutex<Vec<Requested>>>);

impl HeldSink for Asks {
    fn ask(&self, _: &ChatId, requested: &Requested) -> String {
        self.0.lock().unwrap().push(requested.clone());
        "purlis has asked the person.".to_owned()
    }
}

/// A route as the app makes it: a held sink installed, then `tune`.
fn holding(
    asks: &Asks,
    tune: impl FnOnce(purlis_core::forge::proxy::ForgeProxy) -> purlis_core::forge::proxy::ForgeProxy,
) -> Scene {
    let asks = asks.clone();
    Scene::tuned(Objects::default(), move |proxy| {
        tune(proxy.with_held(Box::new(asks)))
    })
}

fn scene() -> Scene {
    holding(&Asks::default(), |proxy| proxy)
}

/// What chat-a, proved by its sentinel, is told for `method host target` with `body`, refused
/// for `refusal`.
fn said_of(
    s: &Scene,
    method: &str,
    host: &str,
    target: &str,
    body: &[u8],
    refusal: Refusal,
) -> String {
    let out = s.send(method, host, target, body);
    assert!(is_refused(&out, refusal), "{method} {target}: {out:?}");
    s.proxy
        .refused_said(
            &s.a,
            &RequestHead::new(method, host, 443, target),
            &headers(Auth::Bearer(s.a_sentinel.expose())),
            refusal,
        )
        .unwrap_or_else(|| panic!("{method} {target} has no sentence"))
}

fn said(s: &Scene, method: &str, host: &str, target: &str, refusal: Refusal) -> String {
    said_of(s, method, host, target, b"{}", refusal)
}

#[test]
fn an_unknown_forge_request_says_no_setting_can_allow_it_and_names_the_repo() {
    purlis_core::unsteered!();
    let s = scene();
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
fn an_exact_merge_release_or_fork_is_held_and_never_told_a_refusal() {
    purlis_core::unsteered!();
    let asks = Asks::default();
    let s = holding(&asks, |proxy| proxy);
    let sha = "a".repeat(40);
    for (method, target, body) in [
        (
            "PUT",
            "/repos/acme/app/pulls/7/merge",
            format!(r#"{{"sha":"{sha}"}}"#),
        ),
        (
            "POST",
            "/repos/acme/app/releases",
            r#"{"tag_name":"v1.0.0"}"#.to_owned(),
        ),
        ("POST", "/repos/acme/app/forks", "{}".to_owned()),
    ] {
        let out = s.send(method, "api.github.com", target, body.as_bytes());
        assert!(matches!(out, Outcome::Held(_)), "{target}: {out:?}");
    }
    assert_eq!(asks.0.lock().unwrap().len(), 3);
}

#[test]
fn a_known_forge_act_that_was_not_held_says_nobody_was_asked_and_what_purlis_can_hold() {
    purlis_core::unsteered!();
    let asks = Asks::default();
    let s = holding(&asks, |proxy| proxy);
    // A merge that names no head is not the act the person would be shown.
    let merge = said(
        &s,
        "PUT",
        "api.github.com",
        "/repos/acme/app/pulls/7/merge",
        Refusal::Merge,
    );
    assert_eq!(
        merge,
        format!(
            "A merge on `github.com/acme/app` isn't allowed by `guards.merge`, so it was \
             refused. {NOBODY_ASKED} A merge purlis can hold for the person's press names the \
             head `sha`, with no query."
        )
    );
    // A release with a query is not exactly the act either.
    let release = said_of(
        &s,
        "POST",
        "api.github.com",
        "/repos/acme/app/releases?draft=true",
        br#"{"tag_name":"v1.0.0"}"#,
        Refusal::ReleaseOrTag,
    );
    assert_eq!(
        release,
        format!(
            "A release or tag on `github.com/acme/app` isn't allowed from a chat, so it was \
             refused. {NOBODY_ASKED} A release or tag purlis can hold for the person's press \
             has no query and no bot command in its body."
        )
    );
    let fork = said(
        &s,
        "POST",
        "api.github.com",
        "/repos/acme/app/forks?org=x",
        Refusal::NotOnList,
    );
    assert_eq!(
        fork,
        format!(
            "A fork on `github.com/acme/app` isn't allowed from a chat, so it was refused. \
             {NOBODY_ASKED} A fork purlis can hold for the person's press has no query."
        )
    );
    // A merge route the recogniser does not hold (`Other`) has no exact form to name.
    let gitlab = said(
        &s,
        "POST",
        "gitlab.com",
        "/api/v4/projects/42/merge_requests/7/merge_when_pipeline_succeeds",
        Refusal::Merge,
    );
    assert_eq!(
        gitlab,
        format!(
            "A merge on `gitlab.com/group/proj` isn't allowed from a chat, so it was refused. \
             {NOBODY_ASKED}"
        )
    );
    for text in [&merge, &release, &fork, &gitlab] {
        assert!(!text.contains("held for"), "{text}");
    }
    assert!(asks.0.lock().unwrap().is_empty(), "nobody was asked");
}

#[test]
fn a_request_refused_by_a_segments_name_is_not_claimed_as_that_act() {
    purlis_core::unsteered!();
    let (s, counted) = counting();
    // A label named `merge`, or `releases`, is refused as the decision always refused it, and is
    // told it is a request purlis doesn't carry.
    let merge_label = said(
        &s,
        "DELETE",
        "api.github.com",
        "/repos/acme/app/issues/7/labels/merge",
        Refusal::Merge,
    );
    assert_eq!(
        merge_label,
        "purlis doesn't carry this request (`DELETE /repos/{owner}/{repo}/issues/{n}/labels/merge` \
         on `github.com/acme/app`), so it was refused. No setting can allow it."
    );
    let release_label = said(
        &s,
        "DELETE",
        "api.github.com",
        "/repos/acme/app/issues/8/labels/releases",
        Refusal::ReleaseOrTag,
    );
    assert!(
        release_label.starts_with("purlis doesn't carry this request"),
        "{release_label}"
    );
    // A merge flag in a query is no merge route either.
    let flagged = said(
        &s,
        "PATCH",
        "api.github.com",
        "/repos/acme/app/pulls/7?auto_merge=1",
        Refusal::Merge,
    );
    assert!(
        flagged.starts_with("purlis doesn't carry this request"),
        "{flagged}"
    );
    let rules: Vec<&str> = counted
        .lock()
        .unwrap()
        .iter()
        .map(|(_, r)| r.rule)
        .collect();
    assert_eq!(rules, ["forge.no-action"], "counted once, as no action");
}

#[test]
fn a_floor_act_refusal_says_it_is_the_persons_to_do() {
    purlis_core::unsteered!();
    let s = scene();
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

#[test]
fn a_caller_that_did_not_prove_itself_is_told_nothing_of_where_its_request_went() {
    purlis_core::unsteered!();
    let (s, counted) = counting();
    for (method, target, auth) in [
        ("POST", "/repos/acme/app/milestones", Auth::None),
        (
            "PUT",
            "/repos/acme/app/pulls/7/merge",
            Auth::Bearer("not-the-sentinel"),
        ),
    ] {
        let unproven = headers(match auth {
            Auth::Bearer(token) => Auth::Bearer(token),
            _ => Auth::None,
        });
        let head = RequestHead::new(method, "api.github.com", 443, target);
        let out = s.send_as(&s.a, method, "api.github.com", 443, target, auth, b"{}");
        let Outcome::Refused(refusal) = out else {
            panic!("{target}: {out:?}");
        };
        let text = s
            .proxy
            .refused_said(&s.a, &head, &unproven, refusal)
            .unwrap_or_else(|| panic!("{target} has no sentence"));
        assert_eq!(
            text,
            "purlis doesn't carry this request, so it was refused."
        );
    }
    assert!(
        counted.lock().unwrap().is_empty(),
        "an unproven caller counts nothing"
    );
}

type Counted = Arc<Mutex<Vec<(String, Refused)>>>;

fn counting() -> (Scene, Counted) {
    let counted: Counted = Arc::default();
    let kept = counted.clone();
    let s = holding(&Asks::default(), move |proxy| {
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
    let sent: [(&str, &str, &[u8], &str); 5] = [
        (
            "POST",
            "/repos/acme/app/milestones",
            b"{}",
            "forge.no-action",
        ),
        (
            "GET",
            "/repos/acme/app/hooks",
            b"",
            "forge.no-action.administration",
        ),
        (
            "PUT",
            "/repos/acme/app/pulls/7/merge",
            b"{}",
            "forge.not-allowed.merge",
        ),
        (
            "POST",
            "/repos/acme/app/releases?draft=true",
            br#"{"tag_name":"v1.0.0"}"#,
            "forge.not-allowed.release-or-tag",
        ),
        (
            "POST",
            "/repos/acme/app/forks?org=x",
            b"{}",
            "forge.not-allowed.fork",
        ),
    ];
    for (method, target, body, _) in sent {
        let out = s.send(method, "api.github.com", target, body);
        assert!(matches!(out, Outcome::Refused(_)), "{target}: {out:?}");
    }
    // A read the route carries is not a refusal, and counts nothing.
    let read = s.send("GET", "api.github.com", "/repos/acme/app/pulls/7", b"");
    assert!(matches!(read, Outcome::Forward(_)), "{read:?}");
    let counted = counted.lock().unwrap().clone();
    let rules: Vec<&str> = counted.iter().map(|(_, r)| r.rule).collect();
    assert_eq!(rules, sent.map(|(_, _, _, rule)| rule));
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
fn a_chat_refused_in_a_loop_is_counted_once_for_each_rule_and_host() {
    purlis_core::unsteered!();
    let (s, counted) = counting();
    for n in 0..50 {
        let out = s.send(
            "POST",
            "api.github.com",
            &format!("/repos/acme/app/milestones/{n}"),
            b"{}",
        );
        assert!(is_refused(&out, Refusal::NotOnList), "{out:?}");
    }
    // Another host, and another rule, are each a line of their own.
    s.send(
        "POST",
        "gitlab.com",
        "/api/v4/projects/42/milestones",
        b"{}",
    );
    s.send(
        "PUT",
        "api.github.com",
        "/repos/acme/app/pulls/7/merge",
        b"{}",
    );
    let counted = counted.lock().unwrap().clone();
    let lines: Vec<(&str, &str)> = counted
        .iter()
        .map(|(_, r)| (r.rule, r.host.as_str()))
        .collect();
    assert_eq!(
        lines,
        [
            ("forge.no-action", "api.github.com:443"),
            ("forge.no-action", "gitlab.com:443"),
            ("forge.not-allowed.merge", "api.github.com:443"),
        ]
    );
}

#[test]
fn a_forge_refusal_text_holds_no_body_or_number() {
    purlis_core::unsteered!();
    let (s, counted) = counting();
    let target = "/repos/acme/app/milestones/48213?state=SECRETQUERY";
    let body = br#"{"title":"SECRETBODY 77341"}"#;
    let text = said_of(
        &s,
        "POST",
        "api.github.com",
        target,
        body,
        Refusal::NotOnList,
    );
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
    let s = scene();
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
    let s = scene();
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

#[test]
fn a_non_ascii_or_control_character_target_is_never_drawn() {
    purlis_core::unsteered!();
    let s = scene();
    for target in [
        // `été`, a right-to-left override, a bell, an escape sequence and a line break.
        "/repos/acme/app/milestones/%C3%A9t%C3%A9",
        "/repos/acme/app/%E2%80%AEevil/milestones",
        "/repos/acme/app/milestones/a%07b",
        "/repos/acme/app/milestones/%1B%5B31m",
        "/repos/acme/app/milestones/x%0D%0AInjected: yes",
    ] {
        let head = RequestHead::new("POST", "api.github.com", 443, target);
        let Outcome::Refused(refusal) = s.send("POST", "api.github.com", target, b"{}") else {
            panic!("{target} was not refused");
        };
        let Some(text) = s.proxy.refused_said(
            &s.a,
            &head,
            &headers(Auth::Bearer(s.a_sentinel.expose())),
            refusal,
        ) else {
            // A target that does not read is told what it always was.
            continue;
        };
        assert!(
            text.chars()
                .all(|c| c == '…' || (c.is_ascii() && !c.is_ascii_control())),
            "{target}: {text:?}"
        );
        for drawn in ["été", "evil", "Injected", "31m"] {
            assert!(!text.contains(drawn), "{drawn} in {text}");
        }
    }
    // The first is a route the policy reads, so it is told, with the word left out.
    let text = said(
        &s,
        "POST",
        "api.github.com",
        "/repos/acme/app/milestones/%C3%A9t%C3%A9",
        Refusal::NotOnList,
    );
    assert!(
        text.contains("`POST /repos/{owner}/{repo}/milestones/{…}`"),
        "{text}"
    );
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
    let s = holding(&Asks::default(), |proxy| {
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
        format!(
            "A merge on `github.com/acme/app` was refused because {} {NOBODY_ASKED}",
            enforcement::unaccepted_said(AcceptedOn::ThisMachine)
        )
    );
    // It names no family to loosen, and carries no note on setting one to log or off.
    assert!(!merge.contains("guards."), "{merge}");
    assert!(
        !merge.contains(purlis_core::toolgate::OVERRIDE_NOTE.trim()),
        "{merge}"
    );
    // A merge route accepting would not open is not refused for the acceptance.
    let not_opened = said(
        &s,
        "POST",
        "gitlab.com",
        "/api/v4/projects/42/merge_requests/7/merge_when_pipeline_succeeds",
        Refusal::Merge,
    );
    assert!(!not_opened.contains("accepted"), "{not_opened}");
    assert!(
        not_opened.contains("isn't allowed from a chat"),
        "{not_opened}"
    );
    // An act no policy opens is refused as it always is: the acceptance is not its cause.
    let release = said_of(
        &s,
        "POST",
        "api.github.com",
        "/repos/acme/app/releases?draft=true",
        br#"{"tag_name":"v1.0.0"}"#,
        Refusal::ReleaseOrTag,
    );
    assert!(release.contains("isn't allowed from a chat"), "{release}");
    assert!(!release.contains("accepted"), "{release}");
}
