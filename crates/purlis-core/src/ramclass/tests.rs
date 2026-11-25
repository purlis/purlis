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
fn a_memory_between_classes_takes_the_class_below_it() {
    // A 12 GB machine is in the 8 GB class (ADR 0082 §3), one byte short of 16 GB too.
    assert_eq!(RamClass::of(12 * GIB).gigabytes(), Some(8));
    assert_eq!(RamClass::of(16 * GIB - 1).gigabytes(), Some(8));
}

#[test]
fn a_memory_that_could_not_be_read_targets_one_chat() {
    let unread = RamClass::of_total(None);
    assert_eq!(unread.gigabytes(), None);
    assert_eq!(unread.hot_target(), 1);
}
