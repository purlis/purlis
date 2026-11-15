// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-055, B-2: a run's key-proxy token leaves the host in the start's answer and nowhere else.
//! The real key-proxy crate is started through a test's own implementation of the trait, which
//! keeps everything the key proxy tells the host: its log lines and its network record. A
//! request carrying the token in both credential headers is served, the run is stopped, and the
//! token is in none of those lines, nor in any event the host pushed its window.
//!
//! The host's own log goes to its standard error, which a test in this process cannot read, so
//! the start runs in this test binary run again as a child, and every byte that child writes,
//! on either stream, is read for the token: but for the one line it hands the token back on.

#![cfg(unix)]

use std::process::Command as Program;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_host::{KeyProxyStarted, KeyProxyTell, KeyProxyTold, ProxiesKeys};
use purlis_key_proxy::{Heard, KeyProxy};
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Answer, Client, Command};
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

use crate::the_start_command_brings_up_a_listener_through_the_trait::hosting_with;

const RUN: &str = "01J9RUN000000000000000000C";

/// Set in the child's environment alone: only there does [`the_hosts_run_of_the_start`] run.
const CHILD: &str = "PURLIS_TEST_TOKEN_CHILD";

/// The marker the child hands the token back after, the one occurrence not read for it. libtest
/// may have written its `test … ... ` to the same line without a newline, so the child starts
/// its own line and the parent looks for the marker anywhere in one.
const HANDED_BACK: &str = "purlis-test-handed-back-token:";

/// The real key proxy behind the trait, keeping a copy of all it tells the host.
#[derive(Clone)]
struct KeepsWhatItTells {
    proxy: Arc<KeyProxy>,
    told: Arc<Mutex<Vec<KeyProxyTold>>>,
}

impl ProxiesKeys for KeepsWhatItTells {
    fn start(&self, run: &str, tell: KeyProxyTell) -> Result<KeyProxyStarted, String> {
        let (said, recorded) = (Arc::clone(&self.told), Arc::clone(&self.told));
        let (tell_said, tell_recorded) = (Arc::clone(&tell), tell);
        let started = self
            .proxy
            .start(
                run,
                Heard {
                    say: Arc::new(move |line| {
                        let told = KeyProxyTold::Said(line.to_string());
                        said.lock().unwrap().push(told.clone());
                        tell_said(told);
                    }),
                    record: Arc::new(move |record| {
                        let told = KeyProxyTold::Record {
                            method: record.method.clone(),
                            path: record.path.clone(),
                            status: record.status,
                        };
                        recorded.lock().unwrap().push(told.clone());
                        tell_recorded(told);
                    }),
                },
            )
            .map_err(|why| why.to_string())?;
        Ok(KeyProxyStarted {
            port: started.port,
            token: started.token.reveal().to_string(),
        })
    }

    fn stop(&self, run: &str) -> Result<(), String> {
        if self.proxy.stop(run) {
            Ok(())
        } else {
            Err("no key proxy runs for that run".into())
        }
    }
}

#[test]
fn the_token_never_leaves_the_host_but_in_the_start_reply() {
    let ran = Program::new(std::env::current_exe().expect("the test binary"))
        .args([
            "the_token_never_leaves_the_host_but_in_the_start_reply::the_hosts_run_of_the_start",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, "1")
        .output()
        .expect("the child runs");
    let stdout = String::from_utf8_lossy(&ran.stdout);
    let stderr = String::from_utf8_lossy(&ran.stderr);
    assert!(
        ran.status.success(),
        "the host's run of the start failed\n--- stdout\n{stdout}\n--- stderr\n{stderr}"
    );
    let canary = stdout
        .lines()
        .find_map(|line| line.split_once(HANDED_BACK).map(|(_, token)| token.trim()))
        .expect("the child handed the token back")
        .to_owned();
    assert_eq!(
        canary.len(),
        64,
        "the handed-back token is a 256-bit one, as hex"
    );
    assert!(
        stderr.contains("purlisd: key proxy: "),
        "the host's log is not in what the child wrote, so it was not read: {stderr}"
    );
    let rest = stdout.replacen(&format!("{HANDED_BACK}{canary}"), "", 1);
    for line in rest.lines().chain(stderr.lines()) {
        assert!(
            !line.contains(&canary),
            "the token is in what the host wrote: {line}"
        );
    }
}

/// The start, the request and the stop, in the child that
/// [`the_token_never_leaves_the_host_but_in_the_start_reply`] starts; in any other run it does
/// nothing.
#[tokio::test]
async fn the_hosts_run_of_the_start() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    let proxies = KeepsWhatItTells {
        proxy: Arc::new(KeyProxy::new().expect("its runtime starts")),
        told: Arc::default(),
    };
    let hosting = hosting_with(proxies.clone());
    let held = purlis_session_protocol::auth::Credentials::mint().unwrap();
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            held.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &held)
    );
    tokio::spawn(session::serve(served.unwrap(), hosting.clone(), None));
    let (window, _views, mut events) = Client::new(client.unwrap());
    window
        .subscribe(Default::default())
        .await
        .expect("subscribed");

    let Answer::KeyProxyStarted { port, token } = window
        .call(Command::StartKeyProxy { run: RUN.into() })
        .await
        .expect("started")
    else {
        panic!("not a key proxy's start");
    };
    let canary = token.0.clone();
    assert_eq!(canary.len(), 64, "a 256-bit token, as hex");
    println!("\n{HANDED_BACK}{canary}");

    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    let request = format!(
        "POST /v1/messages?beta=true HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         x-api-key: {canary}\r\nAuthorization: Bearer {canary}\r\n\
         Content-Length: 2\r\nConnection: close\r\n\r\n{{}}"
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut answer = String::new();
    tokio::time::timeout(Duration::from_secs(10), stream.read_to_string(&mut answer))
        .await
        .expect("answered in time")
        .unwrap();
    assert!(
        !answer.contains(&canary),
        "the key proxy's answer to the harness repeats the token"
    );

    window
        .call(Command::StopKeyProxy { run: RUN.into() })
        .await
        .expect("stopped");

    let told = proxies.told.lock().unwrap().clone();
    assert!(
        told.iter().any(|t| matches!(t, KeyProxyTold::Said(_))),
        "the key proxy said nothing for the host's log: {told:?}"
    );
    assert!(
        told.iter()
            .any(|t| matches!(t, KeyProxyTold::Record { .. })),
        "the key proxy recorded no request: {told:?}"
    );
    for line in &told {
        assert!(
            !format!("{line:?}").contains(&canary),
            "the token is in what the key proxy told the host: {line:?}"
        );
    }

    let mut pushed = Vec::new();
    while let Ok(Some(event)) =
        tokio::time::timeout(Duration::from_millis(200), events.recv()).await
    {
        pushed.push(format!("{event:?}"));
    }
    for event in &pushed {
        assert!(
            !event.contains(&canary),
            "the token is in an event the host pushed: {event}"
        );
    }
}
