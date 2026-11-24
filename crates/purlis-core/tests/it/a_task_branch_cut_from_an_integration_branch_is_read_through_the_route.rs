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
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// The forge, as a local repository. It advertises its branches, unless it is `silent`, and counts
/// how often it was asked to list or to fetch every tip.
struct LocalForge {
    path: PathBuf,
    silent: bool,
    listed: AtomicUsize,
}

impl LocalForge {
    fn asked_to_list(&self) -> usize {
        self.listed.load(Ordering::SeqCst)
    }
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
        self.listed.fetch_add(1, Ordering::SeqCst);
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
    /// `workflow-edit`'s tip: a commit of its own on `main`, which edits the workflow.
    workflow_edit: HeadSha,
    /// A commit the forge resolves by id but no branch of it holds: one that was on a branch since
    /// force-pushed away.
    dangling: HeadSha,
    /// A commit of a fork: its objects are in the forge's store, reached by no ref of it.
    fork_only: HeadSha,
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
    // A forge serves a commit by its id, whether or not a branch is at it (GitHub does).
    git(&forge, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
    git(&forge, &["checkout", "-q", "-b", "workflow-edit"]);
    write(
        &forge,
        WORKFLOW,
        "name: ci\non: push\njobs: { edited: {} }\n",
    );
    let workflow_edit = commit(&forge, "edit the workflow");
    git(&forge, &["checkout", "-q", "main"]);
    // A commit on a branch that is then deleted: still in the store, in no ref.
    git(&forge, &["checkout", "-q", "-b", "gone"]);
    write(&forge, "src/secret.txt", "a secret\n");
    let dangling = commit(&forge, "a secret, force-pushed away");
    git(&forge, &["checkout", "-q", "main"]);
    git(&forge, &["branch", "-q", "-D", "gone"]);
    // A fork's commit, fetched by id with no ref made for it.
    let fork = root.join("fork");
    git(&root, &["clone", "-q", &forge.to_string_lossy(), "fork"]);
    write(&fork, "src/fork.txt", "the fork's\n");
    let fork_only = commit(&fork, "a commit only the fork has");
    git(
        &forge,
        &[
            "fetch",
            "-q",
            "--no-tags",
            &fork.to_string_lossy(),
            fork_only.as_str(),
        ],
    );
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
        workflow_edit,
        dangling,
        fork_only,
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
        listed: AtomicUsize::new(0),
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
fn a_small_change_with_no_delta_on_an_integration_branch_is_taken_in() {
    purlis_core::unsteered!();
    let s = scene();
    // A new small file: the pack carries no delta, so `index-pack --strict` misses the parent
    // commit itself, not a delta base.
    let tip = task_commit(&s, |chat| write(chat, "notes.txt", "small\n"));
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
fn a_truncated_pack_is_refused_at_once_and_the_forge_is_not_asked_for_its_tips() {
    purlis_core::unsteered!();
    let s = scene();
    let tip = task_commit(&s, |chat| write(chat, "src/new.txt", "new\n"));
    let parent = tempfile::tempdir().unwrap();
    let forge = forge_of(&s, false);
    let quarantine = Quarantine::open(parent.path(), &s.default_tip, &forge).unwrap();
    let pack = pack_of(&s.chat, &tip, &s.integration);
    let cut = &pack[..pack.len() / 2];
    assert!(
        quarantine
            .take_in_known(cut, &tip, &s.default_tip, &forge)
            .is_err()
    );
    assert_eq!(
        forge.asked_to_list(),
        0,
        "a truncated pack triggers no fetch"
    );
}

#[test]
fn a_garbage_pack_is_refused_at_once_and_the_forge_is_not_asked_for_its_tips() {
    purlis_core::unsteered!();
    let s = scene();
    let tip = task_commit(&s, |chat| write(chat, "src/new.txt", "new\n"));
    let parent = tempfile::tempdir().unwrap();
    let forge = forge_of(&s, false);
    let quarantine = Quarantine::open(parent.path(), &s.default_tip, &forge).unwrap();
    let garbage: Vec<u8> = (0..4096u32).map(|i| (i * 31 % 251) as u8).collect();
    assert!(
        quarantine
            .take_in_known(&garbage, &tip, &s.default_tip, &forge)
            .is_err()
    );
    assert_eq!(forge.asked_to_list(), 0, "a garbage pack triggers no fetch");
}

#[test]
fn a_pack_short_of_objects_asks_the_forge_for_its_tips_once() {
    purlis_core::unsteered!();
    let s = scene();
    let tip = task_commit(&s, |chat| {
        write(
            chat,
            "src/big.txt",
            &format!("{}integration\ntask\n", lines(300)),
        )
    });
    let forge = forge_of(&s, false);
    through_the_route(&s, &forge, &tip).expect("it takes in");
    assert_eq!(forge.asked_to_list(), 1);
}

#[test]
fn a_pack_built_on_a_commit_the_forge_does_not_advertise_is_refused_after_the_fetch() {
    purlis_core::unsteered!();
    let s = scene();
    // A commit of the chat's own, never pushed, and a task commit on top of it: the pack leaves
    // the first out, as `git push` does of what it thinks the forge has.
    git(
        &s.chat,
        &["checkout", "-q", "-B", "task", s.integration.as_str()],
    );
    write(
        &s.chat,
        "src/big.txt",
        &format!("{}integration\nlocal\n", lines(300)),
    );
    let local = commit(&s.chat, "chat-local work");
    write(
        &s.chat,
        "src/big.txt",
        &format!("{}integration\nlocal\ntask\n", lines(300)),
    );
    let tip = commit(&s.chat, "task work");
    let parent = tempfile::tempdir().unwrap();
    let forge = forge_of(&s, false);
    let quarantine = Quarantine::open(parent.path(), &s.default_tip, &forge).unwrap();
    let pack = pack_of(&s.chat, &tip, &local);
    let refused = quarantine.take_in_known(&pack, &tip, &s.default_tip, &forge);
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(
        forge.asked_to_list(),
        1,
        "the forge's tips were fetched first"
    );
}

/// A push to a branch that is there (`created == false`): the quarantine holds the branch's old
/// tip and the default branch's tip, and the pack is read against the old tip.
#[test]
fn a_push_to_an_existing_branch_on_an_integration_base_is_read_against_the_default_branch() {
    purlis_core::unsteered!();
    let s = scene();
    // `task` is on the forge already, one commit on the integration branch.
    git(
        &s.forge,
        &["checkout", "-q", "-b", "task", s.integration.as_str()],
    );
    write(&s.forge, "src/other.txt", "other\n");
    let old = commit(&s.forge, "task on the forge");
    git(&s.forge, &["checkout", "-q", "main"]);
    // The chat adds a commit on it.
    git(&s.chat, &["fetch", "-q", "origin", "task"]);
    git(&s.chat, &["checkout", "-q", "-B", "task", old.as_str()]);
    write(
        &s.chat,
        "src/big.txt",
        &format!("{}integration\nmore\n", lines(300)),
    );
    let tip = commit(&s.chat, "more task work");

    let parent = tempfile::tempdir().unwrap();
    let forge = forge_of(&s, false);
    let quarantine = Quarantine::open(parent.path(), &old, &forge).unwrap();
    quarantine.also_have(&forge, &s.default_tip).unwrap();
    let pack = pack_of(&s.chat, &tip, &old);
    quarantine
        .take_in_known(&pack, &tip, &old, &forge)
        .expect("it takes in");
    assert_eq!(
        forge.asked_to_list(),
        0,
        "the old tip holds what the pack is built on"
    );
    let checked = check_push_since(
        Kind::GitHub,
        None,
        &quarantine.trees(),
        &s.default_tip,
        Some(&old),
        &tip,
    );
    assert!(matches!(checked, PushCheck::NoCiFiles), "{checked:?}");
}

/// A forge that advertises a list with repeats, other cases, junk and more tips than the cap,
/// and records the tips it was asked to fetch.
struct Noisy {
    inner: LocalForge,
    list: Vec<String>,
    asked: std::sync::Mutex<Vec<HeadSha>>,
    real: HeadSha,
}

impl BaseObjects for Noisy {
    fn fetch(&self, into: &Path, base: &HeadSha) -> Result<(), String> {
        self.inner.fetch(into, base)
    }

    fn advertised(&self, _into: &Path) -> Option<Vec<String>> {
        Some(self.list.clone())
    }

    fn fetch_all(&self, into: &Path, tips: &[HeadSha]) -> Result<(), String> {
        *self.asked.lock().unwrap() = tips.to_vec();
        self.inner.fetch(into, &self.real)
    }
}

#[test]
fn the_tips_fetched_are_distinct_full_ids_and_at_most_the_cap() {
    purlis_core::unsteered!();
    let s = scene();
    let tip = task_commit(&s, |chat| {
        write(
            chat,
            "src/big.txt",
            &format!("{}integration\ntask\n", lines(300)),
        )
    });
    let real = s.integration.as_str().to_owned();
    let mut list = vec![
        real.clone(),
        real.to_ascii_uppercase(),
        real.clone(),
        "not-a-sha".to_owned(),
        "abc123".to_owned(),
    ];
    list.extend((0..purlis_core::diffscan::KNOWN_AT_MOST + 50).map(|i| format!("{i:040x}")));
    let noisy = Noisy {
        inner: forge_of(&s, false),
        list,
        asked: std::sync::Mutex::new(Vec::new()),
        real: s.integration.clone(),
    };
    let parent = tempfile::tempdir().unwrap();
    let quarantine = Quarantine::open(parent.path(), &s.default_tip, &noisy).unwrap();
    let pack = pack_of(&s.chat, &tip, &s.integration);
    quarantine
        .take_in_known(&pack, &tip, &s.default_tip, &noisy)
        .expect("it takes in");
    let asked = noisy.asked.lock().unwrap();
    assert!(asked.len() <= purlis_core::diffscan::KNOWN_AT_MOST);
    let mut distinct: Vec<&str> = asked.iter().map(HeadSha::as_str).collect();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), asked.len(), "no tip twice");
    assert_eq!(
        asked.iter().filter(|t| t.as_str() == real).count(),
        1,
        "a tip listed in two cases is one"
    );
}

/// The pack git sends when the forge has every object: nothing.
fn empty_pack(s: &Scene) -> Vec<u8> {
    pack_of(&s.chat, &s.default_tip, &s.default_tip)
}

/// What the gate does for a push that sends `pack` for `tip` over a branch at `base` (`already`
/// where the branch is there, as for a push to an existing branch): a quarantine on `base`, the
/// pack taken in, and the tree read against the default branch.
fn pushed(
    s: &Scene,
    forge: &dyn BaseObjects,
    base: &HeadSha,
    already: bool,
    tip: &HeadSha,
    pack: &[u8],
) -> Result<PushCheck, String> {
    let parent = tempfile::tempdir().unwrap();
    let quarantine = Quarantine::open(parent.path(), base, forge)?;
    quarantine.also_have(forge, &s.default_tip)?;
    quarantine.take_in_known(pack, tip, base, forge)?;
    Ok(check_push_since(
        Kind::GitHub,
        None,
        &quarantine.trees(),
        &s.default_tip,
        already.then_some(base),
        tip,
    ))
}

fn an_empty_pack_through_the_route(
    s: &Scene,
    forge: &LocalForge,
    tip: &HeadSha,
) -> Result<PushCheck, String> {
    pushed(s, forge, &s.default_tip, false, tip, &empty_pack(s))
}

const OUTSIDE: &str = "neither in the push nor in a branch of this repo";

#[test]
fn a_branch_made_at_a_commit_a_branch_of_the_repo_holds_is_read_not_refused() {
    purlis_core::unsteered!();
    let s = scene();
    let checked = an_empty_pack_through_the_route(&s, &forge_of(&s, false), &s.integration)
        .expect("it is read");
    assert!(matches!(checked, PushCheck::NoCiFiles), "{checked:?}");
}

#[test]
fn a_branch_made_at_a_commit_in_the_middle_of_a_branchs_history_is_read() {
    purlis_core::unsteered!();
    let s = scene();
    // The default branch's tip is an ancestor of every other branch, and not any branch's tip
    // but its own: a commit reachable from a tip is enough.
    git(&s.forge, &["checkout", "-q", "phase-1-proven"]);
    write(&s.forge, "src/more.txt", "more\n");
    commit(&s.forge, "integration moves on");
    git(&s.forge, &["checkout", "-q", "main"]);
    let checked = an_empty_pack_through_the_route(&s, &forge_of(&s, false), &s.integration)
        .expect("it is read");
    assert!(matches!(checked, PushCheck::NoCiFiles), "{checked:?}");
}

#[test]
fn a_push_of_a_commit_a_branch_holds_that_changes_ci_files_is_held() {
    purlis_core::unsteered!();
    let s = scene();
    match an_empty_pack_through_the_route(&s, &forge_of(&s, false), &s.workflow_edit)
        .expect("it is read")
    {
        PushCheck::Hold(diff) => {
            assert_eq!(diff.files, vec![WORKFLOW.to_owned()]);
            assert_eq!(diff.against, s.default_tip);
        }
        other => panic!("held, got {other:?}"),
    }
}

#[test]
fn a_commit_only_a_fork_has_is_refused_though_the_forge_resolves_it_by_id() {
    purlis_core::unsteered!();
    let s = scene();
    let why = an_empty_pack_through_the_route(&s, &forge_of(&s, false), &s.fork_only)
        .expect_err("refused");
    assert!(why.contains(OUTSIDE), "{why}");
}

#[test]
fn a_dangling_commit_is_refused_though_the_forge_resolves_it_by_id() {
    purlis_core::unsteered!();
    let s = scene();
    let why = an_empty_pack_through_the_route(&s, &forge_of(&s, false), &s.dangling)
        .expect_err("refused");
    assert!(why.contains(OUTSIDE), "{why}");
}

#[test]
fn a_commit_nothing_has_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    let tip = task_commit(&s, |chat| write(chat, "src/new.txt", "new\n"));
    let why = an_empty_pack_through_the_route(&s, &forge_of(&s, false), &tip).expect_err("refused");
    assert!(why.contains(OUTSIDE), "{why}");
}

#[test]
fn a_pack_that_sends_other_objects_but_not_the_tip_is_refused_for_a_dangling_tip() {
    purlis_core::unsteered!();
    let s = scene();
    let other = task_commit(&s, |chat| write(chat, "src/new.txt", "new\n"));
    let pack = pack_of(&s.chat, &other, &s.integration);
    let why = pushed(
        &s,
        &forge_of(&s, false),
        &s.default_tip,
        false,
        &s.dangling,
        &pack,
    )
    .expect_err("refused");
    assert!(why.contains(OUTSIDE), "{why}");
}

#[test]
fn a_tip_is_refused_where_the_forge_does_not_list_its_branches() {
    purlis_core::unsteered!();
    let s = scene();
    for tip in [&s.integration, &s.dangling] {
        let why =
            an_empty_pack_through_the_route(&s, &forge_of(&s, true), tip).expect_err("refused");
        assert!(why.contains("could not be read"), "{why}");
    }
}

#[test]
fn a_fast_forward_of_an_existing_branch_to_a_commit_a_branch_holds_is_read() {
    purlis_core::unsteered!();
    let s = scene();
    // The branch is at the default tip; the push moves it to the integration branch's tip.
    let checked = pushed(
        &s,
        &forge_of(&s, false),
        &s.default_tip,
        true,
        &s.integration,
        &empty_pack(&s),
    )
    .expect("it is read");
    assert!(matches!(checked, PushCheck::NoCiFiles), "{checked:?}");
}

#[test]
fn a_rewind_of_an_existing_branch_to_a_commit_a_branch_holds_is_read() {
    purlis_core::unsteered!();
    let s = scene();
    // The branch is at the integration tip; the push moves it back to the default tip.
    let checked = pushed(
        &s,
        &forge_of(&s, false),
        &s.integration,
        true,
        &s.default_tip,
        &empty_pack(&s),
    )
    .expect("it is read");
    assert!(matches!(checked, PushCheck::NoCiFiles), "{checked:?}");
}

#[test]
fn a_fast_forward_or_a_rewind_to_a_dangling_commit_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    for base in [&s.default_tip, &s.integration] {
        let why = pushed(
            &s,
            &forge_of(&s, false),
            base,
            true,
            &s.dangling,
            &empty_pack(&s),
        )
        .expect_err("refused");
        assert!(why.contains(OUTSIDE), "{why}");
    }
}

/// A forge that lists a branch it no longer has, as one deleted since it was listed: its fetch
/// skips what it cannot serve.
struct Listing {
    inner: LocalForge,
}

impl BaseObjects for Listing {
    fn fetch(&self, into: &Path, base: &HeadSha) -> Result<(), String> {
        self.inner.fetch(into, base)
    }

    fn advertised(&self, into: &Path) -> Option<Vec<String>> {
        let mut list = self.inner.advertised(into)?;
        list.push(format!("{:040x}", 7));
        Some(list)
    }

    fn fetch_all(&self, into: &Path, tips: &[HeadSha]) -> Result<(), String> {
        let have = self.inner.advertised(into).unwrap();
        for tip in tips
            .iter()
            .filter(|tip| have.iter().any(|id| id == tip.as_str()))
        {
            self.inner.fetch(into, tip)?;
        }
        Ok(())
    }
}

#[test]
fn a_listed_branch_gone_since_is_skipped_and_never_lets_an_outside_commit_by() {
    purlis_core::unsteered!();
    let s = scene();
    let listing = Listing {
        inner: forge_of(&s, false),
    };
    let empty = empty_pack(&s);
    let read = pushed(&s, &listing, &s.default_tip, false, &s.integration, &empty)
        .expect("a commit of a branch is read");
    assert!(matches!(read, PushCheck::NoCiFiles), "{read:?}");
    let why =
        pushed(&s, &listing, &s.default_tip, false, &s.dangling, &empty).expect_err("refused");
    assert!(why.contains(OUTSIDE), "{why}");
}

/// The tips each cache keeps as bases: only a fetch of a base adds one.
#[test]
fn a_pushed_tip_is_never_fetched_by_name() {
    purlis_core::unsteered!();
    let s = scene();
    struct Named(LocalForge, std::sync::Mutex<Vec<HeadSha>>);
    impl BaseObjects for Named {
        fn fetch(&self, into: &Path, base: &HeadSha) -> Result<(), String> {
            self.1.lock().unwrap().push(base.clone());
            self.0.fetch(into, base)
        }
        fn advertised(&self, into: &Path) -> Option<Vec<String>> {
            self.0.advertised(into)
        }
    }
    let forge = Named(forge_of(&s, false), std::sync::Mutex::new(Vec::new()));
    for tip in [&s.fork_only, &s.dangling, &s.integration] {
        let _ = pushed(&s, &forge, &s.default_tip, false, tip, &empty_pack(&s));
    }
    let named = forge.1.lock().unwrap();
    for tip in [&s.fork_only, &s.dangling] {
        assert!(!named.contains(tip), "{tip:?} was asked of the forge by id");
    }
}
