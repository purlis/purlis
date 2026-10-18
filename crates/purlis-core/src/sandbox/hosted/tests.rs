// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What an Allow allows in a confinement a session host started (#2510, ruled 2026-10-18).

use super::{Allowing, Planned, allowed_as};
use crate::sandbox::hosts::Host;
use crate::sandbox::reach::By;

#[test]
fn a_service_the_runner_left_out_is_allowed_for_this_chat_alone_whatever_its_spelling() {
    for asked in [
        "127.0.0.1:5432",
        "[::1]:5432",
        "localhost:5432",
        "this-mac.internal:5432",
    ] {
        for by in [By::Chat, By::You, By::Project] {
            assert_eq!(
                allowed_as(Planned::Elsewhere, &[5432], asked, by),
                Ok(Allowing::ThisMachine(5432)),
                "{asked} at {by:?}"
            );
        }
    }
}

#[test]
fn a_port_of_the_runner_it_did_not_leave_out_is_never_allowed_on_a_relayed_plan() {
    for asked in ["127.0.0.1:22", "[::1]:22", "localhost:22"] {
        let refused = allowed_as(Planned::Elsewhere, &[5432], asked, By::Chat).expect_err(asked);
        assert!(refused.contains("22"), "{refused}");
    }
    for every_port in ["127.0.0.1", "localhost", "[::1]"] {
        assert!(
            allowed_as(Planned::Elsewhere, &[5432], every_port, By::Chat).is_err(),
            "{every_port}"
        );
    }
}

#[test]
fn a_service_of_this_machine_is_never_allowed_live_on_a_plan_made_here() {
    assert!(allowed_as(Planned::Here, &[5432], "127.0.0.1:5432", By::Chat).is_err());
}

#[test]
fn a_host_off_this_machine_is_allowed_as_it_says() {
    for planned in [Planned::Here, Planned::Elsewhere] {
        assert_eq!(
            allowed_as(planned, &[5432], "api.example.com", By::You),
            Ok(Allowing::Host(
                Host::parse("api.example.com").expect("a host"),
                By::You
            ))
        );
    }
}
