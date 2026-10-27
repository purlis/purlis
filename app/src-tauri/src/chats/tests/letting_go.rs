// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's start against its project's close (#1949): a start now runs off the window's
//! thread, and so does a close, so the two can meet.

use std::sync::Arc;
use std::time::Duration;

use purlis_core::engine::Size;
use purlis_core::reopen::Chat;

use super::{Chats, LETTING_GO};
use crate::host::SessionHost;
use crate::host::pretend::Pretend;

const SIZE: Size = Size {
    columns: 80,
    rows: 24,
};

fn a_shell() -> Chat {
    Chat {
        program: "/bin/sh".to_owned(),
        name: "a shell".to_owned(),
        ..Default::default()
    }
}

fn chats_on(host: &Pretend) -> Arc<Chats> {
    Arc::new(Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        super::tests::no_project(),
    ))
}

#[test]
fn a_project_being_let_go_of_starts_no_chat() {
    let host = Pretend::default();
    let chats = chats_on(&host);
    chats.end_all();

    let started = chats.start(&a_shell(), SIZE);

    assert_eq!(started, Err(LETTING_GO.to_owned()));
    assert!(host.asked().is_empty(), "a program was started");
    assert!(chats.open_now().is_empty());
}

#[test]
fn a_chat_that_starts_while_its_project_is_let_go_of_is_ended_with_the_rest() {
    // New chat, then Close at once: the start is past its first look and inside the host's
    // start when the close ends every chat it finds. The one it did not find yet is ended
    // too, and no program is left running in a project nobody holds.
    let host = Pretend::default();
    host.takes(Duration::from_millis(300));
    let chats = chats_on(&host);
    let starting = std::thread::spawn({
        let chats = Arc::clone(&chats);
        move || chats.start(&a_shell(), SIZE)
    });
    // The start is in the host's start by now.
    while host.most_at_once() == 0 {
        std::thread::sleep(Duration::from_millis(2));
    }
    chats.end_all();

    let started = starting.join().expect("the start ends");

    assert_eq!(started, Err(LETTING_GO.to_owned()));
    assert!(
        host.running().is_empty(),
        "left running: {:?}",
        host.running()
    );
    assert!(chats.open_now().is_empty());
}
