// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A forced push after a rebase is read for CI files against the merge base with the default
//! branch**, not against the branch's old tip: the CI changes the default branch gained since the
//! branch was cut are on it already, and counting them held every rebased push. A fast-forward is
//! read as before, and a rebased branch's own CI change is held still.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use purlis_core::forge::Kind;
use purlis_core::heldact::{BaseObjects, PushCheck, Quarantine, Trees, check_push};
use purlis_core::mergehold::HeadSha;

use crate::support::git;

const WORKFLOW: &str = ".github/workflows/ci.yml";

fn write(root: &Path, file: &str, text: &str) {
    let at = root.join(file);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, text).unwrap();
}

fn commit(root: &Path, message: &str) -> HeadSha {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", message]);
    sha_of(root, "HEAD")
}

fn sha_of(root: &Path, rev: &str) -> HeadSha {
    let out = git(root, &["rev-parse", rev]);
    HeadSha::try_from(String::from_utf8(out.stdout).unwrap().trim()).unwrap()
}

struct LocalForge(PathBuf);

impl BaseObjects for LocalForge {
    fn fetch(&self, into: &Path, base: &HeadSha) -> Result<(), String> {
        git(
            into,
            &[
                "fetch",
                "--quiet",
                "--no-tags",
                &self.0.to_string_lossy(),
                base.as_str(),
            ],
        );
        Ok(())
    }
}

/// What `git push` sends: a thin pack of `tip` and what is not in `known`.
fn pack_of(from: &Path, tip: &HeadSha, known: &HeadSha) -> Vec<u8> {
    let mut command = crate::support::unsigned();
    command
        .current_dir(from)
        .args(["pack-objects", "--revs", "--thin", "--stdout", "-q"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let mut child = purlis_core::forklock::spawn(&mut command).unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("{}\n^{}\n", tip.as_str(), known.as_str()).as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    out.stdout
}

struct Scene {
    _dir: tempfile::TempDir,
    forge: PathBuf,
    /// The branch's tip on the forge.
    old: HeadSha,
    /// The default branch's tip on the forge, which holds a CI change the branch lacks.
    default_tip: HeadSha,
    chat: PathBuf,
}

/// The forge: `main` at one commit, branch `topic` one commit of its own on it (no CI file), and
/// `main` moved on by a commit that changes the workflow. The chat has both.
fn scene() -> Scene {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let forge = root.join("forge");
    std::fs::create_dir_all(&forge).unwrap();
    git(&forge, &["init", "-q", "-b", "main", "."]);
    git(&forge, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
    write(&forge, WORKFLOW, "name: ci\non: push\njobs: {}\n");
    write(&forge, "README.md", "one\n");
    commit(&forge, "main one");
    git(&forge, &["checkout", "-q", "-b", "topic"]);
    write(&forge, "src/a.txt", "a\n");
    let old = commit(&forge, "topic work");
    git(&forge, &["checkout", "-q", "main"]);
    write(
        &forge,
        WORKFLOW,
        "name: ci\non: [push, pull_request]\njobs: {}\n",
    );
    let default_tip = commit(&forge, "main changes ci");
    let chat = root.join("chat");
    git(
        &root,
        &[
            "clone",
            "-q",
            &forge.to_string_lossy(),
            &chat.to_string_lossy(),
        ],
    );
    Scene {
        _dir: dir,
        forge,
        old,
        default_tip,
        chat,
    }
}

/// The chat rebases `topic` onto `main`, and `edit` writes what its own commit changes.
fn rebased(s: &Scene, edit: impl Fn(&Path)) -> HeadSha {
    git(&s.chat, &["checkout", "-q", "-b", "topic", "main"]);
    edit(&s.chat);
    commit(&s.chat, "topic work, rebased")
}

/// The check of a push of `tip` over `old`, read as the app reads it: in a quarantine that holds
/// the old tip, the default branch's and the pack.
fn checked(s: &Scene, tip: &HeadSha) -> PushCheck {
    let parent = tempfile::tempdir().unwrap();
    let from = LocalForge(s.forge.clone());
    let quarantine = Quarantine::open(parent.path(), &s.old, &from).unwrap();
    quarantine.also_have(&from, &s.default_tip).unwrap();
    quarantine
        .take_in(pack_of(&s.chat, tip, &s.default_tip), tip, &s.old)
        .unwrap();
    let trees = quarantine.trees();
    let compare = trees
        .compare_base(&s.old, tip, &s.default_tip)
        .expect("a merge base");
    check_push(Kind::GitHub, None, &trees, &compare, tip)
}

#[test]
fn a_rebased_branch_whose_own_commits_touch_no_ci_file_is_not_held() {
    purlis_core::unsteered!();
    let s = scene();
    let tip = rebased(&s, |chat| write(chat, "src/a.txt", "a\n"));

    // Read against the old tip it would count the workflow change main gained.
    let against_the_old_tip = {
        let parent = tempfile::tempdir().unwrap();
        let from = LocalForge(s.forge.clone());
        let quarantine = Quarantine::open(parent.path(), &s.old, &from).unwrap();
        quarantine.also_have(&from, &s.default_tip).unwrap();
        quarantine
            .take_in(pack_of(&s.chat, &tip, &s.default_tip), &tip, &s.old)
            .unwrap();
        check_push(Kind::GitHub, None, &quarantine.trees(), &s.old, &tip)
    };
    assert!(
        matches!(against_the_old_tip, PushCheck::Hold(_)),
        "the bug this fixes: {against_the_old_tip:?}"
    );

    assert!(
        matches!(checked(&s, &tip), PushCheck::NoCiFiles),
        "{:?}",
        checked(&s, &tip)
    );
}

#[test]
fn a_rebased_branch_whose_own_commit_touches_a_ci_file_is_still_held() {
    purlis_core::unsteered!();
    let s = scene();
    let tip = rebased(&s, |chat| {
        write(chat, "src/a.txt", "a\n");
        write(
            chat,
            WORKFLOW,
            "name: ci\non: [push, pull_request]\njobs: { x: {} }\n",
        );
    });

    match checked(&s, &tip) {
        PushCheck::Hold(diff) => assert_eq!(diff.files, vec![WORKFLOW.to_owned()]),
        other => panic!("held, got {other:?}"),
    }
}

#[test]
fn a_fast_forward_is_read_against_the_branchs_old_tip_as_before() {
    purlis_core::unsteered!();
    let s = scene();
    // The chat adds a workflow commit on top of the forge's `topic`, with no rebase.
    git(&s.chat, &["fetch", "-q", "origin", "topic"]);
    git(&s.chat, &["checkout", "-q", "-b", "topic", "FETCH_HEAD"]);
    write(
        &s.chat,
        ".github/workflows/new.yml",
        "name: new\non: push\njobs: {}\n",
    );
    let tip = commit(&s.chat, "a workflow");

    let trees = {
        let parent = tempfile::tempdir().unwrap();
        let from = LocalForge(s.forge.clone());
        let quarantine = Quarantine::open(parent.path(), &s.old, &from).unwrap();
        quarantine.also_have(&from, &s.default_tip).unwrap();
        quarantine
            .take_in(pack_of(&s.chat, &tip, &s.old), &tip, &s.old)
            .unwrap();
        let compare = quarantine
            .trees()
            .compare_base(&s.old, &tip, &s.default_tip)
            .unwrap();
        assert_eq!(compare, s.old, "a fast-forward keeps the old tip");
        check_push(Kind::GitHub, None, &quarantine.trees(), &compare, &tip)
    };
    assert!(matches!(trees, PushCheck::Hold(_)), "{trees:?}");
}

#[test]
fn a_push_with_no_common_commit_with_the_default_branch_is_an_error_never_a_lesser_comparison() {
    purlis_core::unsteered!();
    let s = scene();
    git(&s.chat, &["checkout", "-q", "--orphan", "island"]);
    git(&s.chat, &["rm", "-rfq", "."]);
    write(&s.chat, "x.txt", "x\n");
    let tip = commit(&s.chat, "unrelated root");
    let parent = tempfile::tempdir().unwrap();
    let from = LocalForge(s.forge.clone());
    let quarantine = Quarantine::open(parent.path(), &s.old, &from).unwrap();
    quarantine.also_have(&from, &s.default_tip).unwrap();
    quarantine
        .take_in(pack_of(&s.chat, &tip, &s.default_tip), &tip, &s.old)
        .unwrap();

    assert!(
        quarantine
            .trees()
            .compare_base(&s.old, &tip, &s.default_tip)
            .is_err()
    );
}
