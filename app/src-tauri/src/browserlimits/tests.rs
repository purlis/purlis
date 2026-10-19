// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn each_limit_shows_what_is_in_force_its_default_and_whether_it_was_set() {
    let defaults = Limits::for_memory(Some(32 * 1024 * 1024 * 1024));
    let set = purlis_core::machine::BrowserLimits {
        on_this_device: Some(5),
        idle_minutes: Some(0),
        ..Default::default()
    };

    let shown = shown(defaults, set);

    assert_eq!(
        shown.on_this_device,
        BrowserLimitShown {
            value: 5,
            default: 8,
            set: true,
            least: 1,
            most: 64
        }
    );
    assert_eq!((shown.per_project.value, shown.per_project.set), (3, false));
    assert_eq!(
        (shown.idle_minutes.value, shown.idle_minutes.default),
        (0, 10)
    );
}
