// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's GraphQL merge or enqueue is held for the person, with an ask that names it**
//! (#7543). Under `guards.merge` = `enforce` the route refuses a `mergePullRequest`,
//! `enablePullRequestAutoMerge` or `enqueuePullRequest`; it used to do so with no ask, a dead end.
//! Now it hands the pull request, named by number, to the [`HeldSink`], which raises one ask the
//! person's Merge press answers, and the chat is told its number. **Nothing is forwarded**: the
//! merge happens only on the press. Where the pull request cannot be named, or the document is
//! anything else, nobody is asked and the chat is refused as it was.

use std::sync::{Arc, Mutex};

use purlis_core::forge::proxy::{ChatId, HeldSink, Kind, ObjectRepo, Outcome, Refusal};
use purlis_core::heldact::Requested;
use purlis_core::mergehold::Repo;

use crate::support::forge_proxy::{Auth, Scene, graphql};

/// The asks the sink was handed, and what it tells the chat.
#[derive(Default)]
struct Asked(Mutex<Vec<Requested>>);

struct Sink(Arc<Asked>);

impl HeldSink for Sink {
    fn ask(&self, _: &ChatId, requested: &Requested) -> String {
        let mut asked = self.0.0.lock().unwrap();
        asked.push(requested.clone());
        format!(
            "purlis holds merges. It has asked the person; the merge happens when they approve \
             it in Asks. Ask {}: merge in acme/app.",
            asked.len() + 40
        )
    }
}

/// Which repo, and which pull request number, a node id is.
struct Pulls;

impl ObjectRepo for Pulls {
    fn repo_of(&self, _: Kind, _: &str, id: &str) -> Option<String> {
        match id {
            "PR_kwDO_1" | "PR_nonumber" => Some("acme/app".to_owned()),
            "PR_other" => Some("acme/other".to_owned()),
            _ => None,
        }
    }

    fn pull_of(&self, kind: Kind, host: &str, id: &str) -> Option<(String, u64)> {
        let path = self.repo_of(kind, host, id)?;
        (id == "PR_kwDO_1").then_some((path, 7))
    }
}

/// An enforcing scene (the default stance holds every merge) and what its sink was asked.
fn scene() -> (Scene, Arc<Asked>) {
    let asked = Arc::new(Asked::default());
    let sink = Arc::clone(&asked);
    let s = Scene::tuned(Pulls, move |proxy| proxy.with_held(Box::new(Sink(sink))));
    (s, asked)
}

fn mutate(s: &Scene, field: &str, id: &str) -> Outcome {
    let q =
        format!("mutation{{ {field}(input:{{pullRequestId:\"{id}\"}}){{ clientMutationId }} }}");
    s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(&q, serde_json::json!({})),
    )
}

fn seven() -> Requested {
    Requested::merge_of(Repo::try_from("acme/app".to_owned()).unwrap(), 7)
}

#[test]
fn a_merge_raises_one_ask_and_names_it() {
    purlis_core::unsteered!();
    let (s, asked) = scene();
    let out = mutate(&s, "mergePullRequest", "PR_kwDO_1");
    let Outcome::Held(held) = &out else {
        panic!("not held, so refused or forwarded: {out:?}");
    };
    assert_eq!(held.refusal, Refusal::Merge);
    assert!(held.said.contains("Ask 41"), "{}", held.said);
    assert_eq!(*asked.0.lock().unwrap(), [seven()], "one ask");
}

/// Auto-merge and the merge queue are not held: the person's press merges at once, so holding
/// them would turn "merge when green" or "through the queue" into "merge now". They are refused
/// with a reason, and nobody is asked.
#[test]
fn auto_merge_and_the_merge_queue_are_refused_with_a_reason_and_ask_nobody() {
    purlis_core::unsteered!();
    for field in ["enablePullRequestAutoMerge", "enqueuePullRequest"] {
        let (s, asked) = scene();
        let out = mutate(&s, field, "PR_kwDO_1");
        let Outcome::Held(held) = &out else {
            panic!("{field}: not answered with a reason: {out:?}");
        };
        assert_eq!(held.refusal, Refusal::Merge, "{field}");
        assert!(
            held.said.contains("Nobody was asked"),
            "{field}: {}",
            held.said
        );
        assert!(
            held.said.contains("merges at once"),
            "{field}: {}",
            held.said
        );
        assert!(asked.0.lock().unwrap().is_empty(), "{field}: no ask");
    }
}

#[test]
fn a_held_merge_is_never_forwarded() {
    purlis_core::unsteered!();
    let (s, _) = scene();
    for field in [
        "mergePullRequest",
        "enablePullRequestAutoMerge",
        "enqueuePullRequest",
    ] {
        let out = mutate(&s, field, "PR_kwDO_1");
        assert!(!matches!(out, Outcome::Forward(_)), "{field}: {out:?}");
    }
}

#[test]
fn a_pull_request_the_forge_does_not_number_is_refused_with_its_reason_and_asks_nobody() {
    purlis_core::unsteered!();
    let (s, asked) = scene();
    let out = mutate(&s, "mergePullRequest", "PR_nonumber");
    let Outcome::Held(held) = &out else {
        panic!("not answered with a reason: {out:?}");
    };
    assert_eq!(held.refusal, Refusal::Merge);
    assert!(held.said.contains("Nobody was asked"), "{}", held.said);
    assert!(asked.0.lock().unwrap().is_empty());
}

#[test]
fn another_repos_pull_request_an_unknown_one_and_an_undoing_ask_nobody() {
    purlis_core::unsteered!();
    let (s, asked) = scene();
    for (field, id) in [
        ("mergePullRequest", "PR_other"),
        ("mergePullRequest", "PR_unknown"),
        ("dequeuePullRequest", "PR_kwDO_1"),
        ("disablePullRequestAutoMerge", "PR_kwDO_1"),
    ] {
        let out = mutate(&s, field, id);
        assert!(
            !matches!(out, Outcome::Held(_) | Outcome::Forward(_)),
            "{field} {id}: {out:?}"
        );
    }
    assert!(asked.0.lock().unwrap().is_empty());
}

#[test]
fn a_request_without_the_chats_sentinel_asks_nobody() {
    purlis_core::unsteered!();
    let (s, asked) = scene();
    let q = "mutation{ mergePullRequest(input:{pullRequestId:\"PR_kwDO_1\"}){ clientMutationId } }";
    let out = s.send_as(
        &s.a,
        "POST",
        "api.github.com",
        443,
        "/graphql",
        Auth::Bearer("not-the-sentinel"),
        &graphql(q, serde_json::json!({})),
    );
    assert!(matches!(out, Outcome::Refused(_)), "{out:?}");
    assert!(asked.0.lock().unwrap().is_empty());
}
