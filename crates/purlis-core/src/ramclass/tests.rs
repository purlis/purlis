// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const GIB: u64 = 1024 * 1024 * 1024;

/// ADR 0082 §3's table, one row for each memory the issue names.
#[test]
fn the_class_and_target_of_each_memory_are_the_tables() {
    let rows = [
        (4, None, 1),
        (8, Some(8), 5),
        (16, Some(16), 15),
        (32, Some(32), 35),
        (64, Some(64), 50),
        (128, Some(64), 50),
    ];
    for (gib, class, target) in rows {
        let got = RamClass::of(gib * GIB);
        assert_eq!(got.gigabytes(), class, "{gib} GB: class");
        assert_eq!(got.hot_target(), target, "{gib} GB: target");
    }
}

#[test]
fn a_memory_well_between_classes_takes_the_class_below_it() {
    // A 12 GB machine is in the 8 GB class (ADR 0082 §3), and so is one with 14 GB.
    assert_eq!(RamClass::of(12 * GIB).gigabytes(), Some(8));
    assert_eq!(RamClass::of(14 * GIB).gigabytes(), Some(8));
    assert_eq!(RamClass::of(15 * GIB).gigabytes(), Some(8));
}

#[test]
fn a_memory_a_little_under_a_class_is_that_class() {
    // An operating system reports a little less than is installed: 15.5 GB is a 16 GB machine.
    assert_eq!(RamClass::of(31 * GIB / 2).gigabytes(), Some(16));
    assert_eq!(RamClass::of(76 * GIB / 10).gigabytes(), Some(8));
    assert_eq!(RamClass::of(7 * GIB).gigabytes(), None);
}

/// Linux's `MemTotal` sits below the installed memory, by what the kernel keeps for itself.
#[test]
fn linux_s_reported_total_gives_the_installed_class() {
    let rows = [
        ("MemTotal:        8029508 kB", Some(8), 5),
        ("MemTotal:       16265220 kB", Some(16), 15),
        ("MemTotal:       32775460 kB", Some(32), 35),
        ("MemTotal:       65842252 kB", Some(64), 50),
    ];
    for (meminfo, class, target) in rows {
        let total = crate::memorypressure::meminfo_total(meminfo).expect(meminfo);
        let got = RamClass::of(total);
        assert_eq!(
            (got.gigabytes(), got.hot_target()),
            (class, target),
            "{meminfo}"
        );
    }
}
