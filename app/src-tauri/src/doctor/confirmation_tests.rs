// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0
//! The `forge confirmation` row's test, in a module of its own: it trusts a clone as the window
//! does (`vouch_clone`), which only the window's sources may call outside a test module.

use super::{DoctorStatus, unconfirmed_clones};

fn plane() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = std::fs::canonicalize(dir.path()).expect("it resolves");
    std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("a manifest");
    for d in ["personas", "inventory", "workspaces"] {
        std::fs::create_dir_all(root.join(d)).expect("a directory");
    }
    (dir, root)
}

#[test]
fn a_trusted_clone_that_is_not_confirmed_is_a_warning_naming_it_and_where_to_confirm() {
    let (_dir, root) = plane();
    let home = tempfile::tempdir().expect("a config home");
    let clone = root.join("workspaces/ws/api");
    std::fs::create_dir_all(&clone).expect("a clone folder");
    for args in [
        vec!["init", "-q"],
        vec!["remote", "add", "origin", "https://github.com/acme/api.git"],
    ] {
        let mut git = std::process::Command::new("git");
        git.arg("-C").arg(&clone).args(args);
        assert!(
            purlis_core::forklock::output(&mut git)
                .expect("git runs")
                .status
                .success()
        );
    }
    assert!(
        unconfirmed_clones(&root, home.path()).is_none(),
        "a clone nobody trusted is the clone Notice's to show, not this row's"
    );
    purlis_core::forge::gitcredential::vouch::vouch_clone(
        &purlis_core::forge::route::HostScope::for_a_test(),
        home.path(),
        &root,
        &clone,
        "github.com/acme/api",
    )
    .expect("trusted");

    let row = unconfirmed_clones(&root, home.path()).expect("a row");

    assert_eq!(row.status, DoctorStatus::Warn);
    assert!(row.detail.contains("workspaces/ws/api"), "{row:?}");
    assert!(row.hint.contains("Settings"), "{row:?}");
}
