// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login: the route honours `[policy] guards.merge` as the hook's merge guard does (ADR 0128,
//! amended 2026-11-22).
//!
//! `enforce` holds a merge for the person; `log` forwards it and records one line (the chat and
//! the repo, never a body); `off` forwards it. Every shape a chat merges by is covered: REST and
//! GraphQL on GitHub (merge, auto-merge, the merge queue and their undoing) and on GitLab (merge,
//! merge when the pipeline succeeds). What `log` and `off` open is only that: another repo, a
//! branch merge, a release, a tag, a ruleset, administration and a request with no sentinel stay
//! refused, an administrator's lock keeps the family held, and a policy that cannot be read is
//! `enforce`.

use std::path::Path;
use std::sync::{Arc, Mutex};

use purlis_core::enforcement::{self, Family, Mode};
use purlis_core::forge::mergestance::PolicyMergeStance;
use purlis_core::forge::proxy::{ChatId, HeldSink, Outcome, Refusal};
use purlis_core::heldact::Requested;
use purlis_core::sandbox::policy::Locks;

use crate::support::forge_proxy::{Auth, Objects, Scene, graphql, is_refused};

struct Asks;

impl HeldSink for Asks {
    fn ask(&self, _: &ChatId, _: &Requested) -> String {
        "asked".to_owned()
    }
}

type Lines = Arc<Mutex<Vec<(String, String)>>>;

/// A scene whose `guards.merge` is whatever `mode` says on each request, with the log lines it
/// records.
fn scene(mode: impl Fn() -> Mode + Send + Sync + 'static) -> (Scene, Lines) {
    let lines: Lines = Arc::default();
    let kept = lines.clone();
    let stance = PolicyMergeStance::reading(mode, move |chat, said| {
        kept.lock()
            .unwrap()
            .push((chat.to_owned(), said.to_owned()));
    });
    let mut objects = Objects::default();
    objects.0.insert("PR_kwDO_1".into(), "acme/app".into());
    objects.0.insert("PR_other".into(), "acme/other".into());
    let s = Scene::tuned(objects, |proxy| {
        proxy
            .with_held(Box::new(Asks))
            .with_merge_stance(Arc::new(stance))
    });
    (s, lines)
}

fn in_mode(mode: Mode) -> (Scene, Lines) {
    scene(move || mode)
}

struct Shape {
    what: &'static str,
    method: &'static str,
    host: &'static str,
    target: &'static str,
    body: Vec<u8>,
}

fn rest(
    what: &'static str,
    method: &'static str,
    host: &'static str,
    target: &'static str,
    body: &str,
) -> Shape {
    Shape {
        what,
        method,
        host,
        target,
        body: body.as_bytes().to_vec(),
    }
}

fn mutation(what: &'static str, host: &'static str, target: &'static str, q: &str) -> Shape {
    Shape {
        what,
        method: "POST",
        host,
        target,
        body: graphql(q, serde_json::json!({})),
    }
}

/// Every shape a chat merges by that `log` and `off` let through.
fn shapes() -> Vec<Shape> {
    let gh = |what, name: &str| {
        mutation(
            what,
            "api.github.com",
            "/graphql",
            &format!(
                "mutation{{ {name}(input:{{pullRequestId:\"PR_kwDO_1\"}}){{ clientMutationId }} }}"
            ),
        )
    };
    vec![
        rest(
            "github rest merge",
            "PUT",
            "api.github.com",
            "/repos/acme/app/pulls/7/merge",
            r#"{"sha":"abc","merge_method":"squash"}"#,
        ),
        gh("github mergePullRequest", "mergePullRequest"),
        gh(
            "github enablePullRequestAutoMerge",
            "enablePullRequestAutoMerge",
        ),
        gh(
            "github disablePullRequestAutoMerge",
            "disablePullRequestAutoMerge",
        ),
        gh("github enqueuePullRequest", "enqueuePullRequest"),
        gh("github dequeuePullRequest", "dequeuePullRequest"),
        rest(
            "gitlab rest merge",
            "PUT",
            "gitlab.com",
            "/api/v4/projects/group%2Fproj/merge_requests/3/merge",
            r#"{"sha":"abc"}"#,
        ),
        rest(
            "gitlab merge when the pipeline succeeds",
            "PUT",
            "gitlab.com",
            "/api/v4/projects/42/merge_requests/3/merge",
            r#"{"sha":"abc","merge_when_pipeline_succeeds":true}"#,
        ),
        rest(
            "gitlab merge when the pipeline succeeds, as a query",
            "PUT",
            "gitlab.com",
            "/api/v4/projects/42/merge_requests/3/merge?merge_when_pipeline_succeeds=true",
            r#"{"sha":"abc"}"#,
        ),
        rest(
            "gitlab cancel of merge when the pipeline succeeds",
            "POST",
            "gitlab.com",
            "/api/v4/projects/42/merge_requests/3/cancel_merge_when_pipeline_succeeds",
            "{}",
        ),
        mutation(
            "gitlab mergeRequestAccept",
            "gitlab.com",
            "/api/graphql",
            "mutation{ mergeRequestAccept(input:{projectPath:\"group/proj\", iid:\"3\", sha:\"abc\"}){ errors } }",
        ),
    ]
}

/// Not sent to the forge: refused for `why`, or handed over as the held act that refusal is (a
/// sentence for the chat, and nothing forwarded).
fn stopped(out: &Outcome, why: Refusal) -> bool {
    match out {
        Outcome::Held(held) => held.refusal == why,
        other => is_refused(other, why),
    }
}

fn send(s: &Scene, shape: &Shape) -> Outcome {
    s.send(shape.method, shape.host, shape.target, &shape.body)
}

#[test]
fn every_shape_is_held_under_enforce() {
    purlis_core::unsteered!();
    let (s, lines) = in_mode(Mode::Enforce);
    for shape in shapes() {
        let out = send(&s, &shape);
        assert!(stopped(&out, Refusal::Merge), "{}: {out:?}", shape.what);
    }
    assert!(lines.lock().unwrap().is_empty());
}

#[test]
fn a_rest_merge_that_pins_its_head_is_asked_for_under_enforce() {
    purlis_core::unsteered!();
    let (s, _) = in_mode(Mode::Enforce);
    let out = s.send(
        "PUT",
        "api.github.com",
        "/repos/acme/app/pulls/7/merge",
        br#"{"sha":"abc"}"#,
    );
    assert!(matches!(out, Outcome::Held(_)), "{out:?}");
}

#[test]
fn every_shape_is_forwarded_and_logged_once_under_log() {
    purlis_core::unsteered!();
    let (s, lines) = in_mode(Mode::Log);
    let all = shapes();
    for shape in &all {
        let out = send(&s, shape);
        assert!(
            matches!(out, Outcome::Forward(_)),
            "{}: {out:?}",
            shape.what
        );
    }
    let lines = lines.lock().unwrap();
    assert_eq!(lines.len(), all.len(), "one line for each merge");
    for (chat, said) in lines.iter() {
        assert_eq!(chat, "chat-a");
        assert!(
            said == "github.com/acme/app" || said == "gitlab.com/group/proj",
            "the line names the repo and nothing the request said: {said}"
        );
    }
}

#[test]
fn every_shape_is_forwarded_and_not_logged_under_off() {
    purlis_core::unsteered!();
    let (s, lines) = in_mode(Mode::Off);
    for shape in shapes() {
        let out = send(&s, &shape);
        assert!(
            matches!(out, Outcome::Forward(_)),
            "{}: {out:?}",
            shape.what
        );
    }
    assert!(lines.lock().unwrap().is_empty());
}

#[test]
fn the_mode_is_read_on_every_request() {
    purlis_core::unsteered!();
    let now = Arc::new(Mutex::new(Mode::Enforce));
    let read = now.clone();
    let (s, lines) = scene(move || *read.lock().unwrap());
    let merge = || {
        s.send(
            "PUT",
            "api.github.com",
            "/repos/acme/app/pulls/7/merge",
            br#"{"sha":"abc"}"#,
        )
    };
    assert!(matches!(merge(), Outcome::Held(_)));
    *now.lock().unwrap() = Mode::Log;
    assert!(matches!(merge(), Outcome::Forward(_)));
    *now.lock().unwrap() = Mode::Enforce;
    assert!(matches!(merge(), Outcome::Held(_)));
    assert_eq!(lines.lock().unwrap().len(), 1);
}

#[test]
fn a_repo_that_is_not_the_chats_is_still_refused_under_log_and_off() {
    purlis_core::unsteered!();
    for mode in [Mode::Log, Mode::Off] {
        let (s, lines) = in_mode(mode);
        let out = s.send(
            "PUT",
            "api.github.com",
            "/repos/acme/other/pulls/1/merge",
            br#"{"sha":"abc"}"#,
        );
        assert!(is_refused(&out, Refusal::OtherRepo), "{mode:?}: {out:?}");
        let q =
            "mutation{ mergePullRequest(input:{pullRequestId:\"PR_other\"}){ clientMutationId } }";
        let out = s.send(
            "POST",
            "api.github.com",
            "/graphql",
            &graphql(q, serde_json::json!({})),
        );
        assert!(!matches!(out, Outcome::Forward(_)), "{mode:?}: {out:?}");
        // An object the proxy cannot tie to any repo is another repo's.
        let q = "mutation{ mergePullRequest(input:{pullRequestId:\"PR_unknown\"}){ clientMutationId } }";
        let out = s.send(
            "POST",
            "api.github.com",
            "/graphql",
            &graphql(q, serde_json::json!({})),
        );
        assert!(!matches!(out, Outcome::Forward(_)), "{mode:?}: {out:?}");
        assert!(lines.lock().unwrap().is_empty(), "{mode:?}");
    }
}

#[test]
fn a_request_without_the_chats_sentinel_is_refused_under_log_and_off() {
    purlis_core::unsteered!();
    for mode in [Mode::Log, Mode::Off] {
        let (s, lines) = in_mode(mode);
        for auth in [Auth::None, Auth::Bearer("not-the-sentinel")] {
            let out = s.send_as(
                &s.a,
                "PUT",
                "api.github.com",
                443,
                "/repos/acme/app/pulls/7/merge",
                auth,
                br#"{"sha":"abc"}"#,
            );
            assert!(matches!(out, Outcome::Refused(_)), "{mode:?}: {out:?}");
        }
        assert!(lines.lock().unwrap().is_empty(), "{mode:?}");
    }
}

#[test]
fn only_the_merge_routes_open_and_everything_else_stays_refused() {
    purlis_core::unsteered!();
    for mode in [Mode::Log, Mode::Off] {
        let (s, lines) = in_mode(mode);
        for (what, method, host, target, body, why) in [
            (
                "a branch merge",
                "POST",
                "api.github.com",
                "/repos/acme/app/merges",
                "{}",
                Refusal::Merge,
            ),
            (
                "a query on the merge that is more than its one flag",
                "PUT",
                "gitlab.com",
                "/api/v4/projects/42/merge_requests/3/merge?merge_when_pipeline_succeeds=true&squash=true",
                "{}",
                Refusal::Merge,
            ),
            (
                "the merge flag with a value that is no boolean",
                "PUT",
                "gitlab.com",
                "/api/v4/projects/42/merge_requests/3/merge?merge_when_pipeline_succeeds=yes",
                "{}",
                Refusal::Merge,
            ),
            (
                "the merge flag in a query on an update, which is no merge route",
                "PUT",
                "gitlab.com",
                "/api/v4/projects/42/merge_requests/3?merge_when_pipeline_succeeds=true",
                "{}",
                Refusal::Merge,
            ),
            (
                "auto-merge by a query",
                "PUT",
                "api.github.com",
                "/repos/acme/app/pulls/7?auto_merge=true",
                "{}",
                Refusal::Merge,
            ),
            (
                "a release",
                "POST",
                "api.github.com",
                "/repos/acme/app/releases",
                "{}",
                Refusal::ReleaseOrTag,
            ),
            (
                "a tag",
                "POST",
                "api.github.com",
                "/repos/acme/app/git/tags",
                "{}",
                Refusal::ReleaseOrTag,
            ),
            (
                "branch protection",
                "PUT",
                "api.github.com",
                "/repos/acme/app/branches/main/protection",
                "{}",
                Refusal::Ruleset,
            ),
            (
                "a ruleset",
                "POST",
                "api.github.com",
                "/repos/acme/app/rulesets",
                "{}",
                Refusal::Ruleset,
            ),
            (
                "a hook",
                "POST",
                "api.github.com",
                "/repos/acme/app/hooks",
                "{}",
                Refusal::Admin,
            ),
            (
                "an approval",
                "POST",
                "api.github.com",
                "/repos/acme/app/pulls/7/reviews",
                "{}",
                Refusal::Approval,
            ),
            (
                "a gitlab approval",
                "POST",
                "gitlab.com",
                "/api/v4/projects/42/merge_requests/3/approve",
                "{}",
                Refusal::Approval,
            ),
            (
                "a merge command in a merge's message",
                "PUT",
                "api.github.com",
                "/repos/acme/app/pulls/7/merge",
                r#"{"sha":"abc","commit_message":"bors r+"}"#,
                // Not let through as a merge `log` allows: it falls back to the hold it was.
                Refusal::Merge,
            ),
        ] {
            let out = s.send(method, host, target, body.as_bytes());
            assert!(stopped(&out, why), "{mode:?}, {what}: {out:?}");
        }
        for q in [
            "mutation{ mergeBranch(input:{repositoryId:\"R_acme_app\", base:\"main\", head:\"x\"}){ clientMutationId } }",
            "mutation{ addPullRequestReview(input:{pullRequestId:\"PR_kwDO_1\", event:APPROVE}){ clientMutationId } }",
            "mutation{ createRelease(input:{repositoryId:\"R_acme_app\", tagName:\"v1\"}){ clientMutationId } }",
        ] {
            let out = s.send(
                "POST",
                "api.github.com",
                "/graphql",
                &graphql(q, serde_json::json!({})),
            );
            assert!(
                matches!(out, Outcome::Refused(_) | Outcome::Held(_)),
                "{mode:?}, {q}: {out:?}"
            );
        }
        assert!(lines.lock().unwrap().is_empty(), "{mode:?}");
    }
}

#[test]
fn a_merge_request_aimed_at_another_project_is_still_refused() {
    purlis_core::unsteered!();
    let (s, _) = in_mode(Mode::Log);
    let out = s.send(
        "PUT",
        "gitlab.com",
        "/api/v4/projects/42/merge_requests/3/merge",
        br#"{"sha":"abc","target_project_id":7}"#,
    );
    assert!(!matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn a_policy_that_cannot_be_read_holds_every_merge() {
    purlis_core::unsteered!();
    // Outside the app, a project's file is never read, and nobody answered this process: what
    // cannot be read is `enforce`.
    let stance = PolicyMergeStance::of(Path::new("/nonexistent/plane"));
    let s = Scene::tuned(Objects::default(), |proxy| {
        proxy
            .with_held(Box::new(Asks))
            .with_merge_stance(Arc::new(stance))
    });
    let out = s.send(
        "PUT",
        "api.github.com",
        "/repos/acme/app/pulls/7/merge",
        br#"{"sha":"abc"}"#,
    );
    assert!(matches!(out, Outcome::Held(_)), "{out:?}");
}

#[test]
fn an_administrators_lock_keeps_a_merge_held_whatever_the_project_says() {
    purlis_core::unsteered!();
    let settings = enforcement::Settings::from_text(Some("[policy]\nguards.merge = \"log\"\n"));
    let locked = Locks::parse(
        r#"{"owner": "it-security", "enforcement": ["guards.merge"]}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    let (s, lines) = scene(move || enforcement::under(&settings, &locked, Family::GuardsMerge));
    let out = s.send(
        "PUT",
        "api.github.com",
        "/repos/acme/app/pulls/7/merge",
        br#"{"sha":"abc"}"#,
    );
    assert!(matches!(out, Outcome::Held(_)), "{out:?}");
    assert!(lines.lock().unwrap().is_empty());
}

#[test]
fn a_refused_policy_file_holds_every_merge() {
    purlis_core::unsteered!();
    let settings = enforcement::Settings::from_text(Some("[policy]\nguards.merge = \"log\"\n"));
    // A file purlis refuses locks every family, as `Locks` says.
    let refused = Locks::parse("not json", Path::new("/etc/purlis/policy.json"));
    let (s, _) = scene(move || enforcement::under(&settings, &refused, Family::GuardsMerge));
    let out = s.send(
        "PUT",
        "api.github.com",
        "/repos/acme/app/pulls/7/merge",
        br#"{"sha":"abc"}"#,
    );
    assert!(matches!(out, Outcome::Held(_)), "{out:?}");
}

#[test]
fn an_administrators_pin_still_wins_over_a_merge_let_through() {
    purlis_core::unsteered!();
    use purlis_core::forge::pin::PolicyPin;
    let pinned = Locks::parse(
        r#"{"owner": "it-security", "sandbox": {"never-hosts": ["github.com"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    let pin = PolicyPin::reading(move || pinned.clone());
    let mut objects = Objects::default();
    objects.0.insert("PR_kwDO_1".into(), "acme/app".into());
    let s = Scene::tuned(objects, |proxy| {
        proxy
            .with_pin(Arc::new(pin))
            .with_merge_stance(Arc::new(PolicyMergeStance::reading(
                || Mode::Off,
                |_, _| {},
            )))
    });
    let out = s.send(
        "PUT",
        "api.github.com",
        "/repos/acme/app/pulls/7/merge",
        br#"{"sha":"abc"}"#,
    );
    assert!(is_refused(&out, Refusal::Pinned), "{out:?}");
}
