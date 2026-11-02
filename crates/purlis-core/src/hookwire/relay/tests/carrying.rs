// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner's host carries what its chats queued to the window itself** (#4804): the queue
//! is on the runner's disk, beside the hook socket its host binds, where the window does not
//! read. A `Deliver` a chat asks is answered by this host sending each message on, oldest first;
//! what it could not send waits, and goes once the window hears this host again.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
use crate::dispatched::{Answered, Asked, What};
use crate::dispatchqueue::{self, Queued};
use crate::hookwire::ChatToken;

/// The chat whose messages are queued: one of those [`now_and_then_with`] holds a token of.
const CHAT: u32 = 3;

/// What the window holds back, as a limit between two chats does: it stays queued.
const HELD_BACK: &str = "held back";

/// A project's hook socket on a runner, chat [`CHAT`]'s key remembered there as its host
/// remembers it when it issues the chat its token.
fn a_runner_project() -> (tempfile::TempDir, PathBuf, ChatToken) {
    let root = tempfile::tempdir().expect("a folder");
    let app = root.path().join(".purlis/app");
    std::fs::create_dir_all(&app).expect("the app folder");
    let socket = app.join("hooks.sock");
    let token = ChatToken::from("tok-three");
    crate::hookwire::spool::remember(&app.join("spool"), CHAT, &token).expect("the key");
    (root, socket, token)
}

/// Chat [`CHAT`] queues a note saying `text`, as `purlis dispatch note` does: its id.
fn queues(socket: &Path, token: &ChatToken, text: &str) -> String {
    let note = Queued::Note {
        text: Some(text.to_owned()),
        ack: None,
    };
    dispatchqueue::queue(socket, CHAT, token, note, 10).expect("queued")
}

/// Chat [`CHAT`]'s ask that the app deliver message `id`, which it has just queued.
fn deliver(id: &str) -> Ask {
    Ask::Task(Box::new(Asked {
        chat: CHAT,
        what: What::Deliver { id: id.to_owned() },
    }))
}

/// The ids of what chat [`CHAT`] still has queued, oldest first.
fn still_queued(socket: &Path) -> Vec<String> {
    dispatchqueue::waiting(socket, CHAT)
        .into_iter()
        .filter_map(|found| match found {
            dispatchqueue::Found::Message(message) => Some(message.id),
            _ => None,
        })
        .collect()
}

/// What the window heard carried, in order.
fn carried(window: &Window) -> Vec<String> {
    window
        .heard()
        .into_iter()
        .filter(|heard| heard.starts_with("carried "))
        .collect()
}

/// A runner's host whose window is there while `here` says so, carrying the queues beside
/// `socket`.
fn a_runner_host(
    window: &Arc<Window>,
    holding: &Arc<Holding>,
    here: &Arc<AtomicBool>,
    socket: &Path,
) -> Hearing {
    let (hearing, relaying, _tokens) = now_and_then_with(window, holding, here);
    relaying.carrying_queues_beside(socket);
    hearing
}

/// Waits until the window has heard `count` messages carried.
fn until_carried(window: &Window, count: usize) {
    let start = std::time::Instant::now();
    while carried(window).len() < count {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "{:?}",
            window.heard()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_message_a_runner_chat_queued_is_carried_to_the_window_and_answered_as_it_answers() {
    let (window, holding) = a_window();
    let here = Arc::new(AtomicBool::new(true));
    let (_root, socket, token) = a_runner_project();
    let host = a_runner_host(&window, &holding, &here, &socket);
    let id = queues(&socket, &token, "halfway");

    let answer = (host.answer)(42, deliver(&id));

    assert_eq!(carried(&window), [format!("carried {id} halfway")]);
    assert_eq!(
        answer,
        Answer::Ticket {
            ticket: "t42".into()
        }
    );
    assert!(
        still_queued(&socket).is_empty(),
        "delivered, and so not kept"
    );
}

#[test]
fn messages_queued_while_the_link_is_down_are_carried_once_in_order_when_it_comes_back() {
    let (window, holding) = a_window();
    let here = Arc::new(AtomicBool::new(false));
    let (_root, socket, token) = a_runner_project();
    let host = a_runner_host(&window, &holding, &here, &socket);
    let first = queues(&socket, &token, "one");
    let waits = (host.answer)(42, deliver(&first));
    let second = queues(&socket, &token, "two");
    let _ = (host.answer)(43, deliver(&second));

    assert_eq!(
        waits,
        Answer::Task(Box::new(Answered::Queued {
            id: first.clone(),
            delivered: false,
        })),
        "the chat is told it waits, not that nobody is there"
    );
    assert!(carried(&window).is_empty(), "nobody heard it");
    assert_eq!(still_queued(&socket), [first.clone(), second.clone()]);

    here.store(true, Ordering::SeqCst);
    (host.each)(report(CHAT)).expect("the window is back and takes the report");
    until_carried(&window, 2);
    // The window hears this host again and again: nothing goes twice.
    (host.each)(report(CHAT)).expect("taken");
    std::thread::sleep(Duration::from_millis(200));

    assert_eq!(
        carried(&window),
        [
            format!("carried {first} one"),
            format!("carried {second} two")
        ]
    );
    assert!(still_queued(&socket).is_empty());
}

#[test]
fn a_message_the_window_holds_back_stays_queued_and_so_does_every_one_after_it() {
    let (window, holding) = a_window();
    let here = Arc::new(AtomicBool::new(true));
    let (_root, socket, token) = a_runner_project();
    let host = a_runner_host(&window, &holding, &here, &socket);
    let held = queues(&socket, &token, HELD_BACK);
    let after = queues(&socket, &token, "after it");

    let answer = (host.answer)(42, deliver(&after));

    assert_eq!(carried(&window), [format!("carried {held} {HELD_BACK}")]);
    assert_eq!(
        answer,
        Answer::Task(Box::new(Answered::Queued {
            id: after.clone(),
            delivered: false,
        })),
        "it waits behind the one held back"
    );
    assert_eq!(still_queued(&socket), [held, after]);
}

#[test]
fn a_message_queued_before_the_host_carried_is_never_carried() {
    // Queued under a host that handed the `Deliver` on, and refused there saying nothing was
    // sent (#4560): this host, started since, must not send it after all.
    let (window, holding) = a_window();
    let here = Arc::new(AtomicBool::new(true));
    let (_root, socket, token) = a_runner_project();
    let refused = queues(&socket, &token, "refused before");
    let host = a_runner_host(&window, &holding, &here, &socket);

    (host.each)(report(CHAT)).expect("the window hears this host");
    let fresh = queues(&socket, &token, "fresh");
    let _ = (host.answer)(42, deliver(&fresh));
    std::thread::sleep(Duration::from_millis(200));

    assert_eq!(carried(&window), [format!("carried {fresh} fresh")]);
    assert!(
        !carried(&window)
            .iter()
            .any(|heard| heard.contains(&refused)),
        "{:?}",
        window.heard()
    );
    assert!(still_queued(&socket).is_empty(), "and it is not kept");
}

#[test]
fn a_host_that_does_not_carry_hands_a_deliver_on_as_it_came() {
    // The window's own host: the window reads the queue beside it.
    let (window, holding) = a_window();
    let here = Arc::new(AtomicBool::new(true));
    let (_root, socket, token) = a_runner_project();
    let (host, _relaying, _tokens) = now_and_then_with(&window, &holding, &here);
    let id = queues(&socket, &token, "halfway");

    let answer = (host.answer)(42, deliver(&id));

    assert!(carried(&window).is_empty(), "{:?}", window.heard());
    assert_eq!(
        answer,
        Answer::Ticket {
            ticket: "t42".into()
        }
    );
    assert_eq!(still_queued(&socket), [id], "the window delivers it");
}

#[test]
fn a_message_queued_while_the_link_was_down_is_still_carried_after_its_host_restarts() {
    // `purlis serve` restarts with the link down (#4895): its chat was told the message waits,
    // so the host started since carries it, and retires nothing.
    let (window, holding) = a_window();
    let here = Arc::new(AtomicBool::new(false));
    let (_root, socket, token) = a_runner_project();
    let before = a_runner_host(&window, &holding, &here, &socket);
    let waits = queues(&socket, &token, "waits");
    let _ = (before.answer)(42, deliver(&waits));
    drop(before);

    let after = a_runner_host(&window, &holding, &here, &socket);
    here.store(true, Ordering::SeqCst);
    (after.each)(report(CHAT)).expect("the window is back and takes the report");
    until_carried(&window, 1);
    // Removed once its answer is back, a moment after the window heard it.
    let start = std::time::Instant::now();
    while !still_queued(&socket).is_empty() && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(carried(&window), [format!("carried {waits} waits")]);
    assert!(still_queued(&socket).is_empty(), "carried, and so not kept");
}
