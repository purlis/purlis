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
fn a_label_with_a_control_character_is_refused_whatever_it_spells() {
    purlis_core::unsteered!();
    let s = Scene::new();
    // A trailing NUL is stripped by a Ruby forge or bot: `automerge` by another name.
    for label in [
        "automerge\u{0}",
        "auto\u{0}merge",
        "bug\u{7}",
        "\u{1b}[0mbug",
        "ok\u{0}",
    ] {
        let out = s.send(
            "POST",
            "api.github.com",
            "/repos/acme/app/issues/7/labels",
            &json(serde_json::json!({ "labels": [label] })),
        );
        assert!(
            is_refused(&out, Refusal::MergeCommand),
            "{label:?}: {out:?}"
        );
    }
    // The same through a comma list, which a forge splits.
    let out = s.send(
        "POST",
        "api.github.com",
        "/repos/acme/app/issues/7/labels",
        &json(serde_json::json!({ "labels": ["bug,automerge\u{0}"] })),
    );
    assert!(is_refused(&out, Refusal::MergeCommand), "{out:?}");
}

#[test]
fn a_graphql_label_is_split_at_commas_and_read_as_a_rest_body_is() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let q = "mutation($i:CreateIssueInput!){ createIssue(input:$i){ clientMutationId } }";
    for labels in [
        serde_json::json!(["bug,automerge"]),
        serde_json::json!("bug, ready-to-merge"),
        serde_json::json!(["automerge\u{0}"]),
    ] {
        let bad = graphql(
            q,
            serde_json::json!({"i": {"projectPath": "group/proj", "title": "t", "labels": labels}}),
        );
        let out = s.send("POST", "gitlab.com", "/api/graphql", &bad);
        assert!(is_refused(&out, Refusal::MergeCommand), "{labels}: {out:?}");
    }
    // Ordinary names are not merge labels.
    let fine = graphql(
        q,
        serde_json::json!({"i": {"projectPath": "group/proj", "title": "t", "labels": ["bug,docs"]}}),
    );
    let out = s.send("POST", "gitlab.com", "/api/graphql", &fine);
    assert!(!is_refused(&out, Refusal::MergeCommand), "{out:?}");
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
