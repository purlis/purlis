// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The direct-connection target: what an escape cell that bypasses the proxy could reach.

use std::io;
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

/// How often the UDP listener looks for its stop.
const UDP_WAKE: Duration = Duration::from_millis(200);

/// An address of the documentation range (RFC 5737): a UDP socket "connected" to it learns which
/// interface address this machine would send from, and no packet is sent.
const ROUTED_TO: ([u8; 4], u16) = ([192, 0, 2, 1], 9);

/// **This machine's non-loopback interface address**: the one it would send from to the
/// internet, learnt without sending anything. An error where there is none (no route out).
pub fn interface_address() -> io::Result<IpAddr> {
    let socket = UdpSocket::bind(("0.0.0.0", 0))?;
    socket.connect(SocketAddr::from(ROUTED_TO))?;
    let ip = socket.local_addr()?.ip();
    if ip.is_loopback() || ip.is_unspecified() {
        return Err(io::Error::other(format!(
            "this machine has no non-loopback interface address to listen on ({ip})"
        )));
    }
    Ok(ip)
}

/// **TCP and UDP listeners on [`interface_address`]**, recording what reached them. The UDP one
/// answers a DNS query `REFUSED` and records the name asked, so a probe's direct lookup
/// (`dig @addr -p port name`) is seen; with the protection on, nothing reaches either.
pub struct DirectTarget {
    tcp: SocketAddr,
    udp: SocketAddr,
    connections: Arc<Mutex<Vec<SocketAddr>>>,
    queries: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
}

impl std::fmt::Debug for DirectTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectTarget")
            .field("tcp", &self.tcp)
            .field("udp", &self.udp)
            .finish_non_exhaustive()
    }
}

impl DirectTarget {
    /// Both listeners, on new ports of [`interface_address`].
    pub fn start() -> io::Result<Self> {
        let ip = interface_address()?;
        let tcp = TcpListener::bind((ip, 0))?;
        let udp = UdpSocket::bind((ip, 0))?;
        udp.set_read_timeout(Some(UDP_WAKE))?;
        let (tcp_at, udp_at) = (tcp.local_addr()?, udp.local_addr()?);
        let connections: Arc<Mutex<Vec<SocketAddr>>> = Arc::default();
        let queries: Arc<Mutex<Vec<String>>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let (recording, stopping) = (Arc::clone(&connections), Arc::clone(&stop));
        std::thread::Builder::new()
            .name("scenario-direct-tcp".into())
            .spawn(move || {
                for stream in tcp.incoming() {
                    if stopping.load(Ordering::SeqCst) {
                        return;
                    }
                    if let Ok(peer) = stream.and_then(|stream| stream.peer_addr()) {
                        recording
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .push(peer);
                    }
                }
            })?;
        let (recording, stopping) = (Arc::clone(&queries), Arc::clone(&stop));
        std::thread::Builder::new()
            .name("scenario-direct-udp".into())
            .spawn(move || {
                let mut buf = [0u8; 512];
                while !stopping.load(Ordering::SeqCst) {
                    let Ok((len, from)) = udp.recv_from(&mut buf) else {
                        continue;
                    };
                    let Some((name, reply)) = refused(&buf[..len]) else {
                        continue;
                    };
                    recording
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push(name);
                    let _ = udp.send_to(&reply, from);
                }
            })?;
        Ok(Self {
            tcp: tcp_at,
            udp: udp_at,
            connections,
            queries,
            stop,
        })
    }

    /// Where the TCP listener is.
    pub fn tcp(&self) -> SocketAddr {
        self.tcp
    }

    /// Where the UDP (DNS) listener is.
    pub fn udp(&self) -> SocketAddr {
        self.udp
    }

    /// Every TCP connection that reached it, by its peer.
    pub fn connections(&self) -> Vec<SocketAddr> {
        self.connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Every name a DNS query asked it for.
    pub fn queries(&self) -> Vec<String> {
        self.queries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// **"Saw nothing"**: no connection and no query.
    pub fn saw_nothing(&self) -> bool {
        self.connections().is_empty() && self.queries().is_empty()
    }
}

impl Drop for DirectTarget {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wakes the accept; the UDP listener wakes by itself within its timeout.
        let _ = TcpStream::connect_timeout(&self.tcp, UDP_WAKE);
    }
}

/// The name a DNS query asks for, and its answer: the same question, `REFUSED`. `None` for
/// anything that is not a query with one question.
fn refused(query: &[u8]) -> Option<(String, Vec<u8>)> {
    const HEADER: usize = 12;
    let flags = u16::from_be_bytes([*query.get(2)?, *query.get(3)?]);
    let questions = u16::from_be_bytes([*query.get(4)?, *query.get(5)?]);
    // A response, or not exactly one question: not a query this answers.
    if flags & 0x8000 != 0 || questions != 1 {
        return None;
    }
    let mut at = HEADER;
    let mut labels: Vec<String> = Vec::new();
    loop {
        let len = usize::from(*query.get(at)?);
        at += 1;
        if len == 0 {
            break;
        }
        // A compression pointer has no place in a question.
        if len & 0xC0 != 0 {
            return None;
        }
        labels.push(String::from_utf8_lossy(query.get(at..at + len)?).to_ascii_lowercase());
        at += len;
    }
    // QTYPE and QCLASS.
    let end = at + 4;
    query.get(at..end)?;
    let mut reply = Vec::with_capacity(end);
    reply.extend_from_slice(&query[..2]);
    // QR, the query's opcode and RD, RA, and REFUSED (5).
    let flags = 0x8000 | (flags & 0x7900) | 0x0080 | 5;
    reply.extend_from_slice(&flags.to_be_bytes());
    reply.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 0]);
    reply.extend_from_slice(&query[HEADER..end]);
    Some((labels.join("."), reply))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `dig example.test A` as it goes on the wire, id 0x1234, recursion desired.
    fn query() -> Vec<u8> {
        let mut out = vec![0x12, 0x34, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
        for label in ["Example", "test"] {
            out.push(u8::try_from(label.len()).expect("short"));
            out.extend_from_slice(label.as_bytes());
        }
        out.extend_from_slice(&[0, 0, 1, 0, 1]);
        out
    }

    #[test]
    fn a_direct_dns_query_is_recorded_by_name_and_answered_refused() {
        let (name, reply) = refused(&query()).expect("a query");
        assert_eq!(name, "example.test");
        assert_eq!(&reply[..2], &[0x12, 0x34], "the query's id");
        assert_eq!(reply[2] & 0x80, 0x80, "a response");
        assert_eq!(reply[3] & 0x0F, 5, "REFUSED");
        assert_eq!(&reply[12..], &query()[12..], "the same question");
    }

    #[test]
    fn a_dns_response_or_a_truncated_query_is_not_answered() {
        let mut response = query();
        response[2] |= 0x80;
        assert!(refused(&response).is_none());
        assert!(refused(&query()[..14]).is_none());
    }
}
