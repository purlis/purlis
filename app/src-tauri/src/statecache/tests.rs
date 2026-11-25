// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::cell::RefCell;
use std::process::Command;
use std::time::Instant;

/// How long one folder's read takes here: `git status` took 0.8 s a folder on the machine that
/// measured 47 s; this keeps the shape (130 folders, eight at a time) and runs in seconds.
const A_READ: Duration = Duration::from_millis(300);

/// How often the tests' watcher polls.
const POLL: Duration = Duration::from_millis(20);

/// How long the tests' watcher holds a flush marker's event back: a few polls.
const HOLD: Duration = Duration::from_millis(100);

/// The sandbox this runs in refuses FSEvents, so the tests watch by polling every 20 ms. Two
/// things a platform watcher never does are undone, so the events are the platform's, in its
/// order, which is all the cache relies on: a poll's walk can reach a marker made after a
/// change before it reaches the change, so a marker's event is held back a few polls; and a
/// marker removed between a poll's listing and its look at it is an error, not a removal.
fn polled(
    events: std::sync::mpsc::Sender<notify::Event>,
) -> Option<Box<dyn notify::Watcher + Send>> {
    let config = poll_config();
    Some(Box::new(
        notify::PollWatcher::new(as_platform(events), config).ok()?,
    ))
}

/// Polls every [`POLL`], comparing contents: a poll's times are whole seconds, so a file
/// rewritten within the second it was made would otherwise say nothing.
fn poll_config() -> notify::Config {
    notify::Config::default()
        .with_poll_interval(POLL)
        .with_compare_contents(true)
}

fn is_marker(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with(FLUSH_PREFIX))
}

/// The cache's own handler, fed as a platform watcher would feed it (see [`polled`]).
fn as_platform(events: std::sync::mpsc::Sender<notify::Event>) -> impl EventHandler {
    let mut inner = handler(held_back(events));
    move |event: notify::Result<notify::Event>| {
        if let Err(err) = &event {
            let gone = matches!(&err.kind, notify::ErrorKind::Io(io) if io.kind() == std::io::ErrorKind::NotFound);
            if gone && !err.paths.is_empty() && err.paths.iter().all(|p| is_marker(p)) {
                return;
            }
        }
        inner.handle_event(event);
    }
}

/// Sends every event on at once but a flush marker's, which goes [`HOLD`] later.
fn held_back(
    events: std::sync::mpsc::Sender<notify::Event>,
) -> std::sync::mpsc::Sender<notify::Event> {
    let (send, take) = std::sync::mpsc::channel::<notify::Event>();
    std::thread::spawn(move || {
        let mut held: VecDeque<(Instant, notify::Event)> = VecDeque::new();
        loop {
            let wait = held.front().map_or(Duration::from_secs(60), |(at, _)| {
                at.saturating_duration_since(Instant::now())
            });
            match take.recv_timeout(wait) {
                Ok(event)
                    if !event.paths.is_empty() && event.paths.iter().all(|p| is_marker(p)) =>
                {
                    held.push_back((Instant::now() + HOLD, event));
                }
                Ok(event) => {
                    if events.send(event).is_err() {
                        return;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            }
            while held.front().is_some_and(|(at, _)| *at <= Instant::now()) {
                let (_, event) = held.pop_front().unwrap();
                if events.send(event).is_err() {
                    return;
                }
            }
        }
    });
    send
}

struct Fixture {
    _dir: tempfile::TempDir,
    folders: Vec<Folder>,
    cache: StateCache,
}

fn fixture(count: usize) -> Fixture {
    let dir = tempfile::tempdir().expect("a temp dir");
    let base = dir.path().canonicalize().expect("a real path");
    let folders = (0..count)
        .map(|n| {
            let path = base.join(format!("branch-{n:03}"));
            std::fs::create_dir(&path).expect("a folder");
            Folder { path }
        })
        .collect();
    let cache = StateCache::watched_by(polled, move |_| {
        std::thread::sleep(A_READ);
        (Some(0), None)
    });
    Fixture {
        _dir: dir,
        folders,
        cache,
    }
}

/// What asking until every folder is answered looked like: each ask's time of return and how
/// many folders it had answered, and the whole time.
struct Asked {
    asks: Vec<(Duration, usize)>,
    took: Duration,
}

/// Asks as the window does: again, at once, while any folder is still being read.
fn ask(f: &Fixture) -> Asked {
    let start = Instant::now();
    let mut asks = Vec::new();
    loop {
        let said = f.cache.states(&f.folders);
        let answered = said.iter().flatten().count();
        asks.push((start.elapsed(), answered));
        if answered == f.folders.len() {
            return Asked {
                asks,
                took: start.elapsed(),
            };
        }
    }
}

#[test]
fn the_first_answer_of_130_folders_comes_within_a_second() {
    let f = fixture(130);
    let asked = ask(&f);
    let (at, answered) = asked.asks[0];
    assert!(answered > 0, "the first ask answered nothing");
    assert!(at < Duration::from_secs(1), "first answer after {at:?}");
    // Each ask brings more, and the last has every folder.
    assert!(
        asked.asks.windows(2).all(|w| w[0].1 < w[1].1),
        "{:?}",
        asked.asks
    );
    assert_eq!(asked.asks.last().unwrap().1, 130);
}

#[test]
fn asking_again_with_nothing_changed_reads_no_folder_and_takes_under_a_second() {
    let f = fixture(130);
    ask(&f);
    let before = f.cache.read_count();
    let asked = ask(&f);
    assert_eq!(asked.asks.last().unwrap().1, 130);
    assert_eq!(f.cache.read_count(), before, "a folder was read again");
    assert!(
        asked.took < Duration::from_secs(1),
        "the second ask took {:?}",
        asked.took
    );
}

#[test]
fn a_folder_that_changed_is_read_again_and_the_others_are_not() {
    let f = fixture(12);
    ask(&f);
    let before = f.cache.read_count();
    std::fs::write(f.folders[7].path.join("new.txt"), "x").unwrap();
    ask(&f);
    assert_eq!(
        f.cache.read_count(),
        before + 1,
        "only the changed folder is read again"
    );
}

#[test]
fn a_watch_that_lost_track_moves_every_folder() {
    let f = fixture(12);
    ask(&f);
    let before = f.cache.read_count();
    f.cache.lost_track();
    ask(&f);
    assert_eq!(f.cache.read_count(), before + 12);
}

#[test]
fn a_refusal_is_never_kept() {
    let f = fixture(1);
    let cache = StateCache::watched_by(polled, |_| (None, Some("not a repository".into())));
    while cache.states(&f.folders).iter().any(Option::is_none) {}
    while cache.states(&f.folders).iter().any(Option::is_none) {}
    assert_eq!(cache.read_count(), 2);
}

#[test]
fn with_no_watch_nothing_is_kept() {
    let f = fixture(3);
    let cache = StateCache::watched_by(|_| None, |_| (Some(0), None));
    while cache.states(&f.folders).iter().any(Option::is_none) {}
    while cache.states(&f.folders).iter().any(Option::is_none) {}
    assert_eq!(cache.read_count(), 6);
}

/// Asks until every folder is answered, and gives the answers.
fn answered(cache: &StateCache, folders: &[Folder]) -> Vec<Read> {
    loop {
        let said = cache.states(folders);
        if said.iter().all(Option::is_some) {
            return said.into_iter().flatten().collect();
        }
    }
}

/// What the window's reader says of a folder: `git status`'s count, as `worktrees::counted`.
fn status(path: &Path) -> Read {
    match purlis_core::repos::state_of(path) {
        Ok(state) => (Some(state.tracked + state.untracked), None),
        Err(unreadable) => (None, Some(unreadable.why)),
    }
}

fn git(dir: &Path, args: &[&str]) {
    let out = purlis_core::forklock::output(
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A clone at `main` with two commits, and linked worktrees `side` (at the first, on branch
/// `side`) and `other`, under `trees/` beside it as purlis keeps them.
struct Repo {
    _dir: tempfile::TempDir,
    main: PathBuf,
    side: PathBuf,
    other: PathBuf,
}

fn repo() -> Repo {
    let dir = tempfile::tempdir().expect("a temp dir");
    let base = dir.path().canonicalize().expect("a real path");
    let main = base.join("main");
    std::fs::create_dir(&main).unwrap();
    git(&main, &["init", "-q", "-b", "main"]);
    std::fs::write(main.join("f"), "1").unwrap();
    git(&main, &["add", "f"]);
    git(&main, &["commit", "-q", "-m", "one"]);
    std::fs::write(main.join("f"), "2").unwrap();
    git(&main, &["commit", "-q", "-am", "two"]);
    let side = base.join("trees/side");
    let other = base.join("trees/other");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            side.to_str().unwrap(),
            "HEAD~1",
        ],
    );
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "other",
            other.to_str().unwrap(),
        ],
    );
    Repo {
        _dir: dir,
        main,
        side,
        other,
    }
}

fn folder(path: &Path) -> Folder {
    Folder {
        path: path.to_path_buf(),
    }
}

#[test]
fn a_branch_moved_from_the_main_clone_is_never_answered_clean_in_its_worktree() {
    let r = repo();
    let cache = StateCache::watched_by(polled, status);
    let side = [folder(&r.side)];
    assert_eq!(answered(&cache, &side), [(Some(0), None)]);
    git(&r.main, &["update-ref", "refs/heads/side", "main"]);
    assert_eq!(answered(&cache, &side), [(Some(1), None)]);
}

#[test]
fn an_exclude_emptied_in_the_repo_is_heard_by_every_worktree() {
    let r = repo();
    std::fs::write(r.main.join(".git/info/exclude"), "ign\n").unwrap();
    std::fs::write(r.side.join("ign"), "x").unwrap();
    std::fs::write(r.other.join("ign"), "x").unwrap();
    let cache = StateCache::watched_by(polled, status);
    let both = [folder(&r.side), folder(&r.other)];
    assert_eq!(answered(&cache, &both), [(Some(0), None), (Some(0), None)]);
    std::fs::write(r.main.join(".git/info/exclude"), "").unwrap();
    assert_eq!(answered(&cache, &both), [(Some(1), None), (Some(1), None)]);
}

#[test]
fn a_worktree_whose_gitdir_is_relative_hears_its_own_index() {
    let r = repo();
    std::fs::write(
        r.side.join(".git"),
        "gitdir: ../../main/.git/worktrees/side\n",
    )
    .unwrap();
    let cache = StateCache::watched_by(polled, status);
    let side = [folder(&r.side)];
    assert_eq!(answered(&cache, &side), [(Some(0), None)]);
    // Only the index, in the gitdir, changes: `D  f` and `?? f`.
    git(&r.side, &["rm", "-q", "--cached", "f"]);
    assert_eq!(answered(&cache, &side), [(Some(2), None)]);
}

#[test]
fn a_folder_whose_gitdir_cannot_be_found_is_never_kept() {
    let f = fixture(1);
    std::fs::write(f.folders[0].path.join(".git"), "gitdir: ../nowhere\n").unwrap();
    let cache = StateCache::watched_by(polled, |_| (Some(0), None));
    answered(&cache, &f.folders);
    answered(&cache, &f.folders);
    assert_eq!(cache.read_count(), 2);
}

#[test]
fn a_worktree_gone_from_its_repos_listing_is_forgotten_and_read_afresh_when_back() {
    let r = repo();
    let cache = StateCache::watched_by(polled, status);
    let both = [folder(&r.side), folder(&r.other)];
    answered(&cache, &both);
    answered(&cache, &[folder(&r.side)]);
    assert_eq!(cache.known(), 1, "the unlisted worktree is still held");
    std::fs::write(r.other.join("new"), "x").unwrap();
    assert_eq!(answered(&cache, &both)[1], (Some(1), None));
}

/// A handler shared between a watcher and the test that speaks through it.
type SharedHandler = Arc<Mutex<Box<dyn EventHandler>>>;

thread_local! {
    /// What the scripted watcher does in the gap of its next watch.
    static IN_THE_GAP: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
    /// Every folder the scripted watcher was asked to watch.
    static WATCHED: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
    /// The scripted watcher's handler, to tell it an error the platform would.
    static HANDLER: RefCell<Option<SharedHandler>> = const { RefCell::new(None) };
}

/// A watcher that, as notify 8.2.2's FSEvents one does, starts again from now whenever a
/// folder is added or removed: it is the polling watcher, its watches taken down and put back
/// (so a change in between is never told), with [`IN_THE_GAP`] run while they are down.
struct Restarting {
    inner: notify::PollWatcher,
    paths: Vec<PathBuf>,
}

impl notify::Watcher for Restarting {
    fn new<F: EventHandler>(handler: F, config: notify::Config) -> notify::Result<Self> {
        Ok(Self {
            inner: notify::PollWatcher::new(handler, config)?,
            paths: Vec::new(),
        })
    }

    fn watch(&mut self, path: &Path, _: RecursiveMode) -> notify::Result<()> {
        WATCHED.with(|w| w.borrow_mut().push(path.to_path_buf()));
        self.restart(|paths| paths.push(path.to_path_buf()))
    }

    fn unwatch(&mut self, path: &Path) -> notify::Result<()> {
        self.restart(|paths| paths.retain(|p| p != path))
    }

    fn kind() -> notify::WatcherKind {
        notify::WatcherKind::PollWatcher
    }
}

impl Restarting {
    fn restart(&mut self, change: impl FnOnce(&mut Vec<PathBuf>)) -> notify::Result<()> {
        for path in &self.paths {
            let _ = self.inner.unwatch(path);
        }
        if let Some(gap) = IN_THE_GAP.with(|g| g.borrow_mut().take()) {
            gap();
        }
        change(&mut self.paths);
        for path in &self.paths {
            self.inner.watch(path, RecursiveMode::Recursive)?;
        }
        Ok(())
    }
}

fn restarting(events: std::sync::mpsc::Sender<notify::Event>) -> Option<Box<dyn Watcher + Send>> {
    let shared: SharedHandler = Arc::new(Mutex::new(Box::new(as_platform(events))));
    HANDLER.with(|h| *h.borrow_mut() = Some(Arc::clone(&shared)));
    let forward = move |event: notify::Result<notify::Event>| lock(&shared).handle_event(event);
    let config = poll_config();
    Some(Box::new(Restarting::new(forward, config).ok()?))
}

#[test]
fn a_change_while_another_repo_is_being_watched_is_heard() {
    let one = fixture(1);
    let two = fixture(1);
    let cache = StateCache::watched_by(restarting, |_| (Some(0), None));
    answered(&cache, &one.folders);
    let changed = one.folders[0].path.join("new.txt");
    IN_THE_GAP
        .with(|g| *g.borrow_mut() = Some(Box::new(move || std::fs::write(changed, "x").unwrap())));
    answered(&cache, &two.folders);
    let before = cache.read_count();
    answered(&cache, &one.folders);
    assert_eq!(
        cache.read_count(),
        before + 1,
        "the change in the gap was never heard"
    );
}

#[test]
fn a_worktree_is_watched_itself_with_its_repo_never_the_folder_it_sits_in() {
    let r = repo();
    let cache = StateCache::watched_by(restarting, status);
    answered(&cache, &[folder(&r.side)]);
    let watched = WATCHED.with(|w| w.borrow().clone());
    assert_eq!(watched, [r.side.clone(), r.main.join(".git")]);
}

#[test]
fn past_the_platforms_watch_limit_no_folder_is_kept() {
    let f = fixture(1);
    let cache = StateCache::watched_by(restarting, |_| (Some(0), None));
    answered(&cache, &f.folders);
    let limit = notify::Error::new(notify::ErrorKind::MaxFilesWatch);
    HANDLER.with(|h| lock(h.borrow().as_ref().unwrap()).handle_event(Err(limit)));
    std::thread::sleep(READ_EVERY);
    answered(&cache, &f.folders);
    std::thread::sleep(READ_EVERY);
    let before = cache.read_count();
    answered(&cache, &f.folders);
    assert_eq!(
        cache.read_count(),
        before + 1,
        "a folder was kept past the limit"
    );
}

#[test]
fn a_folder_that_keeps_moving_is_read_at_most_once_every_two_seconds_and_never_clean_meanwhile() {
    let f = fixture(1);
    let target = f.folders[0].path.join("target");
    std::fs::create_dir(&target).unwrap();
    answered(&f.cache, &f.folders);
    let before = f.cache.read_count();
    let building = Duration::from_secs(3);
    let start = Instant::now();
    let build = std::thread::spawn(move || {
        let mut n = 0u64;
        while start.elapsed() < building {
            std::fs::write(target.join("out"), n.to_string()).unwrap();
            n += 1;
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    // The build's first writes are heard before the ask.
    std::thread::sleep(Duration::from_millis(100));
    let said = answered(&f.cache, &f.folders);
    let at = start.elapsed();
    build.join().unwrap();
    assert_eq!(said, [(Some(0), None)]);
    assert!(
        at >= building,
        "answered clean at {at:?}, while the build still wrote"
    );
    let reads = f.cache.read_count() - before;
    assert!(reads <= 4, "{reads} reads in {at:?}");
}

#[test]
fn flush_markers_a_crash_left_are_swept_when_their_folder_is_watched() {
    let f = fixture(1);
    let dir = f.folders[0].path.join(format!("{FLUSH_PREFIX}1-0"));
    let file = f.folders[0].path.join(format!("{FLUSH_PREFIX}1-1"));
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(&file, "").unwrap();
    answered(&f.cache, &f.folders);
    assert!(!dir.exists() && !file.exists(), "a marker was left");
}

#[test]
fn no_flush_marker_is_remembered_once_its_flush_is_done() {
    let f = fixture(3);
    for _ in 0..5 {
        answered(&f.cache, &f.folders);
    }
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(f.cache.markers_held(), 0);
}

#[test]
fn a_worktrees_flush_marker_a_crash_left_in_its_gitdir_is_swept() {
    let r = repo();
    let gitdir = r.main.join(".git/worktrees/side");
    let stale = gitdir.join(format!("{FLUSH_PREFIX}1-0"));
    std::fs::create_dir(&stale).unwrap();
    let cache = StateCache::watched_by(polled, status);
    answered(&cache, &[folder(&r.side)]);
    assert!(!stale.exists(), "the marker was left");
}
