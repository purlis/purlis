// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a host keeps of each chat it composed in a sandbox (#2047).

use std::sync::{Arc, Mutex};

use purlis_session_protocol::session::{Layer, Listed, ReachNow};

use super::*;
use crate::sandbox::reach::{By, Decision};

fn composed(persona: &str) -> Composed {
    Composed {
        persona: Some(persona.into()),
        folder: PathBuf::from("/work/chat"),
        harness: None,
        confines: Confines::default(),
        live: None,
    }
}

/// What a chat's confinement here allows live, as a test sets it.
fn reach_now(hosts: &[(&str, Layer)]) -> ReachNow {
    ReachNow {
        asked: Vec::new(),
        allowed_live: hosts
            .iter()
            .map(|(host, layer)| Listed {
                host: (*host).to_owned(),
                layer: *layer,
            })
            .collect(),
    }
}

/// What its sandbox's run decides a connection to `host` on 443 by.
fn decided(asker: &Asker, host: &str) -> Decision {
    asker
        .confines
        .as_ref()
        .expect("held to its sandbox")
        .decides()
        .decide(host, 443, &[443], &[])
}

/// #3407: a host the person allowed for the chat since it started, on its confinement here, is
/// one its next run reaches, at the scope it was allowed at; one taken back again is not.
#[test]
fn a_chats_run_here_reaches_the_hosts_its_confinement_allows_live_now() {
    let root = tempfile::tempdir().unwrap();
    let now = Arc::new(Mutex::new(reach_now(&[])));
    let live: Live = {
        let now = Arc::clone(&now);
        Arc::new(move || now.lock().unwrap().clone())
    };
    record(
        root.path(),
        3,
        Composed {
            live: Some(Reached(live)),
            ..composed("devops")
        },
    );
    assert_eq!(
        decided(&asker(root.path(), 3).unwrap(), "pypi.example.com"),
        Decision::Ask,
        "not compiled in"
    );

    *now.lock().unwrap() = reach_now(&[("pypi.example.com", Layer::Chat)]);
    assert_eq!(
        decided(&asker(root.path(), 3).unwrap(), "pypi.example.com"),
        Decision::Allowed(By::Chat),
        "allowed live since it started"
    );

    *now.lock().unwrap() = reach_now(&[]);
    assert_eq!(
        decided(&asker(root.path(), 3).unwrap(), "pypi.example.com"),
        Decision::Ask,
        "taken back again"
    );
}

#[test]
fn a_chat_composed_here_asks_as_this_host_recorded_it() {
    let root = tempfile::tempdir().unwrap();
    record(root.path(), 3, composed("devops"));

    let recorded = asker(root.path(), 3).expect("recorded");

    assert_eq!(recorded.chat, 3);
    assert_eq!(recorded.persona.as_deref(), Some("devops"));
    assert_eq!(recorded.folder.as_deref(), Some(Path::new("/work/chat")));
    assert!(recorded.confines.is_some(), "held to its sandbox here");
    assert!(
        asker(root.path(), 4).is_none(),
        "another number is no chat of its"
    );
}

#[test]
fn a_chat_forgotten_or_of_another_project_has_no_record_here() {
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    record(root.path(), 3, composed("devops"));

    assert!(asker(other.path(), 3).is_none(), "another project's chat 3");
    forget(root.path(), 3);
    assert!(asker(root.path(), 3).is_none(), "forgotten");
}

#[test]
fn a_chat_composed_again_under_the_same_number_is_the_new_one() {
    let root = tempfile::tempdir().unwrap();
    record(root.path(), 3, composed("devops"));
    record(root.path(), 3, composed("writer"));

    assert_eq!(
        asker(root.path(), 3)
            .and_then(|recorded| recorded.persona)
            .as_deref(),
        Some("writer")
    );
}
