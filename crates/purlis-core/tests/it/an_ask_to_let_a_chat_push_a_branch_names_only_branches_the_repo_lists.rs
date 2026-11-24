// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The ask to let a chat push a branch it has no record of is raised only for a branch the
//! repo's advertisement lists, with a name that can be said** (ADR 0128, amended 2026-11-23,
//! #7415). A push the route refuses for [`Refusal::NotYourBranch`] hands the policy's reading of
//! it to the [`BranchAsk`]: an invented name, a name that is not printable ASCII, a delete, a
//! tag and a protected ref raise nothing, and the push is refused as it was.

use std::sync::{Arc, Mutex};

use purlis_core::forge::proxy::{BranchAsk, ChatId, NotYours, Outcome, Refusal};

use crate::support::forge_proxy::{Objects, Scene, is_refused, push_body};

/// A sink that keeps the branches it was asked about, and answers that it asked.
#[derive(Default)]
struct Asked(Mutex<Vec<Vec<String>>>);

/// The sink the proxy owns, sharing what it was asked with the test.
struct Sink(Arc<Asked>);

impl BranchAsk for Sink {
    fn ask(&self, _: &ChatId, branches: &[NotYours]) -> Option<String> {
        self.0
            .0
            .lock()
            .unwrap()
            .push(branches.iter().map(|b| b.branch.clone()).collect());
        Some("asked".to_owned())
    }
}

/// A scene whose gitlab repo (forge id 42) has no branch the chat made, and its sink.
fn scene() -> (Scene, Arc<Asked>) {
    let asked = Arc::new(Asked::default());
    let sink = Arc::clone(&asked);
    let s = Scene::tuned(Objects::default(), move |proxy| {
        proxy
            .with_tips(Arc::new(|_, _, _| None))
            .with_branch_ask(Box::new(Sink(sink)))
    });
    (s, asked)
}

fn push(s: &Scene, cmds: &[(&str, &str)]) -> Outcome {
    s.send(
        "POST",
        "gitlab.com",
        "/group/proj.git/git-receive-pack",
        &push_body(cmds),
    )
}

#[test]
fn an_update_of_a_branch_no_chat_owns_is_refused_as_not_your_branch() {
    purlis_core::unsteered!();
    // No sink answers: the refusal itself is what git is told.
    let bare = Scene::tuned(Objects::default(), |proxy| {
        proxy.with_tips(Arc::new(|_, _, _| None))
    });
    bare.advertise("gitlab.com", "group/proj", &["main", "theirs"]);
    let out = push(&bare, &[("update", "refs/heads/theirs")]);
    assert!(is_refused(&out, Refusal::NotYourBranch), "{out:?}");
}

#[test]
fn only_a_listed_branch_with_a_sayable_name_is_asked_about() {
    purlis_core::unsteered!();
    let (s, asked) = scene();
    s.advertise(
        "gitlab.com",
        "group/proj",
        &["main", "theirs", "naïve", "-dash"],
    );
    let out = push(
        &s,
        &[
            ("update", "refs/heads/theirs"),
            ("update", "refs/heads/invented"),
            ("update", "refs/heads/naïve"),
            ("update", "refs/heads/-dash"),
        ],
    );
    assert!(matches!(out, Outcome::Held(_)), "{out:?}");
    assert_eq!(*asked.0.lock().unwrap(), [vec!["theirs".to_owned()]]);
}

#[test]
fn an_invented_or_unsayable_name_raises_no_ask_and_the_push_is_refused() {
    purlis_core::unsteered!();
    let (s, asked) = scene();
    s.advertise("gitlab.com", "group/proj", &["main", "naïve", "-dash"]);
    for name in ["invented", "naïve", "-dash"] {
        let out = push(&s, &[("update", &format!("refs/heads/{name}"))]);
        assert!(is_refused(&out, Refusal::NotYourBranch), "{name}: {out:?}");
    }
    assert!(asked.0.lock().unwrap().is_empty());
}

#[test]
fn a_repo_whose_branches_were_never_read_asks_nothing() {
    purlis_core::unsteered!();
    let (s, asked) = scene();
    let out = push(&s, &[("update", "refs/heads/theirs")]);
    assert!(is_refused(&out, Refusal::NotYourBranch), "{out:?}");
    assert!(asked.0.lock().unwrap().is_empty());
}

#[test]
fn a_delete_a_tag_or_a_protected_ref_beside_an_unowned_branch_raises_no_ask() {
    purlis_core::unsteered!();
    let (s, asked) = scene();
    s.advertise("gitlab.com", "group/proj", &["main", "theirs", "other"]);
    for (kind, name) in [
        ("delete", "refs/heads/theirs"),
        ("update", "refs/tags/v1"),
        ("update", "refs/heads/main"),
        ("delete", "refs/heads/other"),
    ] {
        let alone = push(&s, &[(kind, name)]);
        assert!(!matches!(alone, Outcome::Held(_)), "{name}: {alone:?}");
        let mixed = push(&s, &[("update", "refs/heads/theirs"), (kind, name)]);
        assert!(!matches!(mixed, Outcome::Held(_)), "{name}: {mixed:?}");
    }
    assert!(asked.0.lock().unwrap().is_empty());
}

#[test]
fn a_push_of_several_unowned_branches_hands_each_once_to_the_ask() {
    purlis_core::unsteered!();
    let (s, asked) = scene();
    s.advertise("gitlab.com", "group/proj", &["main", "theirs", "other"]);
    let out = push(
        &s,
        &[
            ("update", "refs/heads/theirs"),
            ("update", "refs/heads/other"),
            ("update", "refs/heads/theirs"),
        ],
    );
    assert!(matches!(out, Outcome::Held(_)), "{out:?}");
    assert_eq!(
        *asked.0.lock().unwrap(),
        [vec!["theirs".to_owned(), "other".to_owned()]]
    );
}
