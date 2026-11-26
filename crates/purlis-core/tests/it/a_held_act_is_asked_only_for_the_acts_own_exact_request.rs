// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An ask is raised only for the act's own exact request** (one login, slice K; ADR 0128 D8). These
//! are the properties slice J's own registry of asks held, each now held on the one path there is,
//! the proxy's hand-off to F's registry: a request that is not exactly the act, that carries text a
//! merge bot would act on, that does not pin the head it shows, or that a chat could use to smuggle
//! another act in raises nobody's ask, and is refused with its own reason.

use purlis_core::forge::proxy::{
    AllowedRepo, ChatId, ForgeProxy, Inbound, Kind, NoCredentials, NoObjects, Outcome, RequestHead,
};

use crate::a_refused_tag_release_or_fork_on_a_chat_route_is_asked_and_the_press_makes_what_was_shown::{
    Scene, scene, said,
};
use crate::support::held_acts::{Rec, Second, NoTrees, press, sha, the_person, world};

fn is_held(outcome: &Outcome) -> bool {
    matches!(outcome, Outcome::Held(_))
}

fn tag_body() -> String {
    format!(r#"{{"ref":"refs/tags/v1","sha":"{}"}}"#, sha('2').as_str())
}

fn nobody_was_asked(s: &Scene) {
    assert!(s.held.every().is_empty(), "an ask was raised");
}

// J: `an_act_has_the_shape_of_its_own_request_and_no_other`,
//    `a_refusal_with_a_query_that_is_not_the_acts_own_request_raises_no_ask`.
#[test]
fn a_target_with_a_query_or_another_method_than_the_acts_own_is_a_plain_refusal() {
    purlis_core::unsteered!();
    let s = scene(true);
    for (method, target, body) in [
        ("POST", "/repos/acme/app/git/refs?x=1", tag_body()),
        ("POST", "/repos/acme/app/git/refs#frag", tag_body()),
        (
            "PUT",
            "/repos/acme/app/pulls/12/merge?auto_merge=1",
            format!(r#"{{"sha":"{}"}}"#, sha('2').as_str()),
        ),
        // A cancel of auto-merge is not a merge the person is asked to make.
        (
            "DELETE",
            "/repos/acme/app/pulls/12/merge?auto_merge=1",
            String::new(),
        ),
        ("GET", "/repos/acme/app/git/refs", String::new()),
        ("PUT", "/repos/acme/app/git/refs", tag_body()),
    ] {
        let out = s.own(method, target, &body);
        if target.contains("auto_merge") && method == "PUT" {
            // An auto-merge is refused with its own reason, which the chat is told as a refusal
            // (`Outcome::Held` carries the words of a refusal too): no ask stands behind it.
            let said = said(out);
            assert!(said.contains("Nobody was asked"), "{said}");
            assert!(said.contains("auto-merge would merge any head"), "{said}");
        } else {
            assert!(!is_held(&out), "{method} {target} was held: {out:?}");
        }
        nobody_was_asked(&s);
    }
}

// J: `a_release_whose_text_holds_a_merge_command_is_not_asked`,
//    `a_merge_command_is_found_in_any_string_of_a_body`.
#[test]
fn text_a_merge_bot_would_act_on_raises_no_ask_whatever_string_it_is_in() {
    purlis_core::unsteered!();
    let s = scene(true);
    for notes in [
        "bors r+",
        "/merge",
        "@dependabot merge",
        "ship it\n/merge\n",
    ] {
        for body in [
            serde_json::json!({"tag_name": "v1", "body": notes}),
            serde_json::json!({"tag_name": "v1", "name": notes}),
            serde_json::json!({"tag_name": "v1", "body": "x", "extra": {"deep": [notes]}}),
        ] {
            let out = s.own("POST", "/repos/acme/app/releases", &body.to_string());
            assert!(!is_held(&out), "{notes:?} was held: {out:?}");
        }
    }
    nobody_was_asked(&s);
    // The same release without the text is asked.
    let out = s.own(
        "POST",
        "/repos/acme/app/releases",
        &serde_json::json!({"tag_name": "v1", "body": "notes"}).to_string(),
    );
    assert!(is_held(&out), "{out:?}");
}

// J: `a_merge_is_asked_only_when_its_body_pins_the_head_with_a_sha`,
//    `a_body_pins_the_head_only_with_one_non_empty_field_and_a_form_only_where_allowed`.
#[test]
fn a_merge_is_asked_only_with_one_non_empty_sha_and_a_form_only_on_gitlab() {
    purlis_core::unsteered!();
    let s = scene(true);
    for body in [
        "{}",
        r#"{"sha":""}"#,
        r#"{"sha":"   "}"#,
        r#"{"sha":["a","b"]}"#,
    ] {
        let out = s.own("PUT", "/repos/acme/app/pulls/12/merge", body);
        assert!(!is_held(&out), "{body} was held: {out:?}");
    }
    nobody_was_asked(&s);
    let good = format!(r#"{{"sha":"{}"}}"#, sha('2').as_str());
    assert!(is_held(&s.own(
        "PUT",
        "/repos/acme/app/pulls/12/merge",
        &good
    )));
}

// J: `a_graphql_refusal_raises_no_ask_whatever_the_document`,
//    `a_push_of_a_tag_raises_no_ask_and_a_mixed_push_is_refused_as_its_most_severe_class`.
#[test]
fn graphql_and_a_git_push_never_raise_an_ask() {
    purlis_core::unsteered!();
    let s = scene(true);
    for query in [
        r#"mutation { mergePullRequest(input:{pullRequestId:"PR_1"}) { clientMutationId } }"#,
        r#"mutation { createRef(input:{repositoryId:"R_1", name:"refs/tags/v1", oid:"2222222222222222222222222222222222222222"}) { clientMutationId } }"#,
    ] {
        let body = serde_json::json!({ "query": query }).to_string();
        let out = s.own("POST", "/graphql", &body);
        assert!(!is_held(&out), "{query} was held: {out:?}");
    }
    let head = RequestHead::new("POST", "github.com", 443, "/acme/app.git/git-receive-pack");
    let headers = vec![
        (
            "Authorization".to_owned(),
            format!("Basic {}", s.sentinel.expose()),
        ),
        (
            "Content-Type".to_owned(),
            "application/x-git-receive-pack-request".to_owned(),
        ),
    ];
    let pkt = "0000";
    let out = s.proxy.handle(
        &s.chat,
        &Inbound {
            head: &head,
            headers: &headers,
            body: pkt.as_bytes(),
        },
    );
    assert!(!is_held(&out), "{out:?}");
    nobody_was_asked(&s);
}

// J: `an_approval_is_asked_only_when_it_pins_the_head_and_carries_no_merge_command` (an approval is
// now refused outright, ADR 0128 D8: the chat does not approve its own work as the person).
#[test]
fn an_approval_is_refused_outright_and_asks_nobody() {
    purlis_core::unsteered!();
    let s = scene(true);
    for (method, target, body) in [
        (
            "POST",
            "/repos/acme/app/pulls/12/reviews",
            format!(
                r#"{{"event":"APPROVE","commit_id":"{}"}}"#,
                sha('2').as_str()
            ),
        ),
        (
            "POST",
            "/repos/acme/app/pulls/12/reviews",
            r#"{"event":"APPROVE"}"#.to_owned(),
        ),
    ] {
        let out = s.own(method, target, &body);
        assert!(matches!(out, Outcome::Refused(_)), "{out:?}");
    }
    nobody_was_asked(&s);
}

// J: `a_run_holds_a_bounded_number_of_asks_and_the_refusal_says_so`,
//    `a_dismissed_ask_is_removed_without_performing_and_makes_room`.
#[test]
fn a_chat_holds_a_bounded_number_of_asks_and_a_dont_removes_one_without_performing_it() {
    purlis_core::unsteered!();
    let s = scene(true);
    let mut asked = Vec::new();
    let mut full = None;
    for n in 0..64 {
        let body = format!(
            r#"{{"ref":"refs/tags/v{n}","sha":"{}"}}"#,
            sha('2').as_str()
        );
        let out = s.own("POST", "/repos/acme/app/git/refs", &body);
        let said = said(out);
        if said.contains("as many held acts waiting") {
            full = Some(n);
            break;
        }
        asked.push(n);
    }
    let full = full.expect("the chat's asks are bounded");
    assert_eq!(s.held.every().len(), asked.len());
    assert!(!asked.is_empty() && asked.len() == full);
    // A Don't removes one, performs nothing, and makes room for the next.
    let first = s.held.every()[0].id;
    assert!(s.held.dont(first, the_person()).unwrap());
    assert!(s.forge.log().is_empty(), "a Don't made something");
    let body = format!(
        r#"{{"ref":"refs/tags/vnext","sha":"{}"}}"#,
        sha('2').as_str()
    );
    assert!(is_held(&s.own("POST", "/repos/acme/app/git/refs", &body)));
}

// J: `an_ask_is_not_performed_for_a_repo_the_project_no_longer_records`,
//    `an_act_does_not_land_when_the_chat_ended_while_the_credential_was_fetched`.
#[test]
fn a_press_makes_nothing_for_a_repo_no_longer_recorded() {
    purlis_core::unsteered!();
    let s = scene(true);
    let said = said(s.own("POST", "/repos/acme/app/git/refs", &tag_body()));
    assert!(said.contains("Ask 1"), "{said}");
    let (_, digest) = s.held.shown(1).unwrap();
    let (second, none) = (Second::there(), Rec(vec![]));
    let world = world(&s.forge, &NoTrees, &second, &none);
    let first = press(&s.held, &world, 1, &digest);
    let again = press(&s.held, &world, 1, &digest);
    assert!(
        first.is_err() && again.is_err(),
        "a press went ahead: {first:?} {again:?}"
    );
    assert!(s.forge.log().is_empty());
    // The refused press put the ask away: nothing of it is left to press.
    assert!(s.held.every().is_empty());
}

// J: `a_held_act_on_a_repo_the_run_does_not_record_raises_no_ask`: an act of a repo on another
// host, or another repo of the same host, is refused whole.
#[test]
fn an_act_of_a_repo_the_chat_does_not_hold_on_that_host_raises_no_ask() {
    purlis_core::unsteered!();
    let proxy = ForgeProxy::new(Box::new(NoCredentials), Box::new(NoObjects));
    let chat = ChatId::new("chat-b");
    let _ = proxy.enrol(
        &chat,
        vec![AllowedRepo::new(Kind::GitHub, "ghe.example", 443, "acme/app", "main").unwrap()],
    );
    let s = scene(true);
    // The scene's chat holds acme/app on github.com: acme/app on any other host is nobody's.
    let head = RequestHead::new("POST", "api.ghe.example", 443, "/repos/acme/app/git/refs");
    let headers = vec![
        (
            "Authorization".to_owned(),
            format!("Bearer {}", s.sentinel.expose()),
        ),
        ("Content-Type".to_owned(), "application/json".to_owned()),
    ];
    let out = s.proxy.handle(
        &s.chat,
        &Inbound {
            head: &head,
            headers: &headers,
            body: tag_body().as_bytes(),
        },
    );
    assert!(!is_held(&out), "{out:?}");
    nobody_was_asked(&s);
}
