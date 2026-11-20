// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A (instructions 178 item 4, 182 item 6): a push may create a branch, and move
//! or delete only the branches the chat itself created (or was given). That is what refuses a
//! force-push or a non-fast-forward anywhere else: the proxy sees old and new ids, never history.

use purlis_core::forge::proxy::{Outcome, Refusal};

use crate::support::forge_proxy::{Scene, is_refused, push_body};

fn push(s: &Scene, cmds: &[(&str, &str)]) -> Outcome {
    s.send(
        "POST",
        "github.com",
        "/acme/app.git/git-receive-pack",
        &push_body(cmds),
    )
}

#[test]
fn a_branch_the_chat_created_it_may_move_and_delete_afterwards() {
    purlis_core::unsteered!();
    let s = Scene::new();
    assert!(matches!(
        push(&s, &[("create", "refs/heads/mine")]),
        Outcome::Forward(_)
    ));
    assert!(matches!(
        push(&s, &[("update", "refs/heads/mine")]),
        Outcome::Forward(_)
    ));
    assert!(matches!(
        push(&s, &[("delete", "refs/heads/mine")]),
        Outcome::Forward(_)
    ));
}

#[test]
fn a_branch_it_did_not_create_it_may_neither_move_nor_delete() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for kind in ["update", "delete"] {
        let out = push(&s, &[(kind, "refs/heads/colleagues")]);
        assert!(is_refused(&out, Refusal::ProtectedRef), "{kind}: {out:?}");
    }
}

#[test]
fn a_refused_push_records_no_branch() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let out = push(
        &s,
        &[("create", "refs/heads/half"), ("update", "refs/heads/main")],
    );
    assert!(is_refused(&out, Refusal::ProtectedRef));
    assert!(is_refused(
        &push(&s, &[("update", "refs/heads/half")]),
        Refusal::ProtectedRef
    ));
}

#[test]
fn a_branch_is_the_chats_own_on_one_repo_only() {
    purlis_core::unsteered!();
    let s = Scene::new();
    assert!(matches!(
        push(&s, &[("create", "refs/heads/mine")]),
        Outcome::Forward(_)
    ));
    let ghes = s.send(
        "POST",
        "ghe.corp.example",
        "/acme/app.git/git-receive-pack",
        &push_body(&[("update", "refs/heads/mine")]),
    );
    assert!(is_refused(&ghes, Refusal::ProtectedRef), "{ghes:?}");
}

#[test]
fn a_branch_the_chat_was_given_at_the_start_is_its_own() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let out = s.send(
        "POST",
        "gitlab.com",
        "/group/proj.git/git-receive-pack",
        &push_body(&[("update", "refs/heads/work")]),
    );
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn a_pages_branch_is_never_pushed_to() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for name in ["refs/heads/gh-pages", "refs/heads/GH-Pages"] {
        assert!(
            is_refused(&push(&s, &[("create", name)]), Refusal::ProtectedRef),
            "{name}"
        );
    }
}

#[test]
fn a_push_with_push_options_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    // `-o merge_request.merge_when_pipeline_succeeds` rides as a push option: the capability and
    // the options after the commands.
    let zero = "0".repeat(40);
    let new = "1".repeat(40);
    let line = format!("{zero} {new} refs/heads/mine\0 report-status push-options agent=git/2");
    let mut body = format!("{:04x}{line}0000", line.len() + 4).into_bytes();
    let opt = "merge_request.merge_when_pipeline_succeeds";
    body.extend(format!("{:04x}{opt}0000PACK", opt.len() + 4).as_bytes());
    let out = s.send(
        "POST",
        "github.com",
        "/acme/app.git/git-receive-pack",
        &body,
    );
    assert!(is_refused(&out, Refusal::PushOption), "{out:?}");
}

#[test]
fn lfs_is_not_carried_because_its_answer_hands_back_credentials() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for (method, target) in [
        ("POST", "/acme/app.git/info/lfs/objects/batch"),
        ("GET", "/acme/app.git/info/lfs/locks"),
        ("POST", "/group/proj.git/info/lfs/objects/batch"),
    ] {
        let host = if target.starts_with("/group") {
            "gitlab.com"
        } else {
            "github.com"
        };
        let out = s.send(method, host, target, b"{}");
        assert!(is_refused(&out, Refusal::Lfs), "{target}: {out:?}");
    }
}
