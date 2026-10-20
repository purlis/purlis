// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the window's answer to an ask names in a confinement a session host started (#2510,
//! ruled 2026-10-18).

use super::{Answered, Planned, answered_about, left_out_of};
use crate::sandbox::hosts::Host;
use crate::sandbox::plan::{Beside, Plan};
use crate::sandbox::reach::Reach;

#[test]
fn a_service_the_runner_left_out_is_answered_by_its_port_whatever_its_spelling() {
    for planned in [Planned::Elsewhere, Planned::Composed] {
        for asked in [
            "127.0.0.1:5432",
            "[::1]:5432",
            "localhost:5432",
            "this-mac.internal:5432",
        ] {
            assert_eq!(
                answered_about(planned, &[5432], asked),
                Ok(Answered::LeftOut(5432)),
                "{asked}"
            );
        }
    }
}

#[test]
fn a_port_of_the_runner_it_did_not_leave_out_is_never_answered_on_a_relayed_or_composed_plan() {
    for planned in [Planned::Elsewhere, Planned::Composed] {
        for asked in ["127.0.0.1:22", "[::1]:22", "localhost:22"] {
            let refused = answered_about(planned, &[5432], asked).expect_err(asked);
            assert!(refused.contains("22"), "{refused}");
        }
        for every_port in ["127.0.0.1", "localhost", "[::1]"] {
            assert!(
                answered_about(planned, &[5432], every_port).is_err(),
                "{every_port}"
            );
        }
    }
}

#[test]
fn a_service_of_this_machine_is_never_answered_live_on_a_plan_made_here() {
    assert!(answered_about(Planned::Here, &[5432], "127.0.0.1:5432").is_err());
}

#[test]
fn a_host_off_this_machine_is_answered_as_it_says() {
    for planned in [Planned::Here, Planned::Elsewhere, Planned::Composed] {
        assert_eq!(
            answered_about(planned, &[5432], "api.example.com"),
            Ok(Answered::Host(
                Host::parse("api.example.com").expect("a host")
            ))
        );
    }
}

/// The locks of a policy file saying `sandbox` (#1343).
fn locked(sandbox: &str) -> crate::sandbox::policy::Locks {
    crate::sandbox::policy::Locks::parse(
        &format!(r#"{{"sandbox": {sandbox}}}"#),
        std::path::Path::new("/etc/purlis/policy.json"),
    )
}

/// Under the runner's policy `sandbox`, a service left out of a relayed plan, or of one the
/// runner composed (#2590), is asked about nowhere, and no Allow of it is taken (#2510).
fn never_asked_under(sandbox: &str) {
    let project = tempfile::tempdir().expect("a project");
    crate::sandbox::policy::set_for_this_test(locked(sandbox));
    let asked = super::askable_here(project.path(), vec![5432]);
    let (_, composed_asked) = super::started_here(
        Planned::Composed,
        composed(Vec::new()),
        &[5432],
        project.path(),
    );
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::none());
    assert!(asked.is_empty(), "nothing is asked about");
    assert!(composed_asked.is_empty(), "nothing composed is asked about");
    for planned in [Planned::Elsewhere, Planned::Composed] {
        assert!(
            answered_about(planned, &asked, "127.0.0.1:5432").is_err(),
            "no Allow is taken"
        );
    }
}

#[test]
fn a_service_left_out_is_never_asked_about_where_the_runners_policy_forbids_services() {
    never_asked_under(r#"{"socket-grants": false}"#);
}

#[test]
fn a_service_left_out_is_never_asked_about_where_the_runners_policy_turns_live_asks_off() {
    never_asked_under(r#"{"live-asks": false}"#);
}

#[test]
fn a_service_left_out_is_never_asked_about_where_the_runners_policy_keeps_no_one_chat_scope() {
    never_asked_under(r#"{"allow-scopes": ["you", "machine"]}"#);
}

#[test]
fn a_service_left_out_is_asked_about_where_the_runners_policy_keeps_a_one_chat_scope() {
    let project = tempfile::tempdir().expect("a project");
    crate::sandbox::policy::set_for_this_test(locked(r#"{"allow-scopes": ["chat"]}"#));
    let kept = super::askable_here(project.path(), vec![5432]);
    let (_, composed_kept) = super::started_here(
        Planned::Composed,
        composed(Vec::new()),
        &[5432],
        project.path(),
    );
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::none());
    assert_eq!(kept, [5432]);
    assert_eq!(composed_kept, [5432]);
    assert_eq!(
        super::askable_here(project.path(), vec![5432]),
        [5432],
        "no policy"
    );
}

/// A plan the host composed, giving the services on `given`.
fn composed(given: Vec<u16>) -> Plan {
    Plan {
        reach: Reach::of(Vec::new()).written(),
        tunnel_ports: Vec::new(),
        plain_ports: Vec::new(),
        any_port: false,
        beside: Beside::Wrapped,
        given,
    }
}

#[test]
fn a_composed_chat_is_started_without_what_was_allowed_for_it_alone_and_the_runner_does_not_give() {
    assert_eq!(
        left_out_of(&composed(vec![6379]), &[6379, 5432, 5432, 8080]),
        [5432, 8080]
    );
    assert!(left_out_of(&composed(vec![5432]), &[5432]).is_empty());
    assert!(left_out_of(&composed(vec![5432]), &[]).is_empty());
}

/// A port given back after a re-link is held to this machine's sandbox for the project as it is
/// then (#2738): given while it is yours for the project and policy leaves services open, and
/// refused once you remove it or policy forbids services.
#[test]
fn a_port_is_given_back_only_while_this_machines_sandbox_gives_it_now() {
    let project = tempfile::tempdir().expect("a project");
    crate::sandbox::local::grant_port(project.path(), 5432).unwrap();
    assert_eq!(super::gives_here(project.path(), 5432), Ok(()));
    assert!(
        super::gives_here(project.path(), 6379).is_err(),
        "never given"
    );

    crate::sandbox::policy::set_for_this_test(locked(r#"{"socket-grants": false}"#));
    let forbidden = super::gives_here(project.path(), 5432);
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::none());
    assert!(forbidden.is_err(), "policy forbids services");

    crate::sandbox::local::revoke_port(project.path(), 5432).unwrap();
    assert!(super::gives_here(project.path(), 5432).is_err(), "removed");
}
