// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The old `[harness_plugins]` keys (#3467): said once, and removed only by an Apply.

use std::sync::Mutex;

use super::*;
use crate::proposals::{From, apply_in};

const SHARED: &str = "[plane]\nname = \"p\"\n\n[harness_plugins.claude]\n\"figma@x\" = true\n";
const LOCAL: &str = "harness_plugins.codex.\"a@b\" = false\n# mine\n[memory]\nshare = true\n";

fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a folder");
    let root = dir.path().canonicalize().expect("real");
    for (name, text) in files {
        std::fs::write(root.join(name), text).expect("written");
    }
    (dir, root)
}

fn read(root: &Path, name: &str) -> String {
    std::fs::read_to_string(root.join(name)).expect("the file")
}

#[test]
fn a_project_without_the_old_keys_has_no_notice() {
    let (_dir, root) = project(&[("charter.toml", "[plane]\nname = \"p\"\n")]);
    assert_eq!(notice_of(&root), None);
}

#[test]
fn a_project_with_the_old_keys_is_told_once_and_dismiss_keeps_it_away() {
    let (_dir, root) = project(&[("charter.toml", SHARED)]);
    let notice = notice_of(&root).expect("the notice");
    assert_eq!(notice.files, ["charter.toml"]);
    assert!(notice.removable);
    assert!(notice.said.contains("is ignored"), "{}", notice.said);
    answer(&root).expect("kept");
    assert_eq!(notice_of(&root), None, "said once");
    assert_eq!(read(&root, "charter.toml"), SHARED, "nothing rewritten");
}

#[test]
fn remove_holds_a_change_per_file_and_writes_nothing_until_it_is_applied() {
    let (_dir, root) = project(&[("charter.toml", SHARED), ("charter.local.toml", LOCAL)]);
    let proposals = Proposals::default();
    let ids = removal_held(&root, &proposals, 7).expect("held");
    assert_eq!(ids.len(), 2);
    assert_eq!(read(&root, "charter.toml"), SHARED, "only an Apply writes");
    assert_eq!(read(&root, "charter.local.toml"), LOCAL);
    assert_eq!(notice_of(&root), None, "Remove answers the notice");

    let audit = |_: &purlis_core::proposed::Applied<'_>| Ok(());
    for id in ids {
        let one = proposals.get(id).expect("it waits");
        assert_eq!(one.session, BY_PURLIS, "no chat's");
        apply_in(&root, &one, None, &audit, &Mutex::new(()), From::Tab).expect("applied");
    }
    assert_eq!(read(&root, "charter.toml"), "[plane]\nname = \"p\"\n");
    assert_eq!(
        read(&root, "charter.local.toml"),
        "# mine\n[memory]\nshare = true\n"
    );
}

#[test]
fn a_change_purlis_proposes_is_stale_once_the_file_moves() {
    let (_dir, root) = project(&[("charter.toml", SHARED)]);
    let proposals = Proposals::default();
    let ids = removal_held(&root, &proposals, 7).expect("held");
    std::fs::write(root.join("charter.toml"), "[plane]\nname = \"q\"\n").expect("moved");
    let audit = |_: &purlis_core::proposed::Applied<'_>| Ok(());
    let one = proposals.get(ids[0]).expect("it waits");
    assert_eq!(
        apply_in(&root, &one, None, &audit, &Mutex::new(()), From::Tab),
        Err(crate::proposals::Refusal::Stale)
    );
}

#[test]
fn a_workspaces_old_settings_are_named_and_given_no_change() {
    let (_dir, root) = project(&[("charter.toml", "[plane]\nname = \"p\"\n")]);
    std::fs::create_dir_all(root.join("workspaces/alpha")).expect("a workspace");
    std::fs::write(
        root.join("workspaces/alpha/workspace.json"),
        r#"{"name": "alpha", "settings": {"harness_plugins": {"claude": {"a@b": true}}}}"#,
    )
    .expect("its manifest");
    let notice = notice_of(&root).expect("the notice");
    assert_eq!(notice.files, ["workspaces/alpha/workspace.json"]);
    assert!(!notice.removable);
    let proposals = Proposals::default();
    assert_eq!(removal_held(&root, &proposals, 1), Ok(Vec::new()));
}
