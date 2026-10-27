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
    let files: Vec<&str> = ids.iter().map(|one| one.file.as_str()).collect();
    assert_eq!(files, ["charter.toml", "charter.local.toml"]);
    for held in ids {
        let one = proposals.get(held.id).expect("it waits");
        assert_eq!(one.path, held.file);
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
    let one = proposals.get(ids[0].id).expect("it waits");
    assert_eq!(
        apply_in(&root, &one, None, &audit, &Mutex::new(()), From::Tab),
        Err(crate::proposals::Refusal::Stale)
    );
}

/// A workspace `alpha` whose manifest holds the old key, as purlis's own writer wrote it
/// (`stamped`) or as a hand did.
fn a_workspace_with_the_old_key(root: &Path, stamped: bool) -> String {
    std::fs::create_dir_all(root.join("workspaces/alpha")).expect("a workspace");
    let mut doc = serde_json::json!({
        "name": "alpha",
        "settings": {"harness_plugins": {"claude": {"a@b": true}}, "theme": {"color": "teal"}},
    });
    purlis_core::manifest::stamp(&mut doc, stamped, purlis_core::manifest::key_for(root));
    let text = purlis_core::pyjson::dumps_indent2(&doc);
    std::fs::write(root.join(MANIFEST), &text).expect("its manifest");
    text
}

const MANIFEST: &str = "workspaces/alpha/workspace.json";

#[test]
fn a_workspaces_old_key_gets_a_change_and_nothing_is_written_until_apply() {
    let (_dir, root) = project(&[("charter.toml", "[plane]\nname = \"p\"\n")]);
    let before = a_workspace_with_the_old_key(&root, true);
    let notice = notice_of(&root).expect("the notice");
    assert_eq!(notice.files, [MANIFEST]);
    assert!(notice.removable, "Remove reaches a workspace's manifest");
    let proposals = Proposals::default();
    let held = removal_held(&root, &proposals, 1).expect("held");
    assert_eq!(
        held.iter().map(|one| one.file.as_str()).collect::<Vec<_>>(),
        [MANIFEST]
    );
    assert_eq!(read(&root, MANIFEST), before, "only an Apply writes");
    assert_eq!(notice_of(&root), None, "Remove answers the notice");
    let one = proposals.get(held[0].id).expect("it waits");
    assert_eq!(one.session, BY_PURLIS);
    match crate::proposals::now_of(&root, &one) {
        crate::proposals::Now::Made { before: shown, after } => {
            assert_eq!(shown, before, "shown against the manifest as it is");
            assert!(!after.contains("harness_plugins"), "{after}");
        }
        other => panic!("the change is shown whole: {other:?}"),
    }
}

#[test]
fn after_apply_a_manifest_purlis_wrote_keeps_its_stamp_as_after_a_form_save() {
    let (_dir, root) = project(&[("charter.toml", "[plane]\nname = \"p\"\n")]);
    a_workspace_with_the_old_key(&root, true);
    let proposals = Proposals::default();
    let held = removal_held(&root, &proposals, 1).expect("held");
    let one = proposals.get(held[0].id).expect("it waits");
    let audit = |_: &purlis_core::proposed::Applied<'_>| Ok(());
    apply_in(&root, &one, None, &audit, &Mutex::new(()), From::Tab).expect("applied");
    let after = read(&root, MANIFEST);
    assert!(!after.contains("harness_plugins"), "{after}");
    assert!(after.contains("\"theme\""), "every other setting kept: {after}");
    assert_eq!(
        purlis_core::manifest::ownership(Some(&after)),
        purlis_core::manifest::Ownership::Charter,
        "still purlis's: {after}"
    );

    // The same as a form's save of that removal would have written.
    let (_other, twin) = project(&[("charter.toml", "[plane]\nname = \"p\"\n")]);
    let base = a_workspace_with_the_old_key(&twin, true);
    purlis_core::settings::workspace::save(
        &twin,
        "alpha",
        Some(&base),
        &[purlis_core::settings::Edit {
            path: vec![purlis_core::settings::Step::Key(
                "harness_plugins".to_owned(),
            )],
            value: None,
        }],
    )
    .expect("a form's save");
    assert_eq!(after, read(&twin, MANIFEST));
}

#[test]
fn after_apply_a_hand_written_manifest_stays_the_persons() {
    let (_dir, root) = project(&[("charter.toml", "[plane]\nname = \"p\"\n")]);
    a_workspace_with_the_old_key(&root, false);
    let proposals = Proposals::default();
    let held = removal_held(&root, &proposals, 1).expect("held");
    let one = proposals.get(held[0].id).expect("it waits");
    let audit = |_: &purlis_core::proposed::Applied<'_>| Ok(());
    apply_in(&root, &one, None, &audit, &Mutex::new(()), From::Tab).expect("applied");
    let after = read(&root, MANIFEST);
    assert!(!after.contains("harness_plugins"), "{after}");
    assert_eq!(
        purlis_core::manifest::ownership(Some(&after)),
        purlis_core::manifest::Ownership::Operator
    );
}

#[test]
fn a_manifest_that_moved_since_remove_is_stale_at_apply() {
    let (_dir, root) = project(&[("charter.toml", "[plane]\nname = \"p\"\n")]);
    a_workspace_with_the_old_key(&root, true);
    let proposals = Proposals::default();
    let held = removal_held(&root, &proposals, 1).expect("held");
    std::fs::write(root.join(MANIFEST), "{\"name\": \"alpha\"}\n").expect("moved");
    let one = proposals.get(held[0].id).expect("it waits");
    let audit = |_: &purlis_core::proposed::Applied<'_>| Ok(());
    assert_eq!(
        apply_in(&root, &one, None, &audit, &Mutex::new(()), From::Tab),
        Err(crate::proposals::Refusal::Stale)
    );
    assert_eq!(read(&root, MANIFEST), "{\"name\": \"alpha\"}\n");
}
