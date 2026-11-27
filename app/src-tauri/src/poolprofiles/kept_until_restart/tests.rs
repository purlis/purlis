// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What is remembered is one process's, so each test keeps to a pool of its own name.

use std::cell::Cell;

use super::*;

fn work() -> onrunner::Kept {
    onrunner::Kept {
        name: "work".to_owned(),
        harness: "claude".to_owned(),
        sandbox_only: false,
        pinned: None,
        settings_in: None,
        limits: Default::default(),
        highest: Default::default(),
        mcp_state: None,
    }
}

fn running(device: &str, started: u64) -> Option<RunnerNow> {
    Some(RunnerNow {
        device: device.to_owned(),
        started: Some(started),
    })
}

/// A start on `pool` of `kept`, with the runner `now`: the links made are counted in `links`,
/// each answering `said`.
fn start(
    links: &Cell<u32>,
    now: Option<RunnerNow>,
    pool: &str,
    kept: &onrunner::Kept,
    said: Result<Option<Standing>, String>,
) -> Result<Option<Standing>, String> {
    kept_or_linked(&|| now.clone(), pool, kept, |_| {
        links.set(links.get() + 1);
        said.map(|standing| standing.map(on_a_runner_that_can_be_held_to_it))
    })
    .map(|readied| readied.standing)
}

/// What a runner of 1.99 or later answers a keep that put its settings in place.
fn on_a_runner_that_can_be_held_to_it(standing: Standing) -> KeptThere {
    KeptThere {
        standing,
        held: Some("as-kept".to_owned()),
    }
}

const SIGNED_IN: Result<Option<Standing>, String> = Ok(Some(Standing::SignedIn));

#[test]
fn a_second_start_with_the_same_profile_on_a_running_runner_makes_no_profile_link() {
    let links = Cell::new(0);
    let pool = "same-profile";
    let first = start(&links, running("d1", 100), pool, &work(), SIGNED_IN);
    let second = start(&links, running("d1", 100), pool, &work(), SIGNED_IN);
    assert_eq!(first, SIGNED_IN);
    assert_eq!(
        second, SIGNED_IN,
        "the standing remembered is what it answers"
    );
    assert_eq!(
        links.get(),
        1,
        "only the first start linked to keep the profile"
    );
}

#[test]
fn a_changed_profile_makes_the_link_again() {
    let links = Cell::new(0);
    let pool = "changed-profile";
    start(&links, running("d1", 100), pool, &work(), SIGNED_IN).unwrap();
    let pinned = onrunner::Kept {
        pinned: Some("2.1.0".to_owned()),
        ..work()
    };
    start(&links, running("d1", 100), pool, &pinned, SIGNED_IN).unwrap();
    assert_eq!(links.get(), 2);
}

#[test]
fn a_changed_settings_file_makes_the_link_again() {
    let links = Cell::new(0);
    let pool = "changed-settings";
    let folder = tempfile::tempdir().unwrap();
    std::fs::write(folder.path().join("settings.json"), "{}").unwrap();
    let kept = onrunner::Kept {
        settings_in: Some(folder.path().to_path_buf()),
        ..work()
    };
    start(&links, running("d1", 100), pool, &kept, SIGNED_IN).unwrap();
    start(&links, running("d1", 100), pool, &kept, SIGNED_IN).unwrap();
    assert_eq!(links.get(), 1, "the same settings are not sent again");
    std::fs::write(
        folder.path().join("settings.json"),
        r#"{"permissions":{"allow":["Bash(ls)"]}}"#,
    )
    .unwrap();
    start(&links, running("d1", 100), pool, &kept, SIGNED_IN).unwrap();
    assert_eq!(links.get(), 2, "a settings file changed since is sent");
}

#[test]
fn a_new_runner_makes_the_link_again() {
    let links = Cell::new(0);
    let pool = "new-runner";
    start(&links, running("d1", 100), pool, &work(), SIGNED_IN).unwrap();
    start(&links, running("d2", 100), pool, &work(), SIGNED_IN).unwrap();
    assert_eq!(links.get(), 2);
}

#[test]
fn a_restarted_runner_makes_the_link_again() {
    let links = Cell::new(0);
    let pool = "restarted-runner";
    start(&links, running("d1", 100), pool, &work(), SIGNED_IN).unwrap();
    start(&links, running("d1", 200), pool, &work(), SIGNED_IN).unwrap();
    assert_eq!(links.get(), 2, "started again since it kept the profile");
    start(&links, None, pool, &work(), SIGNED_IN).unwrap();
    assert_eq!(
        links.get(),
        3,
        "a runner that does not run is woken by the link"
    );
}

#[test]
fn a_runners_device_id_is_compared_without_case() {
    let links = Cell::new(0);
    let pool = "device-case";
    start(&links, running("AbC", 100), pool, &work(), SIGNED_IN).unwrap();
    start(&links, running("abc", 100), pool, &work(), SIGNED_IN).unwrap();
    assert_eq!(links.get(), 1);
}

#[test]
fn a_profile_not_signed_in_is_never_remembered() {
    let links = Cell::new(0);
    let pool = "signed-out";
    let signed_out = Ok(Some(Standing::SignedOut));
    start(
        &links,
        running("d1", 100),
        pool,
        &work(),
        signed_out.clone(),
    )
    .unwrap();
    start(&links, running("d1", 100), pool, &work(), signed_out).unwrap();
    let fetching = Ok(Some(Standing::Fetching));
    start(&links, running("d1", 100), pool, &work(), fetching.clone()).unwrap();
    start(&links, running("d1", 100), pool, &work(), fetching).unwrap();
    start(&links, running("d1", 100), pool, &work(), Ok(None)).unwrap();
    start(&links, running("d1", 100), pool, &work(), Ok(None)).unwrap();
    assert_eq!(links.get(), 6);
}

#[test]
fn a_refused_keep_forgets_what_was_remembered() {
    let links = Cell::new(0);
    let pool = "refused";
    start(&links, running("d1", 100), pool, &work(), SIGNED_IN).unwrap();
    let pinned = onrunner::Kept {
        pinned: Some("2.1.0".to_owned()),
        ..work()
    };
    let refused = start(
        &links,
        running("d1", 100),
        pool,
        &pinned,
        Err("no".to_owned()),
    );
    assert!(refused.is_err());
    start(&links, running("d1", 100), pool, &work(), SIGNED_IN).unwrap();
    assert_eq!(links.get(), 3, "the runner may hold neither as it was");
}

#[test]
fn a_profile_forgotten_makes_the_link_again_on_that_pool_only() {
    let links = Cell::new(0);
    let (pool, other) = ("forgotten", "forgotten-beside");
    start(&links, running("d1", 100), pool, &work(), SIGNED_IN).unwrap();
    start(&links, running("d9", 100), other, &work(), SIGNED_IN).unwrap();
    forget(pool, "work");
    start(&links, running("d1", 100), pool, &work(), SIGNED_IN).unwrap();
    start(&links, running("d9", 100), other, &work(), SIGNED_IN).unwrap();
    assert_eq!(links.get(), 3);
}

#[test]
fn what_is_remembered_is_the_runner_as_it_is_after_the_link() {
    // A start that wakes a suspended runner: asked before, it does not run; after, it does.
    let links = Cell::new(0);
    let pool = "woken";
    let asked = Cell::new(0);
    let now = || {
        asked.set(asked.get() + 1);
        if asked.get() == 1 {
            None
        } else {
            running("d1", 300)
        }
    };
    kept_or_linked(&now, pool, &work(), |_| {
        links.set(links.get() + 1);
        Ok(Some(on_a_runner_that_can_be_held_to_it(Standing::SignedIn)))
    })
    .unwrap();
    start(&links, running("d1", 300), pool, &work(), SIGNED_IN).unwrap();
    assert_eq!(links.get(), 1);
}

#[test]
fn a_runner_older_than_a_start_can_be_held_to_is_kept_on_at_every_start() {
    let links = Cell::new(0);
    let pool = "older-runner";
    let older = || {
        kept_or_linked(&|| running("d1", 100), pool, &work(), |_| {
            links.set(links.get() + 1);
            Ok(Some(KeptThere {
                standing: Standing::SignedIn,
                held: None,
            }))
        })
    };
    let first = older().unwrap();
    assert_eq!(older().unwrap(), first);
    assert_eq!(first.standing, Some(Standing::SignedIn));
    assert_eq!(links.get(), 2, "nothing it holds can be checked at a start");
    assert_eq!(first.held, None);
}

/// The readying of a start on `pool` of `work`, on a runner that runs since one start.
fn readied(links: &Cell<u32>, pool: &str) -> Readied {
    kept_or_linked(&|| running("d1", 100), pool, &work(), |_| {
        links.set(links.get() + 1);
        Ok(Some(on_a_runner_that_can_be_held_to_it(Standing::SignedIn)))
    })
    .unwrap()
}

#[test]
fn a_start_after_a_skipped_keep_names_what_the_runner_must_still_hold() {
    let links = Cell::new(0);
    let pool = "named-at-start";
    let kept = readied(&links, pool);
    let skipped = readied(&links, pool);
    assert_eq!(links.get(), 1);
    assert_eq!(kept.held.as_deref(), Some("as-kept"), "the keep it made");
    assert_eq!(
        skipped.held.as_deref(),
        Some("as-kept"),
        "the keep it skipped"
    );
}

/// **A profile whose harness makes again, as it starts, what a keep clears is kept at every
/// start** (#7863): opencode's `package.json` and `node_modules` are there after its first chat,
/// so no runner could show it holds the profile as a keep leaves it, and a skip would only be
/// refused.
#[test]
fn a_profile_whose_harness_remakes_what_a_keep_clears_is_kept_at_every_start() {
    let links = Cell::new(0);
    let pool = "remade-by-its-harness";
    let opencode = onrunner::Kept {
        harness: "opencode".to_owned(),
        ..work()
    };
    let first = start(&links, running("d1", 100), pool, &opencode, SIGNED_IN);
    let second = kept_or_linked(&|| running("d1", 100), pool, &opencode, |_| {
        links.set(links.get() + 1);
        Ok(Some(on_a_runner_that_can_be_held_to_it(Standing::SignedIn)))
    })
    .unwrap();
    assert_eq!(first, SIGNED_IN);
    assert_eq!(links.get(), 2);
    assert_eq!(second.held, None, "nothing for the start to name");
}

/// **A runner that once did not hold a profile as kept is kept on at every start after** (#7863):
/// whatever it found, a rule a chat left or a state file no keep can read, the next skip would
/// most likely be refused the same way, so it costs no refused start again until the app
/// restarts.
#[test]
fn a_runner_that_once_refused_a_start_as_changed_is_kept_on_at_every_start() {
    let links = Cell::new(0);
    let pool = "refused-once";
    start(&links, running("d1", 100), pool, &work(), SIGNED_IN).unwrap();
    start(&links, running("d1", 100), pool, &work(), SIGNED_IN).unwrap();
    assert_eq!(links.get(), 1);

    refused_as_changed(pool, "work");
    start(&links, running("d1", 100), pool, &work(), SIGNED_IN).unwrap();
    start(&links, running("d1", 100), pool, &work(), SIGNED_IN).unwrap();
    assert_eq!(links.get(), 3);
}
