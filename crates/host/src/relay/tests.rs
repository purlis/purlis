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
