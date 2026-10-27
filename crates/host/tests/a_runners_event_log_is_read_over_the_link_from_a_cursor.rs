// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A runner's own event log is read by its desktop's window over the link, from a cursor of the
//! window's own (1.41, #3393, ADR 0078 §7): `log_end` says where host B's log ends and by which
//! device, and a subscription whose `since` names that device is pushed each line after the
//! cursor, then each as it is written, through host A's relay. Nothing is copied into host A's
//! log, a subscription that names no such device is pushed none of it, and a link that may not
//! read it is pushed none. Host B's log is a stand-in and its links are in memory.

#![cfg(unix)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_host::standin::StandIns;
use purlis_host::{Compose, Dial, Hosting, ReadsLog, Relay, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::hostlink::{self, Handshake, Identity, LinkKey, Liveness, Pinned};
use purlis_session_protocol::link::{self, Link};
use purlis_session_protocol::session::{self, Client, Event, Events, Pushed, Since, Told};
use purlis_terminal::session::Spec;
use tokio::io::duplex;
use tokio::time::timeout;

const RUNNER: &str = "01JB0RUNNER0000000000000000";
const PATIENCE: Duration = Duration::from_secs(10);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// Host B's event log: its lines, which a test writes more of as it goes.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Event>>>);

impl Log {
    fn write(&self, kind: &str) {
        let mut lines = self.0.lock().unwrap();
        let seq = lines.len() as u64 + 1;
        lines.push(Event {
            v: 1,
            device_id: RUNNER.into(),
            seq,
            ulid: format!("01JB{seq:022}"),
            chat: Some("01JBCHAT0000000000000000000".into()),
            run: None,
            parent_run: None,
            kind: kind.into(),
            body: serde_json::json!({}),
        });
    }
}

struct From {
    log: Log,
    after: u64,
}

impl ReadsLog for From {
    fn read(&mut self) -> Result<Vec<Pushed>, String> {
        let lines: Vec<Event> = self
            .log
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|line| line.seq > self.after)
            .cloned()
            .collect();
        if let Some(last) = lines.last() {
            self.after = last.seq;
        }
        Ok(lines.into_iter().map(Pushed::Event).collect())
    }
}

impl purlis_host::Record for Log {
    fn record(&self, _entry: serde_json::Value) -> Result<(), String> {
        Ok(())
    }

    fn ended(&self, _ended: &purlis_host::Ended<'_>) {}

    fn log_end(&self) -> Result<(String, u64), String> {
        Ok((RUNNER.into(), self.0.lock().unwrap().len() as u64))
    }

    fn read_log(&self, after: u64) -> Result<Box<dyn ReadsLog>, String> {
        Ok(Box::new(From {
            log: self.clone(),
            after,
        }))
    }
}

async fn as_scope(scope: Scope, host: impl session::Host) -> Link {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(a, session::speaks(), scope, HELD.of(scope)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), host, None));
    client.unwrap()
}

fn identity(device: &str) -> Identity {
    Identity {
        device: device.into(),
        key: LinkKey::generate().unwrap(),
    }
}

/// Host B, reached as the link between hosts reaches it: served as `remote-link`.
fn over_the_link(b: Hosting) -> Dial {
    Arc::new(move || {
        let b = b.clone();
        Box::pin(async move {
            let (a_end, b_end) = duplex(256 * 1024);
            let (host_a, host_b) = (identity("01JA0DESKT0P00000000000000"), identity(RUNNER));
            let pinned = |identity: &Identity| Pinned {
                device: identity.device.clone(),
                key: identity.key.public(),
            };
            let (peers, b_pinned) = (pinned(&host_a), pinned(&host_b));
            tokio::spawn(async move {
                let answered = hostlink::answer(
                    b_end,
                    Handshake::THIS_BUILD,
                    &host_b,
                    &peers,
                    Liveness::default(),
                )
                .await
                .unwrap();
                session::serve(answered.link, b, None).await
            });
            hostlink::connect(
                a_end,
                Handshake::THIS_BUILD,
                &host_a,
                &b_pinned,
                Liveness::default(),
            )
            .await
            .map_err(|why| why.to_string())
        })
    })
}

/// The window on host A, relayed to host B, whose log is `log`.
async fn a_window(log: &Log) -> (Client, Events) {
    let b = Hosting::running_with(Echo, StandIns::default()).recording_with(log.clone());
    let (client, _views, events) =
        Client::new(as_scope(Scope::LocalUi, Relay::new(over_the_link(b))).await);
    (client, events)
}

/// The lines pushed to `events`, by their `seq`, until `want` have come.
async fn lines(events: &mut Events, want: usize) -> Vec<u64> {
    let mut seen = Vec::new();
    while seen.len() < want {
        match timeout(PATIENCE, events.recv()).await {
            Ok(Some(Told::Pushed(Pushed::Event(event)))) => {
                assert_eq!(event.device_id, RUNNER);
                seen.push(event.seq);
            }
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => panic!("only {seen:?} came"),
        }
    }
    seen
}

#[tokio::test]
async fn the_window_reads_a_runners_log_from_where_it_ended_and_on_as_it_is_written() {
    let log = Log::default();
    log.write("run.started");
    log.write("run.ended");
    let (window, mut events) = a_window(&log).await;

    let (device, end) = window
        .log_end()
        .await
        .expect("host B says where its log ends");
    assert_eq!((device.as_str(), end), (RUNNER, 2));
    window
        .subscribe(Since::from([(device, end)]))
        .await
        .unwrap();
    log.write("hook.stop");
    log.write("run.ended");

    assert_eq!(lines(&mut events, 2).await, vec![3, 4]);
}

#[tokio::test]
async fn a_cursor_further_back_reads_on_from_there() {
    let log = Log::default();
    for _ in 0..3 {
        log.write("run.started");
    }
    let (window, mut events) = a_window(&log).await;

    window
        .subscribe(Since::from([(RUNNER.to_owned(), 1)]))
        .await
        .unwrap();

    assert_eq!(lines(&mut events, 2).await, vec![2, 3]);
}

#[tokio::test]
async fn a_subscription_that_names_no_cursor_for_the_runner_is_pushed_none_of_its_log() {
    let log = Log::default();
    let (window, mut events) = a_window(&log).await;
    window.subscribe(Since::new()).await.unwrap();
    log.write("run.started");

    let pushed = timeout(Duration::from_millis(2500), async {
        loop {
            if let Some(Told::Pushed(Pushed::Event(event))) = events.recv().await {
                return event;
            }
        }
    })
    .await;
    assert!(pushed.is_err(), "pushed {pushed:?}");
}

#[tokio::test]
async fn a_link_that_may_not_read_the_log_is_pushed_none_of_it_and_refused_its_end() {
    let log = Log::default();
    log.write("run.started");
    let b = Hosting::running_with(Echo, StandIns::default()).recording_with(log.clone());
    let (terminal, _views, mut events) = Client::new(as_scope(Scope::Terminal, b).await);

    let refused = terminal.log_end().await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|refusal| refusal.code.as_str()),
        Some(session::NOT_ALLOWED)
    );
    terminal
        .subscribe(Since::from([(RUNNER.to_owned(), 0)]))
        .await
        .unwrap();
    let pushed = timeout(Duration::from_millis(2500), async {
        loop {
            if let Some(Told::Pushed(Pushed::Event(event))) = events.recv().await {
                return event;
            }
        }
    })
    .await;
    assert!(pushed.is_err(), "pushed {pushed:?}");
}
