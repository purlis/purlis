// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The dialing host keeps its link up (#2042, ADR 0078 §2): a link that drops comes back on
//! its own, one that is half-open (open, carrying nothing) is noticed within the ping's deadline
//! at both ends, and a link renews itself ahead of a limit on a stream's life, the new link up
//! before the old one ends, and the old one answering until its reader has moved to the new one
//! (#2215). Each runs on a cable the test cuts or freezes, in the runtime's
//! virtual time.

use std::time::Duration;

use purlis_session_protocol::hostlink::{self, Handshake, Linked, Links, Liveness};
use purlis_session_protocol::link::Link;
use purlis_session_protocol::session::Client;
use tokio::time::Instant;

use crate::common;
use common::hosts::{Board, CHAT, Cable, ToB, a_as_pinned, b_as_pinned, host_a, serve_b};

fn keep(to_b: &ToB, liveness: Liveness) -> Links {
    hostlink::keep_linked(
        to_b.clone(),
        Handshake::THIS_BUILD,
        host_a(),
        b_as_pinned(),
        liveness,
    )
}

async fn up(links: &mut Links) -> Link {
    match links.next().await.expect("the link is kept") {
        Linked::Up(link) => *link,
        Linked::Down(why) => panic!("the link did not come up: {why}"),
    }
}

async fn down(links: &mut Links) -> String {
    match links.next().await.expect("the link is kept") {
        Linked::Down(down) => down.why,
        Linked::Up(_) => panic!("a link came up while the last one was meant to be down"),
    }
}

/// The session protocol works on `link`: host B lists its chat.
async fn lists(link: Link) -> Client {
    let (client, _views, _events) = Client::new(link);
    let chats = client.list().await.expect("host B answers");
    assert_eq!(chats[0].chat, CHAT);
    client
}

#[tokio::test(start_paused = true)]
async fn a_dropped_link_links_again_on_its_own() {
    let (b, _told) = serve_b(Board::default(), a_as_pinned(), Liveness::default());
    let to_b = ToB::new(b);
    let mut links = keep(&to_b, Liveness::default());
    let first = lists(up(&mut links).await).await;

    to_b.set(Cable::Cut);

    let why = down(&mut links).await;
    assert!(why.contains("closed"), "{why}");
    lists(up(&mut links).await).await;
    assert_eq!(to_b.dials(), 2);
    assert!(first.list().await.is_err(), "the dropped link is gone");
}

#[tokio::test(start_paused = true)]
async fn a_half_open_link_is_noticed_at_both_ends_within_the_pings_deadline() {
    let board = Board::default();
    let liveness = Liveness::default();
    let (b, _told) = serve_b(board.clone(), a_as_pinned(), liveness);
    let to_b = ToB::new(b);
    let mut links = keep(&to_b, liveness);
    let _held = lists(up(&mut links).await).await;
    // A while of pings back and forth, so nothing is left of the link's first exchange.
    tokio::time::sleep(Duration::from_secs(95)).await;
    let frozen = Instant::now();

    to_b.set(Cable::Frozen);

    let why = down(&mut links).await;
    assert!(
        frozen.elapsed() <= liveness.deadline,
        "noticed after {:?}",
        frozen.elapsed()
    );
    assert!(why.contains("sent nothing"), "{why}");
    while board.links_ended() == 0 {
        assert!(
            frozen.elapsed() <= liveness.deadline,
            "host B noticed only after {:?}",
            frozen.elapsed()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    lists(up(&mut links).await).await;
}

#[tokio::test(start_paused = true)]
async fn a_link_that_stays_quiet_but_answers_its_pings_lives_on() {
    let board = Board::default();
    let liveness = Liveness::default();
    let (b, _told) = serve_b(board.clone(), a_as_pinned(), liveness);
    let to_b = ToB::new(b);
    let mut links = keep(&to_b, liveness);
    let client = lists(up(&mut links).await).await;

    tokio::time::sleep(Duration::from_secs(50 * 60)).await;

    assert!(client.list().await.is_ok(), "the link is still up");
    assert_eq!(board.links_ended(), 0);
    assert_eq!(to_b.dials(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_link_renews_itself_ahead_of_its_limit_with_the_new_one_up_first() {
    let board = Board::default();
    let liveness = Liveness {
        renew_after: Some(Duration::from_secs(60 * 60)),
        ..Liveness::default()
    };
    let (b, _told) = serve_b(board.clone(), a_as_pinned(), liveness);
    let to_b = ToB::new(b);
    let mut links = keep(&to_b, liveness);
    let first = lists(up(&mut links).await).await;
    let linked = Instant::now();

    let second = lists(up(&mut links).await).await;

    let renewed = linked.elapsed();
    assert!(
        renewed >= Duration::from_secs(60 * 60) && renewed < Duration::from_secs(61 * 60),
        "renewed after {renewed:?}"
    );
    assert_eq!(to_b.dials(), 2);
    assert!(second.list().await.is_ok());
    while first.list().await.is_ok() {
        assert!(
            linked.elapsed() < Duration::from_secs(62 * 60),
            "the old link never ended"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(start_paused = true)]
async fn a_renewed_link_answers_what_was_in_flight_until_its_reader_has_moved_to_the_new_one() {
    let board = Board::default();
    let liveness = Liveness {
        renew_after: Some(Duration::from_secs(60 * 60)),
        ..Liveness::default()
    };
    let (b, _told) = serve_b(board.clone(), a_as_pinned(), liveness);
    let to_b = ToB::new(b);
    let mut links = keep(&to_b, liveness);
    let first = lists(up(&mut links).await).await;
    links.moved();

    let second = lists(up(&mut links).await).await;
    let renewed = Instant::now();

    // Until the reader has moved, the link it was using still answers.
    tokio::time::sleep(liveness.deadline / 2).await;
    let still = first.list().await;
    assert!(
        still.is_ok(),
        "the renewed link ended before the reader moved: {still:?}"
    );
    links.moved();
    while first.list().await.is_ok() {
        assert!(
            renewed.elapsed() < liveness.deadline,
            "the renewed link outlived the move"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(second.list().await.is_ok());
}
