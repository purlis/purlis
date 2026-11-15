// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-055: the host starts a run's key proxy through a trait its binary supplies, beside
//! `Confine`, with a session-protocol command, and links no HTTP code doing it
//! (`the_host_links_no_git_http_or_keyring_code.rs` stays green). Here the trait is a test's
//! own, a plain TCP listener: the command brings it up, answers its port and token, and the stop
//! tears it down.

#![cfg(unix)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use purlis_host::standin::StandIns;
use purlis_host::{Compose, Hosting, KeyProxyStarted, KeyProxyTell, ProxiesKeys, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Answer, Client, Command};
use purlis_terminal::session::Spec;
use tokio::io::duplex;

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

const RUN: &str = "01J9RUN000000000000000000A";

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// A key proxy that is a plain listener on `127.0.0.1`, by run: what the trait is asked.
#[derive(Clone, Default)]
pub struct ListensPlainly {
    pub listening: Arc<Mutex<HashMap<String, std::net::TcpListener>>>,
    pub starts: Arc<Mutex<u32>>,
}

impl ProxiesKeys for ListensPlainly {
    fn start(&self, run: &str, _tell: KeyProxyTell) -> Result<KeyProxyStarted, String> {
        *self.starts.lock().unwrap() += 1;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let port = listener.local_addr().unwrap().port();
        self.listening
            .lock()
            .unwrap()
            .insert(run.to_string(), listener);
        Ok(KeyProxyStarted {
            port,
            token: "ab".repeat(32),
        })
    }

    fn stop(&self, run: &str) -> Result<(), String> {
        self.listening
            .lock()
            .unwrap()
            .remove(run)
            .map(drop)
            .ok_or_else(|| "no key proxy runs for that run".into())
    }
}

pub async fn linked_as(hosting: &Hosting, scope: Scope) -> Client {
    let (a, b) = duplex(64 * 1024);
    // A link between hosts proves a device, not a scope's credential.
    let (client, served) = if scope == Scope::RemoteLink {
        tokio::join!(
            link::connect_as_a_device(a, session::speaks()),
            link::serve_as_a_device(b, session::speaks(), link::ProvenDevice::stand_in())
        )
    } else {
        tokio::join!(
            link::connect(a, session::speaks(), scope, HELD.of(scope)),
            link::serve_any(b, session::speaks(), &HELD)
        )
    };
    tokio::spawn(session::serve(served.unwrap(), hosting.clone(), None));
    Client::new(client.unwrap()).0
}

pub fn hosting_with(proxies: impl ProxiesKeys) -> Hosting {
    Hosting::running_with(Echo, StandIns::default()).proxying_keys_with(proxies)
}

#[tokio::test]
async fn the_start_command_brings_up_a_listener_through_the_trait() {
    let proxies = ListensPlainly::default();
    let window = linked_as(&hosting_with(proxies.clone()), Scope::LocalUi).await;

    let answer = window
        .call(Command::StartKeyProxy { run: RUN.into() })
        .await
        .expect("the window's start is answered");
    let Answer::KeyProxyStarted { port, token } = answer else {
        panic!("answered {answer:?}");
    };
    assert_eq!(
        token.0,
        "ab".repeat(32),
        "the trait's token is the answer's"
    );
    std::net::TcpStream::connect(("127.0.0.1", port)).expect("its port accepts a connection");

    window
        .call(Command::StopKeyProxy { run: RUN.into() })
        .await
        .expect("the stop is answered");
    assert!(
        proxies.listening.lock().unwrap().is_empty(),
        "the stop tore the listener down"
    );
    assert!(
        std::net::TcpStream::connect(("127.0.0.1", port)).is_err(),
        "its port still accepts after the stop"
    );
}

#[tokio::test]
async fn a_host_given_no_way_to_start_a_key_proxy_refuses_the_start() {
    let window = linked_as(
        &Hosting::running_with(Echo, StandIns::default()),
        Scope::LocalUi,
    )
    .await;
    let refused = window
        .call(Command::StartKeyProxy { run: RUN.into() })
        .await
        .expect_err("a host without the trait starts nothing");
    let refusal = refused.refusal().expect("refused in words");
    assert_eq!(refusal.code, session::NOT_STARTED);
    assert!(refusal.why.contains("key proxy"), "{}", refusal.why);
}
