// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::path::Path;
use std::sync::mpsc;

const PATIENCE: Duration = Duration::from_secs(10);

#[test]
fn branches_are_resolved_apart_and_answered_in_their_order() {
    let started = Instant::now();
    let answered = each_apart(vec![300u64, 0, 300, 7, 300], |ms| {
        std::thread::sleep(Duration::from_millis(ms));
        // One that does not resolve is left out.
        (ms != 7).then_some(ms)
    });
    assert_eq!(answered, [300, 0, 300, 300]);
    // In turn they would take 900 ms; apart, about one wait.
    let took = started.elapsed();
    assert!(took < Duration::from_millis(800), "{took:?}");
}

#[test]
fn a_branch_that_found_the_readers_busy_is_asked_again_and_one_that_failed_is_not() {
    let busy = || {
        purlis_core::files::Refused::Read(format!(
            "{}it was busy reading other branches for 30 seconds",
            purlis_core::files::READ_FAILED
        ))
    };
    let mut asks = 0;
    let answered = past_busy(|| {
        asks += 1;
        if asks < 3 { Err(busy()) } else { Ok(asks) }
    });
    assert_eq!(answered, Some(3));

    let mut asks = 0;
    let failed: Option<()> = past_busy(|| {
        asks += 1;
        Err(purlis_core::files::Refused::Read(format!(
            "{}the read did not finish within 30 seconds",
            purlis_core::files::READ_FAILED
        )))
    });
    assert_eq!((failed, asks), (None, 1));

    let mut asks = 0;
    let never: Option<()> = past_busy(|| {
        asks += 1;
        Err(busy())
    });
    assert_eq!((never, asks), (None, BUSY_TRIES));
}

fn read_failed(why: &str) -> purlis_core::files::Refused {
    purlis_core::files::Refused::Read(format!("{}{why}", purlis_core::files::READ_FAILED))
}

/// #1745: a read the reader gave no answer for, a gate busy past every retry included, is
/// asked for again later rather than dropped: once its pause is over when it was paused,
/// else after [`AGAIN_AFTER`]. A refusal is answered, with nothing.
#[test]
fn a_read_the_reader_gave_no_answer_for_is_asked_for_later() {
    let mut asks = 0;
    let never: Asked<()> = asked_or_later(|| {
        asks += 1;
        Err(read_failed(
            "it was busy reading other branches for 30 seconds",
        ))
    });
    assert!(matches!(never, Asked::Later(AGAIN_AFTER)), "{never:?}");
    assert_eq!(asks, BUSY_TRIES);

    let mut asks = 0;
    let paused: Asked<()> = asked_or_later(|| {
        asks += 1;
        Err(read_failed(
            "its last 2 reads ran past their time or memory, so it is read again in 45 seconds",
        ))
    });
    assert!(
        matches!(paused, Asked::Later(after) if after == Duration::from_secs(45)),
        "{paused:?}"
    );
    assert_eq!(asks, 1, "a pause is not asked again at once");

    let hung: Asked<()> =
        asked_or_later(|| Err(read_failed("the read did not finish within 30 seconds")));
    assert!(matches!(hung, Asked::Later(AGAIN_AFTER)), "{hung:?}");

    let refused: Asked<()> =
        asked_or_later(|| Err(purlis_core::files::Refused::Read("gone".into())));
    assert!(matches!(refused, Asked::Answered(None)), "{refused:?}");

    let found = asked_or_later(|| Ok(7)).map(|n| n * 2);
    assert!(matches!(found, Asked::Answered(Some(14))), "{found:?}");
}

fn later(piece: &str) -> Later {
    Later {
        plane: PathBuf::from("/plane"),
        branch: named(piece),
        due: Instant::now(),
    }
}

/// #1745: a branch whose reader stayed busy past its retries is asked for again until the
/// reader answers, then handed on as found; one the reader refuses is handed on as
/// nothing. The window's unlistened branches are said each time that list changes.
#[test]
fn a_branch_whose_reader_stayed_busy_is_asked_again_until_it_answers() {
    let asks = std::cell::RefCell::new(Vec::new());
    let found = std::cell::RefCell::new(Vec::new());
    let said = std::cell::RefCell::new(Vec::new());

    listen_later(
        || true,
        vec![later("busy"), later("gone")],
        |one| {
            let piece = one.branch.piece.clone().unwrap();
            asks.borrow_mut().push(piece.clone());
            let times = asks.borrow().iter().filter(|p| **p == piece).count();
            match piece.as_str() {
                "busy" if times < 3 => Asked::Later(Duration::from_millis(20)),
                "busy" => Asked::Answered(Some(times)),
                _ => Asked::Answered(None),
            }
        },
        |branch, answer| {
            found.borrow_mut().push((branch, answer));
            true
        },
        |still| said.borrow_mut().push(still),
    );

    let asked = asks.borrow();
    assert_eq!(asked.iter().filter(|p| *p == "busy").count(), 3);
    assert_eq!(asked.iter().filter(|p| *p == "gone").count(), 1);
    assert_eq!(
        *found.borrow(),
        [(named("gone"), None), (named("busy"), Some(3))]
    );
    assert_eq!(*said.borrow(), [vec![named("busy")], Vec::new()]);
}

/// #1745: once the window asks for another set, or a found branch's set no longer holds,
/// nothing of the old set is asked for again.
#[test]
fn a_window_that_asked_for_another_set_lets_go_of_the_old_ones_late_branches() {
    listen_later(
        || false,
        vec![later("busy")],
        |_| -> Asked<()> { panic!("asked for a branch of a set the window let go of") },
        |_, _| panic!("found a branch of a set the window let go of"),
        |_| panic!("said something of a set the window let go of"),
    );

    let asks = std::cell::Cell::new(0);
    listen_later(
        || true,
        vec![later("one"), later("two")],
        |_| {
            asks.set(asks.get() + 1);
            Asked::Answered(Some(()))
        },
        |_, _| false,
        |_| panic!("said something of a set the window let go of"),
    );
    assert_eq!(asks.get(), 1);
}

/// #1745: a branch found late is listened to, and told to the window as moved, so the
/// window reads its status again: whatever changed before its watch held was heard by
/// nothing (the window's read raced the watch, diag-wsx2). A set the window has since
/// replaced takes nothing found late, and tells nothing.
#[test]
fn a_branch_found_late_is_listened_to_and_told_as_moved() {
    let (_dir, root, _piece) = plane();
    let (tx, told) = mpsc::channel();
    let tx = Mutex::new(tx);
    let watch = BranchWatch::<crate::watchset::raw::Raw>::with_config(
        Arc::new(move |window: &str, branches| {
            let _ = tx.lock().unwrap().send((window.to_string(), branches));
        }),
        crate::reader(),
        notify::Config::default(),
    );
    let ticket = watch.ticket();
    assert!(watch.set_from("main", ticket, Vec::new()).unwrap());
    assert!(watch.wants("main", ticket));
    let found = root_of(&root, "piece");
    let at = found.path().to_path_buf();

    assert!(watch.found_late("main", ticket, vec![(branch(), found.clone(), How::Whole)]));

    assert_eq!(crate::watchset::raw::watched(&at), 1);
    let (window, branches) = told.recv_timeout(PATIENCE).expect("told as moved");
    assert_eq!((window.as_str(), branches), ("main", vec![branch()]));

    let newer = watch.ticket();
    assert!(watch.set_from("main", newer, Vec::new()).unwrap());
    assert!(!watch.wants("main", ticket));
    assert!(!watch.found_late("main", ticket, vec![(branch(), found, How::Whole)]));
    assert!(told.recv_timeout(Duration::from_millis(300)).is_err());
}

#[cfg(target_os = "macos")]
type Source = notify::PollWatcher;
#[cfg(not(target_os = "macos"))]
type Source = notify::RecommendedWatcher;

fn git(dir: &Path, args: &[&str]) {
    let ran = purlis_core::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.email=t@e.invalid", "-c", "user.name=t"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args),
    )
    .unwrap();
    assert!(ran.status.success(), "git {args:?}: {ran:?}");
}

/// A plane with a clone holding `src/` and an ignored `target/`, and one piece cut from it.
fn plane() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    let clone = root.join("workspaces/alpha/thing");
    std::fs::create_dir_all(clone.join("src/deep")).unwrap();
    git(&clone, &["init", "-q", "-b", "main", "."]);
    std::fs::write(clone.join("src/deep/lib.rs"), "\n").unwrap();
    std::fs::write(clone.join(".gitignore"), "target/\n").unwrap();
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-q", "-m", "one"]);
    let piece = purlis_core::worktree::add(&root, "alpha", "thing", "piece", None)
        .unwrap()
        .path;
    std::fs::create_dir_all(piece.join("target/debug")).unwrap();
    (dir, root, piece)
}

fn branch() -> WatchedBranch {
    named("piece")
}

fn named(piece: &str) -> WatchedBranch {
    WatchedBranch {
        plane: serde_json::from_value(serde_json::json!("/plane")).unwrap(),
        workspace: "alpha".to_string(),
        repo: "thing".to_string(),
        piece: Some(piece.to_string()),
    }
}

fn root_of(plane: &Path, piece: &str) -> Root {
    purlis_core::files::root(
        &crate::reader(),
        plane,
        purlis_core::files::Branch::piece("alpha", "thing", piece),
    )
    .unwrap()
}

type Heard = mpsc::Receiver<(String, Vec<WatchedBranch>)>;

fn watch_with(reader: Reader) -> (BranchWatch<Source>, Heard) {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let watch = BranchWatch::<Source>::with_config(
        Arc::new(move |window: &str, branches| {
            let _ = tx.lock().unwrap().send((window.to_string(), branches));
        }),
        reader,
        // The poller keeps mtimes to the second (notify 8): a ref rewritten within the
        // second it was made, as a commit right after `checkout -b` does, looks unchanged
        // to it. Its contents are compared too, so the test hears what FSEvents would
        // (#2463).
        notify::Config::default()
            .with_poll_interval(Duration::from_millis(50))
            .with_compare_contents(true),
    );
    (watch, rx)
}

fn listening(
    root: &Path,
    how: impl Fn(&Root) -> How,
) -> (
    BranchWatch<Source>,
    mpsc::Receiver<(String, Vec<WatchedBranch>)>,
) {
    let (watch, rx) = watch_with(crate::reader());
    let found = root_of(root, "piece");
    let chosen = how(&found);
    watch
        .set_from("main", watch.ticket(), vec![(branch(), found, chosen)])
        .unwrap();
    // The poller's first look is its baseline; give it one.
    std::thread::sleep(Duration::from_millis(300));
    (watch, rx)
}

fn both_ways() -> [fn(&Root) -> How; 2] {
    [
        |_| How::Whole,
        |root| How::Folders(root.folders(&crate::reader()).unwrap()),
    ]
}

#[test]
fn a_first_write_deep_in_a_folder_nobody_opened_tells_the_window() {
    for how in both_ways() {
        let (_dir, root, piece) = plane();
        let (_watch, told) = listening(&root, how);

        std::fs::write(piece.join("src/deep/first.rs"), "\n").unwrap();

        let (window, branches) = told.recv_timeout(PATIENCE).expect("the window was told");
        assert_eq!(window, "main");
        assert_eq!(branches, [branch()]);
    }
}

#[test]
fn a_write_only_where_git_ignores_tells_nobody() {
    for how in both_ways() {
        let (_dir, root, piece) = plane();
        let (_watch, told) = listening(&root, how);

        std::fs::write(piece.join("target/debug/out"), "a build's\n").unwrap();

        assert!(told.recv_timeout(Duration::from_secs(2)).is_err());
    }
}

/// A watch of the piece of `root`, and what it tells of repo skills.
fn hearing_skills(root: &Path) -> (BranchWatch<Source>, Heard) {
    let (skills, heard) = mpsc::channel();
    let skills = Mutex::new(skills);
    let (watch, _told) = watch_with(crate::reader());
    let watch = watch.telling_skills(Arc::new(move |window: &str, branches| {
        let _ = skills.lock().unwrap().send((window.to_string(), branches));
    }));
    let found = root_of(root, "piece");
    watch
        .set_from("main", watch.ticket(), vec![(branch(), found, How::Whole)])
        .unwrap();
    std::thread::sleep(Duration::from_millis(300));
    (watch, heard)
}

#[test]
fn a_write_in_a_repo_skills_folder_is_told_as_a_skill_even_where_git_ignores_it() {
    let (_dir, root, piece) = plane();
    std::fs::write(piece.join(".gitignore"), "target/\n.claude/\n").unwrap();
    std::fs::create_dir_all(piece.join(".claude/skills/deploy")).unwrap();
    let (_watch, heard) = hearing_skills(&root);

    std::fs::write(piece.join(".claude/skills/deploy/SKILL.md"), "\n").unwrap();

    let (window, branches) = heard.recv_timeout(PATIENCE).expect("the window was told");
    assert_eq!(window, "main");
    assert_eq!(branches, [branch()]);
}

#[test]
fn a_branch_found_late_is_told_as_moving_its_skills_too() {
    let (_dir, root, _piece) = plane();
    let (tx, heard) = mpsc::channel();
    let tx = Mutex::new(tx);
    let watch = BranchWatch::<crate::watchset::raw::Raw>::with_config(
        Arc::new(|_: &str, _| {}),
        crate::reader(),
        notify::Config::default(),
    )
    .telling_skills(Arc::new(move |window: &str, branches| {
        let _ = tx.lock().unwrap().send((window.to_string(), branches));
    }));
    let ticket = watch.ticket();
    assert!(watch.set_from("main", ticket, Vec::new()).unwrap());

    let found = root_of(&root, "piece");
    assert!(watch.found_late("main", ticket, vec![(branch(), found, How::Whole)]));

    let (window, branches) = heard.recv_timeout(PATIENCE).expect("told");
    assert_eq!((window.as_str(), branches), ("main", vec![branch()]));
}

#[test]
fn a_burst_of_many_other_writes_tells_no_one_of_a_skill() {
    let (_dir, root, piece) = plane();
    let (_watch, heard) = hearing_skills(&root);

    // More than one check looks at, none of them a skill's.
    for at in 0..=purlis_core::files::ASKED {
        std::fs::write(piece.join(format!("target/debug/{at}")), "\n").unwrap();
    }

    assert!(heard.recv_timeout(Duration::from_secs(2)).is_err());
}

#[test]
fn a_write_anywhere_else_tells_no_one_of_a_skill() {
    let (_dir, root, piece) = plane();
    let (_watch, heard) = hearing_skills(&root);

    std::fs::write(piece.join("src/deep/first.rs"), "\n").unwrap();

    assert!(heard.recv_timeout(Duration::from_secs(2)).is_err());
}

#[cfg(unix)]
#[test]
fn a_link_in_the_branch_to_a_large_tree_outside_is_never_walked() {
    let (_dir, root, piece) = plane();
    let outside = tempfile::tempdir().unwrap();
    for folder in 0..300 {
        let at = outside.path().join(format!("f{folder}"));
        std::fs::create_dir_all(&at).unwrap();
        for file in 0..300 {
            std::fs::write(at.join(format!("{file}.txt")), "").unwrap();
        }
    }
    std::os::unix::fs::symlink(outside.path(), piece.join("home")).unwrap();
    // The same branch with nothing behind it: the baseline, under whatever load this runs.
    purlis_core::worktree::add(&root, "alpha", "thing", "plain", None).unwrap();
    // The platform's own watcher, as the app runs it, on the whole branch; each time a fresh
    // one, so nothing one watch learned helps the next.
    let watching = |piece: &str| {
        let watch = BranchWatch::<notify::RecommendedWatcher>::with_config(
            Arc::new(|_: &str, _| {}),
            crate::reader(),
            notify::Config::default(),
        );
        let found = root_of(&root, piece);
        let started = std::time::Instant::now();
        watch
            .set_from(
                "main",
                watch.ticket(),
                vec![(named(piece), found, How::Whole)],
            )
            .unwrap();
        started.elapsed()
    };

    // Interleaved, the fastest of three each: a moment of load slows one sample, not the
    // comparison.
    let (mut linked, mut plain) = (Duration::MAX, Duration::MAX);
    for _ in 0..3 {
        linked = linked.min(watching("piece"));
        plain = plain.min(watching("plain"));
    }

    // Measured on macOS: with a debouncer's file-id cache the 90,000 files behind the link
    // were walked, about a second against 25 ms without it. Without a walk the link costs
    // nothing; the bound is the baseline's, loosely.
    let bound = (plain * 4).max(plain + Duration::from_millis(250));
    assert!(
        linked < bound,
        "watching the linked branch took {linked:?}, the plain one {plain:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_branch_whose_check_hangs_holds_up_no_other_branch() {
    let (_dir, root, piece) = plane();
    let other = purlis_core::worktree::add(&root, "alpha", "thing", "other", None)
        .unwrap()
        .path;
    // An ignore file that blocks whoever opens it: the check of this branch hangs. The write
    // goes in a folder git does not track yet, below it (#2463): a write in `src/deep` also
    // names `src/deep`, a folder holding tracked files, which matters without reading any
    // ignore file, so whenever that was asked first the check answered at once, and the two
    // branches were told in either order. Nothing in `src/fresh` is tracked, so whichever
    // path is asked first, its check reads `src/.gitignore` and hangs.
    let made = purlis_core::forklock::output(
        std::process::Command::new("mkfifo").arg(piece.join("src/.gitignore")),
    )
    .unwrap();
    assert!(made.status.success());
    std::fs::create_dir(piece.join("src/fresh")).unwrap();
    let deadline = Duration::from_secs(6);
    let (watch, told) = watch_with(crate::reader().deadline(deadline));
    let listened = ["piece", "other"]
        .map(|one| (named(one), root_of(&root, one), How::Whole))
        .to_vec();
    watch.set_from("main", watch.ticket(), listened).unwrap();
    std::thread::sleep(Duration::from_millis(300));

    let started = std::time::Instant::now();
    std::fs::write(piece.join("src/fresh/hangs.rs"), "\n").unwrap();
    std::fs::write(other.join("src/deep/moves.rs"), "\n").unwrap();

    // The other branch is told while the first one's check still hangs.
    let (_, first) = told.recv_timeout(PATIENCE).expect("a window was told");
    assert_eq!(first, [named("other")]);
    assert!(started.elapsed() < deadline, "{:?}", started.elapsed());
    // And the hung one, once the reader gave up on it: a check that did not answer matters.
    let (_, then) = told
        .recv_timeout(PATIENCE)
        .expect("the hung branch was told");
    assert_eq!(then, [named("piece")]);
    // Told because its read ran into the reader's deadline, not because it answered (#2463).
    assert!(started.elapsed() >= deadline, "{:?}", started.elapsed());
}

/// A watch on [`crate::watchset::raw::Raw`], listening to the piece whole: the test plays
/// the platform.
fn listening_raw(
    root: &Path,
) -> (
    BranchWatch<crate::watchset::raw::Raw>,
    mpsc::Receiver<(String, Vec<WatchedBranch>)>,
) {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let watch = BranchWatch::<crate::watchset::raw::Raw>::with_config(
        Arc::new(move |window: &str, branches| {
            let _ = tx.lock().unwrap().send((window.to_string(), branches));
        }),
        crate::reader(),
        notify::Config::default(),
    );
    watch
        .set_from(
            "main",
            watch.ticket(),
            vec![(branch(), root_of(root, "piece"), How::Whole)],
        )
        .unwrap();
    (watch, rx)
}

#[test]
fn a_platform_that_lost_track_tells_every_branch() {
    // inotify's queue overflowed, or the watcher erred: anything may have moved, so the
    // branch's markers are read again rather than left as they were (#1139).
    let (_dir, root, _piece) = plane();
    let (_watch, told) = listening_raw(&root);

    crate::watchset::raw::event(
        notify::Event::new(notify::EventKind::Other).set_flag(notify::event::Flag::Rescan),
    );

    let (window, branches) = told.recv_timeout(PATIENCE).expect("the window was told");
    assert_eq!(window, "main");
    assert_eq!(branches, [branch()]);
}

#[test]
fn a_file_made_and_removed_inside_one_burst_is_still_a_move() {
    // A debouncer took the removal as cancelling the creation and told neither (#1139).
    use notify::event::{CreateKind, EventKind, RemoveKind};
    let (_dir, root, piece) = plane();
    let (_watch, told) = listening_raw(&root);
    let brief = std::fs::canonicalize(&piece)
        .unwrap()
        .join("src/deep/brief.rs");

    crate::watchset::raw::raw(EventKind::Create(CreateKind::File), &brief);
    crate::watchset::raw::raw(EventKind::Remove(RemoveKind::File), &brief);

    let (_, branches) = told.recv_timeout(PATIENCE).expect("the window was told");
    assert_eq!(branches, [branch()]);
}

/// What the shared standing asks of the watch for the plane's clone (FD-11).
fn the_clone(root: &Path) -> purlis_core::standings::Wanted {
    purlis_core::standings::Wanted {
        plane: root.to_path_buf(),
        workspace: "alpha".into(),
        repo: "thing".into(),
        path: root.join("workspaces/alpha/thing"),
    }
}

fn standing_of(wanted: &purlis_core::standings::Wanted) -> purlis_core::reposave::Standing {
    purlis_core::reposave::shared_standing(
        &wanted.plane,
        &wanted.workspace,
        &purlis_core::repos::Repo {
            name: wanted.repo.clone(),
            path: wanted.path.clone(),
        },
    )
}

/// Waits, at most [`PATIENCE`], for `done`.
fn until(done: impl Fn() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < PATIENCE {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    done()
}

#[test]
fn a_clone_whose_standing_is_shared_is_read_again_when_its_tree_moves_not_on_a_clock() {
    // FD-11 (#651): the one watch per repo covers the clone, so its standing is read again
    // when something git would show moves in it, and otherwise only on its long backstop.
    let (_dir, root, _piece) = plane();
    let (watch, _told) = watch_with(crate::reader());
    let watch = watch.keeping_clones(|_, _| How::Whole);
    let clone = the_clone(&root);

    watch.want(&clone);

    assert!(until(|| purlis_core::reposave::covered(&clone.path)));
    // The poller's first look is its baseline.
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(standing_of(&clone).changed, 0);
    let started = Instant::now();
    std::fs::write(clone.path.join("src/deep/first.rs"), "\n").unwrap();
    assert!(until(|| standing_of(&clone).changed == 1));
    // Well inside the backstop a clone nothing watches is read again on.
    assert!(
        started.elapsed() < purlis_core::planegit::SHARED_FOR,
        "{:?}",
        started.elapsed()
    );
}

/// #1152 (D-1152-6): a commit that writes no file in a covered clone moves only git's own
/// folder, which the watch passes over and touches no standing for; the clone's shared
/// standing still has it at its next read, as its stamp holds the clone's `HEAD` and refs
/// (`purlis_core::standings::Stamp`). First run on CI: it needs git.
#[test]
fn a_commit_with_no_file_write_is_in_a_covered_clones_next_standing() {
    let (_dir, root, _piece) = plane();
    let (watch, _told) = watch_with(crate::reader());
    let watch = watch.keeping_clones(|_, _| How::Whole);
    let clone = the_clone(&root);
    watch.want(&clone);
    assert!(until(|| purlis_core::reposave::covered(&clone.path)));
    let before = standing_of(&clone).head;
    assert!(before.is_some());

    git(
        &clone.path,
        &["commit", "-q", "--allow-empty", "-m", "no file written"],
    );

    assert_ne!(standing_of(&clone).head, before);
}

#[test]
fn a_plane_let_go_of_no_longer_covers_its_clones() {
    let (_dir, root, _piece) = plane();
    let (watch, _told) = watch_with(crate::reader());
    let watch = watch.keeping_clones(|_, _| How::Whole);
    let clone = the_clone(&root);
    watch.want(&clone);
    assert!(until(|| purlis_core::reposave::covered(&clone.path)));

    watch.let_go_of_plane(&root);

    assert!(!purlis_core::reposave::covered(&clone.path));
    assert!(watch.watching().is_empty(), "{:?}", watch.watching());
}

#[test]
fn a_clone_watched_folder_by_folder_is_never_covered() {
    // D-FD11h: inotify's watches are one per folder, and a folder made, or deleted and
    // made again, between a listing and its watch is an edit nobody hears; the standing
    // keeps its clock there.
    for how_of in [
        (|root: &Root, reader: &Reader| How::Folders(root.folders(reader).unwrap()))
            as fn(&Root, &Reader) -> How,
        |_, _| How::Unread,
    ] {
        let (_dir, root, _piece) = plane();
        let (watch, _told) = watch_with(crate::reader());
        let watch = watch.keeping_clones(how_of);
        let clone = the_clone(&root);
        let at = root_of_clone(&root);

        watch.want(&clone);

        assert!(until(|| watch
            .watching()
            .contains(&at.path().to_path_buf())));
        std::thread::sleep(Duration::from_millis(200));
        assert!(!purlis_core::reposave::covered(&clone.path));
    }
}

#[test]
fn a_lost_watch_uncovers_the_clone_until_its_tree_is_watched_again() {
    let (_dir, root, _piece) = plane();
    let (tx, _rx) = mpsc::channel::<(String, Vec<WatchedBranch>)>();
    let tx = Mutex::new(tx);
    let watch = BranchWatch::<crate::watchset::raw::Raw>::with_config(
        Arc::new(move |window: &str, branches| {
            let _ = tx.lock().unwrap().send((window.to_string(), branches));
        }),
        crate::reader(),
        notify::Config::default(),
    )
    .keeping_clones(|_, _| How::Whole);
    // The platform's watcher, made on this thread, which plays it.
    watch
        .set_from(
            "main",
            watch.ticket(),
            vec![(branch(), root_of(&root, "piece"), How::Whole)],
        )
        .unwrap();
    let clone = the_clone(&root);
    let at = root_of_clone(&root).path().to_path_buf();
    watch.want(&clone);
    assert!(until(|| purlis_core::reposave::covered(&clone.path)));
    assert_eq!(crate::watchset::raw::watched(&at), 1);

    // The platform lost track: watched again, and covered once that watch is made.
    crate::watchset::raw::event(
        notify::Event::new(notify::EventKind::Other).set_flag(notify::event::Flag::Rescan),
    );
    assert!(until(|| crate::watchset::raw::watched(&at) == 2));
    // Covered once the watch is made, which is a step after it is counted: read at once, a
    // loaded runner saw the gap between the two (#1829's run).
    assert!(until(|| purlis_core::reposave::covered(&clone.path)));

    // Lost again, and this time the tree cannot be watched: it stays uncovered.
    crate::watchset::raw::refuse(&at);
    crate::watchset::raw::event(
        notify::Event::new(notify::EventKind::Other).set_flag(notify::event::Flag::Rescan),
    );
    assert!(until(|| !purlis_core::reposave::covered(&clone.path)));
    std::thread::sleep(Duration::from_millis(300));
    assert!(!purlis_core::reposave::covered(&clone.path));
}

/// The plane's clone, found as the standing's watch finds it.
fn root_of_clone(plane: &Path) -> Root {
    purlis_core::files::root(
        &crate::reader(),
        plane,
        purlis_core::files::Branch::repo("alpha", "thing"),
    )
    .unwrap()
}

#[test]
fn a_clone_the_reader_will_not_find_is_not_asked_for_again_at_once() {
    let (_dir, root, _piece) = plane();
    let (watch, _told) = watch_with(crate::reader());
    let mut missing = the_clone(&root);
    missing.repo = "nowhere".into();
    missing.path = root.join("workspaces/alpha/nowhere");

    watch.want(&missing);
    assert!(until(|| {
        let inner = watch.inner.lock().unwrap();
        inner.refused.contains_key(&missing.path) && inner.finding.is_empty()
    }));
    watch.want(&missing);

    assert!(
        watch.inner.lock().unwrap().finding.is_empty(),
        "asked again"
    );
}

/// #874 (L11): a run of losses re-watches at once, then once more when
/// [`WATCHED_AGAIN_EVERY`] is over, however many came meanwhile; the next loss after that
/// re-watches at once again.
#[test]
fn a_run_of_losses_re_watches_once_a_window() {
    let every = Duration::from_secs(2);
    let start = Instant::now();
    let at = |ms: u64| start + Duration::from_millis(ms);
    let mut again = Rewatching::default();

    assert_eq!(again.lost(at(0), every), Again::Now);
    assert_eq!(
        again.lost(at(500), every),
        Again::After(Duration::from_millis(1500))
    );
    assert_eq!(again.lost(at(700), every), Again::Waiting);
    assert_eq!(again.lost(at(1900), every), Again::Waiting);
    again.done(at(2000));
    assert_eq!(
        again.lost(at(2100), every),
        Again::After(Duration::from_millis(1900))
    );
    again.done(at(4000));
    assert_eq!(again.lost(at(6000), every), Again::Now);
}

/// #874 (L11): a loss that names where it happened concerns only the kept trees a named
/// path lies in or above; one that names a path in none of them, or names none, concerns
/// every tree (fail closed).
#[test]
fn a_loss_concerns_only_the_trees_it_names() {
    let trees = [PathBuf::from("/w/a"), PathBuf::from("/w/b")];
    let named = |paths: &[&str]| paths.iter().map(PathBuf::from).collect::<HashSet<_>>();
    let lost = |paths: Option<HashSet<PathBuf>>| {
        let mut got = lost_trees(&trees, paths.as_ref());
        got.sort();
        got
    };

    assert_eq!(lost(Some(named(&["/w/a/src/x"]))), [PathBuf::from("/w/a")]);
    assert_eq!(lost(Some(named(&["/w/b"]))), [PathBuf::from("/w/b")]);
    assert_eq!(lost(Some(named(&["/w"]))), trees);
    assert_eq!(lost(Some(named(&["/w/a/x", "/w/b/y"]))), trees);
    // Not `/w/a` because it starts with its letters.
    assert_eq!(lost(Some(named(&["/w/ab", "/w/a/x"]))), trees);
    assert_eq!(lost(Some(named(&["/elsewhere"]))), trees);
    assert_eq!(lost(None), trees);
}

/// #874 (L11), with the platform played: a rescan that names a path in the clone's tree
/// re-watches that tree, its path kept rather than dropped. First run on CI: it needs git.
#[test]
fn a_rescan_naming_a_path_in_the_tree_re_watches_it() {
    let (_dir, root, _piece) = plane();
    let watch = BranchWatch::<crate::watchset::raw::Raw>::with_config(
        Arc::new(|_: &str, _| {}),
        crate::reader(),
        notify::Config::default(),
    )
    .keeping_clones(|_, _| How::Whole);
    watch
        .set_from(
            "main",
            watch.ticket(),
            vec![(branch(), root_of(&root, "piece"), How::Whole)],
        )
        .unwrap();
    let clone = the_clone(&root);
    let at = root_of_clone(&root).path().to_path_buf();
    watch.want(&clone);
    assert!(until(|| purlis_core::reposave::covered(&clone.path)));

    crate::watchset::raw::event(
        notify::Event::new(notify::EventKind::Other)
            .set_flag(notify::event::Flag::Rescan)
            .add_path(at.join("src")),
    );

    assert!(until(|| crate::watchset::raw::watched(&at) == 2));
    assert!(until(|| purlis_core::reposave::covered(&clone.path)));
}

/// #874 (L11), with the platform played: losses heard in a row re-watch the clone's tree
/// once at once and once more after [`WATCHED_AGAIN_EVERY`], not once each, and every one
/// uncovers it at once until that watch is made. First run on CI: it needs git.
#[test]
fn losses_in_a_row_re_watch_the_tree_once_a_window() {
    let (_dir, root, _piece) = plane();
    let watch = BranchWatch::<crate::watchset::raw::Raw>::with_config(
        Arc::new(|_: &str, _| {}),
        crate::reader(),
        notify::Config::default(),
    )
    .keeping_clones(|_, _| How::Whole);
    // The platform's watcher, made on this thread, which plays it.
    watch
        .set_from(
            "main",
            watch.ticket(),
            vec![(branch(), root_of(&root, "piece"), How::Whole)],
        )
        .unwrap();
    let clone = the_clone(&root);
    let at = root_of_clone(&root).path().to_path_buf();
    watch.want(&clone);
    assert!(until(|| purlis_core::reposave::covered(&clone.path)));
    let lost = || {
        crate::watchset::raw::event(
            notify::Event::new(notify::EventKind::Other).set_flag(notify::event::Flag::Rescan),
        );
    };

    lost();
    assert!(until(|| crate::watchset::raw::watched(&at) == 2));
    assert!(until(|| purlis_core::reposave::covered(&clone.path)));
    let started = Instant::now();
    for _ in 0..3 {
        // Apart by more than a burst's quiet, so each is a burst of its own.
        std::thread::sleep(QUIET_FOR + Duration::from_millis(100));
        lost();
    }
    assert!(until(|| !purlis_core::reposave::covered(&clone.path)));
    assert_eq!(crate::watchset::raw::watched(&at), 2, "re-watched at once");

    assert!(until(|| crate::watchset::raw::watched(&at) == 3));
    assert!(started.elapsed() >= WATCHED_AGAIN_EVERY / 2);
    assert!(until(|| purlis_core::reposave::covered(&clone.path)));
    std::thread::sleep(WATCHED_AGAIN_EVERY);
    assert_eq!(
        crate::watchset::raw::watched(&at),
        3,
        "re-watched once each"
    );
}

/// How many times an [`Unmade`] has been asked to be made.
static UNMADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A platform watcher that cannot be made: out of inotify instances, say.
struct Unmade;

impl notify::Watcher for Unmade {
    fn new<F: notify::EventHandler>(_: F, _: notify::Config) -> notify::Result<Self> {
        UNMADE.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(notify::Error::generic("too many open files"))
    }
    fn watch(&mut self, _: &Path, _: RecursiveMode) -> notify::Result<()> {
        unreachable!()
    }
    fn unwatch(&mut self, _: &Path) -> notify::Result<()> {
        unreachable!()
    }
    fn kind() -> notify::WatcherKind {
        notify::WatcherKind::NullWatcher
    }
}

fn unmade() -> BranchWatch<Unmade> {
    BranchWatch::<Unmade>::with_config(
        Arc::new(|_: &str, _| {}),
        crate::reader(),
        notify::Config::default(),
    )
    .keeping_clones(|_, _| How::Whole)
}

fn made() -> usize {
    UNMADE.load(std::sync::atomic::Ordering::SeqCst)
}

/// #874 (L9): a watcher that cannot be made is not made again by every branch found late
/// for [`REFUSED_FOR`], and is tried again once that is over. Counted apart from the
/// other test making an [`Unmade`].
#[test]
fn a_watcher_that_cannot_be_made_is_not_made_again_by_a_late_branch() {
    let _alone = UNMADE_TESTS.lock().unwrap_or_else(PoisonError::into_inner);
    let watch = unmade();
    let ticket = watch.ticket();
    assert!(watch.set_from("main", ticket, Vec::new()).unwrap());
    let before = made();

    assert!(watch.found_late("main", ticket, Vec::new()));
    assert_eq!(made(), before + 1);
    assert!(watch.found_late("main", ticket, Vec::new()));
    assert_eq!(made(), before + 1, "a watcher was made again at once");

    // Its time over, the next one tries again.
    watch.inner.lock().unwrap().unmade_until = Some(Instant::now());
    assert!(watch.found_late("main", ticket, Vec::new()));
    assert_eq!(made(), before + 2);
}

/// The tests counting [`UNMADE`], one at a time.
static UNMADE_TESTS: Mutex<()> = Mutex::new(());

/// #874 (L9): a watcher that cannot be made is not made again on every read of the
/// standing, which asks for its clone each time: the clone is left alone for
/// [`REFUSED_FOR`], as one the reader would not find is. First run on CI: it needs git.
#[test]
fn a_clone_whose_watcher_cannot_be_made_is_not_asked_for_again_at_once() {
    let _alone = UNMADE_TESTS.lock().unwrap_or_else(PoisonError::into_inner);
    let (_dir, root, _piece) = plane();
    let watch = unmade();
    let clone = the_clone(&root);
    let before = made();

    watch.want(&clone);
    assert!(until(|| {
        let inner = watch.inner.lock().unwrap();
        inner.refused.contains_key(&clone.path) && inner.finding.is_empty()
    }));
    assert_eq!(made(), before + 1);
    assert!(watch.inner.lock().unwrap().kept.is_empty());

    watch.want(&clone);
    assert!(
        watch.inner.lock().unwrap().finding.is_empty(),
        "asked again"
    );
    assert_eq!(made(), before + 1, "a watcher was made again");
}

#[test]
fn a_clone_found_after_its_plane_was_let_go_of_is_not_kept() {
    let (_dir, root, _piece) = plane();
    let (watch, _told) = watch_with(crate::reader());
    let watch = watch.keeping_clones(|_, _| How::Whole);
    let clone = the_clone(&root);

    watch.want(&clone);
    watch.let_go_of_plane(&root);

    assert!(until(|| watch.inner.lock().unwrap().finding.is_empty()));
    assert!(watch.inner.lock().unwrap().kept.is_empty());
    assert!(!purlis_core::reposave::covered(&clone.path));
}

#[test]
fn a_folder_made_in_the_branch_is_listened_in_once_it_appears() {
    let (_dir, root, piece) = plane();
    let (watch, told) = listening(&root, |root| {
        How::Folders(root.folders(&crate::reader()).unwrap())
    });
    let new = std::fs::canonicalize(&piece).unwrap().join("src/new");
    assert!(!watch.watching().contains(&new));

    std::fs::create_dir_all(&new).unwrap();
    std::fs::write(new.join("a.rs"), "\n").unwrap();

    told.recv_timeout(PATIENCE)
        .expect("the new folder was heard");
    assert!(watch.watching().contains(&new), "{:?}", watch.watching());
}

/// #1152, at the seam: the folders holding a cockpit's refs are watched, and a burst that
/// names one of its refs tells its window; a lock file beside it, or another branch's ref
/// in the same folder, tells nobody.
#[test]
fn a_cockpits_ref_moving_tells_its_window_and_nothing_else_does() {
    let refs = [
        PathBuf::from("/clone/.git/worktrees/piece/HEAD"),
        PathBuf::from("/clone/.git/packed-refs"),
        PathBuf::from("/clone/.git/refs/heads/piece"),
    ];
    let focused = branch();
    let cockpits = || std::iter::once(("main", &focused, refs.as_slice()));

    assert_eq!(
        ref_folders(cockpits()),
        HashSet::from([
            PathBuf::from("/clone/.git/worktrees/piece"),
            PathBuf::from("/clone/.git"),
            PathBuf::from("/clone/.git/refs/heads"),
        ])
    );
    let moved = |paths: &[&str]| -> HashSet<PathBuf> { paths.iter().map(PathBuf::from).collect() };
    assert!(
        refs_moved(
            cockpits(),
            &moved(&[
                "/clone/.git/refs/heads/other",
                "/clone/.git/refs/heads/piece.lock",
                "/clone/.git/index",
            ])
        )
        .is_empty()
    );
    assert_eq!(
        refs_moved(
            cockpits(),
            &moved(&[
                "/clone/.git/refs/heads/piece.lock",
                "/clone/.git/refs/heads/piece"
            ])
        ),
        HashMap::from([("main".to_string(), vec![branch()])])
    );
    assert!(refs_moved(std::iter::empty(), &moved(&["/clone/.git/packed-refs"])).is_empty());
}

/// #1152: a burst naming a cockpit's `HEAD` means its folder may have another branch checked
/// out, so its refs are found again; one naming only its branch's ref or `packed-refs` (a
/// commit, a pack) does not, and neither does a `HEAD` lock file or another worktree's HEAD.
#[test]
fn only_a_cockpits_head_moving_finds_its_refs_again() {
    let refs = [
        PathBuf::from("/clone/.git/worktrees/piece/HEAD"),
        PathBuf::from("/clone/.git/packed-refs"),
        PathBuf::from("/clone/.git/refs/heads/piece"),
    ];
    let moved = |paths: &[&str]| -> HashSet<PathBuf> { paths.iter().map(PathBuf::from).collect() };
    assert!(!head_moved(
        &refs,
        &moved(&[
            "/clone/.git/refs/heads/piece",
            "/clone/.git/packed-refs",
            "/clone/.git/worktrees/piece/HEAD.lock",
            "/clone/.git/worktrees/other/HEAD",
            "/clone/.git/HEAD",
        ])
    ));
    assert!(head_moved(
        &refs,
        &moved(&[
            "/clone/.git/worktrees/piece/HEAD.lock",
            "/clone/.git/worktrees/piece/HEAD"
        ])
    ));
    assert!(!head_moved(&[], &moved(&["/clone/.git/HEAD"])));
}

/// What inotify, through notify, watches: one entry per folder, and a recursive watch's
/// removal takes every folder under it, whoever asked for that folder.
#[derive(Default)]
struct Inotifyish {
    live: HashMap<PathBuf, RecursiveMode>,
}

impl notify::Watcher for Inotifyish {
    fn new<F: notify::EventHandler>(_: F, _: notify::Config) -> notify::Result<Self> {
        Ok(Self::default())
    }
    fn watch(&mut self, path: &Path, mode: RecursiveMode) -> notify::Result<()> {
        self.live.insert(path.to_path_buf(), mode);
        Ok(())
    }
    fn unwatch(&mut self, path: &Path) -> notify::Result<()> {
        match self.live.remove(path) {
            Some(RecursiveMode::Recursive) => {
                self.live.retain(|below, _| !below.starts_with(path));
                Ok(())
            }
            Some(RecursiveMode::NonRecursive) => Ok(()),
            None => Err(notify::Error::watch_not_found()),
        }
    }
    fn kind() -> notify::WatcherKind {
        notify::WatcherKind::Inotify
    }
}

/// #1152 (train 55): a clone watched whole stops being watched whole while a cockpit still
/// wants its ref folder. On inotify the whole tree's removal takes the ref folder's watch
/// with it; the folder is watched again, so a commit that writes no file is still heard.
#[test]
fn a_ref_folder_under_a_tree_let_go_of_is_watched_again() {
    let clone = PathBuf::from("/clone");
    let heads = PathBuf::from("/clone/.git/refs/heads");
    let mut platform = Inotifyish::default();
    let mut watched = HashMap::new();
    settle(
        &mut platform,
        &mut watched,
        HashMap::from([
            (clone.clone(), RecursiveMode::Recursive),
            (heads.clone(), RecursiveMode::NonRecursive),
        ]),
    );
    assert_eq!(platform.live.len(), 2, "{:?}", platform.live);

    settle(
        &mut platform,
        &mut watched,
        HashMap::from([(heads.clone(), RecursiveMode::NonRecursive)]),
    );

    assert_eq!(
        platform.live,
        HashMap::from([(heads.clone(), RecursiveMode::NonRecursive)])
    );
    assert_eq!(watched, platform.live);
    // A folder outside the tree, and a tree that was never whole, are left alone.
    let mut watched = HashMap::from([
        (PathBuf::from("/other/.git"), RecursiveMode::NonRecursive),
        (PathBuf::from("/clone"), RecursiveMode::NonRecursive),
        (heads.clone(), RecursiveMode::NonRecursive),
    ]);
    let mut platform = Inotifyish {
        live: watched.clone(),
    };
    unwatch(&mut platform, &mut watched, &clone);
    assert_eq!(watched.len(), 2, "{watched:?}");
    assert!(watched.contains_key(&heads));
}

/// #1130 (L5): a listed folder swapped for a link just before its watch is added has the
/// link's target watched, on a platform that follows links. Looked at once watched, it is
/// let go of, and never counted as watched.
#[cfg(unix)]
#[test]
fn a_folder_swapped_for_a_link_before_its_watch_is_let_go_of() {
    /// [`Inotifyish`], swapping the folder it is asked to watch for a link first.
    struct Swapping {
        platform: Inotifyish,
        elsewhere: PathBuf,
    }
    impl notify::Watcher for Swapping {
        fn new<F: notify::EventHandler>(_: F, _: notify::Config) -> notify::Result<Self> {
            unreachable!()
        }
        fn watch(&mut self, path: &Path, mode: RecursiveMode) -> notify::Result<()> {
            if path.ends_with("swapped") {
                std::fs::remove_dir(path).unwrap();
                std::os::unix::fs::symlink(&self.elsewhere, path).unwrap();
            }
            self.platform.watch(path, mode)
        }
        fn unwatch(&mut self, path: &Path) -> notify::Result<()> {
            self.platform.unwatch(path)
        }
        fn kind() -> notify::WatcherKind {
            notify::WatcherKind::Inotify
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let swapped = dir.path().join("swapped");
    let kept = dir.path().join("kept");
    let elsewhere = dir.path().join("elsewhere");
    for folder in [&swapped, &kept, &elsewhere] {
        std::fs::create_dir(folder).unwrap();
    }
    let mut platform = Swapping {
        platform: Inotifyish::default(),
        elsewhere,
    };
    let mut watched = HashMap::new();

    settle(
        &mut platform,
        &mut watched,
        HashMap::from([
            (swapped.clone(), RecursiveMode::NonRecursive),
            (kept.clone(), RecursiveMode::NonRecursive),
        ]),
    );

    assert_eq!(
        watched,
        HashMap::from([(kept.clone(), RecursiveMode::NonRecursive)])
    );
    assert_eq!(platform.platform.live, watched);
}

/// #1152 (train 49's review): a re-find whose read panics lets go of its folder, so the
/// cockpit's next `HEAD` move finds its refs again rather than finding it still marked as
/// being re-found for good.
#[test]
fn a_re_find_that_panics_lets_its_folder_be_found_again() {
    let watch = BranchWatch::<crate::watchset::raw::Raw>::with_config(
        Arc::new(|_: &str, _| {}),
        crate::reader(),
        notify::Config::default(),
    );
    let at = PathBuf::from("/clone/.purlis/pieces/piece");
    watch
        .inner
        .lock()
        .unwrap()
        .refinding
        .insert(at.clone(), false);
    let handle = Arc::downgrade(&watch.inner);
    let told: Told = Arc::new(|_: &str, _| {});

    let ran = std::thread::spawn({
        let at = at.clone();
        move || {
            refind(&handle, &at, &told, || {
                panic!("the reader's answer did not read")
            })
        }
    })
    .join();

    assert!(ran.is_err(), "the re-find panicked");
    let held = watch.inner.lock().unwrap_or_else(PoisonError::into_inner);
    assert!(!held.refinding.contains_key(&at), "{:?}", held.refinding);
}

/// #1152, with a real watcher (first run on CI: it needs git): a checkout in the cockpit's
/// folder moves its HEAD to another branch, and that branch's ref is watched from then on,
/// so a commit on it that writes no file still tells the window. The branch is named into a
/// folder of its own (`topic/one`), so its ref sits where nothing was watched before.
#[test]
fn a_checkout_in_the_cockpit_has_its_new_branchs_commits_heard() {
    let (_dir, root, piece) = plane();
    let (watch, told) = watch_with(crate::reader());
    watch
        .set_from(
            "main",
            watch.ticket(),
            vec![(branch(), root_of(&root, "piece"), How::Whole)],
        )
        .unwrap();
    watch.focus("main", Some(branch()));
    // The poller's first look is its baseline; give it one.
    std::thread::sleep(Duration::from_millis(300));

    git(&piece, &["checkout", "-q", "-b", "topic/one"]);
    let (window, _) = told.recv_timeout(PATIENCE).expect("the HEAD move was told");
    assert_eq!(window, "main");
    let topic = root_of(&root, "piece")
        .refs()
        .iter()
        .find(|file| file.ends_with("refs/heads/topic/one"))
        .and_then(|file| file.parent())
        .map(PathBuf::from)
        .expect("the new branch's ref is among the refs");
    assert!(
        until(|| watch.watching().contains(&topic)),
        "{:?}",
        watch.watching()
    );
    while told.try_recv().is_ok() {}
    std::thread::sleep(Duration::from_millis(300));

    git(
        &piece,
        &["commit", "-q", "--allow-empty", "-m", "no file written"],
    );
    let (window, branches) = told.recv_timeout(PATIENCE).expect("the window was told");
    assert_eq!(window, "main");
    assert_eq!(branches, [branch()]);
}

/// #2463: a commit made right after a checkout in the cockpit, before the new branch's ref
/// is watched, moves a file no watch holds yet: the folder of `refs/heads/topic/one` is
/// watched only once the cockpit's refs are found again, and a platform reports nothing
/// from before a watch. So once that watch holds, the window is told the cockpit moved, and
/// reads it again. The test plays the platform: it reports only the `HEAD` move.
#[test]
fn a_commit_before_the_new_branchs_ref_is_watched_is_told_once_it_is() {
    use crate::watchset::raw;
    use notify::event::{EventKind, ModifyKind};
    let (_dir, root, piece) = plane();
    let found = root_of(&root, "piece");
    let head = found
        .refs()
        .iter()
        .find(|file| file.ends_with("HEAD"))
        .cloned()
        .expect("the cockpit's HEAD is among its refs");
    let topic = found
        .refs()
        .iter()
        .find(|file| file.ends_with("refs/heads/piece"))
        .and_then(|file| file.parent())
        .expect("the branch's ref is among its refs")
        .join("topic");
    // Each tell says whether the new branch's ref folder was watched when it was sent.
    let (tx, told) = mpsc::channel();
    let tx = Mutex::new(tx);
    let new_ref = topic.clone();
    let watch = BranchWatch::<raw::Raw>::with_config(
        Arc::new(move |window: &str, branches| {
            let watched = raw::watched(&new_ref) > 0;
            let _ = tx
                .lock()
                .unwrap()
                .send((window.to_string(), branches, watched));
        }),
        crate::reader(),
        notify::Config::default(),
    );
    // The re-find waits until the `HEAD` move has been told, so what is told after it is the
    // re-find's own doing, whatever the timing.
    let refind = Latch::default();
    let watch = watch.holding_refinds(refind.clone());
    watch
        .set_from("main", watch.ticket(), vec![(branch(), found, How::Whole)])
        .unwrap();
    watch.focus("main", Some(branch()));

    // Both land before the watch hears the checkout: nothing reports the commit's ref.
    git(&piece, &["checkout", "-q", "-b", "topic/one"]);
    git(
        &piece,
        &["commit", "-q", "--allow-empty", "-m", "no file written"],
    );
    raw::raw(EventKind::Modify(ModifyKind::Any), &head);

    // The `HEAD` move, told at once, before the new branch's ref is watched.
    let (window, branches, watched) = told.recv_timeout(PATIENCE).expect("the HEAD move was told");
    assert_eq!(
        (window.as_str(), branches.as_slice()),
        ("main", [branch()].as_slice())
    );
    assert!(!watched, "the re-find is held");
    refind.open();

    // Once the new ref is watched, the window is told again: the commit is read.
    let (window, branches, watched) = told
        .recv_timeout(PATIENCE)
        .expect("the window was told once the new branch's ref was watched");
    assert_eq!(
        (window.as_str(), branches.as_slice()),
        ("main", [branch()].as_slice())
    );
    assert!(watched, "told before the new branch's ref was watched");
}

/// #1152, with a real watcher (first run on CI: it needs git): a commit that writes no file
/// in the cockpit's folder tells its window; the same in a branch the window listens to but
/// is not focused on does not, and once the window leaves the cockpit its refs are not
/// watched any more.
#[test]
fn a_commit_with_no_file_write_moves_the_cockpit_and_only_the_cockpit() {
    let (_dir, root, piece) = plane();
    let other = purlis_core::worktree::add(&root, "alpha", "thing", "other", None)
        .unwrap()
        .path;
    let (watch, told) = watch_with(crate::reader());
    let (found, found_other) = (root_of(&root, "piece"), root_of(&root, "other"));
    let heads = found
        .refs()
        .iter()
        .find(|file| file.ends_with("refs/heads/piece"))
        .and_then(|file| file.parent())
        .map(PathBuf::from)
        .expect("the branch's ref is among its refs");
    watch
        .set_from(
            "main",
            watch.ticket(),
            vec![
                (branch(), found, How::Whole),
                (named("other"), found_other, How::Whole),
            ],
        )
        .unwrap();
    assert!(!watch.watching().contains(&heads), "no cockpit yet");
    watch.focus("main", Some(branch()));
    assert!(watch.watching().contains(&heads), "{:?}", watch.watching());
    // The poller's first look is its baseline; give it one.
    std::thread::sleep(Duration::from_millis(300));

    git(
        &other,
        &["commit", "-q", "--allow-empty", "-m", "elsewhere"],
    );
    assert!(told.recv_timeout(Duration::from_secs(2)).is_err());

    git(
        &piece,
        &["commit", "-q", "--allow-empty", "-m", "no file written"],
    );
    let (window, branches) = told.recv_timeout(PATIENCE).expect("the window was told");
    assert_eq!(window, "main");
    assert_eq!(branches, [branch()]);

    watch.focus("main", None);
    assert!(!watch.watching().contains(&heads), "{:?}", watch.watching());
}
