// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The key-proxy crate's integration tests, as one test binary (as `purlis-host`'s are, #3613).

mod both_binaries_link_the_key_proxy_crate;
mod key_proxy_work_runs_on_its_own_two_threads;
mod the_crate_serves_on_hypers_own_http1_connection;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_key_proxy::{Heard, Record};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// How long a test waits for the key proxy before it fails.
pub const PATIENCE: Duration = Duration::from_secs(10);

/// A record line, with the name and id of the thread that told it.
pub type Served = (Record, Option<String>, std::thread::ThreadId);

/// What a start's callbacks were told.
#[derive(Clone, Default)]
pub struct Told {
    pub said: Arc<Mutex<Vec<String>>>,
    pub records: Arc<Mutex<Vec<Served>>>,
}

impl Told {
    pub fn heard(&self) -> Heard {
        let said = Arc::clone(&self.said);
        let records = Arc::clone(&self.records);
        Heard {
            say: Arc::new(move |line| said.lock().unwrap().push(line.to_string())),
            record: Arc::new(move |record| {
                let thread = std::thread::current();
                records.lock().unwrap().push((
                    record.clone(),
                    thread.name().map(str::to_string),
                    thread.id(),
                ));
            }),
        }
    }
}

/// Sends `request` as written to `port` and reads what comes back until the connection closes
/// or the answer's head and `Content-Length` body are in.
pub async fn exchange(port: u16, request: &[u8]) -> String {
    let mut stream = tokio::time::timeout(PATIENCE, TcpStream::connect(("127.0.0.1", port)))
        .await
        .expect("connected in time")
        .expect("the listener accepts a connection");
    stream.write_all(request).await.unwrap();
    let mut got = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let read = tokio::time::timeout(PATIENCE, stream.read(&mut buf))
            .await
            .expect("answered in time");
        match read {
            Ok(0) | Err(_) => break,
            Ok(n) => got.extend_from_slice(&buf[..n]),
        }
        if whole(&got) {
            break;
        }
    }
    String::from_utf8_lossy(&got).into_owned()
}

fn whole(got: &[u8]) -> bool {
    let text = String::from_utf8_lossy(got);
    let Some((head, body)) = text.split_once("\r\n\r\n") else {
        return false;
    };
    let length = head.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<usize>().ok())
            .flatten()
    });
    length.is_some_and(|length| body.len() >= length)
}

/// The status of an HTTP/1 answer.
pub fn status(answer: &str) -> u16 {
    answer
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("no status line in {answer:?}"))
}
