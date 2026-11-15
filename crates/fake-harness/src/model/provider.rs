// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The fake provider's server: one thread takes connections, one thread per connection answers.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use flate2::Compression;
use flate2::write::GzEncoder;
use purlis_core::clock::{Clock, Monotonic};
use rustls::{ServerConnection, StreamOwned};

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
    seen: Arc<Mutex<Seen>>,
    stopped: Arc<AtomicBool>,
}

/// What it has seen, for a test to read back.
#[derive(Default)]
struct Seen {
    accepts: usize,
    requests: Vec<Request>,
    usage: Vec<Usage>,
    sends: Vec<ChunkSend>,
    peer_closes: Vec<PeerClose>,
}

/// What every connection's thread answers with.
struct Serving {
    answers: Vec<(String, Recording)>,
    tls: Option<Arc<rustls::ServerConfig>>,
    clock: Arc<dyn Clock>,
    accept_delay: Duration,
    pace: Option<u32>,
    seen: Arc<Mutex<Seen>>,
}

/// A connection's stream, plain or TLS.
trait Stream: Read + Write + Send {}
impl<T: Read + Write + Send> Stream for T {}

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
        self.seen().requests.clone()
    }

    /// How many connections it has accepted.
    pub fn accepts(&self) -> usize {
        self.seen().accepts
    }

    /// The usage of each answer it sent, in order.
    pub fn usage(&self) -> Vec<Usage> {
        self.seen().usage.clone()
    }

    /// Each paced chunk it sent, in the order sent.
    pub fn sends(&self) -> Vec<ChunkSend> {
        self.seen().sends.clone()
    }

    /// Each close of a client's side it saw mid-answer.
    pub fn peer_closes(&self) -> Vec<PeerClose> {
        self.seen().peer_closes.clone()
    }

    fn seen(&self) -> MutexGuard<'_, Seen> {
        lock(&self.seen)
    }
}

impl Drop for Provider {
    /// Stops taking connections: the accepting thread is woken by one last connection of its
    /// own, and ends. Connections already open end with their clients.
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.address);
    }
}

fn lock(seen: &Mutex<Seen>) -> MutexGuard<'_, Seen> {
    seen.lock().unwrap_or_else(PoisonError::into_inner)
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

    /// Reads deadlines, delays and pacing off `clock`: an injected one in unit and integration
    /// tests, the monotonic one (the default) in `bench` and `stress` runs (I-11).
    pub fn clock(mut self, clock: Arc<dyn Clock>) -> Builder {
        self.clock = clock;
        self
    }

    /// Holds each accepted connection `delay` on its clock before reading from it, so its TLS
    /// handshake waits as long.
    pub fn accept_delay(mut self, delay: Duration) -> Builder {
        self.accept_delay = delay;
        self
    }

    /// Sends a streamed answer's chunks at `per_second` on its clock, recording each send.
    pub fn paced(mut self, per_second: u32) -> Builder {
        self.pace = Some(per_second.max(1));
        self
    }

    /// Starts serving on a port of `127.0.0.1`.
    pub fn start(self) -> io::Result<Provider> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let address = listener.local_addr()?;
        let seen = Arc::new(Mutex::new(Seen::default()));
        let stopped = Arc::new(AtomicBool::new(false));
        let serving = Arc::new(Serving {
            answers: self.answers,
            tls: self.tls.as_ref().map(TestRoot::server_config),
            clock: self.clock,
            accept_delay: self.accept_delay,
            pace: self.pace,
            seen: Arc::clone(&seen),
        });
        let stop = Arc::clone(&stopped);
        std::thread::Builder::new()
            .name("fake-provider-accept".to_owned())
            .spawn(move || accept(&listener, &serving, &stop))?;
        Ok(Provider {
            address,
            seen,
            stopped,
        })
    }
}

/// Takes connections until the provider is dropped, each answered on a thread of its own.
fn accept(listener: &TcpListener, serving: &Arc<Serving>, stopped: &AtomicBool) {
    for stream in listener.incoming() {
        if stopped.load(Ordering::SeqCst) {
            return;
        }
        let Ok(stream) = stream else { continue };
        let accepted = serving.clock.now();
        let connection = {
            let mut seen = lock(&serving.seen);
            seen.accepts += 1;
            seen.accepts
        };
        let serving = Arc::clone(serving);
        let _ = std::thread::Builder::new()
            .name(format!("fake-provider-{connection}"))
            .spawn(move || {
                if !serving.accept_delay.is_zero() {
                    serving.clock.sleep_until(accepted + serving.accept_delay);
                }
                let _ = serving.connection(stream, connection);
            });
    }
}

impl Serving {
    /// Answers every request `stream` carries, until its client closes it.
    fn connection(&self, stream: TcpStream, connection: usize) -> io::Result<()> {
        let stream: Box<dyn Stream> = match &self.tls {
            Some(config) => {
                let server = ServerConnection::new(Arc::clone(config)).map_err(io::Error::other)?;
                Box::new(StreamOwned::new(server, stream))
            }
            None => Box::new(stream),
        };
        let mut reader = BufReader::new(stream);
        while let Some(request) = read_request(&mut reader, connection)? {
            let path = request
                .path
                .split('?')
                .next()
                .unwrap_or_default()
                .to_owned();
            let recording = self
                .answers
                .iter()
                .find(|(answered, _)| *answered == path)
                .map_or_else(not_found, |(_, recording)| recording.clone());
            {
                let mut seen = lock(&self.seen);
                seen.requests.push(request);
                seen.usage.push(recording.usage());
            }
            if !self.answer(reader.get_mut(), &recording, connection)? {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Sends `recording`; false when the client closed its side before the end.
    fn answer(
        &self,
        stream: &mut dyn Stream,
        recording: &Recording,
        connection: usize,
    ) -> io::Result<bool> {
        let mut head = format!(
            "HTTP/1.1 {} {}\r\n",
            recording.status,
            reason(recording.status)
        );
        for (name, value) in &recording.headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        if recording.gzip {
            head.push_str("content-encoding: gzip\r\n");
        }
        if !recording.streamed {
            let body = if recording.gzip {
                gzip(&recording.body())?
            } else {
                recording.body()
            };
            head.push_str(&format!("content-length: {}\r\n\r\n", body.len()));
            stream.write_all(head.as_bytes())?;
            stream.write_all(&body)?;
            stream.flush()?;
            return Ok(true);
        }
        head.push_str("transfer-encoding: chunked\r\n\r\n");
        stream.write_all(head.as_bytes())?;
        stream.flush()?;
        // A gzip stream is flushed after each chunk, so each still arrives on its own.
        let mut encoder = recording
            .gzip
            .then(|| GzEncoder::new(Vec::new(), Compression::default()));
        let start = self.clock.now();
        for (index, chunk) in recording.chunks.iter().enumerate() {
            // When a paced chunk went out; recorded once it has been written, so a chunk the
            // peer's close refused is not counted as sent.
            let paced_at = self.pace.map(|per_second| {
                self.clock.sleep_until(start + paced(index, per_second));
                self.clock.now()
            });
            let bytes = match encoder.as_mut() {
                Some(encoder) => {
                    encoder.write_all(chunk)?;
                    encoder.flush()?;
                    std::mem::take(encoder.get_mut())
                }
                None => chunk.clone(),
            };
            if write_chunk(stream, &bytes).is_err() {
                lock(&self.seen).peer_closes.push(PeerClose {
                    connection,
                    sent: index,
                });
                return Ok(false);
            }
            if let Some(at) = paced_at {
                lock(&self.seen).sends.push(ChunkSend {
                    connection,
                    index,
                    at,
                });
            }
        }
        if let Some(encoder) = encoder {
            write_chunk(stream, &encoder.finish()?)?;
        }
        stream.write_all(b"0\r\n\r\n")?;
        stream.flush()?;
        Ok(true)
    }
}

/// How long after the first chunk the chunk at `index` is due, at `per_second`.
fn paced(index: usize, per_second: u32) -> Duration {
    let index = u64::try_from(index).unwrap_or(u64::MAX);
    Duration::from_nanos(index.saturating_mul(1_000_000_000) / u64::from(per_second))
}

/// Reads one request; `None` once the client has closed the connection.
fn read_request(
    reader: &mut BufReader<Box<dyn Stream>>,
    connection: usize,
) -> io::Result<Option<Request>> {
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let mut words = line.split_whitespace();
    let method = words.next().unwrap_or_default().to_owned();
    let path = words.next().unwrap_or_default().to_owned();
    let mut headers = Vec::new();
    let mut length = 0;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            return Ok(None);
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let value = value.trim();
            if name.eq_ignore_ascii_case("content-length") {
                length = value.parse().unwrap_or(0);
            }
            headers.push((name.to_owned(), value.to_owned()));
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(Some(Request {
        connection,
        method,
        path,
        headers,
        body,
    }))
}

/// One HTTP chunk, flushed. An empty one is skipped: it would end the body.
fn write_chunk(stream: &mut dyn Stream, bytes: &[u8]) -> io::Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    stream.write_all(format!("{:x}\r\n", bytes.len()).as_bytes())?;
    stream.write_all(bytes)?;
    stream.write_all(b"\r\n")?;
    stream.flush()
}

fn gzip(body: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(body)?;
    encoder.finish()
}

/// The answer to a path no recording answers, in Messages' error shape.
fn not_found() -> Recording {
    let mut recording = Recording::message(
        r#"{"type":"error","error":{"type":"not_found_error","message":"fake provider: no recording for this path"}}"#,
    );
    recording.status = 404;
    recording
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        529 => "Overloaded",
        _ => "Status",
    }
}
