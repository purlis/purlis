// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #7958: GitHub's `Link` header names the next page of a repo's list by the repo's numeric id,
//! `/repositories/{id}/…`, so `gh api --paginate` asks for page 2 in that form. A path in that
//! form whose id is the one the forge's read recorded for the chat's repo is read exactly as
//! `/repos/{owner}/{repo}/…`: the same rows, the same decision, for a read and for a write. Any
//! other id, or one that is not a number, is another repo's, and a repo whose id was never
//! recorded is reached by no id at all.

use std::sync::{Arc, Mutex};

use purlis_core::enforcement::Mode;
use purlis_core::forge::mergestance::PolicyMergeStance;
use purlis_core::forge::proxy::{ChatId, Held, HeldSink, Outcome, Refusal};
use purlis_core::heldact::Requested;

use crate::support::forge_proxy::{GHES, ObjectsWithHeads, Scene, is_refused};

/// The id the forge's own read gave chat-a's `Acme/App`.
const ID: u64 = 812_345_678;

fn scene() -> Scene {
    let objects = ObjectsWithHeads::default().pull("acme/app", "PR_kwDO_app", 7, "acme/app", "fix");
    Scene::owning_with_github_id(objects, &[("fix", &"1".repeat(40))], ID)
}

/// The id the GHES's own read gave chat-a's `acme/app` there.
const GHES_ID: u64 = 4_242;

/// The held sink the app installs, standing in: each ask is kept.
#[derive(Default, Clone)]
struct Asks(Arc<Mutex<Vec<Requested>>>);

impl HeldSink for Asks {
    fn ask(&self, _: &ChatId, requested: &Requested) -> String {
        self.0.lock().unwrap().push(requested.clone());
        "asked".to_owned()
    }
}

type Lines = Arc<Mutex<Vec<(String, String)>>>;

/// [`scene`] on github.com and a GHES, each repo known by its id, with a held sink installed and
/// `guards.merge` at `mode`: the asks it raised and the lines `log` recorded.
fn holding(mode: Mode) -> (Scene, Asks, Lines) {
    let asks = Asks::default();
    let lines: Lines = Arc::default();
    let kept = lines.clone();
    let stance = PolicyMergeStance::reading(
        move || mode,
        move |chat, said| {
            kept.lock()
                .unwrap()
                .push((chat.to_owned(), said.to_owned()));
        },
    );
    let objects = ObjectsWithHeads::default().pull("acme/app", "PR_kwDO_app", 7, "acme/app", "fix");
    let sink = asks.clone();
    let s = Scene::with_github_ids(
        objects,
        &[("fix", &"1".repeat(40))],
        true,
        &[("github.com", ID), (GHES, GHES_ID)],
        move |proxy| {
            proxy
                .with_held(Box::new(sink))
                .with_merge_stance(Arc::new(stance))
        },
    );
    (s, asks, lines)
}

/// The held acts' exact requests, as a path under the repo and a body.
fn acts() -> Vec<(&'static str, &'static str, String)> {
    let sha = "a".repeat(40);
    vec![
        ("PUT", "pulls/7/merge", format!(r#"{{"sha":"{sha}"}}"#)),
        ("POST", "releases", r#"{"tag_name":"v1.0.0"}"#.to_owned()),
        (
            "POST",
            "git/refs",
            format!(r#"{{"ref":"refs/tags/v1.0.0","sha":"{sha}"}}"#),
        ),
        ("POST", "forks", "{}".to_owned()),
    ]
}

/// The held act's refusal and sentence, where `out` is one.
fn held(out: &Outcome) -> Option<&Held> {
    match out {
        Outcome::Held(held) => Some(held),
        _ => None,
    }
}

/// What the proxy decided, without where it sends it: the class and whether it carries the
/// credential and which body, or the refusal.
fn decision(out: &Outcome) -> String {
    match out {
        Outcome::Forward(f) => format!(
            "forward {:?} credentialed={} body={:?}",
            f.class, f.credentialed, f.body
        ),
        Outcome::Refused(r) => format!("refused {r:?}"),
        Outcome::Held(_) => "held".to_owned(),
    }
}

#[test]
fn page_two_of_an_allowed_read_in_the_repositories_form_is_allowed() {
    purlis_core::unsteered!();
    let s = scene();
    let sha = "a".repeat(40);
    // What GitHub's `Link: <…>; rel="next"` gives for the reads #7958 lost.
    for rest in [
        format!("commits/{sha}/check-runs?per_page=100&page=2"),
        "pulls?state=open&per_page=100&page=2".to_owned(),
        "actions/runs?per_page=100&page=3".to_owned(),
        format!("commits/{sha}/statuses?page=2"),
    ] {
        let by_id = s.send(
            "GET",
            "api.github.com",
            &format!("/repositories/{ID}/{rest}"),
            b"",
        );
        let by_path = s.send(
            "GET",
            "api.github.com",
            &format!("/repos/acme/app/{rest}"),
            b"",
        );
        assert!(matches!(by_id, Outcome::Forward(_)), "{rest}: {by_id:?}");
        assert_eq!(decision(&by_id), decision(&by_path), "{rest}");
    }
}

#[test]
fn a_repositories_path_with_any_other_id_is_another_repo() {
    purlis_core::unsteered!();
    let s = scene();
    for target in [
        format!("/repositories/{}/pulls?page=2", ID + 1),
        "/repositories/1/commits".to_owned(),
        "/repositories/0/pulls".to_owned(),
        "/repositories/acme/pulls".to_owned(),
        "/repositories/-1/pulls".to_owned(),
        format!("/repositories/{ID}0/pulls"),
        // A second spelling of the recorded id is not the id.
        format!("/repositories/0{ID}/pulls"),
        format!("/repositories/+{ID}/pulls"),
        format!("/repositories/0{ID}x/pulls"),
        "/repositories/99999999999999999999999/pulls".to_owned(),
    ] {
        let out = s.send("GET", "api.github.com", &target, b"");
        assert!(is_refused(&out, Refusal::OtherRepo), "{target}: {out:?}");
    }
}

#[test]
fn a_repo_whose_id_was_never_recorded_is_reached_by_no_id() {
    purlis_core::unsteered!();
    // `Scene::new` records no numeric id for `Acme/App`.
    let s = Scene::new();
    let out = s.send("GET", "api.github.com", "/repositories/1/pulls?page=2", b"");
    assert!(is_refused(&out, Refusal::OtherRepo), "{out:?}");
}

#[test]
fn a_write_in_the_repositories_form_is_decided_as_the_repos_form_is() {
    purlis_core::unsteered!();
    let s = scene();
    let comment = serde_json::json!({"body": "looks good"}).to_string();
    let merge_word = serde_json::json!({"body": "/merge"}).to_string();
    for (method, rest, body) in [
        // Allowed writes.
        ("POST", "issues/7/comments", comment.as_str()),
        ("PUT", "pulls/7/update-branch", ""),
        // Refused writes, each for its own reason.
        ("POST", "issues/7/comments", merge_word.as_str()),
        ("PUT", "pulls/7/merge", ""),
        ("POST", "hooks", "{}"),
        ("GET", "hooks", ""),
        ("PATCH", "", "{\"default_branch\": \"x\"}"),
        ("DELETE", "", ""),
        ("POST", "pulls/7/reviews", "{\"event\": \"APPROVE\"}"),
    ] {
        let by_id = format!("/repositories/{ID}/{rest}");
        let by_path = format!("/repos/acme/app/{rest}");
        let (by_id, by_path) = (by_id.trim_end_matches('/'), by_path.trim_end_matches('/'));
        let a = s.send(method, "api.github.com", by_id, body.as_bytes());
        let b = s.send(method, "api.github.com", by_path, body.as_bytes());
        assert_eq!(decision(&a), decision(&b), "{method} {rest}");
    }
    // The update-branch of a pull request whose head the chat did not make is refused alike.
    let out = s.send(
        "PUT",
        "api.github.com",
        &format!("/repositories/{ID}/pulls/8/update-branch"),
        b"",
    );
    assert!(!matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn the_repositories_form_is_the_api_hosts_only() {
    purlis_core::unsteered!();
    let s = scene();
    // On github.com itself the path is git's or a web page's, never the API's.
    let out = s.send(
        "GET",
        "github.com",
        &format!("/repositories/{ID}/pulls"),
        b"",
    );
    assert!(!matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn a_held_act_in_the_repositories_form_is_held_and_asked_as_the_repos_form_is() {
    purlis_core::unsteered!();
    for (host, by_id, by_path) in [
        (
            "api.github.com",
            format!("/repositories/{ID}"),
            "/repos/acme/app".to_owned(),
        ),
        (
            GHES,
            format!("/api/v3/repositories/{GHES_ID}"),
            "/api/v3/repos/acme/app".to_owned(),
        ),
    ] {
        for (method, rest, body) in acts() {
            let (s, asks, _) = holding(Mode::Enforce);
            let a = s.send(method, host, &format!("{by_path}/{rest}"), body.as_bytes());
            let b = s.send(method, host, &format!("{by_id}/{rest}"), body.as_bytes());
            assert!(
                matches!(a, Outcome::Held(_)),
                "{host} {method} {rest}: {a:?}"
            );
            assert_eq!(held(&a), held(&b), "{host} {method} {rest}");
            let asks = asks.0.lock().unwrap();
            assert_eq!(asks.len(), 2, "{host} {method} {rest}: {asks:?}");
            assert_eq!(asks[0], asks[1], "{host} {method} {rest}");
        }
    }
}

#[test]
fn a_held_act_by_another_repos_id_raises_no_ask() {
    purlis_core::unsteered!();
    let (s, asks, _) = holding(Mode::Enforce);
    for (method, rest, body) in acts() {
        for (host, target) in [
            ("api.github.com", format!("/repositories/{}/{rest}", ID + 1)),
            ("api.github.com", format!("/repositories/0{ID}/{rest}")),
            ("api.github.com", format!("/repositories/+{ID}/{rest}")),
            // Each host's id is its own repo's: one host's id names nothing on the other.
            ("api.github.com", format!("/repositories/{GHES_ID}/{rest}")),
            (GHES, format!("/api/v3/repositories/{ID}/{rest}")),
        ] {
            let out = s.send(method, host, &target, body.as_bytes());
            assert!(
                !matches!(out, Outcome::Forward(_) | Outcome::Held(_)),
                "{host}{target}: {out:?}"
            );
        }
    }
    assert!(asks.0.lock().unwrap().is_empty());
}

#[test]
fn a_ghes_read_in_the_repositories_form_is_read_as_its_repos_form() {
    purlis_core::unsteered!();
    let (s, _, _) = holding(Mode::Enforce);
    for rest in [
        "pulls?state=open&per_page=100&page=2",
        "actions/runs?page=3",
    ] {
        let by_id = s.send(
            "GET",
            GHES,
            &format!("/api/v3/repositories/{GHES_ID}/{rest}"),
            b"",
        );
        let by_path = s.send("GET", GHES, &format!("/api/v3/repos/acme/app/{rest}"), b"");
        assert!(matches!(by_id, Outcome::Forward(_)), "{rest}: {by_id:?}");
        assert_eq!(decision(&by_id), decision(&by_path), "{rest}");
    }
    // The GHES form is the GHES's, and api.github.com's is api.github.com's.
    for (host, target) in [
        ("api.github.com", format!("/api/v3/repositories/{ID}/pulls")),
        (GHES, format!("/repositories/{GHES_ID}/pulls")),
        (GHES, format!("/api/v3/repositories/{}/pulls", GHES_ID + 1)),
    ] {
        let out = s.send("GET", host, &target, b"");
        assert!(
            !matches!(out, Outcome::Forward(_)),
            "{host}{target}: {out:?}"
        );
    }
}

#[test]
fn a_merge_in_the_repositories_form_under_log_is_forwarded_and_logged_by_the_repos_name() {
    purlis_core::unsteered!();
    let (s, asks, lines) = holding(Mode::Log);
    let body = format!(r#"{{"sha":"{}"}}"#, "a".repeat(40));
    for (host, target, repo) in [
        (
            "api.github.com",
            format!("/repositories/{ID}/pulls/7/merge"),
            "github.com/acme/app",
        ),
        (
            GHES,
            format!("/api/v3/repositories/{GHES_ID}/pulls/7/merge"),
            "ghe.corp.example/acme/app",
        ),
    ] {
        let out = s.send("PUT", host, &target, body.as_bytes());
        assert!(
            matches!(out, Outcome::Forward(_)),
            "{host}{target}: {out:?}"
        );
        let last = lines.lock().unwrap().last().cloned();
        assert_eq!(
            last,
            Some(("chat-a".to_owned(), repo.to_owned())),
            "{host}{target}"
        );
    }
    // Another repo's id is still another repo, under `log` too.
    let out = s.send(
        "PUT",
        "api.github.com",
        &format!("/repositories/{}/pulls/7/merge", ID + 1),
        body.as_bytes(),
    );
    assert!(is_refused(&out, Refusal::OtherRepo), "{out:?}");
    assert_eq!(lines.lock().unwrap().len(), 2);
    assert!(asks.0.lock().unwrap().is_empty());
}

#[test]
fn a_held_act_in_the_repositories_form_on_the_wrong_host_is_decided_as_the_repos_form_there() {
    purlis_core::unsteered!();
    // Each form where its host does not serve it: the web host, the GHES's form on
    // api.github.com, and api.github.com's form on the GHES.
    for (host, by_id, by_path) in [
        (
            "github.com",
            format!("/repositories/{ID}"),
            "/repos/acme/app",
        ),
        (
            "api.github.com",
            format!("/api/v3/repositories/{ID}"),
            "/api/v3/repos/acme/app",
        ),
        (GHES, format!("/repositories/{GHES_ID}"), "/repos/acme/app"),
    ] {
        for (method, rest, body) in acts() {
            let (s, asks, _) = holding(Mode::Enforce);
            let a = s.send(method, host, &format!("{by_path}/{rest}"), body.as_bytes());
            let path_asks = asks.0.lock().unwrap().len();
            let b = s.send(method, host, &format!("{by_id}/{rest}"), body.as_bytes());
            assert_eq!(decision(&a), decision(&b), "{host} {method} {rest}");
            assert_eq!(
                asks.0.lock().unwrap().len(),
                path_asks * 2,
                "{host} {method} {rest}: the alias asks only where the path form does"
            );
        }
    }
}
