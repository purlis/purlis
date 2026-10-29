// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A link's calls are answered side by side (#1081): a command that waits on the host (a
//! `start` that waits on a harness) holds back no other command's answer and no pushed event
//! on the same link, and neither does a UI RPC command that waits (#1095). What stays in order
//! is what has one: the commands about one chat. And a link holds a bounded number of calls
//! at once, each of which is still answered.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::BoxFuture;
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link;
use purlis_session_protocol::session::{
    self, Answer, Client, Command, Event, Events, Host, Peer, Pushed, Refusal, Since, Start, Told,
};
use purlis_session_protocol::ui::{self, Handler};
use serde_json::{Value, json};
use tokio::io::duplex;
use tokio::sync::{Semaphore, mpsc};

use crate::common;
use common::HELD;

const ONE: &str = "01J9ZQ3V7K8M2N4P6R8T0V2X4Z";
const OTHER: &str = "01J9ZQ3V7K8M2N4P6R8T0V2X5A";
const BUILD: &str = "0.4.0+9055f7a";

/// A host whose `start`, and whose `write` of `wait`, wait until the test lets them go.
#[derive(Clone)]
struct Waiting {
    /// One permit lets one waiting command go.
    go: Arc<Semaphore>,
    /// How many commands are waiting now, and the most that ever were at once.
    waiting: Arc<AtomicUsize>,
    most_waiting: Arc<AtomicUsize>,
    /// How many commands that waited were carried out.
    done: Arc<AtomicUsize>,
    /// What was typed into each chat, in the order the host took it.
    typed: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    pushes: Arc<Mutex<Option<mpsc::Sender<Pushed>>>>,
}

impl Waiting {
    fn new() -> Waiting {
        Waiting {
            go: Arc::new(Semaphore::new(0)),
            waiting: Arc::default(),
            most_waiting: Arc::default(),
            done: Arc::default(),
            typed: Arc::default(),
            pushes: Arc::default(),
        }
    }

    async fn wait(&self) {
        let now = self.waiting.fetch_add(1, Ordering::SeqCst) + 1;
        self.most_waiting.fetch_max(now, Ordering::SeqCst);
        self.go.acquire().await.unwrap().forget();
        self.waiting.fetch_sub(1, Ordering::SeqCst);
        self.done.fetch_add(1, Ordering::SeqCst);
    }

    /// Returns once `count` commands are waiting on the host.
    async fn waits_on(&self, count: usize) {
        soon(async {
            while self.waiting.load(Ordering::SeqCst) < count {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await;
    }

    fn typed(&self, chat: &str) -> Vec<u8> {
        self.typed
            .lock()
            .unwrap()
            .get(chat)
            .cloned()
            .unwrap_or_default()
    }
}

impl Host for Waiting {
    async fn call(&self, command: Command, _peer: &Peer) -> Result<Answer, Refusal> {
        match command {
            Command::StartProgram(_) => {
                self.wait().await;
                Ok(Answer::ProgramStarted {
                    program: ONE.into(),
                    number: None,
                    process: None,
                    confined: None,
                    sandbox: None,
                })
            }
            Command::Write { chat, bytes } => {
                if bytes == b"wait" {
                    self.wait().await;
                }
                self.typed
                    .lock()
                    .unwrap()
                    .entry(chat)
                    .or_default()
                    .extend(bytes);
                Ok(Answer::Done)
            }
            Command::List => Ok(Answer::Chats(Vec::new())),
            _ => Ok(Answer::Done),
        }
    }

    async fn subscribe(
        &self,
        _since: Since,
        _peer: &Peer,
    ) -> Result<mpsc::Receiver<Pushed>, Refusal> {
        let (pushes, events) = mpsc::channel(16);
        *self.pushes.lock().unwrap() = Some(pushes);
        Ok(events)
    }
}

/// The app's commands: `slow` waits until the test lets it go, and any other answers at once.
#[derive(Clone)]
struct Commands {
    go: Arc<Semaphore>,
    waiting: Arc<AtomicUsize>,
}

impl Handler for Commands {
    fn call(&self, method: String, _args: Value) -> BoxFuture<'_, Result<Value, Value>> {
        Box::pin(async move {
            if method == "slow" {
                self.waiting.fetch_add(1, Ordering::SeqCst);
                self.go.acquire().await.unwrap().forget();
            }
            Ok(json!(method))
        })
    }
}

async fn linked(host: Waiting, ui: Option<ui::Server>) -> (Arc<Client>, Events) {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), host, ui));
    let (client, _views, events) = Client::new(client.unwrap());
    (Arc::new(client), events)
}

/// `what`, or a failure when it has not happened in time: an answer held back behind a
/// waiting command never comes, and the test must say so rather than hang.
async fn soon<T>(what: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), what)
        .await
        .expect("it was held back")
}

fn start() -> Start {
    Start {
        project: "01J9PRJCT00000000000000000".into(),
        workspace: None,
        harness: None,
    }
}

fn event(seq: u64) -> Event {
    Event {
        v: 1,
        device_id: "01J9DEVCE00000000000000000".into(),
        seq,
        ulid: format!("01J9EVENT{seq:017}"),
        chat: Some(ONE.into()),
        run: None,
        parent_run: None,
        kind: "run.started".into(),
        body: json!({}),
    }
}

#[tokio::test]
async fn a_start_that_waits_delays_neither_another_answer_nor_a_pushed_event() {
    let host = Waiting::new();
    let (client, mut events) = linked(host.clone(), None).await;

    let starting = tokio::spawn({
        let client = Arc::clone(&client);
        async move { client.start(start()).await }
    });
    host.waits_on(1).await;

    // Another command is answered while the start waits...
    assert_eq!(soon(client.list()).await.unwrap(), Vec::new());
    soon(client.write(OTHER, b"ls")).await.unwrap();
    assert_eq!(host.typed(OTHER), b"ls");
    // ...and so is a subscription, whose events are pushed meanwhile.
    soon(client.subscribe(Since::new())).await.unwrap();
    let pushes = host.pushes.lock().unwrap().clone().unwrap();
    pushes.send(Pushed::Event(event(1))).await.unwrap();
    assert_eq!(
        soon(events.recv()).await.unwrap(),
        Told::Pushed(Pushed::Event(event(1)))
    );
    assert!(!starting.is_finished(), "the start is still waiting");

    host.go.add_permits(1);
    assert_eq!(soon(starting).await.unwrap().unwrap(), ONE);
}

#[tokio::test]
async fn what_is_typed_into_one_chat_is_taken_in_the_order_it_was_sent() {
    // Keystrokes are sent one call after another without waiting for each answer: answered
    // side by side, a later one could overtake an earlier one that waits.
    let host = Waiting::new();
    let (client, _events) = linked(host.clone(), None).await;

    let first = tokio::spawn({
        let client = Arc::clone(&client);
        async move { client.write(ONE, b"wait").await }
    });
    host.waits_on(1).await;
    let second = tokio::spawn({
        let client = Arc::clone(&client);
        async move { client.write(ONE, b", then this").await }
    });

    // Another chat's keystroke does not wait for this chat's.
    soon(client.write(OTHER, b"ls")).await.unwrap();
    assert_eq!(host.typed(OTHER), b"ls");
    assert_eq!(
        host.typed(ONE),
        b"",
        "nothing overtook the write that waits"
    );
    assert!(!second.is_finished());

    host.go.add_permits(1);
    soon(first).await.unwrap().unwrap();
    soon(second).await.unwrap().unwrap();
    assert_eq!(host.typed(ONE), b"wait, then this");
}

#[tokio::test]
async fn a_ui_rpc_command_that_waits_delays_no_session_command() {
    let host = Waiting::new();
    let commands = Commands {
        go: Arc::new(Semaphore::new(0)),
        waiting: Arc::default(),
    };
    let server = ui::Server::of_any_methods_for_tests(BUILD, ["slow", "quick"], commands.clone());
    let (client, _events) = linked(host, Some(server)).await;
    let ui = client.ui(BUILD).await.unwrap();

    let slow = tokio::spawn({
        let ui = ui.clone();
        async move { ui.call("slow", json!({})).await }
    });
    soon(async {
        while commands.waiting.load(Ordering::SeqCst) < 1 {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await;

    assert_eq!(soon(client.list()).await.unwrap(), Vec::new());
    assert_eq!(
        soon(ui.call("quick", json!({}))).await.unwrap(),
        Ok(json!("quick"))
    );
    assert!(!slow.is_finished());

    commands.go.add_permits(1);
    assert_eq!(soon(slow).await.unwrap().unwrap(), Ok(json!("slow")));
}

#[tokio::test]
async fn a_link_has_a_bounded_number_of_calls_in_flight_and_answers_every_one() {
    let host = Waiting::new();
    let (client, mut events) = linked(host.clone(), None).await;
    soon(client.subscribe(Since::new())).await.unwrap();
    let pushes = host.pushes.lock().unwrap().clone().unwrap();

    let sent = session::MOST_CALLS_IN_FLIGHT + 8;
    let starting: Vec<_> = (0..sent)
        .map(|_| {
            let client = Arc::clone(&client);
            tokio::spawn(async move { client.start(start()).await })
        })
        .collect();
    host.waits_on(session::MOST_CALLS_IN_FLIGHT).await;
    // The rest are not taken up while those wait: they stay in the link, which holds them back.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        host.waiting.load(Ordering::SeqCst),
        session::MOST_CALLS_IN_FLIGHT
    );
    // An event is still pushed past a link that is full of waiting calls.
    pushes.send(Pushed::Event(event(1))).await.unwrap();
    assert_eq!(
        soon(events.recv()).await.unwrap(),
        Told::Pushed(Pushed::Event(event(1)))
    );

    host.go.add_permits(sent);
    for started in starting {
        assert_eq!(soon(started).await.unwrap().unwrap(), ONE);
    }
    assert_eq!(
        host.most_waiting.load(Ordering::SeqCst),
        session::MOST_CALLS_IN_FLIGHT
    );
}

#[tokio::test]
async fn a_command_the_host_took_is_carried_out_though_its_link_closes_first() {
    // Ending a link does not undo what it asked for: a start cut off halfway would leave a
    // chat nobody holds.
    let host = Waiting::new();
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    let serving = tokio::spawn(session::serve(served.unwrap(), host.clone(), None));
    let (client, _views, _events) = Client::new(client.unwrap());
    let client = Arc::new(client);

    let starting = tokio::spawn({
        let client = Arc::clone(&client);
        async move { client.start(start()).await }
    });
    host.waits_on(1).await;
    starting.abort();
    let _ = starting.await;
    drop(client);

    host.go.add_permits(1);
    soon(serving).await.unwrap().unwrap();
    assert_eq!(host.done.load(Ordering::SeqCst), 1);
}
