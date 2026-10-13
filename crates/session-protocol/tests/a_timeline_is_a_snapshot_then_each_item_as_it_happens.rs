//! A timeline is one stream: a header, the items the host still holds, `caught_up`, then each
//! item as it happens (iOS design §5.1). Yamux's credit paces it; past the host's queue bound
//! the client is dropped and attaches again, as a view is.

use std::time::Duration;

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link::{self, Link, LinkError};
use purlis_session_protocol::session;
use purlis_session_protocol::stream;
use purlis_session_protocol::timeline::{
    Attacher, Closed, Epoch, Item, Limits, MOST_ITEM_BYTES, Role, TIMELINE, TimelineId, Timelines,
    What,
};
use tokio::io::{AsyncWriteExt, duplex};

mod common;
use common::HELD;

async fn linked() -> (Link, Link) {
    let (a, b) = duplex(256 * 1024);
    let (client, host) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    (client.unwrap(), host.unwrap())
}

fn turn(level: u8, id: &str, began: bool) -> Item {
    Item {
        level,
        what: What::Turn {
            turn: id.into(),
            began,
            stop_reason: (!began).then(|| "end_turn".to_owned()),
        },
    }
}

#[tokio::test]
async fn the_snapshot_comes_first_then_caught_up_then_the_live_items_in_order() {
    let (mut client, host) = linked().await;
    let held = vec![turn(2, "t1", true), turn(2, "t1", false)];
    let feed = Attacher::new(host.opener(), Limits::default())
        .attach(TimelineId(9), held.clone())
        .await
        .unwrap();
    feed.push(&turn(2, "t2", true)).unwrap();

    let mut reader = Timelines::default()
        .accept_opened(stream::take(client.accept().await.unwrap()).await.unwrap())
        .await
        .unwrap();
    assert_eq!(
        (reader.timeline(), reader.epoch()),
        (TimelineId(9), Epoch(1))
    );
    let mut got = Vec::new();
    for _ in 0..4 {
        got.push(reader.next().await.unwrap().unwrap());
    }
    assert_eq!(
        got,
        [
            held[0].clone(),
            held[1].clone(),
            Item {
                level: 2,
                what: What::CaughtUp
            },
            turn(2, "t2", true)
        ]
    );
}

#[tokio::test]
async fn an_empty_snapshot_is_caught_up_at_once() {
    let (mut client, host) = linked().await;
    let _feed = Attacher::new(host.opener(), Limits::default())
        .attach(TimelineId(1), vec![])
        .await
        .unwrap();
    let mut reader = Timelines::default()
        .accept(client.accept().await.unwrap())
        .await
        .unwrap();
    // With nothing held, the level is not known from the items; the host says 0.
    assert_eq!(
        reader.next().await.unwrap().unwrap(),
        Item {
            level: 0,
            what: What::CaughtUp
        }
    );
}

#[tokio::test]
async fn a_dropped_feed_ends_the_timeline_after_what_was_queued() {
    let (mut client, host) = linked().await;
    let feed = Attacher::new(host.opener(), Limits::default())
        .attach(TimelineId(1), vec![])
        .await
        .unwrap();
    feed.push(&turn(3, "t", true)).unwrap();
    drop(feed);
    let mut reader = Timelines::default()
        .accept(client.accept().await.unwrap())
        .await
        .unwrap();
    assert_eq!(reader.next().await.unwrap().unwrap().what, What::CaughtUp);
    assert_eq!(reader.next().await.unwrap().unwrap(), turn(3, "t", true));
    assert!(reader.next().await.is_none(), "the stream ended");
}

#[tokio::test]
async fn a_timeline_whose_client_stops_reading_is_dropped_at_its_bound_and_learns_it() {
    let (mut client, host) = linked().await;
    let limits = Limits {
        most_queued_bytes: 4096,
    };
    let feed = Attacher::new(host.opener(), limits)
        .attach(TimelineId(1), vec![])
        .await
        .unwrap();
    // The client takes the stream and reads nothing of it, so the stream's credit runs out
    // before the host's queue does: the writer drains the queue into the stream until yamux
    // stops it, and only then does the queue fill.
    let mut reader = Timelines::default()
        .accept(client.accept().await.unwrap())
        .await
        .unwrap();
    let chunk = Item {
        level: 3,
        what: What::Message {
            turn: "t".into(),
            role: Role::Agent,
            text: "x".repeat(1000),
            done: false,
        },
    };
    let mut pushed = 0;
    let dropped = loop {
        match feed.push(&chunk) {
            Ok(()) => pushed += 1,
            Err(why) => break why,
        }
        assert!(
            feed.queued_bytes() <= limits.most_queued_bytes,
            "the queue passed its bound"
        );
        assert!(pushed < 100_000, "the queue never filled");
        tokio::task::yield_now().await;
    };
    assert_eq!(dropped, Closed::FellBehind);
    assert_eq!(feed.closed(), Some(Closed::FellBehind));
    assert_eq!(feed.queued_bytes(), 0, "the queue was let go");
    // Far more than the queue holds went into the stream's credit first.
    assert!(
        pushed > 4,
        "{pushed} pushes: the writer never drained into the stream's credit"
    );
    // The client learns by the stream ending, exactly as for a detach (V76): what the credit
    // held still arrives, then the end, and the client attaches again.
    let read = tokio::time::timeout(Duration::from_secs(5), async {
        let mut read = 0;
        loop {
            match reader.next().await {
                Some(Ok(_)) => read += 1,
                Some(Err(_)) | None => break read,
            }
        }
    })
    .await
    .expect("the stream ends");
    assert!(
        read > 0 && read <= pushed + 1,
        "{read} read of {pushed} pushed"
    );
}

#[tokio::test]
async fn an_item_too_big_for_the_wire_is_refused_and_the_timeline_lives_on() {
    let (mut client, host) = linked().await;
    let feed = Attacher::new(host.opener(), Limits::default())
        .attach(TimelineId(1), vec![])
        .await
        .unwrap();
    let huge = Item {
        level: 3,
        what: What::Message {
            turn: "t".into(),
            role: Role::Agent,
            text: "x".repeat(MOST_ITEM_BYTES),
            done: false,
        },
    };
    assert_eq!(feed.push(&huge), Err(Closed::TooBig));
    assert_eq!(
        feed.closed(),
        None,
        "one bad item does not close the timeline"
    );
    feed.push(&turn(3, "t", true)).unwrap();
    let mut reader = Timelines::default()
        .accept(client.accept().await.unwrap())
        .await
        .unwrap();
    assert_eq!(reader.next().await.unwrap().unwrap().what, What::CaughtUp);
    assert_eq!(reader.next().await.unwrap().unwrap(), turn(3, "t", true));
}

#[tokio::test]
async fn a_snapshot_with_an_item_too_big_for_the_wire_is_refused_before_the_stream_opens() {
    let (_client, host) = linked().await;
    let huge = Item {
        level: 3,
        what: What::Message {
            turn: "t".into(),
            role: Role::Agent,
            text: "x".repeat(MOST_ITEM_BYTES),
            done: false,
        },
    };
    let refused = Attacher::new(host.opener(), Limits::default())
        .attach(TimelineId(1), vec![huge])
        .await;
    assert!(refused.is_err(), "a snapshot nobody can read was opened");
}

#[tokio::test]
async fn a_timeline_whose_link_died_says_so_on_the_next_push() {
    let (client, host) = linked().await;
    let feed = Attacher::new(host.opener(), Limits::default())
        .attach(TimelineId(1), vec![])
        .await
        .unwrap();
    drop(client);
    let gone = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match feed.push(&turn(3, "t", true)) {
                Ok(()) => tokio::task::yield_now().await,
                Err(why) => break why,
            }
        }
    })
    .await
    .expect("the feed learns the link died");
    assert_eq!(gone, Closed::Gone);
    assert_eq!(feed.closed(), Some(Closed::Gone));
}

#[tokio::test]
async fn a_role_from_a_later_minor_reads_as_unknown_and_a_bad_known_item_is_an_error() {
    let (mut client, host) = linked().await;
    let opener = host.opener();
    tokio::spawn(async move {
        let mut s = opener.open().await.unwrap();
        let mut bytes = vec![TIMELINE];
        bytes.extend_from_slice(&5u32.to_be_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        for item in [
            r#"{"level":3,"item":"message","turn":"t","role":"system","text":"hi","done":true}"#,
            r#"{"level":2,"item":"turn","turn":5}"#,
        ] {
            bytes.extend_from_slice(&(item.len() as u32).to_be_bytes());
            bytes.extend_from_slice(item.as_bytes());
        }
        s.write_all(&bytes).await.unwrap();
        s.flush().await.unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;
        drop(s);
    });
    let mut reader = Timelines::default()
        .accept(client.accept().await.unwrap())
        .await
        .unwrap();
    assert_eq!(
        reader.next().await.unwrap().unwrap().what,
        What::Message {
            turn: "t".into(),
            role: Role::Unknown,
            text: "hi".into(),
            done: true
        }
    );
    match reader.next().await.unwrap() {
        Err(LinkError::Io(e)) => assert_eq!(e.kind(), std::io::ErrorKind::InvalidData),
        Err(other) => panic!("refused for another reason: {other}"),
        Ok(item) => panic!("a turn with no `began` was read: {item:?}"),
    }
}

#[tokio::test]
async fn an_item_from_a_later_minor_is_passed_over() {
    let (mut client, host) = linked().await;
    let opener = host.opener();
    tokio::spawn(async move {
        let mut s = opener.open().await.unwrap();
        let mut bytes = vec![TIMELINE];
        bytes.extend_from_slice(&5u32.to_be_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        for item in [
            r#"{"level":2,"item":"mood","mood":"calm"}"#,
            r#"{"level":2,"item":"caught_up"}"#,
        ] {
            bytes.extend_from_slice(&(item.len() as u32).to_be_bytes());
            bytes.extend_from_slice(item.as_bytes());
        }
        s.write_all(&bytes).await.unwrap();
        s.flush().await.unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;
        drop(s);
    });
    let mut reader = Timelines::default()
        .accept(client.accept().await.unwrap())
        .await
        .unwrap();
    assert_eq!(reader.timeline(), TimelineId(5));
    assert_eq!(reader.next().await.unwrap().unwrap().what, What::CaughtUp);
}

#[tokio::test]
async fn a_timeline_attached_again_comes_back_at_the_next_epoch() {
    let (mut client, host) = linked().await;
    let attacher = Attacher::new(host.opener(), Limits::default());
    let first = attacher.attach(TimelineId(3), vec![]).await.unwrap();
    drop(first);
    let _second = attacher.attach(TimelineId(3), vec![]).await.unwrap();
    let mut timelines = Timelines::default();
    let a = timelines
        .accept(client.accept().await.unwrap())
        .await
        .unwrap();
    let b = timelines
        .accept(client.accept().await.unwrap())
        .await
        .unwrap();
    assert_eq!((a.epoch(), b.epoch()), (Epoch(1), Epoch(2)));
}
