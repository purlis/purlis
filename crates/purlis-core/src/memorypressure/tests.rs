// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[cfg(unix)]
#[test]
fn the_reader_is_bounded_in_time_and_in_what_it_reads() {
    use std::process::Command;
    use std::time::{Duration, Instant};
    let said = bounded_output(Command::new("/bin/echo").arg("4"), READ_DEADLINE, READ_MOST);
    assert_eq!(said.as_deref().map(str::trim), Some("4"));
    // A reader that does not answer in time is killed and reads as nothing.
    let began = Instant::now();
    let hung = bounded_output(
        Command::new("/bin/sleep").arg("30"),
        Duration::from_millis(200),
        READ_MOST,
    );
    assert_eq!(hung, None);
    assert!(
        began.elapsed() < Duration::from_secs(5),
        "{:?}",
        began.elapsed()
    );
    // One that fails reads as nothing.
    assert_eq!(
        bounded_output(
            &mut Command::new("/usr/bin/false"),
            READ_DEADLINE,
            READ_MOST
        ),
        None
    );
    // No more than the cap is kept.
    let long = bounded_output(
        Command::new("/bin/echo").arg("1".repeat(1000)),
        READ_DEADLINE,
        READ_MOST,
    );
    assert_eq!(long.map(|s| s.len()), Some(READ_MOST as usize));
}

#[test]
fn macos_is_short_only_at_the_critical_level() {
    assert_eq!(macos_level("1\n"), Memory::Enough);
    assert_eq!(macos_level("2\n"), Memory::Enough, "a warning is not short");
    assert_eq!(macos_level("4\n"), Memory::Short);
    assert_eq!(macos_level(" 8 "), Memory::Short);
    assert_eq!(macos_level("0"), Memory::Unread);
    assert_eq!(macos_level(""), Memory::Unread);
    assert_eq!(macos_level("unknown oid"), Memory::Unread);
}

#[test]
fn linux_is_short_where_every_task_stalls_on_memory_a_tenth_of_the_time() {
    let at = |full: &str| {
        format!(
            "some avg10=55.00 avg60=1.00 avg300=0.50 total=100\n\
             full avg10={full} avg60=0.00 avg300=0.00 total=12\n"
        )
    };
    assert_eq!(psi(&at("0.00")), Memory::Enough, "some alone is not short");
    assert_eq!(psi(&at("9.99")), Memory::Enough);
    assert_eq!(psi(&at("10.00")), Memory::Short);
    assert_eq!(psi(&at("73.12")), Memory::Short);
    assert_eq!(
        psi("some avg10=1.00 avg60=0.00 avg300=0.00 total=0\n"),
        Memory::Unread,
        "no full line"
    );
    assert_eq!(psi(&at("nan-ish")), Memory::Unread);
}

#[test]
fn linux_without_pressure_information_is_short_under_the_smaller_floor() {
    let info = |total_kib: u64, available_kib: u64| {
        format!(
            "MemTotal:       {total_kib} kB\nMemFree:          1000 kB\n\
             MemAvailable:   {available_kib} kB\n"
        )
    };
    // 64 GiB: the floor is 512 MiB, not 5% (3.2 GiB).
    let big = 64 * 1024 * 1024;
    assert_eq!(meminfo(&info(big, 3 * 1024 * 1024)), Memory::Enough);
    assert_eq!(meminfo(&info(big, 511 * 1024)), Memory::Short);
    // 4 GiB: 5% (about 205 MiB) is the smaller.
    let small = 4 * 1024 * 1024;
    assert_eq!(meminfo(&info(small, 300 * 1024)), Memory::Enough);
    assert_eq!(meminfo(&info(small, 150 * 1024)), Memory::Short);
    assert_eq!(meminfo("MemTotal: 100 kB\n"), Memory::Unread);
}

#[test]
fn a_stand_in_answers_in_place_of_the_machine_until_it_is_taken_away() {
    let gauge = Gauge::default();
    assert!(!gauge.stood_in());
    gauge.stand_in(Some(Memory::Short));
    assert!(gauge.stood_in());
    assert_eq!(gauge.read(), Memory::Short);
    gauge.stand_in(Some(Memory::Enough));
    assert_eq!(gauge.read(), Memory::Enough);
    gauge.stand_in(None);
    assert!(!gauge.stood_in());
}

#[test]
fn only_short_holds_a_start() {
    assert!(Memory::Short.is_short());
    assert!(!Memory::Enough.is_short());
    assert!(!Memory::Unread.is_short());
}

/// Linux's reader reads this machine: a reader that always fell through to unread would
/// hold nothing, silently. Not asked on macOS, where a sandboxed test run is refused the
/// `sysctl` read and the answer would say nothing of the reader.
#[cfg(target_os = "linux")]
#[test]
fn this_machine_is_read() {
    assert_ne!(read(), Memory::Unread);
}
