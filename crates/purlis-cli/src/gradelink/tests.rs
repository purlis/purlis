// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn plan() -> GradePlanTold {
    GradePlanTold {
        profile: Some("work".into()),
        model: None,
        sandbox: Some("Full".into()),
        refused: None,
        chats: 6,
        token_cap: 200_000,
        limit_micro_usd: 5_000_000,
        running: None,
        graded: Some("a".into()),
        consent: Some("b".into()),
    }
}

#[test]
fn a_command_in_a_chat_is_known_by_its_number_its_socket_or_its_sandbox() {
    let with = |name: &'static str, value: &'static str| {
        move |asked: &str| (asked == name).then(|| value.to_owned())
    };
    assert!(in_a_chat(&with(purlis_core::hookwire::CHAT_ENV, "7")));
    assert!(in_a_chat(&with(SOCKET_ENV, "/tmp/hooks.sock")));
    assert!(in_a_chat(&with(purlis_core::hookwire::SANDBOXED_ENV, "1")));
    assert!(!in_a_chat(&|_: &str| None));
    assert!(!in_a_chat(&with(SOCKET_ENV, "")), "an empty one names none");
}

#[test]
fn only_a_yes_is_a_yes() {
    for yes in ["y\n", "Y\n", "yes\n", " YES \n"] {
        assert!(is_yes(yes), "{yes}");
    }
    for no in ["", "\n", "n\n", "yeah\n", "y y\n"] {
        assert!(!is_yes(no), "{no}");
    }
}

#[test]
fn the_terms_a_yes_covers_are_printed_with_their_limits() {
    let said = terms_said(&plan());
    for word in [
        "6 chats",
        "work",
        "the profile's own",
        "Full",
        "200000",
        "$5.00",
    ] {
        assert!(said.contains(word), "{said}");
    }
}

#[test]
fn a_plan_that_cannot_start_says_why_before_a_yes_is_asked() {
    assert_eq!(can_start(&plan()), Ok(()));
    let no_sandbox = GradePlanTold {
        refused: Some("No profile.".into()),
        ..plan()
    };
    assert!(can_start(&no_sandbox).unwrap_err().contains("No profile."));
    let running = GradePlanTold {
        running: Some("g".into()),
        ..plan()
    };
    assert!(can_start(&running).unwrap_err().contains("running"));
}
