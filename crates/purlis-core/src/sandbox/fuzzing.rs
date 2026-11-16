// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the fuzz targets reach** (T1.1-47, `crates/purlis-core/fuzz/`): the parsers a sandboxed
//! chat's bytes meet first, each as the proxy, the door or the reach proxy calls it, with no socket
//! and no clock. Built only with the `fuzzing` feature, which only the fuzz crate turns on; nothing
//! in purlis calls it.

use std::io;
use std::time::Instant;

use super::egress::{self, Request, SocksClient};

/// A request head as the egress proxy and the browser reach proxy read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// The host the proxy judges and dials.
    pub host: String,
    pub port: u16,
    /// For a plain request, the head sent on in its place; none for a `CONNECT`.
    pub forward: Option<String>,
    /// For a plain request, its `Host` header.
    pub host_header: Option<String>,
    /// For a plain request, its body's length.
    pub body: Option<u64>,
}

/// **`head` (ending in its blank line) as the egress proxy parses it**
/// (`egress::Request::parse`), or `None` where the proxy answers `400`.
pub fn proxy_request_head(head: &str) -> Option<Head> {
    Request::parse(head).map(|request| Head {
        host: request.host,
        port: request.port,
        forward: request.forward,
        host_header: request.host_header,
        body: request.body,
    })
}

/// **Whether the proxy carries `head` past its `Host` check**: a plain request whose `Host` names
/// another host than its target is answered `403` by the egress proxy and the reach proxy alike
/// (`egress::same_host`). A `CONNECT` has no such check.
pub fn host_header_agrees(head: &Head) -> bool {
    head.host_header
        .as_deref()
        .is_none_or(|named| egress::same_host(named, &head.host, head.port))
}

/// **`stream`, read as the egress proxy reads one**: up to the end of its head and no further,
/// no longer than the proxy's longest head, and as text (`egress::read_head`'s rule, without the
/// socket).
pub fn head_of(stream: &[u8]) -> Option<&str> {
    let end = stream.windows(4).position(|w| w == b"\r\n\r\n")?;
    if end + 4 > egress::HEAD_MAX {
        return None;
    }
    std::str::from_utf8(&stream[..end + 4]).ok()
}

/// What a SOCKS client's bytes came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks {
    /// The host and port a `CONNECT` names, the reply code that refused it, or `None` where the
    /// proxy closed the connection with no reply.
    pub request: Option<Result<(String, u16), u8>>,
    /// What the proxy wrote back before the request was decided: the method reply.
    pub answered: Vec<u8>,
    /// How many of the client's bytes it read.
    pub read: usize,
}

/// **`bytes`, sent to the proxy's SOCKS port as a client's greeting and request**
/// (`egress::socks_request`).
pub fn socks_handshake(bytes: &[u8]) -> Socks {
    struct InMemory<'a> {
        bytes: &'a [u8],
        read: usize,
        answered: Vec<u8>,
    }
    impl SocksClient for InMemory<'_> {
        fn fill(&mut self, buf: &mut [u8], _deadline: Instant) -> Option<()> {
            let next = self.bytes.get(self.read..self.read + buf.len())?;
            buf.copy_from_slice(next);
            self.read += buf.len();
            Some(())
        }

        fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
            self.answered.extend_from_slice(bytes);
            Ok(())
        }
    }
    let mut client = InMemory {
        bytes,
        read: 0,
        answered: Vec::new(),
    };
    // The bytes are all in memory, so nothing waits on the deadline: `fill` never reads it.
    let request = egress::socks_request(&mut client, Instant::now());
    Socks {
        request,
        answered: client.answered,
        read: client.read,
    }
}

/// **The `CONNECT` target `authority` as the proxy reads it**: the head a client sends for it,
/// parsed whole.
pub fn connect_authority(authority: &str) -> Option<(String, u16)> {
    let head = format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n\r\n");
    Request::parse(&head).map(|request| (request.host, request.port))
}

/// The door's judgement of `head` on `port`: the head it sends on, or why it refuses it.
pub fn door_judge(head: &str, port: u16) -> Result<super::door::Carried, super::door::Refused> {
    super::door::judge(head, port, &|_| Vec::new())
}
