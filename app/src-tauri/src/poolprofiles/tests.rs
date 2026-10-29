// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn work() -> onrunner::Kept {
    onrunner::Kept {
        name: "work".to_owned(),
        harness: "claude".to_owned(),
        sandbox_only: false,
        pinned: None,
        settings_in: None,
    }
}

/// **A pool not shown to be the person's own is sent nothing** (#2049, D-2049-5): keeping a
/// profile there answers that the runner is its own to say, and reads nothing of the pool, so a
/// config home that holds none is never asked.
#[test]
fn a_pool_not_shown_to_be_owned_is_sent_nothing_and_is_its_own_to_say() {
    let config = tempfile::tempdir().unwrap();

    let said = keep_on_as(
        Err("DOCKER_HOST names tcp://builds:2375".to_owned()),
        &config.path().join("nowhere"),
        "builds",
        &work(),
        Waking::No,
    );

    assert_eq!(said, Ok(None));
}

/// The same keep on a pool shown to be the person's own goes on to the pool, and is refused
/// where this machine keeps no such pool.
#[test]
fn on_an_owned_pool_it_goes_on_to_the_pool() {
    let config = tempfile::tempdir().unwrap();

    let said = keep_on_as(
        Ok(PathBuf::from("/var/run/docker.sock")),
        config.path(),
        "builds",
        &work(),
        Waking::No,
    );

    assert!(said.is_err(), "{said:?}");
}

/// **One watcher per pool and profile** (#2049): a second Sign in while one asks claims none,
/// and another profile or pool claims its own.
#[test]
fn a_watcher_is_claimed_once_per_pool_and_profile() {
    let mut watched = HashSet::new();
    let key = ("builds".to_owned(), "work".to_owned());

    assert!(claimed(&mut watched, &key));
    assert!(!claimed(&mut watched, &key), "a second Sign in starts none");
    assert!(claimed(
        &mut watched,
        &("builds".to_owned(), "home".to_owned())
    ));
    assert!(claimed(
        &mut watched,
        &("other".to_owned(), "work".to_owned())
    ));
}
