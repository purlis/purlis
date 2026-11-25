// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The figures a refusal tells, and the reader that takes them from git: what they do with a
//! total git (or a forge's `remote:` line) made up, and with a figure that has stopped moving.

use super::*;

fn at(received: u64, total: u64) -> Progress {
    Progress {
        received,
        total,
        ..Progress::default()
    }
}

#[test]
fn the_estimate_is_the_rate_so_far_applied_to_what_is_left() {
    let left = estimate(&at(4500, 10_000), Duration::from_secs(90));
    assert_eq!(left, Duration::from_secs(110));
}

#[test]
fn an_absurd_total_does_not_panic_the_estimate_and_promises_no_more_than_the_fill_may_take() {
    let elapsed = Duration::from_secs(100);
    let left = estimate(&at(1, u64::MAX), elapsed);
    assert_eq!(left, FILL - elapsed);
}

#[test]
fn a_figure_that_has_stopped_moving_promises_no_more_than_what_is_left_of_the_fill() {
    // 1 of 10,000 objects after ten minutes is seventy days at that rate: the fill ends at FILL
    // whatever it has read, so ten minutes in, what is left of FILL is all it can promise.
    let elapsed = Duration::from_secs(600);
    let left = estimate(&at(1, 10_000), elapsed);
    assert_eq!(left, FILL - elapsed);
    assert_eq!(
        estimate(&at(1, 10_000), FILL + Duration::from_secs(60)),
        Duration::ZERO,
        "a fill past its time has none left"
    );
}

#[test]
fn a_line_the_forge_said_is_not_a_figure_of_git_s() {
    let mut reader = ProgressReader::default();
    assert_eq!(
        reader.feed("remote: Receiving objects:  45% (4500/10000), 12.50 MiB | 3.10 MiB/s\r"),
        None
    );
    assert_eq!(
        reader.feed("remote: Resolving deltas:  50% (50/100)\n"),
        None
    );
    let own = reader
        .feed("\rReceiving objects:  45% (4500/10000), 12.50 MiB | 3.10 MiB/s\r")
        .unwrap();
    assert_eq!((own.received, own.total), (4500, 10_000));
}

#[test]
fn a_total_past_what_a_duration_holds_is_read_without_a_panic() {
    let mut reader = ProgressReader::default();
    let seen = reader
        .feed("Receiving objects:   0% (1/18446744073709551615)\r")
        .unwrap();
    assert_eq!((seen.received, seen.total), (1, u64::MAX));
}

#[test]
fn a_line_that_never_ends_is_not_kept_without_bound() {
    let mut reader = ProgressReader::default();
    for _ in 0..8 {
        reader.feed(&"x".repeat(32 * 1024));
    }
    assert!(reader.carry.len() <= MOST_CARRIED, "{}", reader.carry.len());
    // Git's next line is read all the same.
    assert!(reader.feed("\rReceiving objects:   1% (1/10)\r").is_some());
}

/// A cache with a fetch's figure `chunk` read, in the stage `stage` of a first fill.
fn reading_in(stage: Stage, chunk: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let cache = ObjectCache::of_capped(dir.path(), "forge.test", "owner/parts", MOST);
    cache.stage_began(stage);
    cache.fetching.store(true, Ordering::Release);
    (cache.sink())(chunk);
    cache.reading()
}

const HALF: &str = "Receiving objects:  50% (50/100), 1.00 MiB | 1.00 MiB/s\r";

#[test]
fn each_part_of_a_first_fill_says_which_part_it_is_and_only_the_last_gives_an_estimate() {
    let first = reading_in(Stage::Depth(200), HALF);
    assert!(first.contains("part 1 of 3"), "{first}");
    assert!(!first.contains("about"), "{first}");
    let second = reading_in(Stage::Depth(5000), HALF);
    assert!(second.contains("part 2 of 3"), "{second}");
    assert!(!second.contains("about"), "{second}");
    let last = reading_in(Stage::Rest, HALF);
    assert!(last.contains("part 3 of 3"), "{last}");
    assert!(last.contains("try again in about"), "{last}");
}

#[test]
fn a_part_that_has_said_nothing_yet_does_not_repeat_the_part_before_it() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ObjectCache::of_capped(dir.path(), "forge.test", "owner/next", MOST);
    cache.stage_began(Stage::Depth(200));
    cache.fetching.store(true, Ordering::Release);
    let sink = cache.sink();
    sink("Receiving objects: 100% (100/100), 2.00 MiB | 1.00 MiB/s, done.\r");
    sink("Resolving deltas:  10% (10/100)\r");
    let resolving = cache.reading();
    assert!(resolving.contains("resolving deltas"), "{resolving}");

    cache.stage_began(Stage::Depth(5000));
    let next = cache.reading();
    assert!(next.contains("part 2 of 3"), "{next}");
    assert!(!next.contains("resolving deltas"), "{next}");
    assert!(!next.contains("minute or two"), "{next}");
    assert!(cache.progress().is_none());
}
