// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_check_is_timed_whatever_it_answers() {
    let mut into = Vec::new();
    let answered: Result<(), &str> = time(&mut into, "disk-full", "lab", || Err("no room"));
    assert_eq!(answered, Err("no room"));
    assert_eq!(into.len(), 1);
    assert_eq!(into[0].cause, "disk-full");
}

#[test]
fn a_duration_is_whole_milliseconds_and_never_less_than_one() {
    assert_eq!(ms(Duration::from_micros(10)), 1);
    assert_eq!(ms(Duration::from_millis(4200)), 4200);
    assert_eq!(ms(Duration::from_secs(u64::MAX / 4)), u32::MAX);
}

#[test]
fn only_a_check_past_thirty_seconds_says_what_it_waited_on() {
    let quick = Timed {
        cause: "disk-full",
        took: SLOW,
    };
    assert_eq!(quick.waited_on("lab"), None);
    let slow = Timed {
        cause: "image-unreachable",
        took: SLOW + Duration::from_millis(1),
    };
    let said = slow
        .waited_on("lab")
        .expect("a slow line says what it waits on");
    assert!(said.contains("lab") && said.contains("download"), "{said}");
}

#[test]
fn a_running_check_past_thirty_seconds_is_listed_until_it_stops() {
    let watch = Stopwatch::start("image-unreachable", "slow-host-for-test");
    assert!(
        !slow_now()
            .iter()
            .any(|(_, on, _)| on == "slow-host-for-test"),
        "a check that has just started waits on nothing yet"
    );
    let later = Instant::now() + SLOW + Duration::from_secs(1);
    let listed = slow_at(later);
    assert!(
        listed
            .iter()
            .any(|(cause, on, said)| *cause == "image-unreachable"
                && on == "slow-host-for-test"
                && said.contains("slow-host-for-test")),
        "{listed:?}"
    );
    let _ = watch.done();
    assert!(
        !slow_at(later)
            .iter()
            .any(|(_, on, _)| on == "slow-host-for-test"),
        "a check that is done waits on nothing"
    );
}

#[test]
fn a_cause_not_named_says_it_waits_on_the_machine() {
    assert!(waiting_on("anything-else", "lab").contains("lab"));
}

#[test]
fn a_duration_is_said_in_words() {
    assert_eq!(words(Duration::from_micros(10)), "1 ms");
    assert_eq!(words(Duration::from_millis(820)), "820 ms");
    assert_eq!(words(Duration::from_millis(4250)), "4.2 s");
    assert_eq!(words(Duration::from_secs(64)), "1 min 4 s");
}

#[test]
fn lines_say_each_time_and_a_slow_one_waits_on_something() {
    let timed = [
        Timed {
            cause: "unconfined",
            took: Duration::from_millis(120),
        },
        Timed {
            cause: "image-unreachable",
            took: SLOW + Duration::from_secs(5),
        },
    ];
    let said = lines(&timed, "lab");
    assert_eq!(said[0].took_ms, Some(120));
    assert_eq!(said[0].waiting_on, None);
    assert_eq!(said[1].took_ms, Some(35_000));
    assert!(
        said[1]
            .waiting_on
            .as_deref()
            .unwrap_or_default()
            .contains("download")
    );
}
