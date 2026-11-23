// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A window-only refusal raises one ask carrying its control, against a plane of real clones
//! (git runs locally, no forge is asked) and a config home of its own.

use std::path::{Path, PathBuf};

use super::*;

/// Runs git in `dir`, which must succeed.
fn git(dir: &Path, args: &[&str]) {
    let mut c = std::process::Command::new("git");
    c.arg("-C").arg(dir).args(args);
    assert!(
        crate::forklock::output(&mut c).unwrap().status.success(),
        "git {args:?}"
    );
}

/// A clone `name` in workspace `ws` of the plane at `root`, its `origin` at `url`.
fn a_clone(root: &Path, name: &str, url: &str) -> PathBuf {
    let clone = root.join("workspaces/ws").join(name);
    std::fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "-q"]);
    git(&clone, &["remote", "add", "origin", url]);
    clone
}

fn trust_api() -> Control {
    Control::TrustClone {
        folder: "workspaces/ws/api".to_owned(),
        place: "github.com/acme/api".to_owned(),
    }
}

#[test]
fn a_window_only_refusal_raises_one_ask_carrying_its_control() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let root = plane.path();
    a_clone(root, "api", "https://github.com/acme/api.git");
    let told = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    telling(root, {
        let told = Arc::clone(&told);
        Arc::new(move || {
            told.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })
    });

    // The same chat refused twice for the same reason is one ask, and the window is told once.
    assert!(raise(root, 4, trust_api()));
    assert!(!raise(root, 4, trust_api()));
    let raised = every(root);
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].session, 4);
    assert_eq!(raised[0].control, trust_api());
    assert_eq!(raised[0].control.label(), "Trust this clone");
    assert!(
        raised[0]
            .says()
            .starts_with("Trust this clone so chats can use GitHub"),
        "{}",
        raised[0].says()
    );
    assert_eq!(told.load(std::sync::atomic::Ordering::SeqCst), 1);

    // Pressing the control clears the refusal: the clone is recorded, and the ask is gone.
    let trusted = trust_this_clone(
        &HostScope::for_a_test(),
        home.path(),
        root,
        "workspaces/ws/api",
        "github.com/acme/api",
    )
    .unwrap();
    assert_eq!(trusted, "github.com/acme/api");
    assert!(crate::forge::gitcredential::vouch::trusted(
        home.path(),
        &root.join("workspaces/ws/api")
    ));
    assert_eq!(every(root), []);
    assert_eq!(told.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[test]
fn a_press_that_trusts_nothing_clears_no_refusal() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let root = plane.path();
    a_clone(root, "api", "https://github.com/acme/other.git");
    raise(root, 4, trust_api());

    // Its remote changed since the ask was raised: nothing is trusted, and the ask stays.
    let refused = trust_this_clone(
        &HostScope::for_a_test(),
        home.path(),
        root,
        "workspaces/ws/api",
        "github.com/acme/api",
    );
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(every(root).len(), 1);
}

#[test]
fn a_chat_s_refusals_go_when_it_ends_and_no_other_chat_s() {
    let plane = tempfile::tempdir().unwrap();
    let root = plane.path();
    raise(root, 4, trust_api());
    raise(root, 5, trust_api());

    forget(root, 4);

    let left: Vec<u32> = every(root).iter().map(|one| one.session).collect();
    assert_eq!(left, [5]);
}

#[test]
fn a_new_ask_for_a_clone_at_another_place_replaces_the_chat_s_old_one() {
    let plane = tempfile::tempdir().unwrap();
    let root = plane.path();
    let moved = Control::TrustClone {
        folder: "workspaces/ws/api".to_owned(),
        place: "github.com/acme/moved".to_owned(),
    };
    raise(root, 4, trust_api());
    raise(root, 5, trust_api());

    // The clone's remote changed: chat 4's old ask would be a dead button, so the new one
    // replaces it rather than stacking. Chat 5's stays until chat 5 is refused again.
    assert!(raise(root, 4, moved.clone()));

    let left: Vec<(u32, Control)> = every(root)
        .into_iter()
        .map(|one| (one.session, one.control))
        .collect();
    assert_eq!(left, [(5, trust_api()), (4, moved)]);
}

#[test]
fn a_project_that_ended_keeps_no_ask_and_tells_no_one() {
    let plane = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let root = plane.path();
    let told = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    telling(root, {
        let told = Arc::clone(&told);
        Arc::new(move || {
            told.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })
    });
    raise(root, 4, trust_api());
    raise(other.path(), 4, trust_api());
    let told_before = told.load(std::sync::atomic::Ordering::SeqCst);

    project_ended(root);

    assert_eq!(every(root), []);
    assert_eq!(every(other.path()).len(), 1, "another project's ask stays");
    // Its listener is gone: an ask raised later tells no one.
    raise(root, 6, trust_api());
    assert_eq!(told.load(std::sync::atomic::Ordering::SeqCst), told_before);
    project_ended(root);
    project_ended(other.path());
}

#[test]
fn a_window_only_refusal_is_raised_as_an_ask_as_policy_consents_runs() {
    // Q308b: the ask is `consents`'. Logged, it is not raised, and one line says it would be.
    for (policy, asked, lines) in [("", true, 0), ("consents = \"log\"\n", false, 1)] {
        let root = tempfile::tempdir().expect("a project");
        std::fs::write(
            crate::names::manifest(root.path()),
            format!("[policy]\ntemplate = \"strict\"\n{policy}"),
        )
        .expect("the manifest");
        assert_eq!(asks(root.path()), asked, "{policy:?}");
        let logged = crate::enforcement::read_log(root.path(), crate::enforcement::UNKNOWN_CHAT);
        assert_eq!(logged.len(), lines, "{policy:?}");
        assert!(
            logged
                .iter()
                .all(|line| line.family == "consents" && line.rule == "window-only.ask")
        );
    }
}

fn unowned(branch: &str) -> crate::forge::proxy::NotYours {
    crate::forge::proxy::NotYours {
        host: "github.com".to_owned(),
        repo: "acme/app".to_owned(),
        repo_id: 41,
        branch: branch.to_owned(),
    }
}

fn push_asks(root: &Path, session: Option<u32>) -> PushAsks {
    std::fs::write(
        crate::names::manifest(root),
        "[policy]\ntemplate = \"strict\"\n",
    )
    .expect("the manifest");
    PushAsks {
        root: root.to_path_buf(),
        chat: "chat-1".to_owned(),
        session: Box::new(move || session),
        owner_of: Box::new(|_, _, branch| (branch == "owned").then(|| "chat-2".to_owned())),
    }
}

#[test]
fn a_chat_that_has_ended_is_asked_nothing_and_asks_nothing() {
    use crate::forge::proxy::{BranchAsk, ChatId};
    let root = tempfile::tempdir().unwrap();
    let asks = push_asks(root.path(), None);
    assert_eq!(asks.ask(&ChatId::new("chat-1"), &[unowned("theirs")]), None);
    assert!(every(root.path()).is_empty());
}

#[test]
fn an_ask_says_who_owns_the_branch_where_and_that_history_can_be_replaced() {
    use crate::forge::proxy::{BranchAsk, ChatId};
    let root = tempfile::tempdir().unwrap();
    let asks = push_asks(root.path(), Some(5));
    asks.ask(&ChatId::new("chat-1"), &[unowned("owned")])
        .unwrap();
    asks.ask(&ChatId::new("chat-1"), &[unowned("nobody")])
        .unwrap();
    let said: Vec<String> = every(root.path()).iter().map(Refusal::says).collect();
    assert!(
        said[0].contains("owned in acme/app on github.com"),
        "{}",
        said[0]
    );
    assert!(said[0].contains("Chat chat-2 owns it"), "{}", said[0]);
    assert!(said[1].contains("Nobody owns it"), "{}", said[1]);
    for one in &said {
        assert!(
            one.contains("replace the branch's history, not only add to it"),
            "{one}"
        );
    }
    project_ended(root.path());
}

#[test]
fn no_more_than_three_asks_wait_for_a_chat_and_one_is_raised_for_a_push() {
    use crate::forge::proxy::{BranchAsk, ChatId};
    let root = tempfile::tempdir().unwrap();
    let asks = push_asks(root.path(), Some(5));
    let chat = ChatId::new("chat-1");
    // A push of many branches raises one.
    let many: Vec<_> = (0..50).map(|n| unowned(&format!("b{n}"))).collect();
    asks.ask(&chat, &many).unwrap();
    assert_eq!(every(root.path()).len(), 1);
    asks.ask(&chat, &many).unwrap();
    asks.ask(&chat, &many).unwrap();
    assert_eq!(every(root.path()).len(), 3);
    let said = asks.ask(&chat, &many).unwrap();
    assert_eq!(
        every(root.path()).len(),
        3,
        "past the cap nothing is raised"
    );
    assert!(said.contains("already waiting"), "{said}");
    // Another chat has its own three.
    let other = push_asks(root.path(), Some(6));
    other.ask(&chat, &many).unwrap();
    assert_eq!(every(root.path()).len(), 4);
    project_ended(root.path());
}

#[test]
fn a_project_whose_consents_are_logged_or_off_raises_no_push_ask() {
    use crate::forge::proxy::{BranchAsk, ChatId};
    for consents in ["log", "off"] {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            crate::names::manifest(root.path()),
            format!("[policy]\ntemplate = \"strict\"\nconsents = \"{consents}\"\n"),
        )
        .unwrap();
        // The helper writes its own manifest, so build the ask by hand.
        let asks = PushAsks {
            root: root.path().to_path_buf(),
            chat: "chat-1".to_owned(),
            session: Box::new(|| Some(5)),
            owner_of: Box::new(|_, _, _| None),
        };
        assert_eq!(
            asks.ask(&ChatId::new("chat-1"), &[unowned("theirs")]),
            None,
            "{consents}"
        );
        assert!(every(root.path()).is_empty(), "{consents}");
    }
}
