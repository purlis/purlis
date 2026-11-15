// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How the key proxy reaches an upstream** (R-695, R-700, design §5.4).
//!
//! Every outbound connection the key proxy makes goes through an [`Outbound`]: in a build that
//! ships, [`Direct`], which resolves the name with the system and connects, to the upstream or to
//! the outbound proxy in its place. A test swaps in a [`RecordingOutbound`], which logs each
//! [`Destination`] it is asked for (R-731 and the gate's recording outbound proxy read it).
//!
//! **The test seams** (only with `test-support`, never an environment variable, file or setting a
//! release build reads; `tests/it/test_seams_are_absent_from_a_release_build.rs` holds it):
//! [`TestReach`], an entry that resolves one name to a loopback address and lets it be reached,
//! handed to [`Direct::with_test_reach`]; [`RecordingOutbound`]; and [`trust`]'s
//! `trusting_test_root`. With them a built-in's real `https` origin, `api.anthropic.com`, is
//! served by the fake provider on this machine's loopback under TLS.

use std::io;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};

pub mod trust;

/// Where one outbound connection goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    /// The upstream's host name, as the route names it.
    pub host: String,
    /// The upstream's port.
    pub port: u16,
    /// The outbound proxy (`host:port`) the connection goes to in the upstream's place, when one
    /// is set; the tunnel through it is its caller's to ask for.
    pub proxy: Option<String>,
}

impl Destination {
    /// A connection straight to `host:port`.
    pub fn direct(host: &str, port: u16) -> Destination {
        Destination {
            host: host.to_owned(),
            port,
            proxy: None,
        }
    }

    /// A connection to `host:port` through the outbound proxy at `proxy` (`host:port`).
    pub fn through(host: &str, port: u16, proxy: &str) -> Destination {
        Destination {
            proxy: Some(proxy.to_owned()),
            ..Destination::direct(host, port)
        }
    }
}

/// What makes the key proxy's outbound connections.
pub trait Outbound: Send + Sync {
    /// A connection to `to`: to its proxy when it names one, to the upstream otherwise.
    fn connect(&self, to: &Destination) -> io::Result<TcpStream>;
}

/// **The outbound every build that ships uses**: the system resolves the name, and the first
/// address that answers is connected to.
#[derive(Debug, Default, Clone)]
pub struct Direct {
    #[cfg(feature = "test-support")]
    reach: Vec<TestReach>,
}

impl Direct {
    /// The system's resolver, and nothing else.
    pub fn new() -> Direct {
        Direct::default()
    }

    /// The addresses `host:port` resolves to.
    fn resolve(&self, host: &str, port: u16) -> io::Result<Vec<SocketAddr>> {
        #[cfg(feature = "test-support")]
        if let Some(entry) = self.reach.iter().find(|entry| entry.host == host) {
            return Ok(vec![entry.address]);
        }
        Ok((host, port).to_socket_addrs()?.collect())
    }
}

impl Outbound for Direct {
    fn connect(&self, to: &Destination) -> io::Result<TcpStream> {
        let _ = (to, Direct::resolve);
        Err(io::Error::other("stub"))
    }
}

#[cfg(feature = "test-support")]
pub use seams::{RecordingOutbound, TestReach};

#[cfg(feature = "test-support")]
mod seams;
