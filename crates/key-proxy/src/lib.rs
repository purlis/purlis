// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **purlis's key proxy** (V36's shared passthrough crate, R-053): a listener per run, on
//! `127.0.0.1`, that the run's harness sends its model calls to, with a 256-bit token of its own.
//!
//! Skeleton: the interface the tests are written against, built in the next commit.

use std::io;
use std::sync::Arc;
use std::time::Duration;

/// The name of each of the key proxy's runtime threads (R-054).
pub const THREAD_NAME: &str = "purlis-key-proxy";

/// How many worker threads its runtime has (R-054).
pub const WORKER_THREADS: usize = 2;

/// How long a request's head may take to arrive (design §3.3, the spike's P10).
pub const HEAD_TIMEOUT: Duration = Duration::from_secs(30);

/// The most headers a request may carry: past it hyper answers 431 (the spike's knob).
pub const MOST_HEADERS: usize = 100;

/// The most bytes a connection buffers while it reads: hyper's own default, named here so that a
/// change is a choice. A head past it is answered 431.
pub const MOST_BUFFERED: usize = 8192 + 4096 * 100;

/// One line of the network record: a request the key proxy answered, and how. Never a header,
/// so never the token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub run: String,
    pub method: String,
    pub path: String,
    pub status: u16,
}

/// What the key proxy tells the host, through callbacks the host supplies with each start.
#[derive(Clone)]
pub struct Heard {
    /// A line for the host's log.
    pub say: Arc<dyn Fn(&str) + Send + Sync>,
    /// A line of the network record.
    pub record: Arc<dyn Fn(&Record) + Send + Sync>,
}

/// A run's token: never written out by `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// The token itself, for the start's caller alone.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(..)")
    }
}

/// A listener started for a run.
#[derive(Debug)]
pub struct Started {
    pub port: u16,
    pub token: Token,
}

/// The key proxy: its runtime and the listeners it runs.
pub struct KeyProxy;

impl KeyProxy {
    /// A key proxy with its own runtime.
    pub fn new() -> io::Result<KeyProxy> {
        Err(io::Error::other("not built yet"))
    }

    /// Starts a listener for `run`.
    pub fn start(&self, _run: &str, _heard: Heard) -> io::Result<Started> {
        Err(io::Error::other("not built yet"))
    }

    /// Stops `run`'s listener: whether one ran.
    pub fn stop(&self, _run: &str) -> bool {
        false
    }
}
