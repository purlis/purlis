// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::path::PathBuf;

/// A plane charter would read, with the settings a workspace's layer is cut from.
fn plane() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = std::fs::canonicalize(dir.path()).expect("it resolves");
    std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("a manifest");
    std::fs::create_dir_all(root.join(".claude")).expect("a settings directory");
    std::fs::write(
        root.join(".claude/settings.json"),
        "{\"env\": {\"CHARTER_HARNESS\": \"claude-code\"}}\n",
    )
    .expect("settings");
    std::fs::create_dir_all(root.join("workspaces")).expect("a workspaces directory");
    (dir, root)
}

fn git(at: &Path, argv: &[&str]) {
    purlis_core::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(at)
            .args(argv),
    )
    .expect("git runs in a test");
}

/// A clone with one commit in it, where a workspace's clone goes.
fn clone_in(root: &Path, workspace: &str, name: &str) -> PathBuf {
    let at = root.join("workspaces").join(workspace).join(name);
    std::fs::create_dir_all(&at).expect("a clone directory");
    git(&at, &["init", "-q", "-b", "main", "."]);
    git(&at, &["config", "user.email", "t@e.invalid"]);
    git(&at, &["config", "user.name", "t"]);
    // **A fixture repository may not inherit the developer's signing configuration.** With
    // `commit.gpgsign = true` and an SSH signer in `~/.gitconfig` — an ordinary setup, and
    // the operator's — the commit below either parks on a biometric prompt or fails with
    // the agent's own error, and every test of this delete goes red for a reason that has
    // nothing to do with it. **CI cannot see it**: a runner has no signing config. charter's
    // Python suite hit exactly this and turned it off everywhere
    // (its 0.54.0 news note, in `diazoxide/charter-plane`); this
    // port carried the helper over without it.
    git(&at, &["config", "commit.gpgsign", "false"]);
    std::fs::write(at.join("README.md"), "one\n").expect("a file");
    git(&at, &["add", "-A"]);
    git(&at, &["commit", "-q", "-m", "one"]);
    at
}

#[test]
fn a_workspace_is_created_with_the_baseline_the_cli_gives_it() {
    let (_dir, root) = plane();

    let said = create_in(&root, "alpha", None, false).expect("a plain name is created");

    let ws = root.join("workspaces/alpha");
    assert!(ws.join("workspace.json").exists(), "{said:?}");
    assert!(ws.join("workspace.md").exists(), "{said:?}");
    assert!(ws.join("memory/MEMORY.md").exists(), "{said:?}");
    assert!(
        said.iter().any(|line| line.contains("Workspace 'alpha'")),
        "{said:?}"
    );
}

#[test]
fn the_name_is_the_cores_rule_and_the_refusal_is_the_cores_sentence() {
    // The whole point of going through `wscmd::create`: the window refuses exactly what a
    // terminal refuses, in the words a terminal uses. A name check written here would be a
    // second answer, and `../escape` is what a second answer gets wrong.
    let (_dir, root) = plane();

    for bad in ["../escape", "/absolute", ".hidden", "", "has space"] {
        let refused = create_in(&root, bad, None, false)
            .expect_err(&format!("'{bad}' is not a workspace name"));
        assert!(
            refused.contains("invalid workspace name"),
            "{bad}: {refused}"
        );
    }
    assert!(
        !root.parent().expect("a parent").join("escape").exists(),
        "nothing was written outside the plane"
    );
    assert!(!root.join("escape").exists());
}

#[test]
fn a_vision_is_recorded_and_an_empty_box_is_not_a_vision() {
    let (_dir, root) = plane();

    create_in(&root, "alpha", Some("  ship the thing  "), false).expect("created");
    create_in(&root, "beta", Some("   "), false).expect("created");
    create_in(&root, "gamma", None, false).expect("created");

    // Read back the way charter reads it, which answers "" for its own placeholder — so
    // "no vision" is the core's own judgement and not this test's reading of a file.
    let vision = |name: &str| {
        purlis_core::workspaces::Plane::open(&root)
            .workspace(name)
            .expect("a workspace")
            .vision()
            .trim()
            .to_owned()
    };
    assert_eq!(vision("alpha"), "ship the thing");
    assert_eq!(vision("beta"), "");
    assert_eq!(vision("gamma"), "");

    // **And `vision()` is not enough to catch this, which is why the file is read too.**
    // `set_vision("")` empties the section, and `vision()` answers "" for an empty section
    // exactly as it answers "" for charter's placeholder — so a window that recorded an
    // empty box would look identical through that door. What actually goes is the PROMPT:
    // the line in `workspace.md` that asks for the one thing a fork inherits. A workspace
    // made with an empty box has to be the workspace made with no vision at all.
    let charter_of = |name: &str| {
        std::fs::read_to_string(root.join("workspaces").join(name).join("workspace.md"))
            .expect("its charter")
    };
    let placeholder = &purlis_core::workspaces::vision_placeholder(&root);
    assert!(
        charter_of("beta").contains(placeholder),
        "{}",
        charter_of("beta")
    );
    assert!(charter_of("gamma").contains(placeholder));
    assert!(!charter_of("alpha").contains(placeholder));
}

#[test]
fn an_empty_workspace_is_deleted() {
    let (_dir, root) = plane();
    create_in(&root, "alpha", None, false).expect("created");

    let said = remove_in(&root, "alpha", false).expect("nothing is at risk in it");

    assert!(!root.join("workspaces/alpha").exists(), "{said:?}");
}

#[test]
fn a_dirty_clone_refuses_the_delete_and_nothing_is_removed() {
    // **The guard, through the command the window calls.** `wscmd::work_at_risk` is what
    // decides, and it decides inside `wscmd::remove`; this asserts the window's own path
    // reaches it, refuses, and leaves the directory where it was.
    let (_dir, root) = plane();
    let clone = clone_in(&root, "alpha", "svc");
    std::fs::write(clone.join("README.md"), "changed\n").expect("a change");

    let refused = remove_in(&root, "alpha", false).expect_err("a dirty clone is refused");

    assert!(
        refused.said.contains("this would discard work"),
        "{refused:?}"
    );
    assert!(
        refused.said.contains("svc: uncommitted changes"),
        "{refused:?}"
    );
    assert!(clone.exists(), "the guard fired, so nothing was deleted");
    assert!(clone.join("README.md").exists());
}

#[test]
fn unticking_a_dirty_repo_is_refused_with_the_list_it_was_refused_on() {
    // The picker's removal goes through the same guard, and the window gets the reading
    // that refused it, not a second one (charter-app#182).
    let (_dir, root) = plane();
    let clone = clone_in(&root, "alpha", "svc");
    std::fs::write(clone.join("README.md"), "changed\n").expect("a change");

    let refused = drop_in(&root, "alpha", "svc").expect_err("a dirty clone is refused");

    assert_eq!(refused.at_risk.len(), 1, "{refused:?}");
    assert_eq!(refused.at_risk[0].what, "svc");
    assert!(clone.join("README.md").exists());
}

#[test]
fn a_repo_not_cloned_here_leaves_the_workspace_and_a_cloned_one_is_refused() {
    let (_dir, root) = plane();
    let kept = clone_in(&root, "alpha", "kept");
    let workspace = purlis_core::workspaces::Plane::open(&root)
        .workspace("alpha")
        .expect("a workspace");
    workspace
        .write_manifest(&serde_json::json!({
            "name": "alpha",
            "repos": [{"name": "kept"}, {"name": "named"}],
        }))
        .expect("a manifest");

    membership_dropped(&root, "alpha", "named").expect("an uncloned repo is dropped");
    let refused = membership_dropped(&root, "alpha", "kept").expect_err("a clone is refused");

    assert!(refused.contains("is cloned"), "{refused}");
    assert!(kept.exists());
    let (doc, _) = workspace.manifest();
    assert_eq!(
        doc.expect("a manifest")["repos"],
        serde_json::json!([{"name": "kept"}])
    );
}

#[test]
fn unticking_a_clean_repo_removes_only_that_clone() {
    let (_dir, root) = plane();
    let gone = clone_in(&root, "alpha", "gone");
    let kept = clone_in(&root, "alpha", "kept");

    drop_in(&root, "alpha", "gone").expect("a clean clone is removed");

    assert!(!gone.exists());
    assert!(kept.exists());
}

#[test]
fn a_worktree_holding_commits_that_exist_nowhere_else_refuses_the_delete() {
    // The second half of the guard, and the half a clone walk cannot see (charter#91): a
    // clone's `.git` is a directory and a linked worktree's is a file, so a delete that
    // only checked clones took worktrees with it while reporting nothing.
    let (_dir, root) = plane();
    let clone = clone_in(&root, "alpha", "svc");
    let piece = root.join("workspaces/alpha/.worktrees/svc/task");
    git(
        &clone,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "task",
            &piece.display().to_string(),
        ],
    );
    std::fs::write(piece.join("work.md"), "x\n").expect("a file");
    git(&piece, &["add", "-A"]);
    git(&piece, &["commit", "-q", "-m", "unique"]);

    let refused = remove_in(&root, "alpha", false).expect_err("unique commits are refused");

    assert!(
        refused
            .said
            .contains("svc/task: 1 commit that exists nowhere else"),
        "{refused:?}"
    );
    assert!(piece.exists());
}

#[test]
fn a_clone_charter_cannot_read_refuses_the_delete() {
    // charter#917, and the sharpest instance of it: what the guard's list is empty of is
    // what is handed to a recursive delete. A `git status` that FAILED must never read as
    // a clean tree.
    let (_dir, root) = plane();
    let clone = root.join("workspaces/alpha/svc");
    std::fs::create_dir_all(clone.join(".git")).expect("a .git that is not a repository");

    let refused = remove_in(&root, "alpha", false).expect_err("an unreadable clone refuses");

    assert!(refused.said.contains("could not be read"), "{refused:?}");
    assert!(clone.exists());
}

#[test]
fn forcing_is_a_second_decision_and_it_goes_through() {
    let (_dir, root) = plane();
    let clone = clone_in(&root, "alpha", "svc");
    std::fs::write(clone.join("README.md"), "changed\n").expect("a change");
    remove_in(&root, "alpha", false).expect_err("refused first");

    remove_in(&root, "alpha", true).expect("the operator said to discard it");

    assert!(!root.join("workspaces/alpha").exists());
}

#[test]
fn what_the_dialog_previews_is_the_guards_own_list() {
    // The preview draws the core's sentences and writes none of its own — so a refusal the
    // operator reads on the dialog is word for word the refusal the delete will give.
    let (_dir, root) = plane();
    let clone = clone_in(&root, "alpha", "svc");
    std::fs::write(clone.join("README.md"), "changed\n").expect("a change");

    let shown = at_risk_in(&root, "alpha");
    let refused = remove_in(&root, "alpha", false).expect_err("refused");

    assert_eq!(shown.len(), 1, "{shown:?}");
    assert_eq!(shown[0].what, "svc");
    assert_eq!(shown[0].said, "svc: uncommitted changes");
    assert!(refused.said.contains(&shown[0].said), "{refused:?}");
}

/// **The refusal carries the list it was refused on, and the preview is not it**
/// (charter-app#182).
///
/// The workspace is made to move between the two reads, which is the whole failure: the
/// preview sees one clone at risk, a second goes dirty while the dialog is up, and the
/// core refuses over both. Before this, the only list the window had to name what it was
/// about to discard was the first one — so the force button named `svc` under a sentence
/// naming `svc` and `lib`, and the operator was reading two descriptions of one act that
/// did not agree.
#[test]
fn a_refusal_names_what_the_core_read_and_not_what_the_preview_read() {
    let (_dir, root) = plane();
    let svc = clone_in(&root, "alpha", "svc");
    let lib = clone_in(&root, "alpha", "lib");
    std::fs::write(svc.join("README.md"), "changed\n").expect("a change");

    let preview = at_risk_in(&root, "alpha");

    // The workspace moves while the dialog is up.
    std::fs::write(lib.join("README.md"), "changed too\n").expect("a second change");

    let refused = remove_in(&root, "alpha", false).expect_err("both clones are at risk now");

    assert_eq!(
        preview.iter().map(|r| &r.what).collect::<Vec<_>>(),
        vec!["svc"],
        "the preview is the older reading, and it stays older"
    );
    assert_eq!(
        refused.at_risk.iter().map(|r| &r.what).collect::<Vec<_>>(),
        vec!["lib", "svc"]
    );
    // The claim: every name the button would draw is a name the sentence above it uses.
    for risk in &refused.at_risk {
        assert!(refused.said.contains(&risk.said), "{refused:?}");
    }
}

#[test]
fn a_workspace_that_is_a_link_out_of_the_plane_is_refused_and_its_target_survives() {
    let (_dir, root) = plane();
    let outside = tempfile::tempdir().expect("a directory");
    std::fs::create_dir_all(outside.path().join("treasure")).expect("a directory");
    std::fs::write(outside.path().join("treasure/keep.txt"), "x").expect("a file");
    std::os::unix::fs::symlink(
        outside.path().join("treasure"),
        root.join("workspaces/alpha"),
    )
    .expect("a link where a workspace goes");

    let refused = remove_in(&root, "alpha", true).expect_err("a link out of the plane");

    assert!(
        refused
            .said
            .contains("does not resolve to a directory inside this plane"),
        "{refused:?}"
    );
    // Not the guard's refusal, so there is nothing to offer forcing past — and a window
    // that drew a force button here would be offering a way through that does not exist.
    assert!(refused.at_risk.is_empty(), "{refused:?}");
    assert!(
        outside.path().join("treasure/keep.txt").exists(),
        "--force is not a way out of the plane"
    );
}

#[test]
fn a_vision_added_later_is_the_one_the_cli_would_record_and_a_refusal_is_the_cores() {
    // #3344: the window's Add a vision, after the workspace was made without one.
    let (_dir, root) = plane();
    create_in(&root, "gamma", None, false).expect("created");

    let said = vision_in(&root, "gamma", "  keep the CI green  ").expect("recorded");
    assert_eq!(said, "gamma has a vision now.");
    let vision = purlis_core::workspaces::Plane::open(&root)
        .workspace("gamma")
        .expect("a workspace")
        .vision();
    assert_eq!(vision.trim(), "keep the CI green");

    // The core's refusals, in its own words.
    assert_eq!(
        vision_in(&root, "gamma", "   "),
        Err("a vision has words: nothing was written".to_owned())
    );
    assert!(vision_in(&root, "nope", "a goal").is_err());
}
