// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The outbound's test seams, built only with `test-support` (see [`super`]).

use std::io;
use std::net::{SocketAddr, TcpStream};
use std::sync::{Mutex, PoisonError};

use super::{Destination, Direct, Outbound};

/// **A test-only reach entry**: `host` resolves to `address`, a loopback port the fake provider
/// serves, and may be reached there. The resolver hook is [`Direct::with_test_reach`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestReach {
    pub(super) host: String,
    pub(super) address: SocketAddr,
}

impl TestReach {
    /// `host` served at `address`.
    pub fn new(host: &str, address: SocketAddr) -> TestReach {
        TestReach {
            host: host.to_owned(),
            address,
        }
    }
}

impl Direct {
    /// The system's resolver, but `entry`'s name resolves to `entry`'s address.
    pub fn with_test_reach(entry: TestReach) -> Direct {
        Direct { reach: vec![entry] }
    }
}

/// **An outbound that logs every destination** before handing the connection to `inner`.
#[derive(Debug, Default)]
pub struct RecordingOutbound<O> {
    inner: O,
    log: Mutex<Vec<Destination>>,
}

impl<O: Outbound> RecordingOutbound<O> {
    /// Records, then connects through `inner`.
    pub fn new(inner: O) -> RecordingOutbound<O> {
        RecordingOutbound {
            inner,
            log: Mutex::new(Vec::new()),
        }
    }

    /// Every destination asked for so far, in order.
    pub fn destinations(&self) -> Vec<Destination> {
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl<O: Outbound> Outbound for RecordingOutbound<O> {
    fn connect(&self, to: &Destination) -> io::Result<TcpStream> {
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(to.clone());
        self.inner.connect(to)
    }
}
