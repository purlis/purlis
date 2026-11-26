// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A merge that asks the forge to merge later is refused, never held** (#7597): the person's
//! press merges at once, and auto-merge would merge any head pushed later, unseen. The hook
//! refuses it with the reason the GraphQL route gives, in every spelling of the flag, and the
//! REST route refuses a merge whose body sets `merge_when_pipeline_succeeds` or `auto_merge`.
//! The same merge without it is still held as a Merge ask.

use purlis_core::forge::Kind;
use purlis_core::heldact::{Call, Recognised, Requested, recognise, said};
use purlis_core::mergehold::{self, AppSaid, Held, LandAs, Request};
use serde_json::{Value, json};

use crate::support::held_acts::{Rec, repo};

const REASON: &str = "auto-merge would merge any head pushed later, unseen";

/// What the hook tells the chat for `cmd`, where it is refused with no ask.
fn refused(cmd: &str) -> String {
    let held = mergehold::held(cmd).unwrap_or_else(|| panic!("{cmd}: not held at all"));
    assert!(
        !matches!(held, Held::Request(_)),
        "{cmd}: an ask would be raised for it"
    );
    mergehold::refusal_for(cmd, &held, &AppSaid::Unheard)
}

#[test]
fn every_spelling_of_an_auto_merge_flag_is_refused_and_nobody_is_asked() {
    purlis_core::unsteered!();
    for cmd in [
        "gh pr merge --auto 12",
        "gh pr merge 12 --auto",
        "gh pr merge 12 --auto --squash",
        "gh pr merge --auto=true 12",
        "gh pr merge --auto=1 12",
        "gh pr merge -R o/r --auto 12",
        "gh pr merge 12 --repo=o/r --auto",
        "glab mr merge 12 --auto-merge",
        "glab mr merge --auto-merge=true 12",
        "glab mr merge 12 --when-pipeline-succeeds",
        "glab mr accept 12 --auto-merge",
        "FOO=1 gh pr merge 12 --auto",
    ] {
        let said = refused(cmd);
        assert!(said.contains(REASON), "{cmd}: {said}");
        assert!(said.contains("Nobody was asked"), "{cmd}: {said}");
    }
}

#[test]
fn a_flags_value_that_reads_auto_is_no_auto_merge() {
    purlis_core::unsteered!();
    assert_eq!(
        mergehold::held("gh pr merge --body --auto 12"),
        Some(Held::Request(Request {
            repo: None,
            number: 12,
            act: LandAs::Merge
        }))
    );
}

#[test]
fn a_merge_without_an_auto_flag_is_still_held_as_a_merge_ask() {
    purlis_core::unsteered!();
    for (cmd, number) in [
        ("gh pr merge 12", 12),
        ("glab mr merge 12", 12),
        ("gh pr merge 12 --squash --delete-branch", 12),
        ("gh pr merge 12 --auto=false", 12),
        ("gh pr merge 12 --disable-auto", 12),
        ("glab mr merge 12 --auto-merge=false", 12),
    ] {
        assert_eq!(
            mergehold::held(cmd),
            Some(Held::Request(Request {
                repo: None,
                number,
                act: LandAs::Merge
            })),
            "{cmd}"
        );
    }
}

fn run(path: &str, query: &[(&str, &str)], body: Value) -> Recognised {
    run_as("PUT", path, query, body)
}

fn run_as(method: &str, path: &str, query: &[(&str, &str)], body: Value) -> Recognised {
    let segments: Vec<String> = path
        .trim_start_matches('/')
        .split('/')
        .map(str::to_owned)
        .collect();
    let query: Vec<(String, String)> = query
        .iter()
        .map(|(k, v)| ((*k).into(), (*v).into()))
        .collect();
    let call = Call {
        method,
        segments: &segments,
        query: &query,
        body: &body,
    };
    recognise(Kind::GitLab, &call, &Rec::acme())
}

const GITLAB_MERGE: &str = "/api/v4/projects/42/merge_requests/12/merge";

#[test]
fn a_rest_merge_that_sets_auto_merge_is_refused_with_a_reason() {
    purlis_core::unsteered!();
    for (body, query) in [
        (json!({"merge_when_pipeline_succeeds": true}), vec![]),
        (json!({"auto_merge": true}), vec![]),
        (
            json!({"sha": "abc", "squash": true, "auto_merge": true}),
            vec![],
        ),
        (json!({"merge_when_pipeline_succeeds": "true"}), vec![]),
        (json!({"merge_request": {"auto_merge": true}}), vec![]),
        (
            json!({"a": [{"merge_when_pipeline_succeeds": true}]}),
            vec![],
        ),
        (json!({}), vec![("merge_when_pipeline_succeeds", "true")]),
    ] {
        match run(GITLAB_MERGE, &query, body.clone()) {
            Recognised::Refuse(why) => {
                let said = said::refused_request(&why);
                assert!(said.contains(REASON), "{body} {query:?}: {said}");
            }
            other => panic!("{body} {query:?}: {other:?}"),
        }
    }
}

#[test]
fn a_rest_merge_with_auto_merge_false_or_absent_is_held() {
    purlis_core::unsteered!();
    for body in [
        json!({}),
        json!({"sha": "abc"}),
        json!({"squash": true}),
        json!({"merge_when_pipeline_succeeds": false}),
        json!({"auto_merge": false, "merge_when_pipeline_succeeds": "false"}),
    ] {
        assert_eq!(
            run(GITLAB_MERGE, &[], body.clone()),
            Recognised::Hold(Requested::Merge(Request {
                repo: Some(repo("acme/app")),
                number: 12,
                act: LandAs::Merge
            })),
            "{body}"
        );
    }
}

#[test]
fn gh_dash_m_is_the_boolean_merge_so_the_auto_flag_after_it_is_refused() {
    purlis_core::unsteered!();
    for cmd in [
        "gh pr merge 12 -m --auto",
        "gh pr merge -m --auto 12",
        "gh pr merge 12 --merge --auto",
        "gh pr merge 12 -s --auto",
    ] {
        let said = refused(cmd);
        assert!(said.contains(REASON), "{cmd}: {said}");
    }
}

#[test]
fn a_value_flag_of_each_cli_takes_the_next_word_so_an_auto_flag_there_is_a_value() {
    purlis_core::unsteered!();
    let held = |cmd: &str| {
        assert_eq!(
            mergehold::held(cmd),
            Some(Held::Request(Request {
                repo: None,
                number: 12,
                act: LandAs::Merge
            })),
            "{cmd}"
        );
    };
    // gh pr merge: every flag that takes a value.
    for flag in [
        "-A",
        "--author-email",
        "-b",
        "--body",
        "-F",
        "--body-file",
        "--match-head-commit",
        "-t",
        "--subject",
    ] {
        held(&format!("gh pr merge 12 {flag} --auto"));
    }
    // glab mr merge: every flag that takes a value, `-m` being `--message` there.
    for flag in ["-m", "--message", "--sha", "--squash-message"] {
        held(&format!("glab mr merge 12 {flag} --auto-merge"));
    }
}

#[test]
fn a_flag_that_takes_no_value_on_one_cli_does_not_hide_an_auto_flag_after_it() {
    purlis_core::unsteered!();
    // `--message` and `--sha` are glab's, and `--body` and `-t` are gh's: on the other CLI the
    // word after them is not their value.
    for cmd in [
        "gh pr merge 12 --message --auto",
        "gh pr merge 12 --sha --auto",
        "gh pr merge 12 --squash-message --auto",
        "glab mr merge 12 --body --auto-merge",
        "glab mr merge 12 -t --auto-merge",
        "glab mr merge 12 --match-head-commit --auto-merge",
    ] {
        let said = refused(cmd);
        assert!(said.contains(REASON), "{cmd}: {said}");
    }
}

#[test]
fn a_rest_merge_reads_every_truthy_spelling_as_auto_merge() {
    purlis_core::unsteered!();
    for on in [
        json!(1),
        json!("1"),
        json!("t"),
        json!("T"),
        json!("yes"),
        json!("y"),
        json!("on"),
        json!("TRUE"),
        json!("True"),
        json!(2),
        json!("anything"),
    ] {
        for body in [
            json!({"auto_merge": on}),
            json!({"merge_when_pipeline_succeeds": on}),
            json!({"merge_request": {"auto_merge": on}}),
            json!({"a": [{"merge_when_pipeline_succeeds": on}]}),
        ] {
            assert!(
                matches!(run(GITLAB_MERGE, &[], body.clone()), Recognised::Refuse(_)),
                "{body}"
            );
        }
    }
    for text in ["1", "t", "yes", "y", "on", "TRUE"] {
        assert!(
            matches!(
                run(GITLAB_MERGE, &[("auto_merge", text)], json!({})),
                Recognised::Refuse(_)
            ),
            "query {text}"
        );
    }
}

#[test]
fn a_rest_merge_reads_every_falsy_spelling_as_no_auto_merge() {
    purlis_core::unsteered!();
    for off in [
        json!(false),
        json!("false"),
        json!("FALSE"),
        json!(0),
        json!("0"),
        json!("f"),
        json!("F"),
        Value::Null,
    ] {
        let body = json!({"auto_merge": off, "merge_when_pipeline_succeeds": off});
        assert!(
            matches!(run(GITLAB_MERGE, &[], body.clone()), Recognised::Hold(_)),
            "{body}"
        );
    }
    for text in ["0", "f", "false"] {
        assert!(
            matches!(
                run(GITLAB_MERGE, &[("auto_merge", text)], json!({})),
                Recognised::Hold(_)
            ),
            "query {text}"
        );
    }
}

#[test]
fn a_merge_train_entry_that_sets_when_pipeline_succeeds_is_refused() {
    purlis_core::unsteered!();
    let train = "/api/v4/projects/42/merge_trains/merge_requests/12";
    for body in [
        json!({"when_pipeline_succeeds": true}),
        json!({"when_pipeline_succeeds": "yes"}),
        json!({"when_pipeline_succeeds": 1}),
    ] {
        assert!(
            matches!(run(train, &[], body.clone()), Recognised::Refuse(_)),
            "{body}"
        );
    }
    assert!(matches!(
        run(train, &[("when_pipeline_succeeds", "true")], json!({})),
        Recognised::Refuse(_)
    ));
    for body in [json!({}), json!({"when_pipeline_succeeds": false})] {
        assert!(
            matches!(run(train, &[], body.clone()), Recognised::Hold(_)),
            "{body}"
        );
    }
}

#[test]
fn a_delete_that_cancels_auto_merge_is_never_read_as_enabling_it() {
    purlis_core::unsteered!();
    for (query, body) in [
        (vec![("auto_merge", "1")], json!({})),
        (vec![], json!({"auto_merge": true})),
        (vec![("merge_when_pipeline_succeeds", "true")], json!({})),
    ] {
        let said = match run_as("DELETE", GITLAB_MERGE, &query, body.clone()) {
            Recognised::Refuse(why) => said::refused_request(&why),
            _ => continue,
        };
        assert!(!said.contains(REASON), "{query:?} {body}: {said}");
    }
}
