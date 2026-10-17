// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::path::PathBuf;

fn plane() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    dir
}

fn git(at: &Path, argv: &[&str]) {
    let run = crate::testgit::run(at, argv);
    assert!(run.ok(), "git {argv:?}: {}", run.err);
}

/// A clone with an upstream, so the enforce-push guard has something real to read.
fn pushed_clone(plane: &Path, ws: &str, name: &str) -> PathBuf {
    let origin = plane.join("origins").join(name);
    std::fs::create_dir_all(&origin).unwrap();
    git(&origin, &["init", "-q", "--bare", "-b", "main"]);
    let seed = plane.join("seed").join(name);
    std::fs::create_dir_all(&seed).unwrap();
    git(&seed, &["init", "-q", "-b", "main"]);
    git(&seed, &["config", "user.email", "t@example.com"]);
    git(&seed, &["config", "user.name", "T"]);
    std::fs::write(seed.join("README.md"), "hi\n").unwrap();
    git(&seed, &["add", "-A"]);
    git(&seed, &["commit", "-qm", "first"]);
    git(
        &seed,
        &["remote", "add", "origin", &origin.display().to_string()],
    );
    git(&seed, &["push", "-q", "-u", "origin", "main"]);

    let clone = plane.join("workspaces").join(ws).join(name);
    std::fs::create_dir_all(clone.parent().unwrap()).unwrap();
    git(
        plane,
        &[
            "clone",
            "-q",
            &origin.display().to_string(),
            &clone.display().to_string(),
        ],
    );
    git(&clone, &["config", "user.email", "t@example.com"]);
    git(&clone, &["config", "user.name", "T"]);
    clone
}

fn run(root: &Path, ws: &str, force: bool) -> (u8, Vec<String>) {
    let mut said = Vec::new();
    let code = snapshot(
        &Request {
            root,
            ws,
            description: None,
            force,
            now: "2026-05-04T11:32:17Z".parse().unwrap(),
        },
        &mut |line: Say| said.push(line.to_string()),
    );
    (code, said)
}

fn manifest(root: &Path, ws: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(root.join("workspaces").join(ws).join("workspace.json")).unwrap(),
    )
    .unwrap()
}

#[test]
fn a_workspace_with_no_clones_has_nothing_to_snapshot() {
    let dir = plane();
    std::fs::create_dir_all(dir.path().join("workspaces/beta")).unwrap();
    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 1);
    assert_eq!(
        said,
        vec!["✗ workspace 'beta' has no repo clones to snapshot."]
    );
}

#[test]
fn a_pushed_clone_is_recorded_with_its_branch() {
    let dir = plane();
    pushed_clone(dir.path(), "beta", "svc");
    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 0, "{said:?}");
    let doc = manifest(dir.path(), "beta");
    assert_eq!(doc["name"], "beta");
    assert_eq!(doc["description"], "");
    assert_eq!(doc["repos"][0]["name"], "svc");
    assert_eq!(doc["repos"][0]["branch"], "main");
    assert_eq!(doc["updated_at"], "2026-05-04T11:32:17+00:00");
    assert!(
        doc.get(crate::manifest::key_for(dir.path())).is_some(),
        "charter stamps what it wrote"
    );
    assert!(said.iter().any(|l| l.contains("  svc @ main")), "{said:?}");
}

#[test]
fn unpushed_work_blocks_the_snapshot_and_force_records_it_anyway() {
    let dir = plane();
    let clone = pushed_clone(dir.path(), "beta", "svc");
    std::fs::write(clone.join("second.md"), "x\n").unwrap();
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-qm", "second"]);

    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 2, "{said:?}");
    assert!(said[0].contains("Refusing to snapshot 'beta'"), "{said:?}");
    assert!(said[1].contains("svc: 1 unpushed commit"), "{said:?}");
    assert!(
        !dir.path().join("workspaces/beta/workspace.json").exists(),
        "a refused snapshot records nothing"
    );

    let (code, said) = run(dir.path(), "beta", true);
    assert_eq!(code, 0, "{said:?}");
    assert_eq!(manifest(dir.path(), "beta")["repos"][0]["branch"], "main");
}

#[test]
fn a_branch_with_no_upstream_blocks_because_restore_could_not_check_it_out() {
    let dir = plane();
    let clone = pushed_clone(dir.path(), "beta", "svc");
    git(&clone, &["checkout", "-q", "-b", "local-only"]);

    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 2, "{said:?}");
    assert!(
        said[1].contains("svc: branch 'local-only' isn't pushed to a remote"),
        "{said:?}"
    );
}

#[test]
fn uncommitted_work_blocks_it() {
    let dir = plane();
    let clone = pushed_clone(dir.path(), "beta", "svc");
    std::fs::write(clone.join("README.md"), "changed\n").unwrap();

    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 2, "{said:?}");
    assert!(said[1].contains("svc: uncommitted changes"), "{said:?}");
}

#[test]
fn a_clone_charter_could_not_read_blocks_it_rather_than_contributing_nothing() {
    // charter#917: the manifest would otherwise assert a state nobody measured.
    let dir = plane();
    pushed_clone(dir.path(), "beta", "svc");
    let broken = dir.path().join("workspaces/beta/other");
    std::fs::create_dir_all(broken.join(".git")).unwrap();

    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 2, "{said:?}");
    assert!(
        said.iter().any(|l| l.contains("other: could not be read")),
        "{said:?}"
    );
}

#[test]
fn a_description_is_set_where_one_is_given_and_kept_where_it_is_not() {
    let dir = plane();
    pushed_clone(dir.path(), "beta", "svc");
    let mut said = Vec::new();
    let code = snapshot(
        &Request {
            root: dir.path(),
            ws: "beta",
            description: Some("the billing work"),
            force: false,
            now: "2026-05-04T11:32:17Z".parse().unwrap(),
        },
        &mut |line: Say| said.push(line.to_string()),
    );
    assert_eq!(code, 0, "{said:?}");
    assert_eq!(
        manifest(dir.path(), "beta")["description"],
        "the billing work"
    );

    let (code, _said) = run(dir.path(), "beta", false);
    assert_eq!(code, 0);
    assert_eq!(
        manifest(dir.path(), "beta")["description"],
        "the billing work",
        "a snapshot with no --description keeps the one that is there"
    );
}

#[test]
fn a_manifest_that_is_not_an_object_is_replaced_rather_than_crashed_over() {
    let dir = plane();
    pushed_clone(dir.path(), "beta", "svc");
    std::fs::write(
        dir.path().join("workspaces/beta/workspace.json"),
        "[\"not a manifest\"]",
    )
    .unwrap();
    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 0, "{said:?}");
    assert_eq!(manifest(dir.path(), "beta")["name"], "beta");
}
