// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A download is bounded by a total time, from its request to its body's last byte, and not
//! only by how long it takes to connect: a server that accepts the connection and then says
//! nothing is given up on, and the request is told to the download's listener as not
//! answered. The server here is a loopback listener that never writes.

use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_browser_engine::install::{Fetch, Https};

#[test]
fn a_server_that_accepts_and_then_says_nothing_is_given_up_on_within_the_total_time() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (held, hold) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let connection = listener.accept().map(|(stream, _)| stream);
        // Kept open, and never written to, until the test is done.
        let _ = hold.recv();
        drop(connection);
    });

    let heard = Arc::new(Mutex::new(Vec::new()));
    let into = Arc::clone(&heard);
    let https = Https::within(Duration::from_secs(2)).listening(Arc::new(move |hop| {
        into.lock()
            .unwrap()
            .push((hop.url.to_owned(), hop.answered));
    }));
    let url = format!("https://127.0.0.1:{port}/engine-manifest.json");
    let started = Instant::now();
    let got = https.fetch(&url);
    let took = started.elapsed();
    assert!(got.is_err(), "a server that says nothing gave an answer");
    assert!(
        took < Duration::from_secs(20),
        "given up on only after {took:?}"
    );
    assert_eq!(*heard.lock().unwrap(), [(url, false)]);

    let _ = held.send(());
    server.join().unwrap();
}
