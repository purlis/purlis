// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Trusting a clone made outside purlis, against a plane of real clones (git runs locally, no
//! forge is asked) and a config home of its own.

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
fn a_clone(root: &Path, ws: &str, name: &str, url: &str) -> PathBuf {
    let clone = root.join("workspaces").join(ws).join(name);
    std::fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "-q"]);
    git(&clone, &["remote", "add", "origin", url]);
    clone
}

fn folders(listed: &[Unvouched]) -> Vec<&str> {
    listed.iter().map(|clone| clone.folder.as_str()).collect()
}

#[test]
fn a_clone_with_no_record_is_listed_with_its_folder_and_the_one_address_its_remote_names() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    a_clone(plane.path(), "ws", "api", "git@github.com:Acme/api.git");

    let listed = unvouched_clones(plane.path(), home.path());

    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].workspace, "ws");
    assert_eq!(listed[0].folder, "workspaces/ws/api");
    assert_eq!(listed[0].place.as_deref(), Some("github.com/acme/api"));
    assert_eq!(listed[0].places, ["github.com/acme/api"]);
}

#[test]
fn a_recorded_clone_and_one_on_no_known_forge_are_not_listed() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let cloned = a_clone(plane.path(), "ws", "api", "https://github.com/acme/api.git");
    a_clone(plane.path(), "ws", "local", "/somewhere/else.git");
    a_clone(
        plane.path(),
        "ws",
        "mine",
        "https://github.com/acme/mine.git",
    );
    super::super::remote::record(home.path(), &cloned, "github.com/acme/api".to_owned()).unwrap();

    let listed = unvouched_clones(plane.path(), home.path());

    assert_eq!(folders(&listed), ["workspaces/ws/mine"]);
}

#[test]
fn a_clone_whose_remote_names_two_addresses_is_listed_with_no_one_place() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let clone = a_clone(plane.path(), "ws", "api", "https://github.com/acme/api.git");
    git(
        &clone,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            "https://github.com/other/api.git",
        ],
    );

    let listed = unvouched_clones(plane.path(), home.path());

    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].place, None);
    assert_eq!(
        listed[0].places,
        ["github.com/acme/api", "github.com/other/api"]
    );
    let refused = vouch_clone(
        &HostScope::for_a_test(),
        home.path(),
        plane.path(),
        &clone,
        "github.com/acme/api",
    );
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.contains("Give origin one address")),
        "{refused:?}"
    );
}

#[test]
fn trusting_a_clone_at_the_address_shown_records_it_and_it_is_no_longer_listed() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let clone = a_clone(plane.path(), "ws", "api", "https://github.com/acme/api.git");

    let trusted = vouch_clone(
        &HostScope::for_a_test(),
        home.path(),
        plane.path(),
        &clone,
        "github.com/acme/api",
    );

    assert_eq!(trusted, Ok("github.com/acme/api".to_owned()));
    assert!(unvouched_clones(plane.path(), home.path()).is_empty());
    assert_eq!(
        super::super::read_push_remotes(home.path())
            .get(&crate::sandbox::local::clone_key(&clone))
            .map(String::as_str),
        Some("github.com/acme/api")
    );
    assert_eq!(
        vouch_clone(
            &HostScope::for_a_test(),
            home.path(),
            plane.path(),
            &clone,
            "github.com/acme/api",
        ),
        Ok("github.com/acme/api".to_owned()),
        "trusting it again answers the same"
    );
}

#[test]
fn a_remote_that_changed_after_it_was_shown_is_refused_and_nothing_is_recorded() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let clone = a_clone(plane.path(), "ws", "api", "https://github.com/acme/api.git");
    git(
        &clone,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/someone/else.git",
        ],
    );

    let refused = vouch_clone(
        &HostScope::for_a_test(),
        home.path(),
        plane.path(),
        &clone,
        "github.com/acme/api",
    );

    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.contains("changed since the window showed it")),
        "{refused:?}"
    );
    assert!(super::super::read_push_remotes(home.path()).is_empty());
}

#[test]
fn a_folder_that_is_no_workspace_clone_is_refused_and_nothing_is_recorded() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    a_clone(plane.path(), "ws", "api", "https://github.com/acme/api.git");
    let outside = tempfile::tempdir().unwrap();
    git(outside.path(), &["init", "-q"]);
    git(
        outside.path(),
        &["remote", "add", "origin", "https://github.com/acme/api.git"],
    );

    let refused = vouch_clone(
        &HostScope::for_a_test(),
        home.path(),
        plane.path(),
        outside.path(),
        "github.com/acme/api",
    );

    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.contains("not a clone in this project")),
        "{refused:?}"
    );
    assert!(super::super::read_push_remotes(home.path()).is_empty());
}

/// **Each clone says when it was made** (T1-42, R-337): its folder's creation time, in seconds
/// since 1970, where its disk keeps one, and `None` where it keeps none. Never the time it was
/// read, nor its last change.
#[test]
fn a_clone_with_no_record_says_when_its_folder_was_made() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let clone = a_clone(plane.path(), "ws", "api", "https://github.com/acme/api.git");
    let created = std::fs::metadata(&clone)
        .and_then(|meta| meta.created())
        .ok()
        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|since| since.as_secs());

    let listed = unvouched_clones(plane.path(), home.path());

    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].made, created);
}

/// Confirms `clone` for the forge's repo `path` (number 1), as `forge-repos.json` holds it.
fn confirmed_as(home: &Path, clone: &Path, path: &str) {
    use crate::forge::Kind;
    use crate::forge::accounts::{ForgeAccount, Method, State};
    use crate::forge::confirmed::{Candidate, confirm};
    let account = ForgeAccount {
        id: "01A".to_owned(),
        kind: Kind::GitHub,
        host: "github.com".to_owned(),
        login: "octocat".to_owned(),
        method: Method::Pat,
        state: State::SignedIn,
        scopes: Some(vec!["repo".to_owned()]),
        client_id: None,
    };
    let candidate = Candidate {
        kind: Kind::GitHub,
        host: "github.com".to_owned(),
        path: path.to_owned(),
        repo_id: 1,
    };
    confirm(
        &HostScope::for_a_test(),
        home,
        clone,
        &candidate,
        &account,
        "client",
    )
    .unwrap();
}

#[test]
fn a_trusted_clone_with_no_confirmed_repo_is_listed_and_a_confirmed_one_is_not() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let old = a_clone(plane.path(), "ws", "old", "https://github.com/acme/old.git");
    let done = a_clone(
        plane.path(),
        "ws",
        "done",
        "https://github.com/acme/done.git",
    );
    a_clone(
        plane.path(),
        "ws",
        "untrusted",
        "https://github.com/acme/u.git",
    );
    super::super::remote::record(home.path(), &old, "github.com/acme/old".to_owned()).unwrap();
    super::super::remote::record(home.path(), &done, "github.com/acme/done".to_owned()).unwrap();
    confirmed_as(home.path(), &done, "acme/done");

    let listed = unconfirmed_clones(plane.path(), home.path());

    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].folder, "workspaces/ws/old");
    assert_eq!(listed[0].place, "github.com/acme/old");
}

#[test]
fn a_clone_confirmed_at_one_address_and_trusted_again_at_another_is_listed() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let moved = a_clone(plane.path(), "ws", "moved", "https://github.com/acme/a.git");
    super::super::remote::record(home.path(), &moved, "github.com/acme/a".to_owned()).unwrap();
    confirmed_as(home.path(), &moved, "acme/a");
    assert!(unconfirmed_clones(plane.path(), home.path()).is_empty());

    super::super::remote::record(home.path(), &moved, "github.com/acme/b".to_owned()).unwrap();

    let listed = unconfirmed_clones(plane.path(), home.path());
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].place, "github.com/acme/b");
}
