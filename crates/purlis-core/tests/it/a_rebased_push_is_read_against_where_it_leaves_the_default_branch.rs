// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A push to a branch is read for CI files against the default branch's tip now**, less what the
//! branch had on the forge already. A rebase onto the current default branch brings its CI files
//! in and is clean; a tip built on an older commit, whose CI files are not the default branch's,
//! is held, as is a new branch cut from one; and a file the push leaves as the branch's old tip
//! had it is not the push's change. Merging with the merge base let a stale base's CI files
//! through, and the held act and the gate now read the one way.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use purlis_core::forge::Kind;
use purlis_core::heldact::{BaseObjects, PushCheck, Quarantine, check_push_since};
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
    /// `topic`'s tip on the forge: one commit of its own on the OLD main, with the old workflow.
    topic: HeadSha,
    /// `fresh`'s tip on the forge: one commit of its own on the CURRENT main.
    fresh: HeadSha,
    /// The commit `topic` was cut from, with the old workflow.
    old_main: HeadSha,
    /// The default branch's tip, whose workflow changed since.
    default_tip: HeadSha,
    chat: PathBuf,
}

/// The forge: `main` at a commit with a workflow, `topic` a commit of its own on it, and `main`
/// moved on by a commit that changes the workflow, from which `fresh` is cut. The chat has all.
fn scene() -> Scene {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let forge = root.join("forge");
    std::fs::create_dir_all(&forge).unwrap();
    git(&forge, &["init", "-q", "-b", "main", "."]);
    git(&forge, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
    write(&forge, WORKFLOW, "name: ci\non: push\njobs: {}\n");
    write(&forge, "README.md", "one\n");
    let old_main = commit(&forge, "main one");
    git(&forge, &["checkout", "-q", "-b", "topic"]);
    write(&forge, "src/a.txt", "a\n");
    let topic = commit(&forge, "topic work");
    git(&forge, &["checkout", "-q", "main"]);
    write(
        &forge,
        WORKFLOW,
        "name: ci\non: [push, pull_request]\njobs: {}\n",
    );
    let default_tip = commit(&forge, "main changes ci");
    git(&forge, &["checkout", "-q", "-b", "fresh"]);
    write(&forge, "src/f.txt", "f\n");
    let fresh = commit(&forge, "fresh work");
    git(&forge, &["checkout", "-q", "main"]);
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
        topic,
        fresh,
        old_main,
        default_tip,
        chat,
    }
}

/// The chat's commit on `parent`, after `edit`.
fn commit_on(s: &Scene, parent: &str, edit: impl Fn(&Path)) -> HeadSha {
    git(&s.chat, &["checkout", "-q", "-B", "work", parent]);
    edit(&s.chat);
    commit(&s.chat, "chat work")
}

/// The check of a push of `tip` over the branch's `old` tip on the forge, read as the gate reads
/// it: in a quarantine that holds the old tip, the default branch's and the pack.
fn over(s: &Scene, old: &HeadSha, tip: &HeadSha) -> PushCheck {
    let parent = tempfile::tempdir().unwrap();
    let from = LocalForge(s.forge.clone());
    let quarantine = Quarantine::open(parent.path(), old, &from).unwrap();
    quarantine.also_have(&from, &s.default_tip).unwrap();
    quarantine
        .take_in(pack_of(&s.chat, tip, &s.default_tip), tip, old)
        .unwrap();
    check_push_since(
        Kind::GitHub,
        None,
        &quarantine.trees(),
        &s.default_tip,
        Some(old),
        tip,
    )
}

/// The check of a push that makes a branch: read against the commit it is cut from, the default
/// branch's tip.
fn creating(s: &Scene, tip: &HeadSha) -> PushCheck {
    let parent = tempfile::tempdir().unwrap();
    let from = LocalForge(s.forge.clone());
    let quarantine = Quarantine::open(parent.path(), &s.default_tip, &from).unwrap();
    quarantine
        .take_in(pack_of(&s.chat, tip, &s.default_tip), tip, &s.default_tip)
        .unwrap();
    check_push_since(
        Kind::GitHub,
        None,
        &quarantine.trees(),
        &s.default_tip,
        None,
        tip,
    )
}

#[test]
fn a_plain_rebase_onto_the_current_default_branch_is_clean() {
    purlis_core::unsteered!();
    let s = scene();
    let tip = commit_on(&s, "main", |chat| write(chat, "src/a.txt", "a\n"));
    let checked = over(&s, &s.topic, &tip);
    assert!(matches!(checked, PushCheck::NoCiFiles), "{checked:?}");
}

#[test]
fn a_rebased_branchs_own_ci_change_is_held() {
    purlis_core::unsteered!();
    let s = scene();
    let tip = commit_on(&s, "main", |chat| {
        write(chat, "src/a.txt", "a\n");
        write(
            chat,
            WORKFLOW,
            "name: ci\non: [push, pull_request]\njobs: { x: {} }\n",
        );
    });
    match over(&s, &s.topic, &tip) {
        PushCheck::Hold(diff) => {
            assert_eq!(diff.files, vec![WORKFLOW.to_owned()]);
            assert_eq!(diff.against, s.default_tip);
        }
        other => panic!("held, got {other:?}"),
    }
}

#[test]
fn a_tip_built_on_a_stale_base_whose_ci_files_differ_from_the_default_branchs_is_held() {
    purlis_core::unsteered!();
    let s = scene();
    // `fresh` on the forge has the current workflow; the chat pushes over it a tip built on the old
    // main, with that main's workflow: no commit of the chat's touches it, yet it is not main's.
    let tip = commit_on(&s, s.old_main.as_str(), |chat| {
        write(chat, "src/s.txt", "s\n")
    });
    match over(&s, &s.fresh, &tip) {
        PushCheck::Hold(diff) => assert_eq!(diff.files, vec![WORKFLOW.to_owned()]),
        other => panic!("held, got {other:?}"),
    }
}

#[test]
fn a_ci_file_the_push_leaves_as_the_branchs_old_tip_had_it_is_not_the_pushs_change() {
    purlis_core::unsteered!();
    let s = scene();
    // `topic` was on the forge with the old workflow; a push that only adds work to it keeps that.
    let tip = commit_on(&s, s.topic.as_str(), |chat| {
        write(chat, "src/more.txt", "m\n")
    });
    let checked = over(&s, &s.topic, &tip);
    assert!(matches!(checked, PushCheck::NoCiFiles), "{checked:?}");
}

#[test]
fn a_fast_forward_that_adds_a_workflow_is_held() {
    purlis_core::unsteered!();
    let s = scene();
    let tip = commit_on(&s, s.fresh.as_str(), |chat| {
        write(
            chat,
            ".github/workflows/new.yml",
            "name: new\non: push\njobs: {}\n",
        )
    });
    match over(&s, &s.fresh, &tip) {
        PushCheck::Hold(diff) => {
            assert_eq!(diff.files, vec![".github/workflows/new.yml".to_owned()]);
        }
        other => panic!("held, got {other:?}"),
    }
}

#[test]
fn a_new_branch_cut_from_an_old_main_with_its_old_ci_files_is_held() {
    purlis_core::unsteered!();
    let s = scene();
    let stale = commit_on(&s, s.old_main.as_str(), |chat| {
        write(chat, "src/s.txt", "s\n")
    });
    match creating(&s, &stale) {
        PushCheck::Hold(diff) => assert_eq!(diff.files, vec![WORKFLOW.to_owned()]),
        other => panic!("held, got {other:?}"),
    }
    // Cut from the current main it is clean.
    let current = commit_on(&s, "main", |chat| write(chat, "src/c.txt", "c\n"));
    let checked = creating(&s, &current);
    assert!(matches!(checked, PushCheck::NoCiFiles), "{checked:?}");
}
