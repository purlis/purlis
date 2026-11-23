// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A thin pack built on an integration branch is taken in** (ADR 0128, amended 2026-11-23): a
//! task branch cut from `phase-1-proven` is a pack whose deltas are against that branch's
//! objects, which the quarantine holds only once the forge's advertised tips are fetched. The
//! pack is still verified, and the tree is still read against the default branch's tip, so a CI
//! file the task branch changes is held.

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

/// The forge, as a local repository. It advertises its branches, unless it is `silent`.
struct LocalForge {
    path: PathBuf,
    silent: bool,
}

impl BaseObjects for LocalForge {
    fn fetch(&self, into: &Path, base: &HeadSha) -> Result<(), String> {
        git(
            into,
            &[
                "fetch",
                "--quiet",
                "--no-tags",
                &self.path.to_string_lossy(),
                base.as_str(),
            ],
        );
        Ok(())
    }

    fn advertised(&self, _into: &Path) -> Option<Vec<String>> {
        if self.silent {
            return None;
        }
        let out = git(
            &self.path,
            &["for-each-ref", "--format=%(objectname)", "refs/heads"],
        );
        Some(
            String::from_utf8(out.stdout)
                .unwrap()
                .lines()
                .map(str::to_owned)
                .collect(),
        )
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
    chat: PathBuf,
    /// The default branch's tip.
    default_tip: HeadSha,
    /// `phase-1-proven`'s tip: a commit of its own on `main`, which edits `src/big.txt`.
    integration: HeadSha,
}

fn lines(n: usize) -> String {
    (0..n)
        .map(|i| format!("line {i} of a long file\n"))
        .collect()
}

fn scene() -> Scene {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let forge = root.join("forge");
    std::fs::create_dir_all(&forge).unwrap();
    git(&forge, &["init", "-q", "-b", "main", "."]);
    write(&forge, WORKFLOW, "name: ci\non: push\njobs: {}\n");
    write(&forge, "src/big.txt", &lines(300));
    let default_tip = commit(&forge, "main one");
    git(&forge, &["checkout", "-q", "-b", "phase-1-proven"]);
    write(
        &forge,
        "src/big.txt",
        &format!("{}integration\n", lines(300)),
    );
    let integration = commit(&forge, "integration work");
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
        chat,
        default_tip,
        integration,
    }
}

/// The chat's commit on `phase-1-proven`, after `edit`.
fn task_commit(s: &Scene, edit: impl Fn(&Path)) -> HeadSha {
    git(
        &s.chat,
        &["checkout", "-q", "-B", "task", s.integration.as_str()],
    );
    edit(&s.chat);
    commit(&s.chat, "task work")
}

/// What the gate does for a push that makes a branch: a quarantine on the default branch's tip,
/// the thin pack over the integration branch, and the tree read against the default branch.
fn through_the_route(s: &Scene, forge: &LocalForge, tip: &HeadSha) -> Result<PushCheck, String> {
    let parent = tempfile::tempdir().unwrap();
    let quarantine = Quarantine::open(parent.path(), &s.default_tip, forge)?;
    let pack = pack_of(&s.chat, tip, &s.integration);
    quarantine.take_in_known(&pack, tip, &s.default_tip, forge)?;
    Ok(check_push_since(
        Kind::GitHub,
        None,
        &quarantine.trees(),
        &s.default_tip,
        None,
        tip,
    ))
}

fn forge_of(s: &Scene, silent: bool) -> LocalForge {
    LocalForge {
        path: s.forge.clone(),
        silent,
    }
}

#[test]
fn a_thin_pack_on_an_integration_branch_is_taken_in_and_a_plain_change_is_clean() {
    purlis_core::unsteered!();
    let s = scene();
    let tip = task_commit(&s, |chat| {
        write(
            chat,
            "src/big.txt",
            &format!("{}integration\ntask\n", lines(300)),
        )
    });
    let checked = through_the_route(&s, &forge_of(&s, false), &tip).expect("it takes in");
    assert!(matches!(checked, PushCheck::NoCiFiles), "{checked:?}");
}

#[test]
fn a_thin_pack_on_an_integration_branch_is_refused_where_the_forge_advertises_nothing() {
    purlis_core::unsteered!();
    let s = scene();
    let tip = task_commit(&s, |chat| {
        write(
            chat,
            "src/big.txt",
            &format!("{}integration\ntask\n", lines(300)),
        )
    });
    let refused = through_the_route(&s, &forge_of(&s, true), &tip);
    assert!(refused.is_err(), "{refused:?}");
}

#[test]
fn a_ci_file_change_on_a_branch_cut_from_an_integration_branch_is_held() {
    purlis_core::unsteered!();
    let s = scene();
    let tip = task_commit(&s, |chat| {
        write(
            chat,
            "src/big.txt",
            &format!("{}integration\ntask\n", lines(300)),
        );
        write(
            chat,
            WORKFLOW,
            "name: ci\non: [push, pull_request]\njobs: { x: {} }\n",
        );
    });
    match through_the_route(&s, &forge_of(&s, false), &tip).expect("it takes in") {
        PushCheck::Hold(diff) => {
            assert_eq!(diff.files, vec![WORKFLOW.to_owned()]);
            assert_eq!(diff.against, s.default_tip);
        }
        other => panic!("held, got {other:?}"),
    }
}

#[test]
fn a_pack_whose_objects_are_missing_is_refused_though_the_forge_advertises_tips() {
    purlis_core::unsteered!();
    let s = scene();
    let tip = task_commit(&s, |chat| write(chat, "src/new.txt", "new\n"));
    // A pack of the tip alone, over a base the forge has no commit for: git cannot complete it.
    let parent = tempfile::tempdir().unwrap();
    let forge = forge_of(&s, false);
    let quarantine = Quarantine::open(parent.path(), &s.default_tip, &forge).unwrap();
    let pack = pack_of(&s.chat, &tip, &s.integration);
    // Truncated: git hashes the pack and refuses it, advertised tips or not.
    let cut = &pack[..pack.len() / 2];
    assert!(
        quarantine
            .take_in_known(cut, &tip, &s.default_tip, &forge)
            .is_err()
    );
}
