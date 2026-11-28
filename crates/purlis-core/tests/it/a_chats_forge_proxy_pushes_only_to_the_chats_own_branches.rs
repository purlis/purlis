// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A (instructions 178 item 4, 182 item 6, 194 H4): a push may make a branch that
//! does not exist, and move only a branch the chat made (once the forge said so) or was given. It
//! may never delete one. That is what refuses a force-push or a non-fast-forward anywhere else:
//! the proxy sees old and new ids, never history.

use purlis_core::forge::proxy::{Outcome, Refusal};

use crate::support::forge_proxy::{Auth, Scene, is_refused, push_body};

fn push(s: &Scene, cmds: &[(&str, &str)]) -> Outcome {
    s.send(
        "POST",
        "github.com",
        "/acme/app.git/git-receive-pack",
        &push_body(cmds),
    )
}

#[test]
fn a_branch_the_chat_made_it_may_move_once_the_forge_has_said_it_was_made() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let made = push(&s, &[("create", "refs/heads/mine")]);
    assert!(matches!(made, Outcome::Forward(_)), "{made:?}");
    // The forge has not said so yet: the branch is not the chat's.
    assert!(is_refused(
        &push(&s, &[("update", "refs/heads/mine")]),
        Refusal::ProtectedRef
    ));
    s.confirm(&made);
    assert!(matches!(
        push(&s, &[("update", "refs/heads/mine")]),
        Outcome::Forward(_)
    ));
}

#[test]
fn a_push_the_forge_refused_makes_no_branch_the_chat_owns() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let _never_confirmed = push(&s, &[("create", "refs/heads/claimed")]);
    assert!(is_refused(
        &push(&s, &[("update", "refs/heads/claimed")]),
        Refusal::ProtectedRef
    ));
}

#[test]
fn a_branch_is_never_deleted_not_even_the_chats_own() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let made = push(&s, &[("create", "refs/heads/mine")]);
    s.confirm(&made);
    for name in ["refs/heads/mine", "refs/heads/colleagues"] {
        let out = push(&s, &[("delete", name)]);
        assert!(is_refused(&out, Refusal::ProtectedRef), "{name}: {out:?}");
    }
}

#[test]
fn a_branch_that_already_exists_is_not_made_so_it_cannot_be_claimed_and_then_moved() {
    purlis_core::unsteered!();
    let s = Scene::new();
    s.advertise("github.com", "acme/app", &["fix", "colleagues"]);
    for name in ["refs/heads/fix", "refs/heads/Fix", "refs/heads/colleagues"] {
        let out = push(&s, &[("create", name)]);
        assert!(is_refused(&out, Refusal::ProtectedRef), "{name}: {out:?}");
    }
    // And then to move it, which is not the chat's either.
    for name in ["refs/heads/fix", "refs/heads/Fix"] {
        let out = push(&s, &[("update", name)]);
        assert!(is_refused(&out, Refusal::ProtectedRef), "{name}: {out:?}");
    }
}

#[test]
fn a_branch_is_owned_with_its_case_as_the_forge_has_it() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let made = push(&s, &[("create", "refs/heads/Mine")]);
    s.confirm(&made);
    assert!(matches!(
        push(&s, &[("update", "refs/heads/Mine")]),
        Outcome::Forward(_)
    ));
    assert!(is_refused(
        &push(&s, &[("update", "refs/heads/mine")]),
        Refusal::ProtectedRef
    ));
}

#[test]
fn a_branch_it_did_not_make_it_may_not_move() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let out = push(&s, &[("update", "refs/heads/colleagues")]);
    assert!(is_refused(&out, Refusal::ProtectedRef), "{out:?}");
}

#[test]
fn a_refused_push_makes_no_ticket_for_a_branch_it_listed() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let out = push(
        &s,
        &[("create", "refs/heads/half"), ("update", "refs/heads/main")],
    );
    assert!(is_refused(&out, Refusal::ProtectedRef));
    s.confirm(&out);
    assert!(is_refused(
        &push(&s, &[("update", "refs/heads/half")]),
        Refusal::ProtectedRef
    ));
}

#[test]
fn a_branch_is_the_chats_own_on_one_repo_only() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let made = push(&s, &[("create", "refs/heads/mine")]);
    s.confirm(&made);
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
fn a_branch_is_not_made_where_the_proxy_was_never_told_the_repos_refs() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let c = purlis_core::forge::proxy::ChatId::new("chat-c");
    let repo = purlis_core::forge::proxy::AllowedRepo::new(
        purlis_core::forge::proxy::Kind::GitHub,
        "github.com",
        443,
        "acme/app",
        "main",
    )
    .unwrap();
    let sentinel = s.proxy.enrol(&c, vec![repo]);
    let out = s.send_as(
        &c,
        "POST",
        "github.com",
        443,
        "/acme/app.git/git-receive-pack",
        Auth::Bearer(sentinel.expose()),
        &push_body(&[("create", "refs/heads/new")]),
    );
    assert!(is_refused(&out, Refusal::NoAdvertisement), "{out:?}");
}

#[test]
fn a_pages_branch_and_the_names_a_deploy_starts_from_are_never_pushed_to() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for name in [
        "refs/heads/gh-pages",
        "refs/heads/GH-Pages",
        "refs/heads/main",
        "refs/heads/master",
        "refs/heads/prod",
        "refs/heads/production",
        "refs/heads/release/1.2",
        "refs/heads/Release/1.2",
        "refs/heads/deploy/staging",
        "refs/heads/prod/eu",
    ] {
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

/// Before a push larger than its post buffer, git sends a probe: the same request, with a body of
/// one flush packet and no command (#7878). It moves nothing, so it is let through; refused, git
/// never sends the push itself, and the person reads "the remote end hung up unexpectedly".
#[test]
fn gits_probe_before_a_large_push_moves_nothing_and_is_let_through() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let probe = s.send(
        "POST",
        "github.com",
        "/acme/app.git/git-receive-pack",
        b"0000",
    );
    assert!(matches!(probe, Outcome::Forward(_)), "{probe:?}");
    // Only a lone flush packet is a probe: anything else that holds no command is refused.
    for body in [&b"00000000"[..], b"0000PACK", b"", b"0004"] {
        let out = s.send("POST", "github.com", "/acme/app.git/git-receive-pack", body);
        assert!(
            is_refused(&out, Refusal::BodyUnreadable),
            "{body:?}: {out:?}"
        );
    }
}
