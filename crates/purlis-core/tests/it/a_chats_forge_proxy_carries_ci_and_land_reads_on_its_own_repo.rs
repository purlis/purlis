// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login follow-up (#7358; ADR 0128 amended 2026-11-22): a chat's ordinary CI and landing
//! work on its own repo is answered: the rules that apply to a branch, a merge queue's state, and
//! a rerun of a workflow run (an approval is refused: it runs a stranger's code). Another repo, a write to a ruleset, a merge and the
//! enqueue of one stay refused.

use purlis_core::forge::proxy::{Class, Outcome, Refusal, TokenKind};

use crate::support::forge_proxy::{Objects, Scene, graphql, is_refused};

fn class(out: Outcome) -> Class {
    match out {
        Outcome::Forward(allowed) => allowed.class,
        other => panic!("not forwarded: {other:?}"),
    }
}

fn query(s: &Scene, q: &str) -> Outcome {
    s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(q, serde_json::json!({})),
    )
}

#[test]
fn the_rulesets_and_branch_rules_of_the_chats_repo_are_read() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for target in [
        "/repos/acme/app/rulesets",
        "/repos/acme/app/rulesets/9",
        "/repos/acme/app/rulesets?per_page=100&page=2",
        "/repos/acme/app/rules/branches/main",
        "/repos/acme/app/rules/branches/main?per_page=100&page=1",
    ] {
        let out = s.send("GET", "api.github.com", target, b"");
        assert!(matches!(out, Outcome::Forward(_)), "{target}: {out:?}");
    }
}

#[test]
fn the_rules_of_another_repo_and_every_write_to_a_rule_are_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for (method, target, why) in [
        ("GET", "/repos/acme/other/rulesets", Refusal::OtherRepo),
        (
            "GET",
            "/repos/acme/other/rules/branches/main",
            Refusal::OtherRepo,
        ),
        ("POST", "/repos/acme/app/rulesets", Refusal::Ruleset),
        ("PUT", "/repos/acme/app/rulesets/9", Refusal::Ruleset),
        ("PATCH", "/repos/acme/app/rulesets/9", Refusal::Ruleset),
        ("DELETE", "/repos/acme/app/rulesets/9", Refusal::Ruleset),
        (
            "GET",
            "/repos/acme/app/branches/main/protection",
            Refusal::Ruleset,
        ),
        (
            "GET",
            "/repos/acme/app/rulesets/rule-suites",
            Refusal::Ruleset,
        ),
    ] {
        let out = s.send(method, "api.github.com", target, b"{}");
        assert!(is_refused(&out, why), "{method} {target}: {out:?}");
    }
}

#[test]
fn a_rerun_of_a_run_of_the_chats_repo_is_answered() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for target in [
        "/repos/acme/app/actions/runs/9/rerun",
        "/repos/acme/app/actions/runs/9/rerun-failed-jobs",
    ] {
        for body in [&b""[..], b"{}"] {
            assert_eq!(
                class(s.send("POST", "api.github.com", target, body)),
                Class::CiWrite,
                "{target}"
            );
        }
    }
}

#[test]
fn an_approval_of_a_run_is_refused_whatever_the_repo() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for target in [
        "/repos/acme/app/actions/runs/9/approve",
        "/repos/acme/app/actions/runs/9/approve?x=1",
    ] {
        for body in [&b""[..], b"{}"] {
            let out = s.send("POST", "api.github.com", target, body);
            assert!(is_refused(&out, Refusal::Approval), "{target}: {out:?}");
        }
    }
}

#[test]
fn a_rerun_that_carries_anything_in_its_body_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for body in [
        &br#"{"enable_debug_logging":true}"#[..],
        br#"{"x":1}"#,
        b"enable_debug_logging=true",
    ] {
        for verb in ["rerun", "rerun-failed-jobs"] {
            let target = format!("/repos/acme/app/actions/runs/9/{verb}");
            let out = s.send("POST", "api.github.com", &target, body);
            assert!(
                matches!(out, Outcome::Refused(_)),
                "{verb} {}: {out:?}",
                String::from_utf8_lossy(body)
            );
        }
    }
}

#[test]
fn a_rerun_or_an_approval_anywhere_else_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for (method, target, why) in [
        // Another repo.
        (
            "POST",
            "/repos/acme/other/actions/runs/9/rerun",
            Refusal::OtherRepo,
        ),
        // Not a run, or not those verbs.
        (
            "POST",
            "/repos/acme/app/actions/runs/x/rerun",
            Refusal::NotOnList,
        ),
        (
            "POST",
            "/repos/acme/app/actions/runs/9/cancel",
            Refusal::NotOnList,
        ),
        (
            "DELETE",
            "/repos/acme/app/actions/runs/9",
            Refusal::NotOnList,
        ),
        // A review's approval and every merge stay what they were.
        ("POST", "/repos/acme/app/pulls/7/reviews", Refusal::Approval),
        ("PUT", "/repos/acme/app/pulls/7/merge", Refusal::Merge),
        // What a chat may not reach through a run: its environments' approvals and the secrets.
        (
            "POST",
            "/repos/acme/app/actions/runs/9/pending_deployments",
            Refusal::NotOnList,
        ),
        ("GET", "/repos/acme/app/actions/secrets", Refusal::Admin),
    ] {
        let out = s.send(method, "api.github.com", target, b"{}");
        assert!(is_refused(&out, why), "{method} {target}: {out:?}");
    }
    // A run carries no query: the forge would read it.
    let out = s.send(
        "POST",
        "api.github.com",
        "/repos/acme/app/actions/runs/9/rerun?enable_debug_logging=true",
        b"",
    );
    assert!(is_refused(&out, Refusal::MalformedRequest), "{out:?}");
}

#[test]
fn the_state_of_a_merge_queue_is_read() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for q in [
        "query{ repository(owner:\"acme\", name:\"app\"){ pullRequest(number:7){ isInMergeQueue isMergeQueueEnabled } } }",
        "query{ repository(owner:\"acme\", name:\"app\"){ pullRequest(number:7){ mergeQueueEntry{ state position enqueuedAt estimatedTimeToMerge solo enqueuer{ login } } } } }",
        "query{ repository(owner:\"acme\", name:\"app\"){ mergeQueue{ url nextEntryEstimatedTimeToMerge entries(first:10){ nodes{ position state pullRequest{ number } } totalCount } } } }",
    ] {
        let out = query(&s, q);
        assert!(matches!(out, Outcome::Forward(_)), "{q}: {out:?}");
    }
}

#[test]
fn a_merge_queue_of_another_repo_its_settings_and_its_enqueue_are_refused() {
    purlis_core::unsteered!();
    let mut objects = Objects::default();
    objects.0.insert("PR_other".into(), "acme/other".into());
    let s = Scene::with(TokenKind::Oauth, objects);
    for q in [
        "query{ repository(owner:\"acme\", name:\"other\"){ mergeQueue{ url } } }",
        "query{ repository(owner:\"acme\", name:\"app\"){ mergeQueue{ configuration{ mergeMethod } } } }",
        "query{ repository(owner:\"acme\", name:\"app\"){ mergeQueue{ repository{ collaborators{ totalCount } } } } }",
        "query{ repository(owner:\"acme\", name:\"app\"){ pullRequest(number:7){ mergeQueueEntry{ somethingNew } } } }",
    ] {
        let out = query(&s, q);
        assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{q}: {out:?}");
    }
    for q in [
        "mutation{ enqueuePullRequest(input:{pullRequestId:\"PR_other\"}){ clientMutationId } }",
        "mutation{ dequeuePullRequest(input:{id:\"PR_other\"}){ clientMutationId } }",
        "mutation{ enablePullRequestAutoMerge(input:{pullRequestId:\"PR_other\"}){ clientMutationId } }",
    ] {
        let out = query(&s, q);
        assert!(matches!(out, Outcome::Refused(_)), "{q}: {out:?}");
    }
}

#[test]
fn opening_a_pull_request_and_an_issue_through_graphql_is_answered_on_the_chats_repo_only() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for q in [
        "mutation{ createPullRequest(input:{repositoryId:\"R_acme_app\", baseRefName:\"main\", headRefName:\"work\", title:\"t\"}){ pullRequest{ url } } }",
        "mutation{ createIssue(input:{repositoryId:\"R_acme_app\", title:\"t\"}){ issue{ url } } }",
    ] {
        assert!(matches!(query(&s, q), Outcome::Forward(_)), "{q}");
    }
    for q in [
        "mutation{ createPullRequest(input:{repositoryId:\"R_acme_other\", baseRefName:\"main\", headRefName:\"work\", title:\"t\"}){ pullRequest{ url } } }",
        "mutation{ createIssue(input:{repositoryId:\"R_acme_other\", title:\"t\"}){ issue{ url } } }",
        "mutation{ createPullRequest(input:{repositoryId:\"R_acme_app\", baseRefName:\"main\", headRefName:\"fork:work\", title:\"t\"}){ pullRequest{ url } } }",
    ] {
        let out = query(&s, q);
        assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{q}: {out:?}");
    }
}
