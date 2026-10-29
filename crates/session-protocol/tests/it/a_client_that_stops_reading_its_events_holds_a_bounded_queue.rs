// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A client that subscribes and stops reading holds a bounded queue of events (#1081). Past
//! the bound it lets go of the subscription, and when it reads again it is told where it fell
//! behind: the cursor to subscribe from, which brings back every event it did not keep. Its
//! calls are answered all the while.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link;
use purlis_session_protocol::session::{
    self, Answer, Client, Command, Event, Events, Host, Peer, Pushed, Refusal, Since, Told,
};
use tokio::io::duplex;
use tokio::sync::mpsc;

use crate::common;
use common::HELD;

const DEVICE: &str = "01J9DEVCE00000000000000000";
const RUNNER: &str = "01J9RVNNR00000000000000000";

fn event_of(device: &str, seq: u64) -> Event {
    Event {
        v: 1,
        device_id: device.into(),
        seq,
        ulid: format!("01J9EVENT{seq:017}"),
        chat: None,
        run: None,
        parent_run: None,
        kind: "run.started".into(),
        body: serde_json::json!({}),
    }
}

fn event(seq: u64) -> Told {
    Told::Pushed(Pushed::Event(event_of(DEVICE, seq)))
}

/// A host whose event log is a channel the test writes, and which keeps every cursor it was
/// subscribed from.
#[derive(Default, Clone)]
struct Logged {
    cursors: Arc<Mutex<Vec<Since>>>,
    pushes: Arc<Mutex<Option<mpsc::Sender<Pushed>>>>,
}

impl Logged {
    fn pushes(&self) -> mpsc::Sender<Pushed> {
        self.pushes.lock().unwrap().clone().unwrap()
    }

    async fn push(&self, seqs: impl IntoIterator<Item = u64>) {
        let pushes = self.pushes();
        for seq in seqs {
            pushes
                .send(Pushed::Event(event_of(DEVICE, seq)))
                .await
                .unwrap();
        }
    }
}

impl Host for Logged {
    async fn call(&self, _command: Command, _peer: &Peer) -> Result<Answer, Refusal> {
        Ok(Answer::Chats(Vec::new()))
    }

    async fn subscribe(
        &self,
        since: Since,
        _peer: &Peer,
    ) -> Result<mpsc::Receiver<Pushed>, Refusal> {
        self.cursors.lock().unwrap().push(since);
        let (pushes, events) = mpsc::channel(256);
        *self.pushes.lock().unwrap() = Some(pushes);
        Ok(events)
    }
}

/// A client whose queue holds `most` events.
async fn linked(host: Logged, most: usize) -> (Client, Events) {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::Terminal,
            HELD.of(Scope::Terminal)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), host, None));
    let (client, _views, events) = Client::holding(client.unwrap(), most);
    (client, events)
}

async fn soon<T>(what: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), what)
        .await
        .expect("it did not happen in time")
}

#[tokio::test]
async fn past_its_bound_the_client_lets_go_and_is_told_where_to_read_on_from() {
    let host = Logged::default();
    let (client, mut events) = linked(host.clone(), 4).await;
    client
        .subscribe(Since::from([(DEVICE.to_owned(), 10)]))
        .await
        .unwrap();

    // Nobody reads. The host writes far more than the queue holds...
    let pushes = host.pushes();
    host.push(11..=110).await;
    // ...and the client lets go of the subscription rather than grow: the host is told to stop.
    soon(pushes.closed()).await;
    // Its calls are answered meanwhile.
    assert_eq!(soon(client.list()).await.unwrap(), Vec::new());

    // Reading again: what the queue kept, in order, then where it fell behind.
    for seq in 11..=14 {
        assert_eq!(soon(events.recv()).await.unwrap(), event(seq));
    }
    let Told::FellBehind { since } = soon(events.recv()).await.unwrap() else {
        panic!("the client was not told it fell behind");
    };
    assert_eq!(since, Since::from([(DEVICE.to_owned(), 14)]));
    assert!(
        events.try_recv().is_err(),
        "nothing was kept past the bound"
    );

    // Subscribing from there brings back every event it did not keep: nothing is lost.
    client.subscribe(since.clone()).await.unwrap();
    assert_eq!(host.cursors.lock().unwrap().last(), Some(&since));
    host.push(15..=17).await;
    for seq in 15..=17 {
        assert_eq!(soon(events.recv()).await.unwrap(), event(seq));
    }
}

#[tokio::test]
async fn a_device_it_took_nothing_from_reads_on_from_the_cursor_it_subscribed_with() {
    let host = Logged::default();
    let (client, mut events) = linked(host.clone(), 2).await;
    let subscribed = Since::from([(DEVICE.to_owned(), 0), (RUNNER.to_owned(), 40)]);
    client.subscribe(subscribed).await.unwrap();

    let pushes = host.pushes();
    // Word that some events are no longer kept moves the cursor as an event does: the gap was
    // told once, and reading on from before it would only tell it again.
    pushes
        .send(Pushed::Missed {
            device: DEVICE.into(),
            after: 0,
            resumes_at: 7,
        })
        .await
        .unwrap();
    host.push(7..=30).await;
    soon(pushes.closed()).await;

    assert!(matches!(
        soon(events.recv()).await.unwrap(),
        Told::Pushed(Pushed::Missed { .. })
    ));
    assert_eq!(soon(events.recv()).await.unwrap(), event(7));
    assert_eq!(
        soon(events.recv()).await.unwrap(),
        Told::FellBehind {
            since: Since::from([(DEVICE.to_owned(), 7), (RUNNER.to_owned(), 40)]),
        }
    );
}

#[tokio::test]
async fn a_client_that_falls_behind_again_before_it_reads_is_told_once_where_from() {
    let host = Logged::default();
    let (client, mut events) = linked(host.clone(), 2).await;
    client.subscribe(Since::new()).await.unwrap();
    let pushes = host.pushes();
    host.push(1..=20).await;
    soon(pushes.closed()).await;

    // It subscribes again without reading a thing: the queue is still full.
    client
        .subscribe(Since::from([(DEVICE.to_owned(), 2)]))
        .await
        .unwrap();
    let pushes = host.pushes();
    host.push(3..=20).await;
    soon(pushes.closed()).await;

    assert_eq!(soon(events.recv()).await.unwrap(), event(1));
    assert_eq!(soon(events.recv()).await.unwrap(), event(2));
    assert_eq!(
        soon(events.recv()).await.unwrap(),
        Told::FellBehind {
            since: Since::from([(DEVICE.to_owned(), 2)]),
        }
    );
    assert!(events.try_recv().is_err());
}

#[tokio::test]
async fn a_client_that_keeps_up_is_never_told_it_fell_behind() {
    let host = Logged::default();
    let (client, mut events) = linked(host.clone(), 2).await;
    client.subscribe(Since::new()).await.unwrap();
    let pushes = host.pushes();
    for seq in 1..=50 {
        pushes
            .send(Pushed::Event(event_of(DEVICE, seq)))
            .await
            .unwrap();
        assert_eq!(soon(events.recv()).await.unwrap(), event(seq));
    }
    assert!(!pushes.is_closed());
}
