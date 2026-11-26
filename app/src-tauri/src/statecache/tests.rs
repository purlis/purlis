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
            std::fs::create_dir_all(path.join(".git")).expect("a folder");
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
    let mut round = None;
    loop {
        let (r, said) = f.cache.states(&f.folders, round);
        round = Some(r);
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
    answered(&cache, &f.folders);
    answered(&cache, &f.folders);
    assert_eq!(cache.read_count(), 2);
}

#[test]
fn with_no_watch_nothing_is_kept() {
    let f = fixture(3);
    let cache = StateCache::watched_by(|_| None, |_| (Some(0), None));
    answered(&cache, &f.folders);
    answered(&cache, &f.folders);
    assert_eq!(cache.read_count(), 6);
}

/// Asks until every folder is answered, and gives the answers.
fn answered(cache: &StateCache, folders: &[Folder]) -> Vec<Read> {
    let mut round = None;
    loop {
        let (r, said) = cache.states(folders, round);
        round = Some(r);
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
    std::fs::remove_dir(f.folders[0].path.join(".git")).unwrap();
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
    let home = f.folders[0].path.join(".git");
    let dir = home.join(format!("{FLUSH_PREFIX}1-0"));
    let file = home.join(format!("{FLUSH_PREFIX}1-1"));
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

/// The folders a [`Dropping`] watcher still hears.
type Live = Arc<Mutex<HashSet<PathBuf>>>;

/// A watcher that, as inotify does, drops a folder's watch when the folder itself is removed
/// or renamed: it tells that once, and nothing in the folder again until it is watched again.
struct Dropping {
    inner: notify::PollWatcher,
    live: Live,
}

impl notify::Watcher for Dropping {
    fn new<F: EventHandler>(handler: F, config: notify::Config) -> notify::Result<Self> {
        Ok(Self {
            inner: notify::PollWatcher::new(handler, config)?,
            live: Live::default(),
        })
    }

    fn watch(&mut self, path: &Path, mode: RecursiveMode) -> notify::Result<()> {
        self.inner.watch(path, mode)?;
        lock(&self.live).insert(path.to_path_buf());
        Ok(())
    }

    fn unwatch(&mut self, path: &Path) -> notify::Result<()> {
        lock(&self.live).remove(path);
        self.inner.unwatch(path)
    }

    fn kind() -> notify::WatcherKind {
        notify::WatcherKind::PollWatcher
    }
}

fn dropping(events: std::sync::mpsc::Sender<notify::Event>) -> Option<Box<dyn Watcher + Send>> {
    let live = Live::default();
    let heard = Arc::clone(&live);
    let mut handler = as_platform(events);
    let forward = move |event: notify::Result<notify::Event>| {
        let paths = match &event {
            Ok(event) => event.paths.clone(),
            Err(err) => err.paths.clone(),
        };
        let mut live = lock(&heard);
        if !paths.is_empty() && !paths.iter().any(|p| live.iter().any(|d| p.starts_with(d))) {
            return;
        }
        if let Ok(event) = &event
            && matches!(
                event.kind,
                EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
            )
        {
            for path in &event.paths {
                live.remove(path);
            }
        }
        drop(live);
        handler.handle_event(event);
    };
    let inner = notify::PollWatcher::new(forward, poll_config()).ok()?;
    Some(Box::new(Dropping { inner, live }))
}

/// How many files a folder holds, its `.git` aside.
fn files(path: &Path) -> Read {
    let n = std::fs::read_dir(path)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name() != ".git")
        .count();
    (Some(n as u32), None)
}

#[test]
fn a_folder_removed_and_made_again_is_watched_again() {
    let f = fixture(1);
    let path = f.folders[0].path.clone();
    let cache = StateCache::watched_by(dropping, files);
    assert_eq!(answered(&cache, &f.folders), [(Some(0), None)]);
    std::fs::remove_dir_all(&path).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    std::fs::create_dir_all(path.join(".git")).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(answered(&cache, &f.folders), [(Some(0), None)]);
    let before = cache.read_count();
    assert_eq!(answered(&cache, &f.folders), [(Some(0), None)]);
    assert_eq!(
        cache.read_count(),
        before,
        "the made-again folder is not watched"
    );
    std::fs::write(path.join("new"), "x").unwrap();
    assert_eq!(answered(&cache, &f.folders), [(Some(1), None)]);
}

thread_local! {
    /// Every folder the refusing watcher was asked to stop watching.
    static UNWATCHED: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

/// A watcher that, as inotify past `max_user_watches`, refuses to watch any `.git`.
struct Refusing(notify::PollWatcher);

impl notify::Watcher for Refusing {
    fn new<F: EventHandler>(handler: F, config: notify::Config) -> notify::Result<Self> {
        Ok(Self(notify::PollWatcher::new(handler, config)?))
    }

    fn watch(&mut self, path: &Path, mode: RecursiveMode) -> notify::Result<()> {
        if path.file_name().is_some_and(|n| n == ".git") {
            return Err(notify::Error::new(notify::ErrorKind::MaxFilesWatch));
        }
        self.0.watch(path, mode)
    }

    fn unwatch(&mut self, path: &Path) -> notify::Result<()> {
        UNWATCHED.with(|u| u.borrow_mut().push(path.to_path_buf()));
        self.0.unwatch(path)
    }

    fn kind() -> notify::WatcherKind {
        notify::WatcherKind::PollWatcher
    }
}

fn refusing(events: std::sync::mpsc::Sender<notify::Event>) -> Option<Box<dyn Watcher + Send>> {
    Some(Box::new(
        Refusing::new(as_platform(events), poll_config()).ok()?,
    ))
}

#[test]
fn a_watch_the_platform_refuses_is_taken_down_and_no_folder_is_kept_after() {
    let r = repo();
    let f = fixture(1);
    let cache = StateCache::watched_by(refusing, |_| (Some(0), None));
    answered(&cache, &[folder(&r.side)]);
    // The refused watch, for what it set up before it failed, and the folder's own.
    let unwatched = UNWATCHED.with(|u| u.borrow().clone());
    assert_eq!(unwatched, [r.main.join(".git"), r.side.clone()]);
    answered(&cache, &f.folders);
    answered(&cache, &f.folders);
    assert_eq!(
        cache.read_count(),
        3,
        "a folder was kept past a refused watch"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn on_linux_no_folder_is_watched() {
    assert!(platform(std::sync::mpsc::channel().0).is_none());
}

#[test]
fn a_folder_asked_while_another_is_read_is_not_held_behind_it() {
    let f = fixture(2);
    let slow = f.folders[0].path.clone();
    let cache = StateCache::watched_by(polled, move |path| {
        if path == slow {
            std::thread::sleep(Duration::from_secs(3));
        }
        (Some(0), None)
    });
    let first = cache.clone();
    let one = vec![f.folders[0].clone()];
    let reading = std::thread::spawn(move || first.states(&one, None).1);
    std::thread::sleep(Duration::from_millis(500));
    let start = Instant::now();
    let said = cache.states(&f.folders, None).1;
    let took = start.elapsed();
    assert!(said[1].is_some(), "the second folder was not answered");
    assert!(took < Duration::from_millis(1500), "it waited {took:?}");
    reading.join().unwrap();
}

#[test]
fn a_read_no_thread_could_start_for_is_answered_unread_and_tried_again() {
    let f = fixture(1);
    let cache = StateCache::watched_by(polled, |_| (Some(0), None));
    NO_THREADS.with(|n| n.set(true));
    let start = Instant::now();
    let said = cache.states(&f.folders, None).1;
    NO_THREADS.with(|n| n.set(false));
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "{:?}",
        start.elapsed()
    );
    assert!(matches!(said[0], Some((None, Some(_)))), "{said:?}");
    assert_eq!(answered(&cache, &f.folders), [(Some(0), None)]);
}

#[test]
fn a_folder_with_no_git_is_never_kept() {
    let dir = tempfile::tempdir().unwrap();
    let plain = [folder(&dir.path().canonicalize().unwrap())];
    let cache = StateCache::watched_by(polled, |_| (Some(0), None));
    answered(&cache, &plain);
    answered(&cache, &plain);
    assert_eq!(cache.read_count(), 2);
}

#[test]
fn a_folder_no_ask_names_for_a_while_is_forgotten_and_unwatched() {
    let r = repo();
    let one = fixture(1);
    let cache = StateCache::watched_by(polled, |_| (Some(0), None));
    answered(&cache, &[folder(&r.side)]);
    std::thread::sleep(FORGET_AFTER + Duration::from_millis(100));
    answered(&cache, &one.folders);
    assert_eq!(cache.known(), 1, "the folder no ask names is still held");
    assert_eq!(cache.watches(), 1, "its watches are still up");
}

#[test]
fn an_unwatched_repo_asked_while_another_is_read_is_read_again_after_an_edit() {
    let slow = fixture(4);
    let edited = fixture(2);
    let slow_base = slow.folders[0].path.parent().unwrap().to_path_buf();
    let cache = StateCache::watched_by(
        |_| None,
        move |path| {
            if path.starts_with(&slow_base) {
                std::thread::sleep(Duration::from_secs(1));
            }
            files(path)
        },
    );
    answered(&cache, &slow.folders);
    answered(&cache, &edited.folders);
    std::fs::write(edited.folders[0].path.join("new"), "x").unwrap();
    // The window asks every repo at once: the slow one's reads are going when this one asks.
    let first = cache.clone();
    let folders = slow.folders.clone();
    let reading = std::thread::spawn(move || answered(&first, &folders));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        answered(&cache, &edited.folders),
        [(Some(1), None), (Some(0), None)],
        "the edited folder was answered from the last refresh"
    );
    reading.join().unwrap();
}

#[test]
fn a_folder_forgotten_while_it_is_read_does_not_hold_its_ask_for_the_full_wait() {
    let f = fixture(1);
    let path = f.folders[0].path.clone();
    let cache = StateCache::watched_by(polled, |_| {
        std::thread::sleep(Duration::from_secs(1));
        (Some(0), None)
    });
    let asking = cache.clone();
    let folders = f.folders.clone();
    let start = Instant::now();
    let ask = std::thread::spawn(move || asking.states(&folders, None).1);
    std::thread::sleep(Duration::from_millis(300));
    std::fs::remove_dir_all(&path).unwrap();
    let said = ask.join().unwrap();
    let took = start.elapsed();
    assert!(took < Duration::from_secs(4), "the ask waited {took:?}");
    assert!(
        said[0].is_none(),
        "a forgotten folder was answered: {said:?}"
    );
}

thread_local! {
    /// Every folder the losing watcher was asked to watch.
    static LOSING_WATCHED: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

/// A watcher whose first watch hears its own folder removed before the watch is set up.
struct Losing {
    inner: notify::PollWatcher,
    events: std::sync::mpsc::Sender<notify::Event>,
}

impl notify::Watcher for Losing {
    fn new<F: EventHandler>(_: F, _: notify::Config) -> notify::Result<Self> {
        unreachable!("made by `losing`")
    }

    fn watch(&mut self, path: &Path, mode: RecursiveMode) -> notify::Result<()> {
        let first = LOSING_WATCHED.with(|w| {
            let mut w = w.borrow_mut();
            w.push(path.to_path_buf());
            w.len() == 1
        });
        if first {
            let removed = notify::Event::new(EventKind::Remove(notify::event::RemoveKind::Folder))
                .add_path(path.to_path_buf());
            let _ = self.events.send(removed);
            std::thread::sleep(Duration::from_millis(200));
        }
        self.inner.watch(path, mode)
    }

    fn unwatch(&mut self, path: &Path) -> notify::Result<()> {
        self.inner.unwatch(path)
    }

    fn kind() -> notify::WatcherKind {
        notify::WatcherKind::PollWatcher
    }
}

fn losing(events: std::sync::mpsc::Sender<notify::Event>) -> Option<Box<dyn Watcher + Send>> {
    let inner = notify::PollWatcher::new(as_platform(events.clone()), poll_config()).ok()?;
    Some(Box::new(Losing { inner, events }))
}

#[test]
fn a_folder_removed_while_its_watch_is_set_up_is_watched_afresh() {
    let f = fixture(1);
    let cache = StateCache::watched_by(losing, |_| (Some(0), None));
    assert_eq!(answered(&cache, &f.folders), [(Some(0), None)]);
    let watched = LOSING_WATCHED.with(|w| w.borrow().clone());
    assert_eq!(
        watched,
        [f.folders[0].path.clone(), f.folders[0].path.clone()],
        "the folder lost while being watched was kept on that watch"
    );
}

#[test]
fn a_refresh_of_a_repo_whose_last_round_is_still_reading_reads_an_edited_folder_again() {
    let f = fixture(3);
    let quick = f.folders[0].path.clone();
    let read_quick = quick.clone();
    let cache = StateCache::watched_by(
        |_| None,
        move |path| {
            if path != read_quick {
                std::thread::sleep(Duration::from_secs(3));
            }
            files(path)
        },
    );
    // The window's first refresh: the quick folder is read, the others still are.
    let (_, said) = cache.states(&f.folders, None);
    assert_eq!(said[0], Some((Some(0), None)));
    assert!(said[1].is_none(), "{said:?}");
    std::fs::write(quick.join("new"), "x").unwrap();
    // The window refreshes while the first refresh's reads still go on: a new round.
    assert_eq!(
        answered(&cache, &f.folders),
        [(Some(1), None), (Some(0), None), (Some(0), None)],
        "the edited folder was answered from the last refresh"
    );
}

#[test]
fn past_the_watch_limit_an_event_in_a_folder_drops_its_unkept_answer() {
    let f = fixture(2);
    let quick = f.folders[0].path.clone();
    let slow = f.folders[1].path.clone();
    let cache = StateCache::watched_by(polled, move |path| {
        if path == slow {
            std::thread::sleep(Duration::from_secs(4));
        }
        files(path)
    });
    let (round, _) = cache.states(&f.folders, None);
    lock(&cache.0.shared.0).limit();
    // The quick folder is read again, unwatched now, while the slow one still is.
    let mut said = cache.states(&f.folders, Some(round)).1;
    while said[0].is_none() {
        said = cache.states(&f.folders, Some(round)).1;
    }
    assert_eq!(said[0], Some((Some(0), None)));
    assert!(said[1].is_none(), "{said:?}");
    std::fs::write(quick.join("new"), "x").unwrap();
    // The watch is still up and hears it.
    std::thread::sleep(Duration::from_millis(300));
    let mut said = cache.states(&f.folders, Some(round)).1;
    while said[0].is_none() {
        said = cache.states(&f.folders, Some(round)).1;
    }
    assert_eq!(
        said[0],
        Some((Some(1), None)),
        "an answer read before the edit was given after it"
    );
}

#[test]
fn an_unwatched_folder_read_between_two_asks_of_one_refresh_is_not_read_again() {
    let f = fixture(2);
    let slow = f.folders[1].path.clone();
    let cache = StateCache::watched_by(
        |_| None,
        move |path| {
            if path == slow {
                std::thread::sleep(Duration::from_secs(1));
            }
            (Some(0), None)
        },
    );
    let (round, said) = cache.states(&f.folders, None);
    assert!(said[1].is_none(), "{said:?}");
    // The slow folder's read, the refresh's last, finishes before the window asks again.
    std::thread::sleep(Duration::from_millis(1500));
    let (_, said) = cache.states(&f.folders, Some(round));
    assert!(said.iter().all(Option::is_some), "{said:?}");
    assert_eq!(cache.read_count(), 2, "the refresh read its folders again");
}
