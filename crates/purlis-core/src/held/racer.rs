// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A test's racer: swaps a store with a link beside it as fast as it can while a reader or a
//! writer runs, as the #1064 tests do, so a test can say what the code under it never reads or
//! writes however the swap falls (#1082, #1083).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// A thread exchanging two names in one directory until it is stopped.
pub(crate) struct Racer {
    stop: Arc<AtomicBool>,
    swaps: Arc<AtomicUsize>,
    thread: std::thread::JoinHandle<()>,
}

impl Racer {
    /// Exchange `one` and `other` (a store and a link beside it) over and over.
    pub(crate) fn swapping(one: PathBuf, other: PathBuf) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let swaps = Arc::new(AtomicUsize::new(0));
        let thread = {
            let (stop, swaps) = (stop.clone(), swaps.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    if rustix::fs::renameat_with(
                        rustix::fs::CWD,
                        &one,
                        rustix::fs::CWD,
                        &other,
                        rustix::fs::RenameFlags::EXCHANGE,
                    )
                    .is_ok()
                    {
                        swaps.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        };
        Self {
            stop,
            swaps,
            thread,
        }
    }

    /// Stop, and how many swaps it made: a test asserts some were, or it proved nothing.
    pub(crate) fn stop(self) -> usize {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.thread.join();
        self.swaps.load(Ordering::Relaxed)
    }
}

/// How many times a test asks its reader while the racer swaps.
pub(crate) const ASKS: usize = 300;

/// A project at a fresh temporary folder, canonical, with `count` memories in each store named
/// (`workspaces/alpha/todos`, `personas/devops/memory`), each titled and indexed `title`.
pub(crate) fn project(stores: &[(&str, usize, &str)]) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    for (store, count, title) in stores {
        let dir = root.join(store);
        std::fs::create_dir_all(&dir).unwrap();
        let mut index = format!("# {title}\n\n");
        for n in 0..*count {
            let name = format!("{}-{n}.md", title.to_lowercase());
            std::fs::write(dir.join(&name), format!("# {title} {n}\n\n{title}.\n")).unwrap();
            index.push_str(&format!("- [{title} {n}]({name})\n"));
        }
        std::fs::write(dir.join(crate::memstore::INDEX), index).unwrap();
    }
    (tmp, root)
}

/// Swap `store` (a path below `root`) with a link beside it, named `swap`, to `target` (a
/// path relative to the store's folder), for as long as `ask` is asked [`ASKS`] times; every
/// answer is handed to `check`. Asserts the racer swapped at least once.
pub(crate) fn while_swapped<T>(
    root: &std::path::Path,
    store: &str,
    target: &str,
    mut ask: impl FnMut() -> T,
    mut check: impl FnMut(T),
) {
    let at = root.join(store);
    let beside = at.with_file_name("swap");
    std::os::unix::fs::symlink(target, &beside).unwrap();
    let racer = Racer::swapping(at.clone(), beside.clone());
    for _ in 0..ASKS {
        check(ask());
    }
    let swaps = racer.stop();
    // Put back as it was, whichever way the last swap left it.
    if std::fs::symlink_metadata(&at).is_ok_and(|m| m.file_type().is_symlink()) {
        let _ = rustix::fs::renameat_with(
            rustix::fs::CWD,
            &at,
            rustix::fs::CWD,
            &beside,
            rustix::fs::RenameFlags::EXCHANGE,
        );
    }
    let _ = std::fs::remove_file(&beside);
    assert!(swaps > 0, "the racer never swapped {store}");
}
