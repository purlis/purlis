// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A workspace's repo left on a chat's branch, or on a moved remote, against real git (#1985):
//! it is found once, its Fix refuses to lose work, and once fixed it is no longer found.

use crate::support;

use std::path::Path;

use purlis_core::staleclone::{self, Fix, Pushes, Stale};

/// The fixture's repo, given a remote of its own (a bare repository beside the plane) with
/// `origin/HEAD` set, as a clone from a forge has it.
fn with_a_remote(f: &support::Fixture) -> tempfile::TempDir {
    let remote = tempfile::tempdir().unwrap();
    let bare = remote.path().join("widget.git");
    support::git(
        remote.path(),
        &[
            "clone",
            "-q",
            "--bare",
            &f.clone.display().to_string(),
            "widget.git",
        ],
    );
    support::git(
        &f.clone,
        &["remote", "add", "origin", &bare.display().to_string()],
    );
    support::git(&f.clone, &["fetch", "-q", "origin"]);
    support::git(&f.clone, &["remote", "set-head", "origin", "main"]);
    remote
}

/// The branch the fixture's repo has checked out.
fn branch_of(at: &Path) -> String {
    String::from_utf8_lossy(&support::git(at, &["branch", "--show-current"]).stdout)
        .trim()
        .to_owned()
}

#[test]
fn a_repo_left_on_another_branch_is_found_and_switched_back_only_once_nothing_is_lost() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("widget");
    let _remote = with_a_remote(&f);
    // Another chat's branch, made with plain git and left checked out in the repo itself.
    support::git(&f.clone, &["switch", "-q", "-c", "chat-3"]);

    let found = staleclone::found(&f.plane).found;
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        found[0].stale,
        [Stale::OffDefault {
            branch: "chat-3".into(),
            default: "main".into(),
        }]
    );

    // A commit only this branch holds: nothing is switched.
    f.commit(&f.clone, "work");
    let refused = staleclone::fix(&f.plane, &f.ws, "widget", Fix::Branch, None).unwrap_err();
    assert!(refused.contains("1 commit only it holds"), "{refused}");
    assert_eq!(branch_of(&f.clone), "chat-3");

    // Pushed, but with a change in the tree: still nothing.
    support::git(&f.clone, &["push", "-q", "origin", "chat-3"]);
    std::fs::write(f.clone.join("draft.txt"), "not yet\n").unwrap();
    let refused = staleclone::fix(&f.plane, &f.ws, "widget", Fix::Branch, None).unwrap_err();
    assert!(refused.contains("has changes"), "{refused}");
    assert_eq!(branch_of(&f.clone), "chat-3");
    assert!(
        f.clone.join("draft.txt").exists(),
        "the change is left as it was"
    );

    // Clean and pushed: switched, the branch kept, and it is no longer found.
    std::fs::remove_file(f.clone.join("draft.txt")).unwrap();
    let fixed = staleclone::fix(&f.plane, &f.ws, "widget", Fix::Branch, None).expect("switched");
    assert_eq!(fixed.said, "widget is on main.");
    assert_eq!(branch_of(&f.clone), "main");
    support::git(
        &f.clone,
        &["rev-parse", "--verify", "-q", "refs/heads/chat-3"],
    );
    assert_eq!(staleclone::found(&f.plane).found, []);
}

#[test]
fn a_repo_the_forge_said_moved_is_found_and_pointed_at_the_new_address() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("widget");
    support::git(
        &f.clone,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/widget.git",
        ],
    );
    // What a fetch from `origin` said, as git words it.
    staleclone::heard(
        &f.clone,
        "warning: redirecting to https://github.com/purlis/widget.git/\n",
    );

    let found = staleclone::found(&f.plane).found;
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        found[0].stale,
        [Stale::RemoteMoved {
            origin: "https://github.com/acme/widget.git".into(),
            now: "https://github.com/purlis/widget.git".into(),
            pushes: Pushes::Origin,
        }]
    );

    // Only the address the window showed is set.
    let other = staleclone::fix(
        &f.plane,
        &f.ws,
        "widget",
        Fix::Remote,
        Some("https://github.com/purlis/other.git"),
    )
    .unwrap_err();
    assert!(
        other.contains("changed since the window showed it"),
        "{other}"
    );
    let shown = Some("https://github.com/purlis/widget.git");
    let fixed = staleclone::fix(&f.plane, &f.ws, "widget", Fix::Remote, shown).expect("set");
    assert_eq!(fixed.said, "widget fetches from github.com/purlis/widget.");
    let url = support::git(&f.clone, &["config", "--get", "remote.origin.url"]);
    assert_eq!(
        String::from_utf8_lossy(&url.stdout).trim(),
        "https://github.com/purlis/widget.git"
    );
    assert_eq!(staleclone::found(&f.plane).found, []);
    // A second press finds nothing to do.
    let again = staleclone::fix(&f.plane, &f.ws, "widget", Fix::Remote, shown).unwrap_err();
    assert!(again.contains("Nothing to fix"), "{again}");
}

#[test]
fn a_fetch_with_no_redirect_lets_a_kept_move_go() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("widget");
    support::git(
        &f.clone,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/widget.git",
        ],
    );
    staleclone::heard(
        &f.clone,
        "warning: redirecting to https://github.com/purlis/widget.git/\n",
    );
    assert_eq!(staleclone::found(&f.plane).found.len(), 1);

    staleclone::heard(&f.clone, "");

    assert_eq!(staleclone::found(&f.plane).found, []);
}

/// The fixture's repo with `origin` at the old address, a push URL for each of `pushes`, and
/// the forge's word that it moved.
fn moved_pushing_to(f: &support::Fixture, pushes: &[&str]) {
    support::git(
        &f.clone,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/widget.git",
        ],
    );
    for push in pushes {
        support::git(
            &f.clone,
            &["config", "--add", "remote.origin.pushurl", push],
        );
    }
    staleclone::heard(
        &f.clone,
        "warning: redirecting to https://github.com/purlis/widget.git/\n",
    );
}

/// Where the repo pushes, as git reads it: its push URLs, or `origin`'s own where it has none.
fn pushes_to(at: &Path) -> Vec<String> {
    let out = support::git(at, &["remote", "get-url", "--push", "--all", "origin"]);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn a_push_url_naming_the_old_place_moves_with_origin() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("widget");
    // origin's own address, as git reads it however it is written.
    moved_pushing_to(
        &f,
        &[
            "https://GitHub.com/acme/widget/",
            "https://github.com/acme/widget.git",
        ],
    );
    let found = staleclone::found(&f.plane).found;
    assert_eq!(
        found[0].stale,
        [Stale::RemoteMoved {
            origin: "https://github.com/acme/widget.git".into(),
            now: "https://github.com/purlis/widget.git".into(),
            pushes: Pushes::Follow,
        }]
    );

    let shown = Some("https://github.com/purlis/widget.git");
    let fixed = staleclone::fix(&f.plane, &f.ws, "widget", Fix::Remote, shown).expect("set");
    assert_eq!(
        fixed.said,
        "widget fetches from and pushes to github.com/purlis/widget."
    );
    assert_eq!(
        pushes_to(&f.clone),
        ["https://github.com/purlis/widget.git"]
    );
    assert_eq!(staleclone::found(&f.plane).found, []);
}

#[test]
fn a_push_url_naming_another_place_or_reaching_it_another_way_is_left_and_said() {
    purlis_core::unsteered!();
    for pushes in [
        // Another place beside the old one.
        &[
            "https://github.com/acme/widget.git",
            "https://gitlab.example/mirror/widget.git",
        ][..],
        // The old place over SSH: dropping it would push over HTTPS instead.
        &["git@github.com:acme/widget.git"][..],
        // The old place as another user: dropping it would push as someone else.
        &["https://git@github.com/acme/widget.git"][..],
    ] {
        let f = support::plane_with_clone("widget");
        moved_pushing_to(&f, pushes);
        let found = staleclone::found(&f.plane).found;
        assert_eq!(
            found[0].stale,
            [Stale::RemoteMoved {
                origin: "https://github.com/acme/widget.git".into(),
                now: "https://github.com/purlis/widget.git".into(),
                pushes: Pushes::Kept(pushes.iter().map(|&url| url.to_owned()).collect()),
            }],
            "{pushes:?}"
        );

        let shown = Some("https://github.com/purlis/widget.git");
        let fixed = staleclone::fix(&f.plane, &f.ws, "widget", Fix::Remote, shown).expect("set");
        assert_eq!(fixed.said, "widget fetches from github.com/purlis/widget.");
        assert_eq!(pushes_to(&f.clone), pushes, "{pushes:?}");
        let url = support::git(&f.clone, &["config", "--get", "remote.origin.url"]);
        assert_eq!(
            String::from_utf8_lossy(&url.stdout).trim(),
            "https://github.com/purlis/widget.git"
        );
    }
}
