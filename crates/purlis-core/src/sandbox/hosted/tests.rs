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

/// Under the runner's policy `sandbox`, a relayed plan or one the runner composed asks the window
/// about no host, so no answer of the window widens its reach there.
fn no_host_asked_under(sandbox: &str) {
    let project = tempfile::tempdir().expect("a project");
    crate::sandbox::policy::set_for_this_test(locked(sandbox));
    let asked: Vec<bool> = [Planned::Composed, Planned::Elsewhere]
        .into_iter()
        .map(|planned| super::hosts_askable(planned, project.path()))
        .collect();
    let here = super::hosts_askable(Planned::Here, project.path());
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::none());
    assert_eq!(asked, [false, false], "no host is asked about");
    assert!(here, "the window here applied its own locks already");
}

#[test]
fn a_host_is_never_asked_about_on_a_relayed_or_composed_plan_where_the_runners_policy_turns_live_asks_off()
 {
    no_host_asked_under(r#"{"live-asks": false}"#);
}

#[test]
fn a_host_is_never_asked_about_on_a_relayed_or_composed_plan_where_the_runners_policy_keeps_no_one_chat_scope()
 {
    no_host_asked_under(r#"{"allow-scopes": ["you", "machine"]}"#);
}

#[test]
fn a_host_is_asked_about_on_a_relayed_or_composed_plan_under_no_policy() {
    let project = tempfile::tempdir().expect("a project");
    for planned in [Planned::Here, Planned::Composed, Planned::Elsewhere] {
        assert!(super::hosts_askable(planned, project.path()), "{planned:?}");
    }
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
        forge: None,
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

/// **The window's Allow next time reaches the host's board** (1.88, T1-34; R-291): `AllowNext`
/// holds the host for the chat's next attempt there, `ForgetNext` takes it off, a proxy that
/// asks nobody refuses both in a sentence, and any other change is not theirs.
///
/// Mutations it kills: `board.allow_next` deleted from `Hosted::change`, and the allow and the
/// removal swapped.
#[test]
fn the_windows_allow_next_time_reaches_the_hosts_board_and_its_removal_takes_it_off() {
    use purlis_session_protocol::session::ReachChange;
    let board = crate::sandbox::asks::Asks::new(
        std::sync::Arc::new(|_: crate::sandbox::asks::Heard| {}),
        &crate::sandbox::timings::Timings::production(),
    );
    let allow = ReachChange::AllowNext {
        host: "api.example.com:443".to_owned(),
    };
    assert_eq!(
        super::next_changed(Some(&board), &allow),
        Some(Ok(Vec::new()))
    );
    assert_eq!(
        board.next_attempts(),
        vec!["api.example.com:443".to_owned()]
    );
    let forget = ReachChange::ForgetNext {
        host: "api.example.com:443".to_owned(),
    };
    assert_eq!(
        super::next_changed(Some(&board), &forget),
        Some(Ok(Vec::new()))
    );
    assert!(board.next_attempts().is_empty());
    assert!(matches!(super::next_changed(None, &allow), Some(Err(_))));
    let not_a_host = ReachChange::AllowNext {
        host: "not a host".to_owned(),
    };
    assert!(matches!(
        super::next_changed(Some(&board), &not_a_host),
        Some(Err(_))
    ));
    let other = ReachChange::GiveUp {
        target: "api.example.com:443".to_owned(),
    };
    assert_eq!(super::next_changed(Some(&board), &other), None);
}

/// **A plan's fit looks its listed names up through the one recorded lookup** (T1.1-06): every
/// name the runner's fit asked the system's resolver for is in `asked_the_system`, so a scenario's
/// "saw nothing" cannot pass while a fit looked a name up behind it.
#[test]
fn a_plans_fit_records_each_name_it_asked_the_system_for() {
    let project = tempfile::tempdir().expect("a project");
    let name = format!("fit-{}.invalid", std::process::id());
    let plan = Plan {
        reach: Reach::of(vec![(name.clone(), crate::sandbox::reach::By::You)]).written(),
        ..composed(Vec::new())
    };
    let _fits = super::fits_here(project.path(), &plan.wired());
    assert!(
        crate::sandbox::names::asked_the_system(&name),
        "the fit looked {name} up without the record seeing it"
    );
}

/// **The host side of [`session::Confine::lifted`]** (1.94, the Q308 follow-up; the review of
/// #7105): a family the window named is lifted, and each host let through for it is told to the
/// window by the family; one it did not name, or one this machine's own locks hold enforced, is
/// not, whatever the window relayed.
#[test]
fn a_runner_lifts_only_what_the_window_named_and_its_own_locks_leave_open() {
    use crate::enforcement::Family;
    use crate::sandbox::policy::Locks;
    use purlis_session_protocol::session::SandboxTold;
    let told = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let tell: super::Tell = {
        let told = std::sync::Arc::clone(&told);
        std::sync::Arc::new(move |one: SandboxTold| told.lock().unwrap().push(one))
    };
    let named = ["asks".to_owned(), "network".to_owned()];
    let open = Locks::none();
    for family in [Family::Asks, Family::Network] {
        let answered = super::lifted(&named, family, &open, &tell).expect("lifted");
        answered("api.example.com", 8443);
        assert!(super::lifted(&[], family, &open, &tell).is_none());
        assert!(super::lifted(&named[1..], Family::Asks, &open, &tell).is_none());
    }
    assert_eq!(
        *told.lock().unwrap(),
        [
            SandboxTold::Lifted {
                family: "asks".to_owned(),
                target: "api.example.com:8443".to_owned(),
            },
            SandboxTold::Lifted {
                family: "network".to_owned(),
                target: "api.example.com:8443".to_owned(),
            },
        ]
    );
    // This machine's administrator holds `network` enforced: never lifted, though named.
    let held = Locks::locking_enforcement(Family::Network);
    assert!(super::lifted(&named, Family::Network, &held, &tell).is_none());
    assert!(super::lifted(&named, Family::Asks, &held, &tell).is_some());
}
