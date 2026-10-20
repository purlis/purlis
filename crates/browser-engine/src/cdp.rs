// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! DevTools over the engine's pipe: a small framed reader and writer, not a DevTools client.
//!
//! With `--remote-debugging-pipe` the engine reads DevTools messages from descriptor 3 and
//! writes them to descriptor 4, each one JSON ended by a single NUL byte. JSON never carries a
//! raw NUL (it escapes every control character), so the byte is an unambiguous end.
//!
//! [`Connection`] writes calls, matches each answer to its call by `id`, and hands every
//! message without an `id` to the event receiver. It knows no DevTools domain: what to call is
//! the broker's business. A message is never held past a bound, [`MAX_MESSAGE_BYTES`] or the
//! smaller one its call asked for ([`Connection::call_within_bounded`]): an answer past it is
//! read past to its NUL, never kept, and fails only its own call (#2694). An event past it is
//! dropped only where nobody reads its kind (what a page writes to its console, its uncaught
//! errors); any other event past it ends the connection, since the driver's view of the page's
//! frames and targets is kept from events and is never left stale. A message that is not JSON
//! ends the connection.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

/// The longest message read from the engine, NUL excluded: 64 MiB, room for a full-page
/// screenshot in base64.
pub const MAX_MESSAGE_BYTES: usize = 64 << 20;

/// How much of a message past its bound is kept, to tell which call it answers.
const HEAD_BYTES: usize = 64;

/// How many events wait for the broker before more are dropped (and counted).
pub const MAX_QUEUED_EVENTS: usize = 1024;

/// How long [`Connection::call`] waits for an answer.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// A message from the engine that answers no call.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// The DevTools method, such as `Page.loadEventFired`.
    pub method: String,
    /// Its parameters, `null` when it had none.
    pub params: Value,
    /// The session (an attached page) it came from, if any.
    pub session_id: Option<String>,
}

/// Why a call has no result.
#[derive(Debug, thiserror::Error)]
pub enum CallError {
    /// The engine answered with an error.
    #[error("the engine refused {code}: {message}")]
    Engine {
        /// DevTools' error code.
        code: i64,
        /// DevTools' own words.
        message: String,
    },
    /// The engine's pipe closed, or carried something that is not a DevTools message.
    #[error("the engine has ended")]
    Ended,
    /// No answer within the time allowed.
    #[error("the engine did not answer in time")]
    TimedOut,
    /// The call could not be written to the engine.
    #[error("the call could not be written to the engine: {0}")]
    Write(#[source] io::Error),
    /// The engine's answer was longer than `most` bytes, the most the call takes: it was read
    /// past and dropped, and the connection still answers (#2694).
    #[error("the engine's answer was too large to read: longer than {most} bytes")]
    TooLarge {
        /// The bound the answer went past.
        most: usize,
    },
}

type Answer = Result<Value, CallError>;

/// A call waiting for its answer, and the most bytes the answer may take.
struct Waiting {
    answer: SyncSender<Answer>,
    most: usize,
}

struct Shared {
    /// `None` once the engine's side has ended: every later call is refused at once.
    pending: Mutex<Option<HashMap<u64, Waiting>>>,
    /// Events dropped because [`MAX_QUEUED_EVENTS`] were already waiting.
    dropped: AtomicU64,
}

/// One DevTools connection over the engine's pipe.
pub struct Connection {
    to_engine: Mutex<Box<dyn Write + Send>>,
    next_id: AtomicU64,
    shared: Arc<Shared>,
}

impl Connection {
    /// Starts reading `from_engine` on a thread of its own, and returns the connection and the
    /// receiver of the engine's events. At most [`MAX_QUEUED_EVENTS`] wait to be read; past
    /// that an event is dropped and counted ([`Connection::dropped_events`]), and answers to
    /// calls are never held up by events nobody reads. Dropping the receiver discards them.
    pub fn new<R, W>(from_engine: R, to_engine: W) -> (Self, Receiver<Event>)
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
    {
        let shared = Arc::new(Shared {
            pending: Mutex::new(Some(HashMap::new())),
            dropped: AtomicU64::new(0),
        });
        let (events, receiver) = mpsc::sync_channel(MAX_QUEUED_EVENTS);
        let reading = shared.clone();
        thread::Builder::new()
            .name("engine-devtools-reader".to_owned())
            .spawn(move || read_until_ended(from_engine, &reading, &events))
            .expect("a thread to read the engine's pipe");
        (
            Self {
                to_engine: Mutex::new(Box::new(to_engine)),
                next_id: AtomicU64::new(1),
                shared,
            },
            receiver,
        )
    }

    /// Calls `method` with `params`, on the page `session` is attached to when given, and waits
    /// up to [`CALL_TIMEOUT`] for its result.
    pub fn call(&self, method: &str, params: Value, session: Option<&str>) -> Answer {
        self.call_within(method, params, session, CALL_TIMEOUT)
    }

    /// [`Connection::call`], waiting up to `timeout`.
    pub fn call_within(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
        timeout: Duration,
    ) -> Answer {
        self.call_within_bounded(method, params, session, timeout, MAX_MESSAGE_BYTES)
    }

    /// [`Connection::call_within`], its answer held to `most` bytes (never more than
    /// [`MAX_MESSAGE_BYTES`]): a longer one fails this call alone with [`CallError::TooLarge`].
    pub fn call_within_bounded(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
        timeout: Duration,
        most: usize,
    ) -> Answer {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (answer, waiting) = mpsc::sync_channel(1);
        {
            let mut pending = lock(&self.shared.pending);
            let Some(pending) = pending.as_mut() else {
                return Err(CallError::Ended);
            };
            pending.insert(
                id,
                Waiting {
                    answer,
                    most: most.min(MAX_MESSAGE_BYTES),
                },
            );
        }
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            message["sessionId"] = Value::from(session);
        }
        let mut bytes = serde_json::to_vec(&message).expect("a JSON value serialises");
        bytes.push(0);
        let written = {
            let mut to_engine = lock(&self.to_engine);
            to_engine.write_all(&bytes).and_then(|()| to_engine.flush())
        };
        if let Err(why) = written {
            self.forget(id);
            return Err(CallError::Write(why));
        }
        match waiting.recv_timeout(timeout) {
            Ok(answer) => answer,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.forget(id);
                Err(CallError::TimedOut)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(CallError::Ended),
        }
    }

    /// How many events were dropped because the queue was full.
    pub fn dropped_events(&self) -> u64 {
        self.shared.dropped.load(Ordering::Relaxed)
    }

    fn forget(&self, id: u64) {
        if let Some(pending) = lock(&self.shared.pending).as_mut() {
            pending.remove(&id);
        }
    }
}

fn lock<T: ?Sized>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Events the runtime sends that nothing reads: what a page writes to its console, and its
/// uncaught errors.
const UNREAD_EVENTS: [&str; 4] = [
    "Runtime.consoleAPICalled",
    "Runtime.exceptionThrown",
    "Runtime.exceptionRevoked",
    "Runtime.inspectRequested",
];

/// The id a message's first bytes give it, as the engine writes an answer: `{"id":N,` first.
fn id_of(head: &[u8]) -> Option<u64> {
    let rest = head.strip_prefix(br#"{"id":"#)?;
    let digits = rest.iter().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 || rest.get(digits) != Some(&b',') {
        return None;
    }
    std::str::from_utf8(&rest[..digits]).ok()?.parse().ok()
}

/// The most bytes the message starting `head` may take: its call's bound, or
/// [`MAX_MESSAGE_BYTES`] for an event or an answer no call waits for.
fn most_for(shared: &Shared, head: &[u8]) -> usize {
    id_of(head)
        .and_then(|id| {
            lock(&shared.pending)
                .as_ref()
                .and_then(|pending| pending.get(&id).map(|waiting| waiting.most))
        })
        .unwrap_or(MAX_MESSAGE_BYTES)
}

/// The method a message's first bytes name, as the engine writes an event: `{"method":"X"` first.
fn method_of(head: &[u8]) -> Option<&str> {
    let rest = head.strip_prefix(br#"{"method":""#)?;
    let end = rest.iter().position(|&b| b == b'"')?;
    std::str::from_utf8(&rest[..end]).ok()
}

/// Events past their bound that may be dropped: kinds nobody here reads.
const DROPPED_WHEN_TOO_LARGE: [&str; 5] = [
    "Runtime.consoleAPICalled",
    "Runtime.exceptionThrown",
    "Runtime.exceptionRevoked",
    "Runtime.inspectRequested",
    "Log.entryAdded",
];

/// A message past its bound, which began `head`: the call it answers fails, alone, and an
/// answer to no call waiting, or an event of a kind nobody reads, is dropped. Any other event
/// ends the connection: false.
fn too_large(shared: &Shared, head: &[u8]) -> bool {
    if let Some(id) = id_of(head) {
        let waiting = lock(&shared.pending).as_mut().and_then(|p| p.remove(&id));
        if let Some(waiting) = waiting {
            let _ = waiting
                .answer
                .try_send(Err(CallError::TooLarge { most: waiting.most }));
        }
        return true;
    }
    method_of(head).is_some_and(|method| DROPPED_WHEN_TOO_LARGE.contains(&method))
}

/// Reads messages until the pipe ends or carries something that is not one, then fails every
/// waiting call and refuses every later one.
fn read_until_ended(from_engine: impl Read, shared: &Shared, events: &SyncSender<Event>) {
    let mut frames = Frames::new(from_engine);
    while let Some(Ok(frame)) = frames.next_message(|head| most_for(shared, head)) {
        let message = match frame {
            Frame::Whole(message) => message,
            Frame::Over { head } if too_large(shared, &head) => continue,
            Frame::Over { .. } => break,
        };
        if message.len() > most_for(shared, &message) {
            if too_large(shared, &message[..message.len().min(HEAD_BYTES)]) {
                continue;
            }
            break;
        }
        let Ok(Value::Object(mut message)) = serde_json::from_slice::<Value>(&message) else {
            break;
        };
        match message.get("id").and_then(Value::as_u64) {
            Some(id) => {
                let answer = match message.remove("error") {
                    Some(error) => Err(CallError::Engine {
                        code: error
                            .get("code")
                            .and_then(Value::as_i64)
                            .unwrap_or_default(),
                        message: error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    }),
                    None => Ok(message.remove("result").unwrap_or(Value::Null)),
                };
                let waiting = lock(&shared.pending).as_mut().and_then(|p| p.remove(&id));
                if let Some(waiting) = waiting {
                    let _ = waiting.answer.try_send(answer);
                }
            }
            None => {
                let Some(Value::String(method)) = message.remove("method") else {
                    break;
                };
                // What a page writes to its console nobody here reads: never queued, so it
                // never takes the place of an event the driver needs (#2138).
                if UNREAD_EVENTS.contains(&method.as_str()) {
                    continue;
                }
                let event = Event {
                    method,
                    params: message.remove("params").unwrap_or(Value::Null),
                    session_id: message
                        .remove("sessionId")
                        .and_then(|s| s.as_str().map(str::to_owned)),
                };
                match events.try_send(event) {
                    Ok(()) | Err(mpsc::TrySendError::Disconnected(_)) => {}
                    Err(mpsc::TrySendError::Full(_)) => {
                        shared.dropped.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
    }
    // Dropping every waiting sender wakes each call with `Ended`.
    lock(&shared.pending).take();
}

/// A message read whole, or one read past because it went past its bound.
enum Frame {
    Whole(Vec<u8>),
    /// Past its bound: only its first [`HEAD_BYTES`] were kept.
    Over {
        head: Vec<u8>,
    },
}

/// NUL-ended messages out of a byte stream, none held past its bound.
struct Frames<R> {
    from: R,
    buffer: Vec<u8>,
    /// How much of `buffer` is known to hold no NUL.
    scanned: usize,
}

impl<R: Read> Frames<R> {
    fn new(from: R) -> Self {
        Self {
            from,
            buffer: Vec::new(),
            scanned: 0,
        }
    }

    /// The next message, `None` at the end of the stream, or an error for a read that failed
    /// or a message past its bound that is no DevTools message. `most` says, from a message's
    /// first bytes, how long it may be: past that it is read past to its NUL, never kept.
    fn next_message(&mut self, most: impl Fn(&[u8]) -> usize) -> Option<io::Result<Frame>> {
        loop {
            if let Some(at) = self.buffer[self.scanned..].iter().position(|&b| b == 0) {
                let end = self.scanned + at;
                let rest = self.buffer.split_off(end + 1);
                let mut message = std::mem::replace(&mut self.buffer, rest);
                message.truncate(end);
                self.scanned = 0;
                return Some(Ok(Frame::Whole(message)));
            }
            self.scanned = self.buffer.len();
            if self.buffer.len() > most(&self.buffer) {
                return self.read_past();
            }
            match self.fill() {
                Ok(true) => {}
                Ok(false) => return None,
                Err(why) => return Some(Err(why)),
            }
        }
    }

    /// Reads past the message in `buffer`, which is past its bound, to its NUL, keeping only its
    /// head. JSON never carries a raw NUL, so the next one ends it.
    fn read_past(&mut self) -> Option<io::Result<Frame>> {
        let head = self.buffer[..self.buffer.len().min(HEAD_BYTES)].to_vec();
        if !head.starts_with(b"{") {
            return Some(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a message from the engine past the bound is no DevTools message",
            )));
        }
        // What was held of the message is let go, not kept as capacity.
        self.buffer = Vec::new();
        self.scanned = 0;
        loop {
            match self.fill() {
                Ok(true) => {}
                Ok(false) => return None,
                Err(why) => return Some(Err(why)),
            }
            if let Some(at) = self.buffer.iter().position(|&b| b == 0) {
                self.buffer.drain(..=at);
                return Some(Ok(Frame::Over { head }));
            }
            self.buffer.clear();
        }
    }

    /// Reads what the stream has next onto `buffer`: false at its end.
    fn fill(&mut self) -> io::Result<bool> {
        let mut chunk = [0_u8; 64 * 1024];
        loop {
            match self.from.read(&mut chunk) {
                Ok(0) => return Ok(false),
                Ok(n) => {
                    self.buffer.extend_from_slice(&chunk[..n]);
                    return Ok(true);
                }
                Err(why) if why.kind() == io::ErrorKind::Interrupted => {}
                Err(why) => return Err(why),
            }
        }
    }
}
