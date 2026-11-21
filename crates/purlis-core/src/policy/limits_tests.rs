// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn key(name: &str) -> &'static Spec {
    spec(name).unwrap_or_else(|| panic!("the table names {name}"))
}

#[test]
fn the_table_reads_every_row_with_its_shipped_value_and_direction() {
    assert_eq!(table().len(), 38, "every row of limits.toml");
    assert_eq!(key("dispatch.running-per-chat").shipped, Shipped::Number(6));
    assert_eq!(key("dispatch.tokens-per-session").shipped, Shipped::Unset);
    assert_eq!(key("browser.on-this-device").shipped, Shipped::ByMemory);
    assert_eq!(key("audit.record-days").loosens, Direction::Lowered);
    assert_eq!(key("trust.unread-max").loosens, Direction::Raised);
    assert!(spec("dispatch.depth").is_none(), "depth stays in Cedar");
}

#[test]
fn every_dispatch_limit_the_organisation_can_write_is_in_the_table() {
    for limit in crate::dispatchlimits::Limit::ALL {
        let named = spec(&format!("dispatch.{}", limit.word())).is_some();
        let plain_setting = limit.notes()
            || matches!(
                limit,
                crate::dispatchlimits::Limit::Depth
                    | crate::dispatchlimits::Limit::StuckFailingTimes
                    | crate::dispatchlimits::Limit::StuckLongMinutes
                    | crate::dispatchlimits::Limit::StuckSilentMinutes
            );
        assert!(
            named != plain_setting,
            "{} is either a table key or a plain setting",
            limit.word()
        );
    }
}

#[test]
fn the_in_force_limit_is_the_stricter_in_its_declared_direction() {
    // A cap: the lower number is the stricter.
    let cap = key("dispatch.running-per-chat");
    assert_eq!(
        in_force(cap, None, [Some(3), None]),
        Some(3),
        "lowering a cap tightens at once"
    );
    assert_eq!(
        in_force(cap, None, [Some(12), None]),
        Some(6),
        "raising it does not"
    );
    assert_eq!(in_force(cap, None, [Some(12), Some(3), Some(9)]), Some(3));
    // The accepted side's own number is what a raise waits on, not the shipped one.
    assert_eq!(in_force(cap, None, [Some(12), Some(10)]), Some(10));
    // A retention: the higher number is the stricter.
    let retention = key("audit.record-days");
    assert_eq!(
        in_force(retention, None, [Some(500), None]),
        Some(500),
        "keeping longer tightens"
    );
    assert_eq!(
        in_force(retention, None, [Some(90), None]),
        Some(365),
        "keeping less loosens"
    );
    // The Organisation's number binds below it, and never raises what a side holds.
    assert_eq!(in_force(cap, Some(2), [Some(5), None]), Some(2));
    assert_eq!(in_force(cap, Some(20), [Some(5), None]), Some(5));
    // Silence everywhere is the shipped value.
    assert_eq!(in_force(cap, None, []), Some(6));
    assert_eq!(in_force(cap, None, [None, None]), Some(6));
}

#[test]
fn a_limit_that_is_off_until_set_is_tightened_by_any_number() {
    let tokens = key("dispatch.tokens-per-session");
    assert_eq!(in_force(tokens, None, [None, None]), None);
    assert_eq!(in_force(tokens, None, [Some(1000), Some(5000)]), Some(1000));
    assert_eq!(
        in_force(tokens, None, [Some(1000), None]),
        Some(1000),
        "a side that says none is no cap there"
    );
}

#[test]
fn under_limits_log_a_limit_is_counted_not_applied() {
    assert_eq!(verdict(Mode::Enforce, Some(6), 7), Verdict::Refuse);
    assert_eq!(verdict(Mode::Log, Some(6), 7), Verdict::Counted);
    assert_eq!(verdict(Mode::Off, Some(6), 7), Verdict::Within);
    assert_eq!(
        verdict(Mode::Enforce, Some(6), 6),
        Verdict::Within,
        "at the cap"
    );
    assert_eq!(
        verdict(Mode::Enforce, None, i64::MAX),
        Verdict::Within,
        "no cap"
    );
}
