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
//! the broker's business. A message longer than [`MAX_MESSAGE_BYTES`] ends the connection
//! rather than growing the host's memory, and so does any message that is not JSON.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

/// The longest message read from the engine, NUL excluded: 64 MiB, room for a full-page
/// screenshot in base64.
pub const MAX_MESSAGE_BYTES: usize = 64 << 20;

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
}

type Answer = Result<Value, CallError>;

struct Shared {
    /// `None` once the engine's side has ended: every later call is refused at once.
    pending: Mutex<Option<HashMap<u64, SyncSender<Answer>>>>,
}

/// One DevTools connection over the engine's pipe.
pub struct Connection {
    to_engine: Mutex<Box<dyn Write + Send>>,
    next_id: AtomicU64,
    shared: Arc<Shared>,
}

impl Connection {
    /// Starts reading `from_engine` on a thread of its own, and returns the connection and the
    /// receiver of the engine's events. Events queue until read; dropping the receiver discards
    /// them.
    pub fn new<R, W>(from_engine: R, to_engine: W) -> (Self, Receiver<Event>)
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
    {
        let shared = Arc::new(Shared {
            pending: Mutex::new(Some(HashMap::new())),
        });
        let (events, receiver) = mpsc::channel();
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
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (answer, waiting) = mpsc::sync_channel(1);
        {
            let mut pending = lock(&self.shared.pending);
            let Some(pending) = pending.as_mut() else {
                return Err(CallError::Ended);
            };
            pending.insert(id, answer);
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

/// Reads messages until the pipe ends or carries something that is not one, then fails every
/// waiting call and refuses every later one.
fn read_until_ended(from_engine: impl Read, shared: &Shared, events: &Sender<Event>) {
    let mut frames = Frames::new(from_engine);
    while let Some(Ok(message)) = frames.next_message() {
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
                    let _ = waiting.try_send(answer);
                }
            }
            None => {
                let Some(Value::String(method)) = message.remove("method") else {
                    break;
                };
                let event = Event {
                    method,
                    params: message.remove("params").unwrap_or(Value::Null),
                    session_id: message
                        .remove("sessionId")
                        .and_then(|s| s.as_str().map(str::to_owned)),
                };
                // Nobody listening is not an error: the event is discarded.
                let _ = events.send(event);
            }
        }
    }
    // Dropping every waiting sender wakes each call with `Ended`.
    lock(&shared.pending).take();
}

/// NUL-ended messages out of a byte stream, each at most [`MAX_MESSAGE_BYTES`].
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

    /// The next message, `None` at the end of the stream, or an error for a message past the
    /// bound or a read that failed.
    fn next_message(&mut self) -> Option<io::Result<Vec<u8>>> {
        loop {
            if let Some(at) = self.buffer[self.scanned..].iter().position(|&b| b == 0) {
                let end = self.scanned + at;
                let rest = self.buffer.split_off(end + 1);
                let mut message = std::mem::replace(&mut self.buffer, rest);
                message.truncate(end);
                self.scanned = 0;
                return Some(Ok(message));
            }
            self.scanned = self.buffer.len();
            if self.buffer.len() > MAX_MESSAGE_BYTES {
                return Some(Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "a message from the engine is longer than the bound",
                )));
            }
            let mut chunk = [0_u8; 64 * 1024];
            match self.from.read(&mut chunk) {
                Ok(0) => return None,
                Ok(n) => self.buffer.extend_from_slice(&chunk[..n]),
                Err(why) if why.kind() == io::ErrorKind::Interrupted => {}
                Err(why) => return Some(Err(why)),
            }
        }
    }
}
