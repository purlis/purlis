// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A small blocking HTTP/1.1 client the fake provider's tests talk to it with, over a plain
//! stream or rustls, and the wait a test uses for a condition another thread makes true.

#![allow(dead_code)]

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, StreamOwned};

/// A stream the client reads and writes.
pub trait Stream: Read + Write + Send {}
impl<T: Read + Write + Send> Stream for T {}

/// One connection, kept for as many exchanges as a test sends on it.
pub struct Connection {
    reader: BufReader<Box<dyn Stream>>,
}

/// An answer's head.
#[derive(Debug, Clone)]
pub struct Head {
    pub status: u16,
    /// The head as sent, status line and headers, each line ending `\r\n`.
    pub raw: String,
    pub headers: Vec<(String, String)>,
}

impl Head {
    /// The first value of header `name`, any case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// A whole answer.
#[derive(Debug, Clone)]
pub struct Answer {
    pub head: Head,
    pub body: Vec<u8>,
}

impl Connection {
    /// Plain HTTP to `address`.
    pub fn plain(address: SocketAddr) -> io::Result<Connection> {
        Ok(Connection::over(TcpStream::connect(address)?))
    }

    /// Plain HTTP over a stream already connected.
    pub fn over(stream: TcpStream) -> Connection {
        Connection {
            reader: BufReader::new(Box::new(stream)),
        }
    }

    /// TLS over `stream`, verified by `config` for `name`; the handshake is finished before it
    /// returns, so a refused certificate is an error here.
    pub fn tls(stream: TcpStream, config: ClientConfig, name: &str) -> io::Result<Connection> {
        let name = ServerName::try_from(name.to_owned()).map_err(io::Error::other)?;
        let client = ClientConnection::new(Arc::new(config), name).map_err(io::Error::other)?;
        let mut stream = StreamOwned::new(client, stream);
        while stream.conn.is_handshaking() {
            stream.conn.complete_io(&mut stream.sock)?;
        }
        Ok(Connection {
            reader: BufReader::new(Box::new(stream)),
        })
    }

    /// Sends one request with a body of its length.
    pub fn send(
        &mut self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> io::Result<()> {
        let mut out = format!("{method} {path} HTTP/1.1\r\nhost: provider\r\n");
        for (name, value) in headers {
            out.push_str(&format!("{name}: {value}\r\n"));
        }
        out.push_str(&format!("content-length: {}\r\n\r\n", body.len()));
        let stream = self.reader.get_mut();
        stream.write_all(out.as_bytes())?;
        stream.write_all(body)?;
        stream.flush()
    }

    /// Reads an answer's head.
    pub fn head(&mut self) -> io::Result<Head> {
        let mut raw = String::new();
        let mut status_line = String::new();
        self.reader.read_line(&mut status_line)?;
        raw.push_str(&status_line);
        let status = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io::Error::other(format!("no status line: {status_line:?}")))?;
        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            self.reader.read_line(&mut line)?;
            raw.push_str(&line);
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                break;
            }
            if let Some((name, value)) = trimmed.split_once(':') {
                headers.push((name.to_owned(), value.trim().to_owned()));
            }
        }
        Ok(Head {
            status,
            raw,
            headers,
        })
    }

    /// The next chunk of a chunked body; `None` after its last.
    pub fn chunk(&mut self) -> io::Result<Option<Vec<u8>>> {
        let mut size = String::new();
        self.reader.read_line(&mut size)?;
        let size = usize::from_str_radix(size.trim(), 16)
            .map_err(|_| io::Error::other(format!("not a chunk size: {size:?}")))?;
        let mut chunk = vec![0; size];
        self.reader.read_exact(&mut chunk)?;
        let mut end = [0; 2];
        self.reader.read_exact(&mut end)?;
        Ok((size > 0).then_some(chunk))
    }

    /// The body that follows `head`: by its length, or chunk by chunk.
    pub fn body(&mut self, head: &Head) -> io::Result<Vec<u8>> {
        if head.header("transfer-encoding") == Some("chunked") {
            let mut body = Vec::new();
            while let Some(chunk) = self.chunk()? {
                body.extend(chunk);
            }
            return Ok(body);
        }
        let length = head
            .header("content-length")
            .and_then(|l| l.parse().ok())
            .unwrap_or(0);
        let mut body = vec![0; length];
        self.reader.read_exact(&mut body)?;
        Ok(body)
    }

    /// Sends a `POST` of `body` to `path` and reads the whole answer.
    pub fn post(&mut self, path: &str, headers: &[(&str, &str)], body: &[u8]) -> io::Result<Answer> {
        self.send("POST", path, headers, body)?;
        let head = self.head()?;
        let body = self.body(&head)?;
        Ok(Answer { head, body })
    }
}

/// Waits until `done` holds, failing the test after 30 s of real time. It waits for another
/// thread, never for a budget: the clock the test is about is the injected one.
pub fn until(what: &str, mut done: impl FnMut() -> bool) {
    let give_up = Instant::now() + Duration::from_secs(30);
    while !done() {
        assert!(Instant::now() < give_up, "waited 30 s for {what}");
        std::thread::sleep(Duration::from_millis(1));
    }
}
