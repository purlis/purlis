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
    waits_on(root, Harness::Codex);
    waits_on(root, Harness::Codex);
    let listed = || super::super::list(homes.config_root()).unwrap();
    assert_eq!(
        waiting(root, &listed())
            .into_iter()
            .map(|w| w.root)
            .collect::<Vec<_>>(),
        vec![root.to_path_buf()]
    );

    let made = make(&homes, Harness::Codex, 100).unwrap();

    assert_eq!(made.record.name, "codex-sandbox");
    assert!(made.record.sandbox_only);
    assert_eq!(made.record.harness, "codex");
    assert_eq!(waiting(root, &listed()), vec![]);
    // A second one takes the next name.
    assert_eq!(
        make(&homes, Harness::Codex, 101).unwrap().record.name,
        "codex-sandbox-2"
    );
}

#[test]
fn a_claude_code_wait_goes_only_with_a_sandbox_only_claude_code_profile() {
    let (_dir, homes) = homes();
    let root = Path::new("/projects/three-2677");
    waits_on(root, Harness::ClaudeCode);
    waits_on(root, Harness::Codex);
    let listed = || super::super::list(homes.config_root()).unwrap();

    make(&homes, Harness::Codex, 100).unwrap();
    let left = waiting(root, &listed());
    assert_eq!(
        left.iter().map(|w| w.harness).collect::<Vec<_>>(),
        vec![Harness::ClaudeCode]
    );
    assert_eq!(
        ask(Harness::ClaudeCode),
        "Claude Code needs a sandbox-only profile here"
    );

    let made = make(&homes, Harness::ClaudeCode, 101).unwrap();
    assert_eq!(made.record.name, "claude-sandbox");
    assert!(made.record.sandbox_only);
    assert_eq!(made.record.harness, "claude");
    assert_eq!(waiting(root, &listed()), vec![]);
}

#[test]
fn a_dismissed_wait_goes_for_its_harness_alone() {
    let root = Path::new("/projects/two-2037");
    waits_on(root, Harness::Codex);
    waits_on(root, Harness::ClaudeCode);
    dismiss(root, Harness::Codex);
    assert_eq!(
        waiting(root, &[])
            .into_iter()
            .map(|w| w.harness)
            .collect::<Vec<_>>(),
        vec![Harness::ClaudeCode]
    );
    dismiss(root, Harness::ClaudeCode);
    assert_eq!(waiting(root, &[]), vec![]);
}
