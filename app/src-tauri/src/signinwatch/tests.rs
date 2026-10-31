// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// **One watcher per open project, pool and profile** (#2049): a second Sign in while one asks
/// claims none and adds its shell, and another profile, pool or project claims its own.
#[test]
fn a_watcher_is_claimed_once_per_project_pool_and_profile() {
    let mut watched = HashMap::new();
    let key =
        |project: usize, pool: &str, name: &str| (project, Some(pool.to_owned()), name.to_owned());

    assert!(claimed(&mut watched, &key(1, "builds", "work"), 1));
    assert!(
        !claimed(&mut watched, &key(1, "builds", "work"), 2),
        "a second Sign in starts none"
    );
    assert_eq!(watched[&key(1, "builds", "work")], vec![1, 2]);
    assert!(claimed(&mut watched, &key(1, "builds", "home"), 3));
    assert!(claimed(&mut watched, &key(1, "other", "work"), 4));
    assert!(claimed(&mut watched, &key(2, "builds", "work"), 5));
}

/// **The watch goes on while any shell of it is open** (#4009), and its claim goes with the last,
/// so a Sign in after that claims a watcher of its own. A shell added meanwhile is not dropped.
#[test]
fn closing_the_last_shell_gives_up_the_claim() {
    let mut watched = HashMap::new();
    let key = (1, Some("builds".to_owned()), "work".to_owned());
    claimed(&mut watched, &key, 1);
    claimed(&mut watched, &key, 2);

    assert!(shells_left(&mut watched, &key, &[1]));
    assert_eq!(watched[&key], vec![2]);
    assert!(!shells_left(&mut watched, &key, &[2]));
    assert!(claimed(&mut watched, &key, 3), "a new Sign in claims anew");
}

/// **Signed in lets every project's Ask go, the person leaving only their project's, and a
/// project closing none**: it opens again with its Ask, and a watcher of its own at its next
/// Sign in (#4009).
#[test]
fn an_ended_watch_lets_go_only_the_asks_it_speaks_for() {
    assert_eq!(asks_go(Watch::SignedIn, true), AsksGo::Everywhere);
    assert_eq!(asks_go(Watch::SignedIn, false), AsksGo::Everywhere);
    assert_eq!(asks_go(Watch::ShellsClosed, true), AsksGo::InTheProject);
    assert_eq!(asks_go(Watch::ShellsClosed, false), AsksGo::None);
}

const NO_WAIT: Pace = Pace {
    every: Duration::ZERO,
    for_: Duration::from_secs(60),
    later_every: Duration::ZERO,
};

/// A watch over `said`, whether the harness said signed in each time, the last repeated, with the
/// shell closed from the answer numbered `closes_at` on. How it ended, and how many times it asked.
fn watched_over(said: &[bool], closes_at: usize) -> (Watch, usize) {
    let asked = std::cell::Cell::new(0);
    let (ended, _) = watched_until(
        NO_WAIT,
        || {
            asked.set(asked.get() + 1);
            said[(asked.get() - 1).min(said.len() - 1)]
        },
        || asked.get() < closes_at,
    );
    (ended, asked.get())
}

/// **Only signed in ends the watch while the shell is open** (#4009, #4223).
#[test]
fn only_signed_in_ends_the_watch_while_the_shell_is_open() {
    let (ended, asked) = watched_over(&[false, false, false, true], usize::MAX);

    assert_eq!(ended, Watch::SignedIn);
    assert_eq!(asked, 4);
}

/// **Closing the shell ends it**, signed in or not: the person has left that sign-in.
#[test]
fn closing_the_shell_ends_the_watch() {
    let (ended, asked) = watched_over(&[false], 3);

    assert_eq!(ended, Watch::ShellsClosed);
    assert_eq!(asked, 3);
}

/// A sign-in that finished just before the shell closed counts as signed in, not as the person
/// leaving.
#[test]
fn signed_in_as_the_shell_closes_is_signed_in() {
    let (ended, _) = watched_over(&[true], 1);

    assert_eq!(ended, Watch::SignedIn);
}

/// **Time only slows the asking**: past it the runner is asked rarely, closing the shell is still
/// seen at once, and nothing but those two ends the watch.
#[test]
fn past_its_time_a_watch_asks_rarely_and_still_sees_the_shell_close() {
    let (asked, looked) = (std::cell::Cell::new(0), std::cell::Cell::new(0));

    let (ended, _) = watched_until(
        Pace {
            every: Duration::ZERO,
            for_: Duration::ZERO,
            later_every: Duration::from_secs(3600),
        },
        || {
            asked.set(asked.get() + 1);
            false
        },
        || {
            looked.set(looked.get() + 1);
            looked.get() < 5
        },
    );

    assert_eq!(ended, Watch::ShellsClosed);
    assert_eq!(asked.get(), 1);
    assert_eq!(looked.get(), 5);
}

/// **It says when it last saw a shell open** (#4203): the moment before that look, so a wait
/// raised during the look counts as raised after it. Not the look that found them closed.
#[test]
fn a_watch_says_when_it_last_saw_a_shell_open() {
    let started = Instant::now();
    let looks = std::cell::Cell::new(0);
    let seen = std::cell::Cell::new(started);

    let (ended, last_open) = watched_until(
        NO_WAIT,
        || false,
        || {
            looks.set(looks.get() + 1);
            if looks.get() < 3 {
                seen.set(Instant::now());
                return true;
            }
            false
        },
    );

    assert_eq!(ended, Watch::ShellsClosed);
    assert!(last_open >= started);
    assert!(last_open <= seen.get(), "the look that saw it open");
}

/// With no look seeing a shell open, it was last seen as the watch began: Sign in had just typed
/// into it.
#[test]
fn a_shell_closed_at_the_first_look_was_last_seen_as_the_watch_began() {
    let before = Instant::now();

    let (_, last_open) = watched_until(NO_WAIT, || false, || false);

    assert!(last_open >= before);
    assert!(last_open <= Instant::now());
}

/// **Every project whose wait went is told, and its own project while it is open** (#4205). A
/// project closed is told nothing: its teller would say an old list of it.
#[test]
fn every_project_whose_wait_went_is_told() {
    let root = |at: &str| PathBuf::from(at);

    assert_eq!(
        to_tell(vec![root("/one"), root("/two")], &root("/one"), true),
        [root("/one"), root("/two")]
    );
    assert_eq!(
        to_tell(vec![root("/two")], &root("/one"), true),
        [root("/two"), root("/one")],
        "its own, though no wait of it went: it may be drawing one"
    );
    assert_eq!(
        to_tell(vec![root("/two")], &root("/one"), false),
        [root("/two")]
    );
    assert!(to_tell(Vec::new(), &root("/one"), false).is_empty());
}
