// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A recording outbound proxy** for the T1a-60 tests: it keeps the head of every request it is
//! sent and either opens the `CONNECT` tunnel it is asked for, to the address the test names in
//! the upstream's place, or answers `407 Proxy Authentication Required`. One thread takes
//! connections and one per connection serves it, as the fake provider does; it ends with the
//! test's process.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// What the proxy does with a `CONNECT`.
#[derive(Debug, Clone, Copy)]
pub enum Answers {
    /// Opens the tunnel, to this address whatever the `CONNECT` named.
    TunnelTo(SocketAddr),
    /// Answers 407 and closes.
    AsksForCredentials,
}

/// The proxy, serving on a port of this machine's loopback.
pub struct OutboundProxy {
    address: SocketAddr,
    heads: Arc<Mutex<Vec<String>>>,
}

impl OutboundProxy {
    pub fn start(answers: Answers) -> OutboundProxy {
        let listener = TcpListener::bind("127.0.0.1:0").expect("the outbound proxy binds");
        let address = listener.local_addr().unwrap();
        let heads = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&heads);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let kept = Arc::clone(&kept);
                std::thread::spawn(move || {
                    let _ = serve(stream, answers, &kept);
                });
            }
        });
        OutboundProxy { address, heads }
    }

    /// Its address as a proxy variable names it.
    pub fn url(&self) -> String {
        format!("http://{}", self.address)
    }

    /// The head of every request it was sent, in order.
    pub fn heads(&self) -> Vec<String> {
        self.heads.lock().unwrap().clone()
    }
}

fn serve(stream: TcpStream, answers: Answers, heads: &Mutex<Vec<String>>) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut head = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(());
        }
        head.push_str(&line);
        if line == "\r\n" {
            break;
        }
    }
    heads.lock().unwrap().push(head);
    let mut client = stream;
    match answers {
        Answers::AsksForCredentials => client.write_all(
            b"HTTP/1.1 407 Proxy Authentication Required\r\n\
              Proxy-Authenticate: Basic realm=\"test\"\r\n\
              Content-Length: 0\r\nConnection: close\r\n\r\n",
        ),
        Answers::TunnelTo(target) => {
            let upstream = TcpStream::connect(target)?;
            client.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")?;
            let (mut from_client, mut to_upstream) = (reader, upstream.try_clone()?);
            let (mut from_upstream, mut to_client) = (upstream, client);
            let up = std::thread::spawn(move || io::copy(&mut from_client, &mut to_upstream));
            let _ = io::copy(&mut from_upstream, &mut to_client);
            let _ = up.join();
            Ok(())
        }
    }
}
