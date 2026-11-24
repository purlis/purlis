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
    assert!(raise(root, 4, trust_api()).new);
    assert!(!raise(root, 4, trust_api()).new);
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
    assert!(raise(root, 4, moved.clone()).new);

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

#[test]
fn raise_answers_whether_the_ask_is_new_and_whether_a_window_listens() {
    let plane = tempfile::tempdir().unwrap();
    let root = plane.path();

    // No window listens for this project: the ask is kept, and the answer says nobody hears it.
    assert!(!listens(root));
    assert_eq!(
        raise(root, 4, trust_api()),
        Raised {
            new: true,
            listened: false
        }
    );

    telling(root, Arc::new(|| {}));
    assert!(listens(root));
    assert_eq!(
        raise(root, 4, trust_api()),
        Raised {
            new: false,
            listened: true
        }
    );
    assert_eq!(
        raise(root, 5, trust_api()),
        Raised {
            new: true,
            listened: true
        }
    );

    // A project that ended has no listener again.
    project_ended(root);
    assert!(!listens(root));
}

#[test]
fn each_new_clone_kind_has_its_label_and_says_what_its_press_does() {
    let one = Control::OneAddress {
        folder: "workspaces/ws/api".to_owned(),
        fetch: "github.com/acme/api".to_owned(),
        push: "github.com/acme/fork".to_owned(),
        to: "github.com/acme/api".to_owned(),
    };
    assert_eq!(one.label(), "Use one address");
    let said = one.says();
    // The row shows the address it will set, and the two it found (Q4).
    for shown in [
        "workspaces/ws/api",
        "github.com/acme/api",
        "github.com/acme/fork",
    ] {
        assert!(said.contains(shown), "{said}");
    }

    let clone = Control::CloneWithSignIn {
        workspace: "ws".to_owned(),
        repo: "github.com/acme/private".to_owned(),
    };
    assert_eq!(clone.label(), "Clone it");
    let said = clone.says();
    // Host, owner, repo and the destination folder, and plainly the person's own sign-in.
    for shown in ["github.com", "acme", "private", "workspaces/ws"] {
        assert!(said.contains(shown), "{said}");
    }
    assert!(said.contains("your sign-in"), "{said}");
}

#[test]
fn a_press_on_a_new_kind_clears_every_chat_that_waited_on_it() {
    let plane = tempfile::tempdir().unwrap();
    let root = plane.path();
    let clone = Control::CloneWithSignIn {
        workspace: "ws".to_owned(),
        repo: "github.com/acme/private".to_owned(),
    };
    raise(root, 4, clone.clone());
    raise(root, 5, clone.clone());
    raise(root, 5, trust_api());

    pressed_by_the_window(root, &clone);

    let left: Vec<Control> = every(root).into_iter().map(|one| one.control).collect();
    assert_eq!(left, [trust_api()]);
    project_ended(root);
}

#[test]
fn nobody_asked_says_so_and_names_the_way_out_in_words() {
    let plane = tempfile::tempdir().unwrap();
    let root = plane.path().join("acme");
    let said = nobody_asked(&root, "press Use one address on the clone");
    assert!(said.starts_with("Nobody was asked"), "{said}");
    assert!(
        said.contains("open purlis on acme, then press Use one address"),
        "{said}"
    );
    assert!(!said.contains("asked the person in Asks"), "{said}");
}

#[test]
fn use_one_address_sets_exactly_the_shown_address_and_fails_if_origin_changed() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let root = plane.path();
    let clone = a_clone(root, "api", "https://github.com/acme/api.git");
    git(
        &clone,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            "https://github.com/acme/fork.git",
        ],
    );
    let shown = || Control::OneAddress {
        folder: "workspaces/ws/api".to_owned(),
        fetch: "github.com/acme/api".to_owned(),
        push: "github.com/acme/fork".to_owned(),
        to: "github.com/acme/api".to_owned(),
    };
    raise(root, 4, shown());

    // Origin moved since the row was raised: the press fails, changes nothing, says so.
    git(
        &clone,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/acme/elsewhere.git",
        ],
    );
    let refused = use_one_address(&HostScope::for_a_test(), home.path(), root, &shown());
    assert!(refused.is_err(), "{refused:?}");
    assert!(refused.unwrap_err().contains("changed since"), "said");
    assert_eq!(
        crate::forge::gitcredential::remote::places_now(&clone).unwrap(),
        ["github.com/acme/elsewhere", "github.com/acme/fork"]
    );
    assert_eq!(every(root).len(), 1, "the ask stays");

    // Origin as the row showed it: the push address is set to the one shown, and nothing else.
    git(
        &clone,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/acme/api.git",
        ],
    );
    let set = use_one_address(&HostScope::for_a_test(), home.path(), root, &shown()).unwrap();
    assert_eq!(set, "github.com/acme/api");
    assert_eq!(
        crate::forge::gitcredential::remote::places_now(&clone).unwrap(),
        ["github.com/acme/api", "github.com/acme/api"]
    );
    // Setting an address is never trusting: the clone still has no record, and the chat that
    // waited is now offered Trust this clone for the one address it has: a second press.
    assert!(!crate::forge::gitcredential::vouch::trusted(
        home.path(),
        &clone
    ));
    let left: Vec<(u32, Control)> = every(root)
        .into_iter()
        .map(|one| (one.session, one.control))
        .collect();
    assert_eq!(
        left,
        [(
            4,
            Control::TrustClone {
                folder: "workspaces/ws/api".to_owned(),
                place: "github.com/acme/api".to_owned()
            }
        )]
    );
    project_ended(root);
}

#[test]
fn use_one_address_shows_the_recorded_address_or_the_fetch_one() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let root = plane.path();
    let clone = a_clone(root, "api", "https://github.com/acme/api.git");
    git(
        &clone,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            "https://github.com/acme/fork.git",
        ],
    );

    // Nothing recorded: the fetch address.
    let shown = one_address_for(home.path(), root, "workspaces/ws/api").unwrap();
    assert_eq!(
        shown,
        Control::OneAddress {
            folder: "workspaces/ws/api".to_owned(),
            fetch: "github.com/acme/api".to_owned(),
            push: "github.com/acme/fork".to_owned(),
            to: "github.com/acme/api".to_owned(),
        }
    );

    // One recorded for the clone: that address.
    crate::forge::gitcredential::remote::record_for_a_test(
        home.path(),
        &clone,
        "github.com/acme/saved",
    )
    .unwrap();
    let Control::OneAddress { to, .. } =
        one_address_for(home.path(), root, "workspaces/ws/api").unwrap()
    else {
        panic!("not a Use one address");
    };
    assert_eq!(to, "github.com/acme/saved");

    // An origin that already has one address needs no press.
    git(
        &clone,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            "https://github.com/acme/api.git",
        ],
    );
    assert_eq!(
        one_address_for(home.path(), root, "workspaces/ws/api"),
        None
    );
}
