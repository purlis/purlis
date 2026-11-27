// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A kept link's steps join the Start it was made for, and no later one** (#7864): its first
//! dial names the Start, a redial after the link dropped names none, and each dial holds its
//! handshake, so a sum of the rows with no parent counts each try once.
//!
//! A binary of its own: it reads what `purlis::step` writes through a subscriber of this thread,
//! and tracing caches whether a callsite is listened to across every thread of a process, so a
//! test beside it that meets the same steps with nobody listening can leave its lines unwritten.

use std::time::Duration;

use purlis_session_protocol::hostlink::{self, Handshake, Linked, Links, Liveness};
use purlis_session_protocol::link::Link;
use purlis_session_protocol::session::Client;

mod common;
use common::hosts::{Board, CHAT, Cable, ToB, a_as_pinned, b_as_pinned, host_a, serve_b};

fn keep(to_b: &ToB) -> Links {
    hostlink::keep_linked(
        to_b.clone(),
        Handshake::THIS_BUILD,
        host_a(),
        b_as_pinned(),
        Liveness::default(),
    )
}

async fn up(links: &mut Links) -> Link {
    match links.next().await.expect("the link is kept") {
        Linked::Up(link) => *link,
        Linked::Down(why) => panic!("the link did not come up: {why}"),
    }
}

async fn down(links: &mut Links) {
    match tokio::time::timeout(Duration::from_secs(60), links.next()).await {
        Ok(Some(Linked::Down(_))) => {}
        _ => panic!("the cut link was not told down"),
    }
}

/// The session protocol works on `link`: host B lists its chat.
async fn lists(link: Link) -> Client {
    let (client, _views, _events) = Client::new(link);
    let chats = client.list().await.expect("host B answers");
    assert_eq!(chats[0].chat, CHAT);
    client
}

/// What `purlis::step` wrote, kept while it is held.
#[derive(Clone, Default)]
struct StepLog(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for StepLog {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for StepLog {
    type Writer = StepLog;
    fn make_writer(&'a self) -> StepLog {
        self.clone()
    }
}

#[tokio::test(start_paused = true)]
async fn a_kept_link_names_its_start_until_it_first_comes_up_and_each_dial_holds_its_handshake() {
    use purlis_session_protocol::steps::{StartId, starting};
    // #7864: a redial an hour later is for no Start, and a sum of the rows with no parent counts
    // each try once.
    let log = StepLog::default();
    let _logging = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_writer(log.clone())
            .with_ansi(false)
            .with_max_level(tracing::Level::INFO)
            .finish(),
    );
    let (b, _told) = serve_b(Board::default(), a_as_pinned(), Liveness::default());
    let to_b = ToB::new(b);
    let mut links = {
        let _start = starting(StartId::parse("01JD3Q4M0R8W2ZK7A6F1B9CXYE").unwrap());
        keep(&to_b)
    };
    let _first = lists(up(&mut links).await).await;
    to_b.set(Cable::Cut);
    down(&mut links).await;
    let _again = lists(up(&mut links).await).await;

    let said = String::from_utf8_lossy(&log.0.lock().unwrap()).into_owned();
    let dials: Vec<&str> = said
        .lines()
        .filter(|line| line.contains("step link_dial "))
        .collect();
    assert_eq!(dials.len(), 2, "{said}");
    assert!(
        dials[0].contains("start: 01JD3Q4M0R8W2ZK7A6F1B9CXYE"),
        "{said}"
    );
    assert!(!dials[1].contains("start"), "{said}");
    let handshakes: Vec<&str> = said
        .lines()
        .filter(|line| line.contains("step link_handshake "))
        .collect();
    assert_eq!(handshakes.len(), 2, "{said}");
    assert!(
        handshakes
            .iter()
            .all(|line| line.contains("parent: link_dial")),
        "{said}"
    );
}
