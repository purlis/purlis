// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The fake provider's server: one thread takes connections, one thread per connection answers.

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use purlis_core::clock::{Clock, Monotonic};

use super::recording::{Recording, Usage};
use super::tls::TestRoot;

/// **What one request carried**, as the provider read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The connection it came on, counted from 1 in accept order.
    pub connection: usize,
    pub method: String,
    /// The request target as sent, query included.
    pub path: String,
    /// The headers in the order sent, as `(name, value)`.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// When one chunk of a paced answer was sent, on the provider's clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkSend {
    pub connection: usize,
    /// Its place in the recording, from 0.
    pub index: usize,
    pub at: Instant,
}

/// A client that closed its side while an answer was still being sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerClose {
    pub connection: usize,
    /// How many of the answer's chunks had been sent when the provider saw it.
    pub sent: usize,
}

/// How a [`Provider`] is set up before it starts.
pub struct Builder {
    answers: Vec<(String, Recording)>,
    tls: Option<TestRoot>,
    clock: Arc<dyn Clock>,
    accept_delay: Duration,
    pace: Option<u32>,
}

/// **The fake provider**, serving on a port of this machine's loopback until it is dropped.
pub struct Provider {
    address: SocketAddr,
}

impl Provider {
    /// A provider that answers nothing yet, plain HTTP, on the monotonic clock.
    pub fn builder() -> Builder {
        Builder {
            answers: Vec::new(),
            tls: None,
            clock: Arc::new(Monotonic),
            accept_delay: Duration::ZERO,
            pace: None,
        }
    }

    /// Where it listens.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Its port.
    pub fn port(&self) -> u16 {
        self.address.port()
    }

    /// Every request it has read, in order.
    pub fn requests(&self) -> Vec<Request> {
        Vec::new()
    }

    /// How many connections it has accepted.
    pub fn accepts(&self) -> usize {
        0
    }

    /// The usage of each answer it sent, in order.
    pub fn usage(&self) -> Vec<Usage> {
        Vec::new()
    }

    /// Each paced chunk it sent, in the order sent.
    pub fn sends(&self) -> Vec<ChunkSend> {
        Vec::new()
    }

    /// Each close of a client's side it saw mid-answer.
    pub fn peer_closes(&self) -> Vec<PeerClose> {
        Vec::new()
    }
}

impl Builder {
    /// Answers a request for `path` (its query aside) with `recording`; any other path gets a
    /// `404`.
    pub fn answer(mut self, path: &str, recording: Recording) -> Builder {
        self.answers.push((path.to_owned(), recording));
        self
    }

    /// Serves TLS under `root`'s certificate.
    pub fn tls(mut self, root: &TestRoot) -> Builder {
        self.tls = Some(root.clone());
        self
    }

    /// Reads deadlines, delays and pacing off `clock`.
    pub fn clock(mut self, clock: Arc<dyn Clock>) -> Builder {
        self.clock = clock;
        self
    }

    /// Holds each accepted connection `delay` on its clock before reading from it.
    pub fn accept_delay(mut self, delay: Duration) -> Builder {
        self.accept_delay = delay;
        self
    }

    /// Sends a streamed answer's chunks at `per_second` on its clock, recording each send.
    pub fn paced(mut self, per_second: u32) -> Builder {
        self.pace = Some(per_second);
        self
    }

    /// Starts serving on a port of `127.0.0.1`.
    pub fn start(self) -> io::Result<Provider> {
        let _ = (self.answers, self.tls, self.clock, self.accept_delay, self.pace);
        Err(io::Error::other("stub"))
    }
}
