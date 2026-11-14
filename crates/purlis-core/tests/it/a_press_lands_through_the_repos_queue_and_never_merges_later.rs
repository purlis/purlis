// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The person's press, through a forge backend** (Q239, S1 and S2 of T1-11's security
//! review): a repo that lands through a merge queue or train has the request put in it, as
//! `purlis change land` does, never merged past it; any other is merged at the head the ask was
//! raised at; and a forge that sets the request to merge later instead is an error the person
//! sees, with the ask still waiting, never a merge. The backends are purlis's own GitHub and
//! GitLab clients over a recorded transport (`purlis_core::forge::recorded`), whose exchanges
//! are the forge contract's recordings (`tests/forge_contract/`).

use std::sync::Arc;

use purlis_core::forge::backend::Caller;
use purlis_core::forge::recorded::Recorded;
use purlis_core::forge::{Forge, Kind};
use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::mergehold::{ByForge, Holds, MergeKey, Merged, NotMerged, Reread, Seen};
use serde_json::{Value, json};

const HEAD: &str = "6dcb09b5b57875f334f61aebed695e2e4193db5e";

fn key() -> MergeKey {
    MergeKey {
        repo: "acme/api".try_into().unwrap(),
        number: 12,
        head: HEAD.try_into().unwrap(),
        base: "main".try_into().unwrap(),
    }
}

/// The exchanges of the forge contract's recording `kind/case`.
fn recorded(kind: &str, case: &str) -> Vec<Value> {
    let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/forge_contract")
        .join(kind)
        .join(format!("{case}.json"));
    let text = std::fs::read_to_string(&at).unwrap();
    serde_json::from_str::<Value>(&text).unwrap()["exchanges"]
        .as_array()
        .unwrap()
        .clone()
}

fn rest(method: Option<&str>, path: &str, fields: Value, out: Value) -> Value {
    json!({"call": {"endpoint": {"rest": {"method": method, "path": path}}, "fields": fields},
           "reply": {"code": 0, "out": out.to_string(), "err": ""}})
}

/// A press of `key()` through `kind`'s backend over `exchanges`; and what was not asked.
fn pressed(kind: Kind, exchanges: Vec<Value>) -> (Result<Merged, NotMerged>, Holds, Vec<String>) {
    let text = json!({"source": "the forge contract's recordings", "exchanges": exchanges});
    let transport = Arc::new(Recorded::parse(&text.to_string()).unwrap());
    let backend = Forge::default_of(kind).backend_over(transport.clone());
    let holds = Holds::default();
    holds.raise(4, key(), facts(), 1_000);
    holds
        .refresh(4, &key(), key(), facts(), Reread::AsksOnceMore)
        .expect("asked once more");
    let forge = ByForge {
        backend: backend.as_ref(),
        caller: Caller::window(),
        path: "acme/api".to_owned(),
    };
    let window = Answerer::admitted(Admitted::LocalUi).unwrap();
    let merged = holds
        .press(4, &key(), Seen::None, window, &forge)
        .map(|(_, merged)| merged);
    let unspent = transport
        .unspent()
        .iter()
        .map(|call| serde_json::to_string(call).unwrap())
        .collect();
    (merged, holds, unspent)
}

fn github_pull() -> Value {
    rest(
        None,
        "repos/acme/api/pulls/12",
        json!([]),
        json!({"number": 12, "title": "Add the parser", "user": {"login": "octo"},
               "head": {"sha": HEAD},
               "base": {"ref": "main", "repo": {"allow_merge_commit": true}}}),
    )
}

#[test]
fn a_press_on_a_repo_with_a_merge_queue_puts_the_request_in_the_queue() {
    purlis_core::unsteered!();
    let mut exchanges = vec![github_pull()];
    exchanges.extend(recorded("github", "lands_through_queue"));
    exchanges.extend(recorded("github", "enqueue_at"));
    let (merged, holds, unspent) = pressed(Kind::GitHub, exchanges);
    assert_eq!(merged, Ok(Merged::Queued));
    assert!(holds.every().is_empty());
    assert_eq!(unspent, Vec::<String>::new(), "no merge past the queue");
}

#[test]
fn a_press_on_a_repo_with_no_queue_merges_at_the_asks_head() {
    purlis_core::unsteered!();
    let mut probe = recorded("github", "lands_through_queue");
    probe[0]["reply"]["out"] = json!(
        json!({"data": {"repository": {"pullRequest": {"id": "PR_kwDOAbc",
               "isMergeQueueEnabled": false}}}})
        .to_string()
    );
    let mut exchanges = vec![github_pull()];
    exchanges.extend(probe);
    exchanges.push(rest(
        Some("PUT"),
        "repos/acme/api/pulls/12/merge",
        json!([{"text": ["merge_method", "merge"]},
               {"text": ["commit_title", "Merge request 12 of acme/api"]},
               {"text": ["commit_message", "Merged in purlis on the person's press."]},
               {"text": ["sha", HEAD]}]),
        json!({"merged": true, "message": "Pull Request successfully merged"}),
    ));
    let (merged, holds, unspent) = pressed(Kind::GitHub, exchanges);
    assert_eq!(merged, Ok(Merged::Now));
    assert!(holds.every().is_empty());
    assert_eq!(unspent, Vec::<String>::new());
}

#[test]
fn a_forge_that_sets_the_request_to_merge_later_is_an_error_and_the_ask_waits() {
    purlis_core::unsteered!();
    let mut probe = recorded("gitlab", "lands_through_queue");
    probe[0]["reply"]["out"] = json!(json!({"id": 3, "merge_trains_enabled": false}).to_string());
    let mut exchanges = vec![rest(
        None,
        "projects/acme%2Fapi/merge_requests/12",
        json!([]),
        json!({"iid": 12, "sha": HEAD, "target_branch": "main", "squash_on_merge": false,
               "title": "Add the parser", "author": {"username": "octo"}}),
    )];
    exchanges.extend(probe);
    exchanges.push(rest(
        Some("PUT"),
        "projects/acme%2Fapi/merge_requests/12/merge",
        json!([{"typed": ["squash", "false"]},
               {"text": ["sha", HEAD]},
               {"text": ["merge_commit_message",
                         "Merge request 12 of acme/api\n\nMerged in purlis on the person's press."]}]),
        json!({"iid": 12, "state": "opened", "merge_when_pipeline_succeeds": true}),
    ));
    exchanges.push(json!({"call": {"endpoint": {"rest": {"method": "POST",
            "path": "projects/acme%2Fapi/merge_requests/12/cancel_merge_when_pipeline_succeeds"}},
            "fields": []},
        "reply": {"code": 1, "out": "", "err": "HTTP 403"}}));
    let (merged, holds, unspent) = pressed(Kind::GitLab, exchanges);
    match merged {
        Err(NotMerged::Forge(why)) => assert!(
            why.contains("set !12 to merge later") && why.contains("Cancel its auto-merge"),
            "{why}"
        ),
        other => panic!("a merge later is never Ok: {other:?}"),
    }
    assert_eq!(holds.every().len(), 1, "the ask waits again");
    assert_eq!(unspent, Vec::<String>::new());
}

/// What the forge said of the request, as a merge ask shows it.
fn facts() -> purlis_core::mergehold::Facts {
    purlis_core::mergehold::Facts {
        title: "Add the parser".to_owned(),
        author: "octo".to_owned(),
        checks: purlis_core::forge::checks::Checks {
            total: Some(1),
            ci: purlis_core::forge::checks::Ci::Passed,
            why: None,
            failed: Vec::new(),
        },
        ruleset: purlis_core::mergehold::Ruleset::None,
        approved: false,
        read_at: 40,
    }
}
