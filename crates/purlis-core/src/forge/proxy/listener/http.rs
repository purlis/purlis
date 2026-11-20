// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! HTTP/1.1 as the listener speaks it: **strict framing, one request per connection**.
//!
//! A chat's client is not trusted to frame its request the way the forge would, so the listener
//! reads exactly one request (a head of at most [`Limits::head`](super::Limits::head) bytes, in CRLF
//! lines only, then a body told by one `Content-Length` or by a single `chunked` coding, never both),
//! answers it with `Connection: close` and closes. Bytes that arrive after the request are never
//! read as another one, so a second request cannot ride the first one's verdict.

use std::io::{self, Read, Write};
use std::time::Instant;

use super::super::normal;

/// A request's line and headers, as read.
#[derive(Debug)]
pub(crate) struct Head {
    pub(crate) method: String,
    pub(crate) target: String,
    pub(crate) headers: Vec<(String, String)>,
}

/// Why a request head could not be read.
#[derive(Debug)]
pub(crate) enum HeadError {
    /// The client closed before it sent a request.
    Closed,
    /// More than the limit without the end of the head.
    TooLarge,
    /// Not HTTP/1.1 as the listener reads it.
    Malformed,
    Io,
}

/// Why a body could not be read.
#[derive(Debug)]
pub(crate) enum BodyError {
    TooLarge,
    Malformed,
    Io,
}

/// How a request's body is told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Framing {
    None,
    Length(u64),
    Chunked,
}

/// What the headers say of the body. The caller has already refused an ambiguous pair
/// ([`super::super::framing_is_unambiguous`]), so this reads the one it finds.
pub(crate) fn framing_of(headers: &[(String, String)]) -> Framing {
    let find = |wanted: &str| {
        headers
            .iter()
            .find(|(n, _)| normal(n) == wanted)
            .map(|(_, v)| v.trim())
    };
    if find("transfer-encoding").is_some() {
        return Framing::Chunked;
    }
    match find("content-length").map(str::parse::<u64>) {
        Some(Ok(length)) => Framing::Length(length),
        _ => Framing::None,
    }
}

/// A stream and what it has read ahead.
pub(crate) struct Framed<S> {
    pub(crate) stream: S,
    buf: Vec<u8>,
    pos: usize,
    deadline: Option<Instant>,
}

const CHUNK: usize = 16 * 1024;

impl<S: Read + Write> Framed<S> {
    pub(crate) fn new(stream: S) -> Self {
        Self {
            stream,
            buf: Vec::new(),
            pos: 0,
            deadline: None,
        }
    }

    /// Reads fail with a timeout once `deadline` has passed, whatever the stream's own timeout.
    pub(crate) fn with_deadline(mut self, deadline: Instant) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// The stream, if nothing was read past what was taken: bytes that came early would be lost
    /// to whatever speaks on it next, so a connection that sent some is not handed on.
    pub(crate) fn into_stream(self) -> Option<S> {
        (self.pos == self.buf.len()).then_some(self.stream)
    }

    /// Read more from the stream. `0` is its end.
    fn fill(&mut self) -> io::Result<usize> {
        if self.deadline.is_some_and(|d| Instant::now() >= d) {
            return Err(io::ErrorKind::TimedOut.into());
        }
        if self.pos > 0 && self.pos == self.buf.len() {
            self.buf.clear();
            self.pos = 0;
        }
        let at = self.buf.len();
        self.buf.resize(at + CHUNK, 0);
        loop {
            match self.stream.read(&mut self.buf[at..]) {
                Ok(n) => {
                    self.buf.truncate(at + n);
                    return Ok(n);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    self.buf.truncate(at);
                    return Err(e);
                }
            }
        }
    }

    /// Read one request head. The line and the headers end in CRLF and nothing else (a bare `LF`
    /// or `CR` is refused, as is a `NUL`), the version is 1.1, and the head is at most `max` bytes.
    pub(crate) fn read_head(&mut self, max: usize) -> Result<Head, HeadError> {
        let end = loop {
            if let Some(at) = find(&self.buf[self.pos..], b"\r\n\r\n") {
                break at + 4;
            }
            if self.buf.len() - self.pos > max {
                return Err(HeadError::TooLarge);
            }
            match self.fill() {
                Ok(0) if self.buf.len() == self.pos => return Err(HeadError::Closed),
                Ok(0) => return Err(HeadError::Malformed),
                Ok(_) => {}
                Err(_) => return Err(HeadError::Io),
            }
        };
        if end > max {
            return Err(HeadError::TooLarge);
        }
        let bytes = &self.buf[self.pos..self.pos + end];
        let bare = bytes.iter().enumerate().any(|(i, b)| match b {
            b'\n' => i == 0 || bytes[i - 1] != b'\r',
            b'\r' => bytes.get(i + 1) != Some(&b'\n'),
            0 => true,
            _ => false,
        });
        if bare {
            return Err(HeadError::Malformed);
        }
        let mut storage = [httparse::EMPTY_HEADER; 96];
        let mut request = httparse::Request::new(&mut storage);
        let Ok(httparse::Status::Complete(read)) = request.parse(bytes) else {
            return Err(HeadError::Malformed);
        };
        if read != end || request.version != Some(1) {
            return Err(HeadError::Malformed);
        }
        let (Some(method), Some(target)) = (request.method, request.path) else {
            return Err(HeadError::Malformed);
        };
        let mut headers = Vec::with_capacity(request.headers.len());
        for header in request.headers.iter() {
            let value = std::str::from_utf8(header.value).map_err(|_| HeadError::Malformed)?;
            headers.push((header.name.to_string(), value.trim().to_string()));
        }
        let head = Head {
            method: method.to_string(),
            target: target.to_string(),
            headers,
        };
        self.pos += end;
        Ok(head)
    }

    /// Read the body `framing` says, at most `max` bytes of it. A chunked body is read whole and
    /// handed on with a length of its own (a trailer is read and dropped).
    pub(crate) fn read_body(&mut self, framing: Framing, max: usize) -> Result<Vec<u8>, BodyError> {
        match framing {
            Framing::None => Ok(Vec::new()),
            Framing::Length(length) => {
                let length = usize::try_from(length).map_err(|_| BodyError::TooLarge)?;
                if length > max {
                    return Err(BodyError::TooLarge);
                }
                self.take(length)
            }
            Framing::Chunked => {
                let mut body = Vec::new();
                loop {
                    let line = self.line(40)?;
                    let size = line.strip_suffix(b"\r\n").ok_or(BodyError::Malformed)?;
                    // A chunk extension is a second thing the forge may read differently.
                    if size.is_empty() || !size.iter().all(u8::is_ascii_hexdigit) || size.len() > 16
                    {
                        return Err(BodyError::Malformed);
                    }
                    let size = u64::from_str_radix(
                        std::str::from_utf8(size).map_err(|_| BodyError::Malformed)?,
                        16,
                    )
                    .map_err(|_| BodyError::Malformed)?;
                    if size == 0 {
                        // Trailers, if any, up to the empty line.
                        for _ in 0..32 {
                            if self.line(8 * 1024)? == b"\r\n" {
                                return Ok(body);
                            }
                        }
                        return Err(BodyError::Malformed);
                    }
                    let size = usize::try_from(size).map_err(|_| BodyError::TooLarge)?;
                    if body.len().saturating_add(size) > max {
                        return Err(BodyError::TooLarge);
                    }
                    body.extend(self.take(size)?);
                    if self.take(2)? != b"\r\n" {
                        return Err(BodyError::Malformed);
                    }
                }
            }
        }
    }

    /// Exactly `n` bytes.
    fn take(&mut self, n: usize) -> Result<Vec<u8>, BodyError> {
        while self.buf.len() - self.pos < n {
            match self.fill() {
                Ok(0) => return Err(BodyError::Malformed),
                Ok(_) => {}
                Err(_) => return Err(BodyError::Io),
            }
        }
        let out = self.buf[self.pos..self.pos + n].to_vec();
        self.pos += n;
        Ok(out)
    }

    /// A line up to and including its `LF`, at most `max` bytes.
    fn line(&mut self, max: usize) -> Result<Vec<u8>, BodyError> {
        loop {
            if let Some(at) = self.buf[self.pos..].iter().position(|b| *b == b'\n') {
                let line = self.buf[self.pos..self.pos + at + 1].to_vec();
                self.pos += at + 1;
                return Ok(line);
            }
            if self.buf.len() - self.pos > max {
                return Err(BodyError::Malformed);
            }
            match self.fill() {
                Ok(0) => return Err(BodyError::Malformed),
                Ok(_) => {}
                Err(_) => return Err(BodyError::Io),
            }
        }
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    memchr::memmem::find(haystack, needle)
}

/// The reason phrase of a status the listener sends, or a generic one.
pub(crate) fn reason(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        206 => "Partial Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Content Too Large",
        422 => "Unprocessable Content",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

/// Write a status line and headers. A header whose name or value would break the line is dropped,
/// never sent.
pub(crate) fn write_head<W: Write>(
    out: &mut W,
    status: u16,
    headers: &[(String, String)],
) -> io::Result<()> {
    let mut head = format!("HTTP/1.1 {status} {}\r\n", reason(status)).into_bytes();
    for (name, value) in headers {
        let ok = !name.is_empty()
            && name.bytes().all(|b| b.is_ascii_graphic() && b != b':')
            && !value.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0);
        if ok {
            head.extend(format!("{name}: {value}\r\n").bytes());
        }
    }
    head.extend(b"Connection: close\r\n\r\n");
    out.write_all(&head)
}

/// A whole answer the listener makes itself: a refusal, an error.
pub(crate) fn write_answer<W: Write>(out: &mut W, status: u16, body: &str) -> io::Result<()> {
    let headers = vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        ("Content-Length".to_string(), body.len().to_string()),
    ];
    write_head(out, status, &headers)?;
    out.write_all(body.as_bytes())?;
    out.flush()
}

/// `{"message": …}`, the shape both forges answer an error in.
pub(crate) fn message(text: &str) -> String {
    serde_json::json!({ "message": text }).to_string()
}

/// One chunk of a chunked body.
pub(crate) fn write_chunk<W: Write>(out: &mut W, bytes: &[u8]) -> io::Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    write!(out, "{:x}\r\n", bytes.len())?;
    out.write_all(bytes)?;
    out.write_all(b"\r\n")
}

pub(crate) fn finish_chunks<W: Write>(out: &mut W) -> io::Result<()> {
    out.write_all(b"0\r\n\r\n")?;
    out.flush()
}
