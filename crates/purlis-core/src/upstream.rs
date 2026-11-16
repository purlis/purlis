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

/// **A name the system's resolver found no address for**: the error inside the `io::Error` an
/// [`Outbound`] answers with when the host it was asked for could not be found, so its caller
/// can tell that from a connect that failed (W81).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostNotFound {
    /// The name that was not found.
    pub host: String,
}

impl HostNotFound {
    /// The `io::Error` saying `host` was not found.
    pub fn error(host: &str) -> io::Error {
        io::Error::new(
            io::ErrorKind::NotFound,
            HostNotFound {
                host: host.to_owned(),
            },
        )
    }

    /// Whether `error` says a host was not found.
    pub fn is(error: &io::Error) -> bool {
        error
            .get_ref()
            .is_some_and(|inner| inner.is::<HostNotFound>())
    }
}

impl std::fmt::Display for HostNotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "no address found for {}", self.host)
    }
}

impl std::error::Error for HostNotFound {}

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

    /// The addresses `host:port` resolves to; [`HostNotFound`] where it resolves to none.
    fn resolve(&self, host: &str, port: u16) -> io::Result<Vec<SocketAddr>> {
        #[cfg(feature = "test-support")]
        if let Some(entry) = self.reach.iter().find(|entry| entry.host == host) {
            return Ok(vec![entry.address]);
        }
        let addresses: Vec<SocketAddr> = (host, port)
            .to_socket_addrs()
            .map_err(|_| HostNotFound::error(host))?
            .collect();
        if addresses.is_empty() {
            return Err(HostNotFound::error(host));
        }
        Ok(addresses)
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

#[cfg(test)]
mod tests {
    use super::{Destination, Direct, HostNotFound, Outbound};

    #[test]
    fn the_resolver_says_a_host_it_cannot_find_was_not_found() {
        // `.invalid` never resolves (RFC 6761), so this asks no server for an answer.
        let failed = Direct::new()
            .connect(&Destination::direct("nowhere.invalid", 443))
            .expect_err("a name under .invalid resolved");
        assert!(HostNotFound::is(&failed), "not named not found: {failed:?}");

        let refused = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
        assert!(!HostNotFound::is(&refused));
    }
}
