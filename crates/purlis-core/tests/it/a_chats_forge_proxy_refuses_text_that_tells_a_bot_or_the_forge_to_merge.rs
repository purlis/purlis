// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A (instruction 178 item 3, 182 item 7): a comment, a description or a label
//! that tells the forge or a merge bot to merge is the merge by other means, and is refused.

use purlis_core::forge::proxy::{Outcome, Refusal, TokenKind};

use crate::support::forge_proxy::{Objects, Scene, graphql, is_refused};

fn json(v: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&v).unwrap()
}

const COMMANDS: &[&str] = &[
    "/merge",
    "/approve",
    "/lgtm",
    "/submit_review approve",
    "bors r+",
    "bors r=someone",
    "bors merge",
    "@mergifyio queue",
    "@dependabot merge",
    "@kodiak merge",
    "/land",
    "/shipit",
];

#[test]
fn a_comment_or_description_with_a_merge_command_on_a_line_of_its_own_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for cmd in COMMANDS {
        for text in [
            cmd.to_string(),
            format!("Looks good.\n{cmd}\nThanks"),
            format!("  {}", cmd.to_uppercase()),
        ] {
            for (host, target, key) in [
                (
                    "api.github.com",
                    "/repos/acme/app/issues/7/comments",
                    "body",
                ),
                ("api.github.com", "/repos/acme/app/pulls", "body"),
                ("api.github.com", "/repos/acme/app/issues", "body"),
                (
                    "gitlab.com",
                    "/api/v4/projects/42/merge_requests/3/notes",
                    "body",
                ),
                (
                    "gitlab.com",
                    "/api/v4/projects/42/merge_requests",
                    "description",
                ),
            ] {
                let body = json(serde_json::json!({ key: text, "title": "t" }));
                let out = s.send("POST", host, target, &body);
                assert!(
                    is_refused(&out, Refusal::MergeCommand),
                    "{cmd:?} in {text:?} to {target}: {out:?}"
                );
            }
        }
    }
}

#[test]
fn the_same_text_form_encoded_is_refused_too() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let out = s.send(
        "POST",
        "gitlab.com",
        "/api/v4/projects/42/issues/1/notes",
        b"body=ok%0A%2Fmerge",
    );
    assert!(is_refused(&out, Refusal::MergeCommand), "{out:?}");
}

#[test]
fn a_label_that_is_a_merge_command_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let out = s.send(
        "POST",
        "api.github.com",
        "/repos/acme/app/issues/7/labels",
        &json(serde_json::json!({"labels":["bors r+"]})),
    );
    assert!(is_refused(&out, Refusal::MergeCommand), "{out:?}");
}

#[test]
fn ordinary_words_about_merging_are_not_commands() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for text in [
        "Please do not merge this yet.",
        "I will ask them to /merge later",
        "see merge_requests/3 and the bors config",
        "Merge conflicts resolved.",
        "/assign @me",
        "/label ~bug",
    ] {
        let out = s.send(
            "POST",
            "api.github.com",
            "/repos/acme/app/issues/7/comments",
            &json(serde_json::json!({"body": text})),
        );
        assert!(matches!(out, Outcome::Forward(_)), "{text:?}: {out:?}");
    }
}

#[test]
fn a_graphql_comment_with_a_merge_command_is_refused() {
    purlis_core::unsteered!();
    let mut objects = Objects::default();
    objects.0.insert("PR_kwDO_1".into(), "acme/app".into());
    let s = Scene::with(TokenKind::Oauth, objects);
    let q = "mutation($i:AddCommentInput!){ addComment(input:$i){ clientMutationId } }";
    let bad = graphql(
        q,
        serde_json::json!({"i": {"subjectId": "PR_kwDO_1", "body": "ok\n@mergifyio queue"}}),
    );
    assert!(is_refused(
        &s.send("POST", "api.github.com", "/graphql", &bad),
        Refusal::MergeCommand
    ));
    let fine = graphql(
        q,
        serde_json::json!({"i": {"subjectId": "PR_kwDO_1", "body": "ok"}}),
    );
    assert!(matches!(
        s.send("POST", "api.github.com", "/graphql", &fine),
        Outcome::Forward(_)
    ));
}
