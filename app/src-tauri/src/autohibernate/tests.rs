// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The gate's decision and the looked-at clock, on a clock held still (SC-4g, #7636).

use super::*;

const MINUTE: Duration = Duration::from_secs(60);
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

fn resting(chat: u32, since: Instant) -> Candidate<u32> {
    Candidate { chat, since }
}

#[test]
fn under_the_target_a_chat_rested_a_day_is_not_hibernated_and_nothing_is_armed() {
    let start = Instant::now();
    let rested = [resting(1, start), resting(2, start)];
    for hot in [1, 2, 3] {
        assert_eq!(choose(hot, 3, &rested, start + DAY, AFTER), Choice::Nothing);
    }
}

#[test]
fn at_the_target_it_stops() {
    let start = Instant::now();
    assert_eq!(
        choose(2, 2, &[resting(1, start)], start + DAY, AFTER),
        Choice::Nothing
    );
}

#[test]
fn over_the_target_one_deadline_is_armed_for_the_first_chat_to_become_rested() {
    let start = Instant::now();
    let candidates = [
        resting(1, start + 10 * MINUTE),
        resting(2, start),
        resting(3, start + 5 * MINUTE),
    ];
    assert_eq!(
        choose(4, 2, &candidates, start + MINUTE, AFTER),
        Choice::ArmFor(start + AFTER)
    );
}

#[test]
fn over_the_target_the_longest_rested_chat_goes_first() {
    let start = Instant::now();
    let candidates = [
        resting(1, start + 10 * MINUTE),
        resting(2, start),
        resting(3, start + 5 * MINUTE),
    ];
    assert_eq!(
        choose(4, 2, &candidates, start + DAY, AFTER),
        Choice::Hibernate(2)
    );
}

#[test]
fn over_the_target_with_nothing_that_can_become_rested_nothing_is_armed() {
    let start = Instant::now();
    assert_eq!(
        choose::<u32>(9, 2, &[], start + DAY, AFTER),
        Choice::Nothing
    );
}

#[test]
fn a_chat_never_seen_rests_from_its_first_read_so_a_start_sets_every_clock_to_now() {
    let clock = LookedAt::default();
    let start = Instant::now();
    assert_eq!(clock.rests_since(7, start + DAY), Some(start + DAY));
    assert_eq!(clock.rests_since(7, start + 2 * DAY), Some(start + DAY));
    assert!(clock.looked_at_within(7, start + DAY + MINUTE, AFTER));
    assert!(!clock.looked_at_within(7, start + DAY + AFTER, AFTER));
}

#[test]
fn a_chat_on_screen_in_a_focused_window_is_looked_at_and_rests_once_it_leaves() {
    let clock = LookedAt::default();
    let start = Instant::now();
    assert!(clock.on_screen(start, [(3, 0)]));
    assert_eq!(clock.rests_since(3, start + DAY), None, "on screen");
    assert!(clock.looked_at_within(3, start + DAY, AFTER));
    assert!(clock.on_screen(start + DAY, []));
    assert_eq!(
        clock.rests_since(3, start + DAY + MINUTE),
        Some(start + DAY)
    );
}

#[test]
fn a_look_between_two_reads_leaves_the_chat_resting_from_the_look() {
    let clock = LookedAt::default();
    let start = Instant::now();
    assert_eq!(clock.rests_since(3, start), Some(start));
    clock.on_screen(start + 20 * MINUTE, [(3, 0)]);
    clock.on_screen(start + 21 * MINUTE, []);
    assert_eq!(
        clock.rests_since(3, start + 40 * MINUTE),
        Some(start + 21 * MINUTE)
    );
    assert!(clock.looked_at_within(3, start + 40 * MINUTE, AFTER));
}

#[test]
fn closing_one_of_two_panes_showing_a_chat_keeps_its_look() {
    let clock = LookedAt::default();
    let start = Instant::now();
    clock.on_screen(start, [(3, 0), (3, 0)]);
    assert!(
        !clock.on_screen(start + MINUTE, [(3, 0)]),
        "nothing changed"
    );
    assert_eq!(clock.rests_since(3, start + DAY), None);
}

#[test]
fn a_window_that_loses_the_focus_starts_the_clocks_of_the_chats_it_shows() {
    let clock = LookedAt::default();
    let start = Instant::now();
    clock.on_screen(start, [(3, 0), (4, 2)]);
    clock.focused(0, false);
    clock.on_screen(start + MINUTE, [(3, 0), (4, 2)]);
    assert_eq!(clock.rests_since(3, start + DAY), Some(start + MINUTE));
    assert_eq!(
        clock.rests_since(4, start + DAY),
        None,
        "its own window has the focus"
    );
    clock.focused(0, true);
    clock.on_screen(start + DAY, [(3, 0), (4, 2)]);
    assert_eq!(
        clock.rests_since(3, start + DAY),
        None,
        "back in front of the person"
    );
}

#[test]
fn a_key_typed_in_a_chat_sets_its_clock_to_now() {
    let clock = LookedAt::default();
    let start = Instant::now();
    assert_eq!(clock.rests_since(3, start), Some(start));
    clock.typed(3, start + 50 * MINUTE);
    assert_eq!(clock.rests_since(3, start + DAY), Some(start + 50 * MINUTE));
}

#[test]
fn the_clock_reads_an_instant_and_nothing_else() {
    // A wall-clock jump cannot make a chat rested: the seam is typed on `Instant`, and no
    // `SystemTime` is read anywhere in this module.
    let source = include_str!("../autohibernate.rs");
    assert!(!source.contains(concat!("System", "Time")));
    fn typed_on_instant(clock: &dyn Clock) -> Instant {
        clock.now()
    }
    let held = HeldClock::new();
    let before = typed_on_instant(&*held);
    held.advance(MINUTE);
    assert_eq!(typed_on_instant(&*held), before + MINUTE);
}
