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
            .control
            .says()
            .starts_with("Trust this clone so chats can use GitHub"),
        "{}",
        raised[0].control.says()
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
