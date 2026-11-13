// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Clones purlis has no record of are adopted only through one ask, never automatically**
//! (R-336, Q184): however many there are, and however often the project's clones are read, the
//! project has one Trust these clones, and the record of clones changes only on the person's
//! answer.
//!
//! Driven as the window drives it: the project's clones are read
//! ([`windowonly::look_at_clones`]), the ask is listed ([`windowonly::clones_ask`]), and the
//! person's Trust is the clone Notice's own press ([`windowonly::trust_this_clone`]). Real clones
//! (git runs locally, no forge is asked) and a config home of the test's own.

use crate::support;

use std::path::{Path, PathBuf};

use purlis_core::forge::gitcredential::vouch;
use purlis_core::forge::route::HostScope;
use purlis_core::windowonly;

/// A plane with workspace `alpha`.
fn plane() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    for sub in ["personas", "inventory", "workspaces/alpha/repos"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    (dir, root)
}

/// A clone `name` in workspace `alpha`'s `repos/`, made in the person's own terminal: its
/// `origin` on github.com, and no record of it anywhere.
fn made_outside(root: &Path, name: &str) -> PathBuf {
    let clone = root.join("workspaces/alpha/repos").join(name);
    std::fs::create_dir_all(&clone).unwrap();
    support::git(&clone, &["init", "-q", "-b", "main", "."]);
    support::git(
        &clone,
        &[
            "remote",
            "add",
            "origin",
            &format!("https://github.com/acme/{name}.git"),
        ],
    );
    clone
}

/// Every file in the config home, with its bytes: the record of clones is one of them.
fn record_of_clones(home: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push((path.clone(), std::fs::read(&path).unwrap_or_default()));
            }
        }
    }
    let mut out = Vec::new();
    walk(home, &mut out);
    out.sort();
    out
}

fn folders(ask: &windowonly::TrustTheseClones) -> Vec<&str> {
    ask.clones
        .iter()
        .map(|clone| clone.folder.as_str())
        .collect()
}

#[test]
fn unrecorded_clones_raise_one_ask_and_nothing_is_trusted_before_it() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    let home = tempfile::tempdir().unwrap();
    let api = made_outside(&root, "api");
    let web = made_outside(&root, "web");
    let before = record_of_clones(home.path());

    // The window reads the project's clones as it opens, as it comes back into focus and as git
    // moves: each read is the same one ask, never a second.
    let seen = windowonly::look_at_clones(&root, home.path());
    windowonly::look_at_clones(&root, home.path());

    assert_eq!(seen.len(), 2, "{seen:?}");
    let ask = windowonly::clones_ask(&root).expect("unrecorded clones raise an ask");
    assert_eq!(
        folders(&ask),
        ["workspaces/alpha/repos/api", "workspaces/alpha/repos/web"],
        "one ask lists both clones"
    );
    assert_eq!(
        ask.clones
            .iter()
            .map(|clone| clone.place.as_deref())
            .collect::<Vec<_>>(),
        [Some("github.com/acme/api"), Some("github.com/acme/web")],
        "each with the one address its remote names"
    );
    // Nothing is trusted before the answer: the record of clones is as it was.
    assert!(!vouch::trusted(home.path(), &api));
    assert!(!vouch::trusted(home.path(), &web));
    assert_eq!(record_of_clones(home.path()), before);

    // The person's Trust, on each clone the ask lists.
    for clone in &ask.clones {
        windowonly::trust_this_clone(
            &HostScope::for_a_test(),
            home.path(),
            &root,
            &clone.folder,
            clone.place.as_deref().expect("one address"),
        )
        .expect("the person's Trust records the clone");
    }

    assert!(vouch::trusted(home.path(), &api));
    assert!(vouch::trusted(home.path(), &web));
    assert_eq!(
        windowonly::clones_ask(&root),
        None,
        "the ask goes once every clone it listed is trusted"
    );
    windowonly::project_ended(&root);
}

#[test]
fn a_workspace_with_a_recorded_clone_still_asks_for_its_unrecorded_one() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    let home = tempfile::tempdir().unwrap();
    let api = made_outside(&root, "api");
    made_outside(&root, "web");
    vouch::vouch_clone(
        &HostScope::for_a_test(),
        home.path(),
        &root,
        &api,
        "github.com/acme/api",
    )
    .expect("api is trusted");

    windowonly::look_at_clones(&root, home.path());

    let ask = windowonly::clones_ask(&root).expect("web still raises the ask");
    assert_eq!(folders(&ask), ["workspaces/alpha/repos/web"]);
    windowonly::project_ended(&root);
}
