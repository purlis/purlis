// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, Mutex, Weak};

use super::*;

const CHAT: u32 = 7;

fn moved(state: &str) -> Moved {
    Moved {
        plane: PlaneId::for_tests(std::path::Path::new("/p")),
        session: CHAT,
        state: state.to_owned(),
        needs_you: false,
        queue: Vec::new(),
        moved_at: 1,
        reports: Vec::new(),
        refusals: Vec::new(),
        stopped: None,
        needs: None,
        asking: None,
        own_work: None,
        background_turn: None,
        only_failures: None,
        sequence: 1,
        children: Vec::new(),
        counts_only: false,
        raised: false,
    }
}

/// A project's watch with no project held, and what it told.
fn noticed() -> (Arc<Noticed>, Arc<Mutex<Vec<ChatNoticed>>>) {
    let told = Arc::new(Mutex::new(Vec::new()));
    let noticed = Arc::new(Noticed::new(PlaneId::for_tests(std::path::Path::new("/p"))));
    let into = Arc::clone(&told);
    noticed.held_as(
        Weak::new(),
        Arc::new(move |one| into.lock().expect("held").push(one)),
    );
    (noticed, told)
}

fn last_stuck(told: &Mutex<Vec<ChatNoticed>>) -> Option<String> {
    told.lock()
        .expect("held")
        .last()
        .and_then(|one| one.stuck.clone())
}

#[test]
fn a_working_chat_whose_command_fails_three_times_is_told_stuck_until_it_s_fine() {
    let (noticed, told) = noticed();
    noticed.moved(&moved("running"));
    for _ in 0..3 {
        noticed.ran(CHAT, "cargo test", true);
    }
    assert_eq!(
        last_stuck(&told).as_deref(),
        Some("the same command failed 3 times in a row")
    );
    noticed.fine(CHAT);
    assert_eq!(last_stuck(&told), None, "It's fine clears it");
    assert_eq!(noticed.all().len(), 1, "what a window that opens is told");
}

#[test]
fn a_chat_not_working_is_never_told_stuck() {
    let (noticed, told) = noticed();
    noticed.moved(&moved("waiting"));
    for _ in 0..3 {
        noticed.ran(CHAT, "cargo test", true);
    }
    assert!(
        told.lock()
            .expect("held")
            .iter()
            .all(|one| one.stuck.is_none())
    );
}

#[test]
fn a_missing_sign_in_is_told_to_the_window_once_and_goes_with_its_chat() {
    let (noticed, told) = noticed();
    let needed = crate::stuck::SignInNeeded {
        host: "github.com".to_owned(),
        ended: false,
    };
    noticed.sign_in_needed(CHAT, needed.clone());
    noticed.sign_in_needed(CHAT, needed.clone());
    assert_eq!(
        told.lock().expect("held").len(),
        1,
        "a repeat tells nothing"
    );
    assert_eq!(
        told.lock()
            .expect("held")
            .last()
            .and_then(|one| one.sign_in.clone()),
        Some(needed)
    );

    // The sign-in ends: that is another cause, told again.
    noticed.sign_in_needed(
        CHAT,
        crate::stuck::SignInNeeded {
            host: "github.com".to_owned(),
            ended: true,
        },
    );
    assert_eq!(told.lock().expect("held").len(), 2);

    noticed.forget(CHAT);
    assert!(noticed.all().is_empty(), "a closed chat keeps nothing");
}

#[test]
fn a_credential_for_the_host_takes_its_sign_in_notice_down_and_another_hosts_stays() {
    let (noticed, told) = noticed();
    noticed.sign_in_needed(
        CHAT,
        crate::stuck::SignInNeeded {
            host: "github.com".to_owned(),
            ended: false,
        },
    );
    noticed.sign_in_ok(CHAT, "gitlab.com");
    assert_eq!(
        told.lock().expect("held").len(),
        1,
        "another host changes nothing"
    );

    noticed.sign_in_ok(CHAT, "github.com");
    noticed.sign_in_ok(CHAT, "github.com");
    let told = told.lock().expect("held");
    assert_eq!(told.len(), 2, "taken down once");
    assert_eq!(told.last().and_then(|one| one.sign_in.clone()), None);
}
