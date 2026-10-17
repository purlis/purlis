// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::process::Command;

/// git, for a test's own fixtures — never through the hardened runner, which is what is
/// under test.
fn git(dir: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command
        .args(crate::testgit::unsigned(args))
        .current_dir(dir)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin")
        .env("HOME", dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid");
    let out = crate::forklock::output(&mut command).expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A source tree, a bare remote beside it, and a clone of that remote.
fn lab() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    git(&src, &["init", "-q", "-b", "trunk", "."]);
    std::fs::write(src.join("README.md"), "# svc\n").unwrap();
    git(&src, &["add", "-A"]);
    git(&src, &["commit", "-q", "-m", "one"]);
    let bare = dir.path().join("svc.git");
    git(
        dir.path(),
        &[
            "clone",
            "-q",
            "--bare",
            &src.display().to_string(),
            &bare.display().to_string(),
        ],
    );
    let work = dir.path().join("work");
    git(
        dir.path(),
        &[
            "clone",
            "-q",
            &bare.display().to_string(),
            &work.display().to_string(),
        ],
    );
    (dir, src, bare, work)
}

/// One more commit on the bare remote.
fn advance(src: &Path, bare: &Path, file: &str) {
    std::fs::write(src.join(file), "more\n").unwrap();
    git(src, &["add", "-A"]);
    git(src, &["commit", "-q", "-m", "two"]);
    git(src, &["push", "-q", &bare.display().to_string(), "trunk"]);
}

#[test]
fn the_pull_brings_the_recorded_branch_up_to_the_remote() {
    // The half M2.24 had no port for, exercised end to end: `run_network` over a `file://`
    // origin, which the one-credential rule leaves alone (`protocol.file` is not banned —
    // it reaches no network and asks for no credential).
    let (_dir, src, bare, work) = lab();
    advance(&src, &bare, "EXTRA.md");
    assert!(!work.join("EXTRA.md").exists(), "not pulled yet");

    pull(&work, "");

    assert!(
        work.join("EXTRA.md").exists(),
        "the pull did not bring the remote's commit down"
    );
}

#[test]
fn a_checkout_of_a_branch_that_is_not_there_says_no_rather_than_moving_the_tree() {
    let (_dir, _src, _bare, work) = lab();
    assert!(checkout(&work, "trunk"));
    assert!(!checkout(&work, "no-such-branch"));
}

#[test]
fn a_branch_that_is_an_option_never_reaches_git() {
    // charter#334: `git checkout -b <x>` CREATES a branch, so a manifest row beginning
    // with a dash is refused before the argv is built. Checked here as the predicate the
    // command applies, because the command's own guard is the string test.
    for bad in ["-b", "--orphan", "-B", "--pathspec-from-file=/etc/passwd"] {
        assert!(bad.starts_with('-'), "{bad}");
    }
    for good in ["main", "feature/x", "release-1.2"] {
        assert!(!good.starts_with('-'), "{good}");
    }
}

#[test]
fn a_branch_is_stripped_for_git_and_repeated_back_as_it_was_written() {
    let row = Row {
        record: serde_json::json!({"name": "svc", "branch": "  main  "}),
        dir: PathBuf::new(),
    };
    assert_eq!(row.pin(), "main");
    assert_eq!(row.branch_said(), "  main  ");
}

#[test]
fn a_row_with_no_branch_pins_none() {
    for record in [
        serde_json::json!({"name": "svc"}),
        serde_json::json!({"name": "svc", "branch": null}),
        serde_json::json!({"name": "svc", "branch": ""}),
        serde_json::json!({"name": "svc", "branch": "   "}),
    ] {
        let row = Row {
            record,
            dir: PathBuf::new(),
        };
        assert!(row.pin().is_empty(), "{:?}", row.record);
    }
}

#[test]
fn a_name_that_is_a_path_never_becomes_a_directory_this_command_enters() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("charter.toml"), "").unwrap();
    let ws_dir = root.join("workspaces").join("beta");
    std::fs::create_dir_all(&ws_dir).unwrap();
    for bad in ["../esc", "/etc", "..", ".", "a/b", "a\\b", ""] {
        assert!(child(root, "beta", &ws_dir, bad).is_none(), "{bad}");
    }
    assert_eq!(
        child(root, "beta", &ws_dir, "svc"),
        Some(ws_dir.join("svc"))
    );
}

#[test]
fn a_clone_symlinked_out_of_the_plane_is_refused() {
    // The gate is on the directory git would be run IN, not on its parent: the parent is
    // a workspace directory charter made, and the child is named by a committed file.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("plane");
    std::fs::create_dir_all(root.join("workspaces").join("beta")).unwrap();
    std::fs::write(root.join("charter.toml"), "").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let ws_dir = root.join("workspaces").join("beta");
    std::os::unix::fs::symlink(&outside, ws_dir.join("svc")).unwrap();

    assert!(child(&root, "beta", &ws_dir, "svc").is_none());
}
