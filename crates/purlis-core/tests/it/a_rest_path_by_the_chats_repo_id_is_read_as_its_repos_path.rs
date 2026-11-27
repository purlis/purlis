// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #7958: GitHub's `Link` header names the next page of a repo's list by the repo's numeric id,
//! `/repositories/{id}/…`, so `gh api --paginate` asks for page 2 in that form. A path in that
//! form whose id is the one the forge's read recorded for the chat's repo is read exactly as
//! `/repos/{owner}/{repo}/…`: the same rows, the same decision, for a read and for a write. Any
//! other id, or one that is not a number, is another repo's, and a repo whose id was never
//! recorded is reached by no id at all.

use purlis_core::forge::proxy::{Outcome, Refusal};

use crate::support::forge_proxy::{ObjectsWithHeads, Scene, is_refused};

/// The id the forge's own read gave chat-a's `Acme/App`.
const ID: u64 = 812_345_678;

fn scene() -> Scene {
    let objects = ObjectsWithHeads::default().pull("acme/app", "PR_kwDO_app", 7, "acme/app", "fix");
    Scene::owning_with_github_id(objects, &[("fix", &"1".repeat(40))], ID)
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
