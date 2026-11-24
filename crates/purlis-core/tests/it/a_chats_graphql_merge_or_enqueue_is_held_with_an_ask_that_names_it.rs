// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's GraphQL merge or enqueue is held for the person, with an ask that names it**
//! (#7543). Under `guards.merge` = `enforce` the route refuses a `mergePullRequest` or
//! `enqueuePullRequest`; it used to do so with no ask, a dead end. Now it hands the pull request,
//! named by number, **and the act the document names** (merge or enqueue) to the [`HeldSink`],
//! which raises one ask the person's press answers by performing that act and no other, and the
//! chat is told its number. **Nothing is forwarded**: the act happens only on the press.
//! Auto-merge pins the head only when it is turned on, so it is refused with a reason and nobody
//! is asked; so is a GitLab accept that sets a merge strategy. Where the pull request cannot be
//! named, or the document is anything else, nobody is asked and the chat is refused as it was.

use std::sync::{Arc, Mutex};

use purlis_core::forge::proxy::{ChatId, HeldSink, Kind, ObjectRepo, Outcome, Refusal};
use purlis_core::heldact::Requested;
use purlis_core::mergehold::{LandAs, Repo};

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
    seven_by(LandAs::Merge)
}

fn seven_by(act: LandAs) -> Requested {
    Requested::land_of(Repo::try_from("acme/app".to_owned()).unwrap(), 7, act)
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

/// Each held verb raises one ask that names its own act, so the person's press performs the one
/// the chat asked for: an enqueue is never held as a merge.
#[test]
fn each_verb_raises_one_ask_that_names_its_act() {
    purlis_core::unsteered!();
    for (field, act) in [
        ("mergePullRequest", LandAs::Merge),
        ("enqueuePullRequest", LandAs::Enqueue),
    ] {
        let (s, asked) = scene();
        let out = mutate(&s, field, "PR_kwDO_1");
        let Outcome::Held(held) = &out else {
            panic!("{field}: not held: {out:?}");
        };
        assert_eq!(held.refusal, Refusal::Merge, "{field}");
        assert_eq!(
            *asked.0.lock().unwrap(),
            [seven_by(act)],
            "{field}: one ask"
        );
    }
}

/// The same pull request asked about by a different verb is a different act, never equal to a
/// merge of it.
#[test]
fn an_enqueue_is_not_a_merge() {
    assert_ne!(seven_by(LandAs::Enqueue), seven());
}

/// Auto-merge would merge any head pushed after it was turned on, unseen: it is refused with that
/// reason, nobody is asked and nothing is forwarded.
#[test]
fn auto_merge_is_refused_with_a_reason_and_nobody_is_asked() {
    purlis_core::unsteered!();
    let (s, asked) = scene();
    let out = mutate(&s, "enablePullRequestAutoMerge", "PR_kwDO_1");
    let Outcome::Held(held) = &out else {
        panic!("not refused with a reason: {out:?}");
    };
    assert_eq!(held.refusal, Refusal::Merge);
    assert!(held.said.contains("any head pushed later"), "{}", held.said);
    assert!(held.said.contains("enqueue"), "{}", held.said);
    assert!(held.said.contains("Nobody was asked"), "{}", held.said);
    assert!(asked.0.lock().unwrap().is_empty(), "nobody was asked");
}

fn accept(s: &Scene, input: &str) -> Outcome {
    let q = format!("mutation{{ mergeRequestAccept(input:{{{input}}}){{ errors }} }}");
    s.send(
        "POST",
        "gitlab.com",
        "/api/graphql",
        &graphql(&q, serde_json::json!({})),
    )
}

/// A GitLab accept with no strategy merges now, which a press does: it is held as a merge.
#[test]
fn a_gitlab_accept_with_no_strategy_is_held_as_a_merge() {
    purlis_core::unsteered!();
    let (s, asked) = scene();
    let out = accept(&s, "projectPath:\"group/proj\", iid:\"3\", sha:\"abc\"");
    assert!(matches!(out, Outcome::Held(_)), "{out:?}");
    assert_eq!(
        *asked.0.lock().unwrap(),
        [Requested::land_of(
            Repo::try_from("group/proj".to_owned()).unwrap(),
            3,
            LandAs::Merge
        )]
    );
}

/// `strategy` and `autoMergeStrategy` ask GitLab for a merge when the pipeline succeeds or a
/// train, not a merge now: refused with a reason, and nobody is asked.
#[test]
fn a_gitlab_accept_that_sets_a_strategy_is_refused_with_a_reason_and_nobody_is_asked() {
    purlis_core::unsteered!();
    for set in [
        "strategy:MERGE_WHEN_PIPELINE_SUCCEEDS",
        "autoMergeStrategy:\"merge_train\"",
        "strategy:null",
    ] {
        let (s, asked) = scene();
        let input = format!("projectPath:\"group/proj\", iid:\"3\", {set}");
        let out = accept(&s, &input);
        let Outcome::Held(held) = &out else {
            panic!("{set}: not refused with a reason: {out:?}");
        };
        assert_eq!(held.refusal, Refusal::Merge, "{set}");
        assert!(held.said.contains("merge strategy"), "{set}: {}", held.said);
        assert!(
            held.said.contains("Nobody was asked"),
            "{set}: {}",
            held.said
        );
        assert!(
            asked.0.lock().unwrap().is_empty(),
            "{set}: nobody was asked"
        );
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
