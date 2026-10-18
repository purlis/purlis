// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the window's answer to an ask names in a confinement a session host started (#2510,
//! ruled 2026-10-18).

use super::{Answered, Planned, answered_about};
use crate::sandbox::hosts::Host;

#[test]
fn a_service_the_runner_left_out_is_answered_by_its_port_whatever_its_spelling() {
    for asked in [
        "127.0.0.1:5432",
        "[::1]:5432",
        "localhost:5432",
        "this-mac.internal:5432",
    ] {
        assert_eq!(
            answered_about(Planned::Elsewhere, &[5432], asked),
            Ok(Answered::LeftOut(5432)),
            "{asked}"
        );
    }
}

#[test]
fn a_port_of_the_runner_it_did_not_leave_out_is_never_answered_on_a_relayed_plan() {
    for asked in ["127.0.0.1:22", "[::1]:22", "localhost:22"] {
        let refused = answered_about(Planned::Elsewhere, &[5432], asked).expect_err(asked);
        assert!(refused.contains("22"), "{refused}");
    }
    for every_port in ["127.0.0.1", "localhost", "[::1]"] {
        assert!(
            answered_about(Planned::Elsewhere, &[5432], every_port).is_err(),
            "{every_port}"
        );
    }
}

#[test]
fn a_service_of_this_machine_is_never_answered_live_on_a_plan_made_here() {
    assert!(answered_about(Planned::Here, &[5432], "127.0.0.1:5432").is_err());
}

#[test]
fn a_host_off_this_machine_is_answered_as_it_says() {
    for planned in [Planned::Here, Planned::Elsewhere] {
        assert_eq!(
            answered_about(planned, &[5432], "api.example.com"),
            Ok(Answered::Host(
                Host::parse("api.example.com").expect("a host")
            ))
        );
    }
}
