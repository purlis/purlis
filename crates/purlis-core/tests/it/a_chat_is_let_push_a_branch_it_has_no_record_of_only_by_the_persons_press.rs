// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A branch pushed before the route existed has no owner, and only the person's press gives it
//! one** (ADR 0128, amended 2026-11-23, #7363). The route refuses a chat's update of an existing
//! branch that no chat owns in the app's record, or another does; it raises one ask, **Let this
//! chat push it**, and tells git so. The person's Allow records the chat as the owner at the tip
//! the forge holds when they press; Keep blocked leaves it refused.
//!
//! Driven as the app drives it: a chat's push through the listener ([`World::asking_in`], whose
//! ask is [`windowonly::PushAsks`]), then the person's press ([`windowonly::let_chat_push`],
//! [`windowonly::keep_push_blocked`]) with the forge's answer a stand-in for the person's read.

use crate::support::forge_listener::{ACME_APP_ID, Answer, PROJECT_ID, SESSION, World, basic, pkt};

use std::path::{Path, PathBuf};

use purlis_core::forge::createdbranches;
use purlis_core::forge::route::HostScope;
use purlis_core::windowonly::{self, Control};

const CHAT: &str = "chat-listener";
const HOST: &str = "github.com";
const REPO: &str = "acme/app";
const OTHER_PROJECT_ID: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S2Q";

/// A project folder whose `charter.toml` keeps the stable id `id`.
fn project(id: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(
        root.join("charter.toml"),
        format!("schema = 1\n\n[project]\nid = \"{id}\"\n"),
    )
    .unwrap();
    (dir, root)
}

fn reply(w: &World, branch: &str, old: &str, new: &str) -> crate::support::forge_listener::Reply {
    let line = format!("{old} {new} refs/heads/{branch}\0 report-status side-band-64k agent=git/2");
    let body = format!("{}0000PACK....", pkt(&line));
    let request = format!(
        "POST /acme/app.git/git-receive-pack HTTP/1.1\r\nHost: github.com\r\nAuthorization: {}\r\nContent-Type: application/x-git-receive-pack-request\r\nContent-Length: {}\r\n\r\n{body}",
        basic("x-access-token", w.sentinel.expose()),
        body.len()
    );
    let ca = purlis_core::forge::proxy::listener::ChatCa::new(&w.proxy.terminable_hosts(&w.chat))
        .unwrap();
    w.fake.answer(Answer::ok(&format!(
        "{}{}0000",
        pkt("unpack ok\n"),
        pkt(&format!("ok refs/heads/{branch}\n"))
    )));
    crate::support::forge_listener::tls(w, &ca, HOST, 443, HOST, request.as_bytes())
        .expect("a handshake")
}

/// Whether the route refused the push itself, in git's words, and the text it said.
fn refused(w: &World, branch: &str, old: &str, new: &str) -> Option<String> {
    let reply = reply(w, branch, old, new);
    let text = reply.text();
    (reply.status == 200 && text.contains(&format!("ng refs/heads/{branch} purlis: ")))
        .then_some(text)
}

fn tip_of(config: &Path, chat: &str, branch: &str) -> Option<String> {
    createdbranches::tip_of(config, PROJECT_ID, chat, HOST, ACME_APP_ID, branch)
}

fn control(branch: &str) -> Control {
    Control::LetChatPush {
        chat: CHAT.to_owned(),
        host: HOST.to_owned(),
        repo: REPO.to_owned(),
        repo_id: ACME_APP_ID,
        branch: branch.to_owned(),
    }
}

/// What the person's read of the forge says the branch is at.
fn forge_at(tip: &str) -> impl Fn(&str, &str, &str) -> Result<String, String> + '_ {
    move |_, _, _| Ok(tip.to_owned())
}

#[test]
fn refused_then_allow_then_the_retry_from_the_recorded_tip_is_allowed() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let (_p, root) = project(PROJECT_ID);
    let w = World::asking_in(config.path().to_owned(), root.clone());
    // The branch was pushed by the person before the route existed. The chat read an older tip
    // of it (its claimed old id), and the forge has moved on since.
    let (claimed, on_forge, next) = ("3".repeat(40), "4".repeat(40), "5".repeat(40));

    let text = refused(&w, "theirs", &claimed, &next).expect("an unowned branch is refused");
    assert!(
        text.contains("Let this chat push it is waiting in the window"),
        "git says an ask is waiting: {text}"
    );
    assert!(w.fake.requests().is_empty(), "never sent");
    // One ask, however often the chat tries again.
    refused(&w, "theirs", &claimed, &next).expect("still refused");
    let waiting = windowonly::every(&root);
    assert_eq!(waiting.len(), 1, "{waiting:?}");
    assert_eq!(waiting[0].session, SESSION);
    assert_eq!(waiting[0].control, control("theirs"));
    assert_eq!(waiting[0].control.label(), "Allow");
    // Nothing is recorded before the person answers.
    assert_eq!(tip_of(config.path(), CHAT, "theirs"), None);

    // Allow: the tip recorded is the forge's, read at the press, never the chat's claimed id.
    let recorded = windowonly::let_chat_push(
        &HostScope::for_a_test(),
        config.path(),
        &root,
        SESSION,
        &control("theirs"),
        &forge_at(&on_forge),
    )
    .unwrap();
    assert_eq!(recorded, on_forge);
    assert_eq!(
        tip_of(config.path(), CHAT, "theirs"),
        Some(on_forge.clone())
    );
    assert!(windowonly::every(&root).is_empty(), "the ask is answered");

    // The tip rule applies as usual: the chat's claimed id is stale and is refused, with no new ask;
    // the recorded tip is allowed, and the confirmed push moves it.
    let stale = refused(&w, "theirs", &claimed, &next).expect("a stale tip is refused");
    assert!(!stale.contains("is waiting in the window"), "{stale}");
    assert!(
        windowonly::every(&root).is_empty(),
        "a stale tip asks nothing"
    );
    assert!(
        refused(&w, "theirs", &on_forge, &next).is_none(),
        "the retry from the recorded tip is let through"
    );
    assert_eq!(w.fake.requests().len(), 1, "sent once");
    assert_eq!(tip_of(config.path(), CHAT, "theirs"), Some(next));
    windowonly::project_ended(&root);
}

#[test]
fn keep_blocked_leaves_it_refused_and_the_chat_is_not_asked_again() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let (_p, root) = project(PROJECT_ID);
    let w = World::asking_in(config.path().to_owned(), root.clone());
    let (old, new) = ("3".repeat(40), "5".repeat(40));
    refused(&w, "theirs", &old, &new).expect("refused");
    assert_eq!(windowonly::every(&root).len(), 1);

    windowonly::keep_push_blocked(&root, SESSION, &control("theirs")).unwrap();
    assert!(windowonly::every(&root).is_empty(), "the ask is answered");
    // Allow can no longer be pressed on it, and records nothing.
    assert!(
        windowonly::let_chat_push(
            &HostScope::for_a_test(),
            config.path(),
            &root,
            SESSION,
            &control("theirs"),
            &forge_at(&old)
        )
        .is_err()
    );

    let text = refused(&w, "theirs", &old, &new).expect("still refused");
    assert!(
        text.contains("kept"),
        "git says it was kept blocked: {text}"
    );
    assert!(
        windowonly::every(&root).is_empty(),
        "and no second ask is raised"
    );
    assert_eq!(tip_of(config.path(), CHAT, "theirs"), None);
    assert!(!createdbranches::file_in(config.path()).exists());
    assert!(w.fake.requests().is_empty(), "never sent");
    windowonly::project_ended(&root);
}

#[test]
fn a_protected_default_deploy_or_pages_branch_raises_no_ask() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let (_p, root) = project(PROJECT_ID);
    let w = World::asking_in(config.path().to_owned(), root.clone());
    let (old, new) = ("3".repeat(40), "5".repeat(40));
    for branch in [
        "main",
        "MAIN",
        "gh-pages",
        "deploy",
        "release/1.0",
        "production/x",
    ] {
        let text = refused(&w, branch, &old, &new).unwrap_or_else(|| panic!("{branch} is refused"));
        assert!(
            !text.contains("is waiting in the window"),
            "{branch}: {text}"
        );
    }
    assert!(windowonly::every(&root).is_empty());
    assert!(w.fake.requests().is_empty(), "never sent");
    windowonly::project_ended(&root);
}

#[test]
fn a_branch_the_chat_owns_at_another_tip_raises_no_ask() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let (_p, root) = project(PROJECT_ID);
    let w = World::asking_in(config.path().to_owned(), root.clone());
    // The chat makes the branch (a create is no update), then pushes from a tip it did not leave.
    assert!(refused(&w, "mine", &"0".repeat(40), &"1".repeat(40)).is_none());
    let text = refused(&w, "mine", &"9".repeat(40), &"2".repeat(40)).expect("refused");
    assert!(!text.contains("is waiting in the window"), "{text}");
    assert!(windowonly::every(&root).is_empty());
    windowonly::project_ended(&root);
}

#[test]
fn an_ask_of_another_projects_chat_cannot_grant_this_one() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let (_a, root_a) = project(PROJECT_ID);
    let (_b, root_b) = project(OTHER_PROJECT_ID);
    let w = World::asking_in(config.path().to_owned(), root_a.clone());
    let (old, new, on_forge) = ("3".repeat(40), "5".repeat(40), "4".repeat(40));
    refused(&w, "theirs", &old, &new).expect("refused");
    assert_eq!(windowonly::every(&root_a).len(), 1);

    // The other project's window presses the same control, for the same session: nothing waits
    // there, so nothing is recorded, for either project, and the first ask still waits.
    let wrong = windowonly::let_chat_push(
        &HostScope::for_a_test(),
        config.path(),
        &root_b,
        SESSION,
        &control("theirs"),
        &forge_at(&on_forge),
    );
    assert!(wrong.is_err(), "{wrong:?}");
    assert!(windowonly::keep_push_blocked(&root_b, SESSION, &control("theirs")).is_err());
    assert!(!createdbranches::file_in(config.path()).exists());
    assert_eq!(windowonly::every(&root_a).len(), 1, "the ask still waits");

    // A control no chat was refused for, and another chat's session, grant nothing either.
    assert!(
        windowonly::let_chat_push(
            &HostScope::for_a_test(),
            config.path(),
            &root_a,
            SESSION,
            &control("never-asked"),
            &forge_at(&on_forge)
        )
        .is_err()
    );
    assert!(
        windowonly::let_chat_push(
            &HostScope::for_a_test(),
            config.path(),
            &root_a,
            SESSION + 1,
            &control("theirs"),
            &forge_at(&on_forge)
        )
        .is_err()
    );
    assert!(!createdbranches::file_in(config.path()).exists());

    // And an allow records under the pressing project's own id: the other project's chat of the
    // same id owns nothing.
    windowonly::let_chat_push(
        &HostScope::for_a_test(),
        config.path(),
        &root_a,
        SESSION,
        &control("theirs"),
        &forge_at(&on_forge),
    )
    .unwrap();
    assert_eq!(tip_of(config.path(), CHAT, "theirs"), Some(on_forge));
    assert_eq!(
        createdbranches::tip_of(
            config.path(),
            OTHER_PROJECT_ID,
            CHAT,
            HOST,
            ACME_APP_ID,
            "theirs"
        ),
        None
    );
    windowonly::project_ended(&root_a);
    windowonly::project_ended(&root_b);
}

#[test]
fn a_forge_that_does_not_answer_a_commit_records_nothing_and_the_ask_still_waits() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let (_p, root) = project(PROJECT_ID);
    let w = World::asking_in(config.path().to_owned(), root.clone());
    refused(&w, "theirs", &"3".repeat(40), &"5".repeat(40)).expect("refused");

    let down = |_: &str, _: &str, _: &str| Err::<String, String>("the forge is down".to_owned());
    assert!(
        windowonly::let_chat_push(
            &HostScope::for_a_test(),
            config.path(),
            &root,
            SESSION,
            &control("theirs"),
            &down
        )
        .is_err()
    );
    assert!(
        windowonly::let_chat_push(
            &HostScope::for_a_test(),
            config.path(),
            &root,
            SESSION,
            &control("theirs"),
            &forge_at("not-a-commit")
        )
        .is_err()
    );
    assert!(!createdbranches::file_in(config.path()).exists());
    assert_eq!(
        windowonly::every(&root).len(),
        1,
        "the person may press again"
    );
    windowonly::project_ended(&root);
}
