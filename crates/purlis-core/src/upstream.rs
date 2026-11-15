// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How the key proxy reaches an upstream** (R-695, R-700, design §5.4).
//!
//! Every outbound connection the key proxy makes goes through an [`Outbound`]: in a build that
//! ships, [`Direct`], which resolves the name with the system and connects, to the upstream or to
//! the outbound proxy in its place. A test swaps in a recording outbound, which logs each
//! [`Destination`] it is asked for (R-731 and the gate's recording outbound proxy read it).
//!
//! **The test seams** live in `upstream/seams.rs` and beside [`trust`]'s client, only with
//! `test-support`, never behind an environment variable, file or setting a release build reads
//! (`crates/fake-harness/tests/test_seams_are_absent_from_a_release_build.rs` holds it: it
//! builds this crate without `test-support` and compiles a use of each seam against that
//! library, each of which must fail to compile). They are: a resolver hook with
//! a test-only reach entry, which resolves one name to a loopback address and lets it be
//! reached; the recording outbound; and a test root added to the trust store. With them a
//! built-in's real `https` origin, `api.anthropic.com`, is served by the fake provider on this
//! machine's loopback under TLS.

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
        let (host, port) = match &to.proxy {
            Some(proxy) => split(proxy)?,
            None => (to.host.as_str(), to.port),
        };
        let addresses = self.resolve(host, port)?;
        TcpStream::connect(&addresses[..])
    }
}

/// `host:port`, split at its last colon (an IPv6 address in brackets keeps its own).
fn split(address: &str) -> io::Result<(&str, u16)> {
    address
        .rsplit_once(':')
        .and_then(|(host, port)| Some((host.trim_matches(['[', ']']), port.parse().ok()?)))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("not a host and port: {address}"),
            )
        })
}

#[cfg(feature = "test-support")]
pub use seams::{RecordingOutbound, TestReach};

#[cfg(feature = "test-support")]
mod seams;
