// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_session_protocol::session::{Event, Exit, SandboxTold, ViewEnd};

/// One of each thing a host pushes.
fn every_push() -> Vec<Pushed> {
    vec![
        Pushed::Event(Event {
            v: 1,
            device_id: "01J9ZZDEVICE0000000000000".into(),
            seq: 1,
            ulid: "01J9ZZEVENT00000000000000".into(),
            chat: None,
            run: None,
            parent_run: None,
            kind: "hook.stop".into(),
            body: serde_json::Value::Null,
        }),
        Pushed::Missed {
            device: "01J9ZZDEVICE0000000000000".into(),
            after: 1,
            resumes_at: 2,
        },
        Pushed::Ended {
            program: "01J9ZZPRGRM00000000000000".into(),
            run: None,
            exit: Exit::Code(0),
        },
        Pushed::ViewEnded {
            view: 1,
            why: ViewEnd::Closed,
        },
        Pushed::Heard {
            project: "01J9PRJCT00000000000000000".into(),
            heard: 1,
            line: serde_json::Value::Null,
            pathless: true,
        },
        Pushed::Sandbox {
            confinement: "01J9CNFNMNT0000000000000000".into(),
            told: SandboxTold::RefusedMore { count: 1 },
        },
        Pushed::Saving {
            project: "01J9PRJCT00000000000000000".into(),
            saved: true,
            moved_tree: false,
        },
        Pushed::WhoIsMidTurn {
            question: 1,
            project: "01J9PRJCT00000000000000000".into(),
            workspace: "alpha".into(),
        },
    ]
}

/// FD-27's table, as the relay meets it: a push reaches a client of the relay that is not
/// the window exactly when some scope that is neither the window's nor a link between hosts
/// may hear it on a host of its own.
#[test]
fn what_the_relay_keeps_from_a_client_that_is_not_the_window_is_what_the_table_grants_the_window_alone()
 {
    for pushed in every_push() {
        let anyones = pushed.heard_with().is_none_or(|power| {
            Scope::ALL
                .into_iter()
                .filter(|scope| !matches!(scope, Scope::LocalUi | Scope::RemoteLink))
                .any(|scope| scope.may(power))
        });
        assert_eq!(!heard_by_the_window_alone(&pushed), anyones, "{pushed:?}");
    }
}

/// #5272: the window's check of a version is of its own host, so the host that relays holds its
/// runner to it, and no value reaches a runner that starts no login clean.
#[test]
fn no_login_is_sent_on_to_a_runner_that_starts_none_clean() {
    use purlis_session_protocol::browsing::{LogIn, Secret};
    let login = Command::LogIn {
        program: "p".to_owned(),
        login: Box::new(LogIn {
            address: "https://shop.example.test/login".to_owned(),
            user: None,
            password: Secret::new("fixture-word".to_owned()),
            keep: None,
            identity: None,
            suggested: false,
            form: None,
            start: None,
        }),
    };
    let before = purlis_session_protocol::version::Version {
        minor: session::KNOWS_CLEAN_LOGINS.minor - 1,
        ..session::KNOWS_CLEAN_LOGINS
    };
    let why = no_login_for(&login, before).expect("refused");
    assert!(why.contains("nothing was filled"), "{why}");
    assert_eq!(no_login_for(&login, session::KNOWS_CLEAN_LOGINS), None);
    assert_eq!(no_login_for(&Command::List, before), None);
}

/// #6291: a runner before 1.86 would drop a login's start and open its page, so no login with one
/// is sent on to it; one without still is.
#[test]
fn no_login_with_a_start_is_sent_on_to_a_runner_that_would_drop_it() {
    use purlis_session_protocol::browsing::{LogIn, Secret};
    let login = |start: Option<&str>| Command::LogIn {
        program: "p".to_owned(),
        login: Box::new(LogIn {
            address: "https://app.example.test/".to_owned(),
            user: None,
            password: Secret::new("fixture-word".to_owned()),
            keep: None,
            identity: None,
            suggested: false,
            form: Some("https://auth.example.test".to_owned()),
            start: start.map(str::to_owned),
        }),
    };
    let before = purlis_session_protocol::version::Version {
        minor: session::KNOWS_LOGIN_START.minor - 1,
        ..session::KNOWS_LOGIN_START
    };
    let started = login(Some("https://auth.example.test/authorize?client_id=app"));
    let why = no_login_for(&started, before).expect("refused");
    assert!(why.contains("nothing was filled"), "{why}");
    assert!(!why.contains("client_id"), "{why}");
    assert_eq!(no_login_for(&started, session::KNOWS_LOGIN_START), None);
    assert_eq!(no_login_for(&login(None), before), None);
}

#[test]
fn neither_a_fill_nor_the_read_of_a_field_for_one_goes_to_a_runner_too_old_for_it() {
    use purlis_session_protocol::browsing::{Act, FillSecret, Secret};
    let read = Command::Browse {
        program: "program-1".into(),
        call_id: "c1".into(),
        act: Act::DescribeField {
            target: "e1".into(),
        },
        reach: None,
    };
    let fill = Command::FillSecret {
        program: "program-1".into(),
        fill: Box::new(FillSecret {
            target: "e1".into(),
            origin: "https://a.test".into(),
            key: "key".into(),
            value: Secret::new("fixture-word".to_owned()),
            multi_line: false,
            also: Vec::new(),
            under_a_grant: false,
        }),
    };
    let before = purlis_session_protocol::version::Version {
        minor: session::KNOWS_FILLS_KEPT_OUT.minor - 1,
        ..session::KNOWS_FILLS_KEPT_OUT
    };
    for command in [&read, &fill] {
        let why = no_fill_for(command, before).expect("refused");
        assert!(why.contains("nothing was filled"), "{why}");
        assert_eq!(no_fill_for(command, session::KNOWS_FILLS_KEPT_OUT), None);
    }
    assert_eq!(no_fill_for(&Command::List, before), None);
}
