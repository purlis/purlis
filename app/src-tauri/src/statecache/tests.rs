// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::time::Instant;

/// How long one folder's read takes here: `git status` took 0.8 s a folder on the machine that
/// measured 47 s; this keeps the shape (130 folders, eight at a time) and runs in seconds.
const A_READ: Duration = Duration::from_millis(300);

/// The sandbox this runs in refuses FSEvents, so the tests watch by polling every 20 ms: the same
/// events in the same order, which is all the cache relies on.
fn polled(events: std::sync::mpsc::Sender<notify::Event>) -> Option<Box<dyn notify::Watcher + Send>> {
    let config = notify::Config::default().with_poll_interval(Duration::from_millis(20));
    let handler = crate::watchset::sender(events);
    Some(Box::new(notify::PollWatcher::new(handler, config).ok()?))
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
            Folder {
                piece: format!("branch-{n:03}"),
                path,
            }
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
    assert!(asked.asks.windows(2).all(|w| w[0].1 < w[1].1), "{:?}", asked.asks);
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
    assert!(asked.took < Duration::from_secs(1), "the second ask took {:?}", asked.took);
}

#[test]
fn a_folder_that_changed_is_read_again_and_the_others_are_not() {
    let f = fixture(12);
    ask(&f);
    let before = f.cache.read_count();
    std::fs::write(f.folders[7].path.join("new.txt"), "x").unwrap();
    ask(&f);
    assert_eq!(f.cache.read_count(), before + 1, "only the changed folder is read again");
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
