// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn gib(n: f64) -> Option<u64> {
    Some((n * GIB as f64) as u64)
}

#[test]
fn the_budget_follows_the_ruled_memory_classes() {
    assert_eq!(budget_for(gib(16.0)), 4);
    assert_eq!(budget_for(gib(32.0)), 8);
    assert_eq!(budget_for(gib(64.0)), 16);
}

#[test]
fn a_device_reporting_a_little_less_than_fitted_keeps_its_class() {
    assert_eq!(budget_for(gib(15.6)), 4);
    assert_eq!(budget_for(gib(31.2)), 8);
}

#[test]
fn a_device_between_classes_is_budgeted_by_the_class_beneath() {
    assert_eq!(budget_for(gib(24.0)), 4);
    assert_eq!(budget_for(gib(48.0)), 8);
    assert_eq!(budget_for(gib(128.0)), 32);
}

#[test]
fn a_small_device_has_a_small_budget_and_never_none() {
    assert_eq!(budget_for(gib(8.0)), 2);
    assert_eq!(budget_for(gib(4.0)), 1);
    assert_eq!(budget_for(gib(2.0)), 1);
    assert_eq!(budget_for(Some(0)), 1);
}

#[test]
fn a_device_whose_memory_could_not_be_read_gets_the_smallest_ruled_budget() {
    assert_eq!(budget_for(None), 4);
}

#[test]
fn the_limits_are_three_a_project_and_ten_idle_minutes() {
    let limits = Limits::for_memory(gib(32.0));
    assert_eq!(limits.per_project, 3);
    assert_eq!(limits.on_this_device, 8);
    assert_eq!(limits.idle_after, Some(Duration::from_secs(600)));
}

#[test]
fn what_the_person_set_takes_the_place_of_each_default_and_zero_idle_minutes_is_never() {
    let set = crate::machine::BrowserLimits {
        per_project: Some(5),
        on_this_device: None,
        idle_minutes: Some(0),
        machine_chrome: false,
    };

    let limits = Limits::for_memory(gib(32.0)).set_to(set);

    assert_eq!(limits.per_project, 5);
    assert_eq!(limits.on_this_device, 8, "the budget its memory gives");
    assert_eq!(limits.idle_after, None);
    let idle = Limits::for_memory(None).set_to(crate::machine::BrowserLimits {
        idle_minutes: Some(25),
        ..crate::machine::BrowserLimits::default()
    });
    assert_eq!(idle.idle_after, Some(Duration::from_secs(25 * 60)));
}

#[test]
fn a_limit_is_set_on_this_machine_back_to_its_default_and_refused_out_of_its_bounds() {
    let machine = tempfile::tempdir().expect("a config home");

    set(machine.path(), Limit::OnThisDevice, Some(6)).expect("it is kept");
    assert_eq!(
        crate::machine::read(machine.path())
            .store
            .browsers
            .on_this_device,
        Some(6)
    );
    let why = set(machine.path(), Limit::PerProject, Some(0)).unwrap_err();
    assert!(why.contains("between 1 and 16"), "{why}");
    set(machine.path(), Limit::OnThisDevice, None).expect("back to its default");
    assert_eq!(
        crate::machine::read(machine.path()).store.browsers,
        crate::machine::BrowserLimits::default()
    );
}

#[test]
fn a_refusal_names_the_setting_that_decides_it_and_opens_with_purlis_side() {
    assert!(project_full_said(3).contains("Settings › This machine › Browsers"));
    let device = device_full_said(1);
    assert!(
        device.starts_with("purlis runs at most 1 browser at once"),
        "{device}"
    );
    assert!(
        device.contains("Settings › This machine › Browsers"),
        "{device}"
    );
    assert!(device_full_told(8).contains("raise the budget in Settings › This machine › Browsers"));
}
