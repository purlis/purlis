// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn homes() -> (tempfile::TempDir, Homes) {
    let dir = tempfile::tempdir().unwrap();
    let homes = Homes::at(&dir.path().join("config"), &dir.path().join("data"));
    (dir, homes)
}

#[test]
fn a_refused_project_waits_once_until_a_sandbox_only_codex_profile_is_made() {
    let (_dir, homes) = homes();
    let root = Path::new("/projects/one-2037");
    waits_on(root);
    waits_on(root);
    let listed = || super::super::list(homes.config_root()).unwrap();
    assert_eq!(
        waiting(root, &listed()).map(|w| w.root),
        Some(root.to_path_buf())
    );

    let made = make(&homes, 100).unwrap();

    assert_eq!(made.record.name, NAME);
    assert!(made.record.sandbox_only);
    assert_eq!(made.record.harness, "codex");
    assert_eq!(waiting(root, &listed()), None);
    // A second one takes the next name.
    assert_eq!(make(&homes, 101).unwrap().record.name, "codex-sandbox-2");
}

#[test]
fn a_dismissed_wait_goes() {
    let root = Path::new("/projects/two-2037");
    waits_on(root);
    dismiss(root);
    assert_eq!(waiting(root, &[]), None);
}
