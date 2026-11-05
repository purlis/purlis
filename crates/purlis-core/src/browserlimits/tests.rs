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
    assert!(project_full_said(3, &[], 3).contains("Settings › This machine › Browsers"));
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

fn own(name: &str, login: Option<&str>, shows: Shows, person_has_it: bool) -> OwnBrowser {
    OwnBrowser {
        name: name.to_owned(),
        login: login.map(str::to_owned),
        shows,
        person_has_it,
    }
}

#[test]
fn a_refusal_at_the_limit_says_what_each_of_the_chats_browsers_holds() {
    let said = project_full_said(
        4,
        &[
            own("b1", None, Shows::Nothing, false),
            own("b2", None, Shows::ErrorPage, false),
            own(
                "b3",
                Some("shop"),
                Shows::Site("shop.test:443".into()),
                false,
            ),
        ],
        1,
    );
    assert!(said.contains("have 4 browsers open"), "{said}");
    assert!(
        said.contains(
            "This chat's: b1 (empty), b2 (on an error page), \
             b3 (holds login shop, on shop.test:443); 1 more is another chat's. \
             Close one of this chat's with browser_close first"
        ),
        "{said}"
    );
}

#[test]
fn a_browser_the_person_has_is_said_as_theirs_and_not_as_empty() {
    assert_eq!(
        own("b1", None, Shows::Nothing, true).to_string(),
        "b1 (the person has taken it over)"
    );
    assert_eq!(
        own("b1", Some("shop"), Shows::Nothing, false).to_string(),
        "b1 (holds login shop)",
        "a browser holding a login isn't empty"
    );
}

#[test]
fn a_refusal_to_a_chat_with_no_browser_offers_it_no_close_of_its_own() {
    let said = project_full_said(2, &[], 2);
    assert!(said.contains("None of them is this chat's"), "{said}");
    assert!(!said.contains("browser_close"), "{said}");
}
