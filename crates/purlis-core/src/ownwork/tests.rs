// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const HOUR: u32 = 3_600;
const LATE: u32 = 300;

fn in_flight(helpers: u16, workflows: u16, monitors: u16, shells: u16) -> InFlight {
    InFlight {
        helpers,
        workflows,
        monitors,
        shells,
    }
}

fn one_shot(due_in: i64) -> WakeUp {
    WakeUp {
        due_in,
        recurring: false,
    }
}

fn recurring(due_in: i64) -> WakeUp {
    WakeUp {
        due_in,
        recurring: true,
    }
}

#[test]
fn helpers_workflows_and_monitors_count_with_no_limit() {
    let said = said(in_flight(2, 1, 2, 0), &[]);
    assert_eq!((said.helpers, said.workflows, said.monitors), (2, 1, 2));
    assert_eq!(said.lapses_in, None);
    assert!(!said.is_nothing());
}

#[test]
fn a_background_shell_counts_for_the_hour() {
    let said = said(in_flight(0, 0, 0, 1), &[]);
    assert_eq!((said.shells, said.lapses_in), (1, Some(HOUR)));
}

#[test]
fn a_shell_beside_a_monitor_waits_with_no_limit() {
    let said = said(in_flight(0, 0, 1, 1), &[]);
    assert_eq!(said.lapses_in, None);
}

#[test]
fn a_one_shot_wake_up_counts_however_far_off_until_it_is_late() {
    let said = said(InFlight::default(), &[one_shot(5 * 3_600)]);
    assert_eq!(said.wakes_in, Some(5 * HOUR));
    assert_eq!(said.lapses_in, Some(5 * HOUR + LATE));
}

#[test]
fn a_wake_up_due_now_or_overdue_lapses_once_it_is_late() {
    for due_in in [0, -600] {
        let said = said(InFlight::default(), &[one_shot(due_in)]);
        assert_eq!(
            (said.wakes_in, said.lapses_in),
            (Some(0), Some(LATE)),
            "{due_in}"
        );
    }
}

#[test]
fn a_recurring_wake_up_counts_only_when_its_next_run_is_within_the_hour() {
    let soon = said(InFlight::default(), &[recurring(1_200)]);
    assert_eq!(
        (soon.wakes_in, soon.lapses_in),
        (Some(1_200), Some(1_200 + LATE))
    );
    assert_eq!(
        said(InFlight::default(), &[recurring(i64::from(HOUR))]).wakes_in,
        Some(HOUR)
    );
    assert!(said(InFlight::default(), &[recurring(i64::from(HOUR) + 60)]).is_nothing());
}

#[test]
fn the_soonest_wake_up_is_said_and_the_latest_end_is_the_limit() {
    let said = said(in_flight(0, 0, 0, 1), &[recurring(2_700), one_shot(600)]);
    assert_eq!(said.wakes_in, Some(600));
    assert_eq!(said.lapses_in, Some(HOUR));
}

#[test]
fn a_wait_is_stamped_at_the_app_s_clock_and_lapses_there() {
    let said = said(in_flight(0, 0, 0, 1), &[one_shot(600)]);
    let work = said.at(1_000).expect("stamped");
    assert_eq!(work.wakes_at, Some(1_600));
    assert_eq!(work.until, Some(1_000 + i64::from(HOUR)));
    assert!(!work.lapsed_at(1_000 + i64::from(HOUR) - 1));
    assert!(work.lapsed_at(1_000 + i64::from(HOUR)));
    // With a limit and no clock said, it is not held.
    assert_eq!(said.at(0), None);
    // With no limit, no clock is needed.
    let open = self::said(in_flight(0, 0, 1, 0), &[]);
    assert_eq!(open.at(0).map(|work| work.monitors), Some(1));
}

#[test]
fn it_reads_back_what_it_writes_and_says_nothing_of_nothing() {
    let said = said(in_flight(0, 0, 1, 0), &[recurring(1_200)]);
    let line = serde_json::to_string(&said).expect("a line");
    assert_eq!(serde_json::from_str::<Said>(&line).expect("read"), said);
    assert_eq!(
        serde_json::to_string(&Said::default()).expect("a line"),
        "{}"
    );
}
