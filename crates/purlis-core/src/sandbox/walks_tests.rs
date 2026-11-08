// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session host's own walks of a profile's folders, which run outside any wrap while a chat
//! on the same profile may be moving folders under them: each must reach nothing a link points
//! at, however the folders it walks are swapped in the middle of a walk.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// How long each race is run: long enough that a walk resolving its paths again by name loses
/// it many times over on any machine measured, short enough to stay a unit test.
const RACE: Duration = Duration::from_secs(3);

/// A folder at `inside` and one at `outside`, swapped for each other again and again until
/// `stop` is set, as a chat that can write both would: each rename is one a sandboxed chat is
/// allowed, since neither makes a link.
fn swapping(
    inside: PathBuf,
    outside: PathBuf,
    stop: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let aside = outside.with_extension("aside");
        while !stop.load(Ordering::Relaxed) {
            let _ = std::fs::rename(&inside, &aside);
            let _ = std::fs::rename(&outside, &inside);
            let _ = std::fs::rename(&aside, &outside);
            let _ = std::fs::rename(&inside, &aside);
            let _ = std::fs::rename(&outside, &inside);
            let _ = std::fs::rename(&aside, &outside);
        }
    })
}

/// The victim: a folder outside every profile, holding a folder, a file and a link.
fn victim(at: &Path) {
    std::fs::create_dir_all(at.join("kept")).expect("a folder");
    std::fs::write(at.join("kept/file"), "kept").expect("a file");
    if std::fs::symlink_metadata(at.join("link")).is_err() {
        std::os::unix::fs::symlink("/etc/hosts", at.join("link")).expect("a link");
    }
}

fn intact(at: &Path) -> bool {
    std::fs::read_to_string(at.join("kept/file")).is_ok_and(|text| text == "kept")
        && at.join("link").is_symlink()
}

/// The race of a walk below `walked`, where the folder `d` in `store` holds a real folder `x`, and
/// the chat's own `e` holds `x` as a link to the victim: `d` and `e` swap while `walk` runs.
fn raced(walked: &Path, store: &Path, d: &str, x: &str, walk: impl Fn()) {
    let place = tempfile::tempdir().expect("a folder");
    let outside = place.path().join("victim");
    let e = place.path().join("e");
    let d = store.join(d);
    std::fs::create_dir_all(&e).expect("the chat's folder");
    victim(&outside);
    let stop = Arc::new(AtomicBool::new(false));
    let swapper = swapping(d.clone(), e.clone(), Arc::clone(&stop));
    let began = Instant::now();
    let mut walks = 0;
    while began.elapsed() < RACE && intact(&outside) {
        // Put back whatever the last walk took, at whichever side each folder is now.
        for side in [&d, &e] {
            let _ = std::fs::create_dir_all(side);
        }
        for side in [&d, &e] {
            let at = side.join(x);
            if std::fs::symlink_metadata(&at).is_err() && std::fs::create_dir(&at).is_ok() {
                let _ = std::fs::create_dir_all(at.join("stray"));
            }
        }
        let linked = e.join(x);
        if std::fs::symlink_metadata(&linked).is_ok_and(|found| !found.is_symlink()) {
            let _ = std::fs::remove_dir_all(&linked);
            let _ = std::os::unix::fs::symlink(&outside, &linked);
        }
        walk();
        walks += 1;
    }
    stop.store(true, Ordering::Relaxed);
    swapper.join().expect("the swapper ends");
    assert!(
        intact(&outside),
        "a walk of {} reached through a link after {walks} walks",
        walked.display()
    );
}

#[test]
fn codex_s_tidy_of_a_day_s_sessions_never_follows_a_folder_swapped_for_a_link() {
    let home = tempfile::tempdir().expect("a home");
    let sessions = home.path().join("sessions");
    // `d` is a month and `x` a day: the walk descends into both by their dated names.
    let store = sessions.join("2026");
    std::fs::create_dir_all(&store).expect("a year");
    raced(&sessions, &store, "11", "05", || {
        super::codex::tidy(&sessions, 3)
    });
}

#[test]
fn the_claude_wrap_s_link_sweep_never_follows_a_folder_swapped_for_a_link() {
    let home = tempfile::tempdir().expect("a home");
    let projects = home.path().join("projects");
    std::fs::create_dir_all(&projects).expect("its folder");
    raced(&projects, &projects, "p", "x", || {
        super::claude_wrap::no_links_below(&projects);
    });
}
