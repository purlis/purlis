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
//! dropped where nobody reads its kind (what a page writes to its console, its uncaught
//! errors) and where it is a network event, which the driver reads only to tell when a page has
//! settled (#2745); any other event past it ends the connection, since the driver's view of the
//! page's frames and targets is kept from events and is never left stale.
//!
//! **What JSON allows is read** (#2745). The engine writes each character of a JavaScript string
//! past ASCII as a `\uXXXX` escape, so a string holding half of a surrogate pair (an emoji a
//! script cut in two) arrives as a lone `\ud83d`: valid JSON that a strict reader refuses. Such a
//! half, and any byte that is not UTF-8, is read as U+FFFD. A message that still does not read
//! as JSON ends the connection.
//!
//! **Every call ends within its limit, writing included** (#2123). A call's message is handed to
//! a writer of its own, on a thread of its own, and the call waits for its answer only as long
//! as its limit: an engine that stops reading fills the pipe and holds the writer, never the
//! call, and never another call. At most [`MAX_QUEUED_CALLS`] wait to be written; past that a
//! call fails at once, as one not answered in time. A call whose limit passed before its turn
//! is not written at all: what its caller was told failed never reaches the engine.
//!
//! **A call on a session that detaches ends as it detaches** (#3081). The engine does not
//! always answer a call made on a frame's session that goes away while the call waits (a frame
//! that loads another document every 100 ms detaches its session each time): as the engine
//! says the session detached, every call still waiting on it fails at once, refused as the
//! engine refuses a call to a session it does not have, and never waits out its limit.
//!
//! **A tapped session is relayed, whole and in order** (#2028): an Inspect's DevTools speak to
//! the inspected page over this one pipe. [`Connection::tap`] hands every message the engine
//! writes on a session, and on each session attached under it, to the tap rather than the
//! driver's events, in the order the engine wrote them, console events included; a call sent on
//! the tap ([`Connection::send_tapped`]) is answered to it under the tap's own id. A message of a
//! tap past its bound is dropped for the tap, never closing the connection, and a tap that takes
//! no more ends.
//!
//! **Why it closed is kept** ([`Connection::closed`], [`Closed`]): the end of the pipe, a read
//! that failed, or the message that ended it, named by its method and never by what it held.

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

/// How much of the end of a message past its bound is kept, to tell which session an event is
/// on (#2028).
const TAIL_BYTES: usize = 256;

/// The session a message names at its end (`,"sessionId":"S"}`), as the engine writes an event.
fn session_at_end(tail: &[u8]) -> Option<&str> {
    const KEY: &[u8] = br#""sessionId":""#;
    let at = tail.windows(KEY.len()).rposition(|w| w == KEY)? + KEY.len();
    let rest = &tail[at..];
    let end = rest.iter().position(|&b| b == b'"')?;
    if rest.get(end + 1..) != Some(b"}".as_slice()) {
        return None;
    }
    std::str::from_utf8(&rest[..end]).ok()
}

/// How many events wait for the broker before more are dropped (and counted).
pub const MAX_QUEUED_EVENTS: usize = 1024;

/// How many calls wait to be written before another fails at once (#2123).
pub const MAX_QUEUED_CALLS: usize = 256;

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

/// **Why the connection closed** (#2745), for the host's log: it names a message by its
/// DevTools method alone, never by anything it held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Closed {
    /// The engine closed its end of the pipe: it ended, or is ending.
    PipeEnded,
    /// Reading the pipe failed.
    ReadFailed(io::ErrorKind),
    /// A message past its bound that may not be dropped: an event of a kind the driver reads,
    /// by its method where its first bytes name one.
    TooLarge {
        /// The event's method.
        method: Option<String>,
        /// The bound it went past.
        most: usize,
    },
    /// A message that does not read as DevTools JSON: an event's method, or the method of the
    /// call an answer was for, where purlis can tell.
    NotDevTools {
        /// The method.
        method: Option<String>,
    },
}

impl std::fmt::Display for Closed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let named = |method: &Option<String>| match method {
            Some(method) => format!(" ({method})"),
            None => String::new(),
        };
        match self {
            Closed::PipeEnded => write!(f, "the engine closed its DevTools pipe"),
            Closed::ReadFailed(kind) => {
                write!(f, "reading the engine's DevTools pipe failed: {kind}")
            }
            Closed::TooLarge { method, most } => write!(
                f,
                "the engine sent an event{} longer than {most} bytes, which purlis does not read",
                named(method)
            ),
            Closed::NotDevTools { method } => write!(
                f,
                "the engine sent a message{} that does not read as DevTools JSON",
                named(method)
            ),
        }
    }
}

type Answer = Result<Value, CallError>;

/// **One message of a tapped session** (#2028), as the engine wrote it.
#[derive(Debug, Clone, PartialEq)]
pub enum Tapped {
    /// An event of the tapped session or of one attached under it.
    Event(Event),
    /// The answer to a call sent on the tap: its result, or the error object as the engine
    /// wrote it.
    Answer {
        /// The id the tap gave the call ([`Connection::send_tapped`]).
        id: u64,
        outcome: Result<Value, Value>,
    },
    /// A message of the tapped session went past its bound: it was read past, and the tap
    /// ends, never the connection.
    TooLarge,
}

/// Where a tap's messages go: false when it takes no more, which ends the tap.
pub type TapSink = Arc<dyn Fn(Tapped) -> bool + Send + Sync>;

/// A tap of one session and those attached under it ([`Connection::tap`]).
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Tap(u64);

impl Tap {
    /// A stand-in engine's tap, numbered `id`.
    #[cfg(test)]
    pub(crate) fn for_a_test(id: u64) -> Tap {
        Tap(id)
    }

    /// Its number.
    #[cfg(test)]
    pub(crate) fn number(&self) -> u64 {
        self.0
    }
}

/// One tap, as the reader routes to it.
struct Tapping {
    id: u64,
    sessions: std::collections::HashSet<String>,
    sink: TapSink,
    /// The connection's id of each call sent on the tap, and the tap's own id for it.
    calls: HashMap<u64, u64>,
}

/// A call waiting for its answer, the most bytes the answer may take, its method, to name an
/// answer that does not read, and the session it was made on.
struct Waiting {
    answer: SyncSender<Answer>,
    most: usize,
    method: String,
    session: Option<String>,
}

/// The code the engine refuses a call to a session it does not have with.
const NO_SUCH_SESSION: i64 = -32001;

struct Shared {
    /// `None` once the engine's side has ended: every later call is refused at once.
    pending: Mutex<Option<HashMap<u64, Waiting>>>,
    /// Events dropped because [`MAX_QUEUED_EVENTS`] were already waiting.
    dropped: AtomicU64,
    /// Why the connection closed, once it has.
    closed: Mutex<Option<Closed>>,
    /// The taps (#2028).
    taps: Mutex<Vec<Tapping>>,
}

/// A call's message on its way to the engine's pipe, by the call's id.
struct Outgoing {
    id: u64,
    bytes: Vec<u8>,
    /// Sent on a tap: written whatever waits, its answer the tap's.
    tapped: bool,
}

/// One DevTools connection over the engine's pipe.
pub struct Connection {
    to_engine: SyncSender<Outgoing>,
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
            closed: Mutex::new(None),
            taps: Mutex::new(Vec::new()),
        });
        let (events, receiver) = mpsc::sync_channel(MAX_QUEUED_EVENTS);
        let reading = shared.clone();
        thread::Builder::new()
            .name("engine-devtools-reader".to_owned())
            .spawn(move || read_until_ended(from_engine, &reading, &events))
            .expect("a thread to read the engine's pipe");
        let (outgoing, to_write) = mpsc::sync_channel(MAX_QUEUED_CALLS);
        let writing = shared.clone();
        thread::Builder::new()
            .name("engine-devtools-writer".to_owned())
            .spawn(move || write_until_dropped(to_engine, &to_write, &writing))
            .expect("a thread to write to the engine's pipe");
        (
            Self {
                to_engine: outgoing,
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
                    method: method.to_owned(),
                    session: session.map(str::to_owned),
                },
            );
        }
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            message["sessionId"] = Value::from(session);
        }
        let mut bytes = serde_json::to_vec(&message).expect("a JSON value serialises");
        bytes.push(0);
        // Written by the writer; this call waits only for its answer, within its limit, however
        // long the writing takes (#2123).
        match self.to_engine.try_send(Outgoing {
            id,
            bytes,
            tapped: false,
        }) {
            Ok(()) => {}
            Err(mpsc::TrySendError::Full(_)) => {
                self.forget(id);
                return Err(CallError::TimedOut);
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.forget(id);
                return Err(CallError::Ended);
            }
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

    /// Why the connection closed, once it has (#2745); `None` while it reads.
    pub fn closed(&self) -> Option<Closed> {
        lock(&self.shared.closed).clone()
    }

    /// How many events were dropped because the queue was full.
    pub fn dropped_events(&self) -> u64 {
        self.shared.dropped.load(Ordering::Relaxed)
    }

    /// **Taps `session`** (#2028): every message the engine writes on it, and on each session
    /// attached under it, goes to `sink` from now, in order, and none to the events receiver,
    /// until [`Connection::untap`], the engine says the session detached, or `sink` takes no
    /// more.
    pub fn tap(&self, session: &str, sink: TapSink) -> Tap {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        lock(&self.shared.taps).push(Tapping {
            id,
            sessions: std::iter::once(session.to_owned()).collect(),
            sink,
            calls: HashMap::new(),
        });
        Tap(id)
    }

    /// Ends `tap`: its sessions' messages are the events receiver's again.
    pub fn untap(&self, tap: &Tap) {
        lock(&self.shared.taps).retain(|one| one.id != tap.0);
    }

    /// Writes a call of `method` on `session`, one of `tap`'s, without waiting: its answer goes
    /// to the tap under `id`. Refused where the session is not the tap's, and where the call
    /// cannot be queued to be written.
    pub fn send_tapped(
        &self,
        tap: &Tap,
        id: u64,
        method: &str,
        params: Value,
        session: &str,
    ) -> Result<(), CallError> {
        let own = self.next_id.fetch_add(1, Ordering::Relaxed);
        {
            let mut taps = lock(&self.shared.taps);
            let Some(tapping) = taps
                .iter_mut()
                .find(|one| one.id == tap.0 && one.sessions.contains(session))
            else {
                return Err(CallError::Engine {
                    code: NO_SUCH_SESSION,
                    message: "the session is not this tap's".to_owned(),
                });
            };
            tapping.calls.insert(own, id);
        }
        let message =
            json!({ "id": own, "method": method, "params": params, "sessionId": session });
        let mut bytes = serde_json::to_vec(&message).expect("a JSON value serialises");
        bytes.push(0);
        match self.to_engine.try_send(Outgoing {
            id: own,
            bytes,
            tapped: true,
        }) {
            Ok(()) => Ok(()),
            Err(why) => {
                if let Some(tapping) = lock(&self.shared.taps)
                    .iter_mut()
                    .find(|one| one.id == tap.0)
                {
                    tapping.calls.remove(&own);
                }
                Err(match why {
                    mpsc::TrySendError::Full(_) => CallError::TimedOut,
                    mpsc::TrySendError::Disconnected(_) => CallError::Ended,
                })
            }
        }
    }

    fn forget(&self, id: u64) {
        if let Some(pending) = lock(&self.shared.pending).as_mut() {
            pending.remove(&id);
        }
    }
}

/// Writes each call's message to the engine, in the order they were made, until the connection
/// is dropped; a call that failed before its turn is not written. A write that fails fails its call, and every later one, with why (#2123): what
/// follows a broken write would reach the engine cut.
fn write_until_dropped(mut to_engine: impl Write, calls: &Receiver<Outgoing>, shared: &Shared) {
    let mut broken: Option<io::ErrorKind> = None;
    for call in calls {
        // A call that already failed (its limit passed while it waited) is never written: what
        // its caller was told failed never reaches the engine (#2123).
        let waiting = call.tapped
            || lock(&shared.pending)
                .as_ref()
                .is_some_and(|pending| pending.contains_key(&call.id));
        if !waiting {
            continue;
        }
        probe_trace("->", &call.bytes);
        let failed = match broken {
            Some(kind) => Some(io::Error::from(kind)),
            None => to_engine
                .write_all(&call.bytes)
                .and_then(|()| to_engine.flush())
                .err(),
        };
        if let Some(why) = failed {
            broken.get_or_insert(why.kind());
            let waiting = lock(&shared.pending)
                .as_mut()
                .and_then(|pending| pending.remove(&call.id));
            if let Some(waiting) = waiting {
                let _ = waiting.answer.try_send(Err(CallError::Write(why)));
            }
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

/// The domain whose events past their bound are dropped too (#2745): the driver reads a
/// request's events only to tell when its page has settled, and the reach proxy judges each
/// request itself.
const NETWORK_EVENTS: &str = "Network.";

/// Whether an event of `method` past its bound may be dropped.
fn droppable(method: &str) -> bool {
    DROPPED_WHEN_TOO_LARGE.contains(&method) || method.starts_with(NETWORK_EVENTS)
}

/// `method` where it is a DevTools method's name (`Domain.name`, letters and digits): nothing a
/// page wrote is ever repeated in the host's log.
pub(crate) fn method_name(method: &str) -> Option<String> {
    let plain = method.len() <= 64
        && method.split_once('.').is_some_and(|(domain, name)| {
            !domain.is_empty()
                && !name.is_empty()
                && domain.chars().all(|c| c.is_ascii_alphanumeric())
                && name.chars().all(|c| c.is_ascii_alphanumeric())
        });
    plain.then(|| method.to_owned())
}

/// A message past its bound, which began `head`: the call it answers fails, alone, and an
/// answer to no call waiting, or an event that may be dropped, is dropped. Any other event
/// ends the connection, and why is said: false.
fn too_large(shared: &Shared, head: &[u8], tail: &[u8]) -> bool {
    // An event names its session at its end: one of a tap's ends the tap alone (#2028).
    if id_of(head).is_none()
        && let Some(session) = session_at_end(tail)
    {
        let mut taps = lock(&shared.taps);
        if let Some(at) = taps.iter().position(|one| one.sessions.contains(session)) {
            let gone = taps.remove(at);
            drop(taps);
            (gone.sink)(Tapped::TooLarge);
            return true;
        }
    }
    if let Some(id) = id_of(head)
        && tapped_answer(
            shared,
            id,
            Err(json!({ "code": -32000, "message": "the answer was too large to relay" })),
        )
    {
        return true;
    }
    if let Some(id) = id_of(head) {
        let waiting = lock(&shared.pending).as_mut().and_then(|p| p.remove(&id));
        if let Some(waiting) = waiting {
            let _ = waiting
                .answer
                .try_send(Err(CallError::TooLarge { most: waiting.most }));
        }
        return true;
    }
    let method = method_of(head);
    if method.is_some_and(droppable) {
        return true;
    }
    close(
        shared,
        Closed::TooLarge {
            method: method.and_then(method_name),
            most: most_for(shared, head),
        },
    );
    false
}

/// Keeps why the connection closed: the first reason given.
fn close(shared: &Shared, why: Closed) {
    lock(&shared.closed).get_or_insert(why);
}

/// The method a message that does not read names: an event's, or that of the call an answer
/// was for.
fn method_of_unread(shared: &Shared, message: &[u8]) -> Option<String> {
    let head = &message[..message.len().min(HEAD_BYTES)];
    if let Some(id) = id_of(head) {
        return lock(&shared.pending)
            .as_ref()
            .and_then(|pending| pending.get(&id).map(|waiting| waiting.method.clone()));
    }
    method_of(head).and_then(method_name)
}

/// `message` as JSON, read as JSON allows (#2745): where the strict reader refuses it, again with
/// every half of a surrogate pair and every byte that is not UTF-8 as U+FFFD ([`repaired`]).
fn parsed(message: &[u8]) -> Option<serde_json::Map<String, Value>> {
    match serde_json::from_slice::<Value>(message) {
        Ok(Value::Object(message)) => return Some(message),
        Ok(_) => return None,
        Err(_) => {}
    }
    match serde_json::from_slice::<Value>(&repaired(message)) {
        Ok(Value::Object(message)) => Some(message),
        _ => None,
    }
}

/// The four hex digits at `at` in `bytes`, as a UTF-16 unit.
fn unit_at(bytes: &[u8], at: usize) -> Option<u16> {
    let digits = std::str::from_utf8(bytes.get(at..at + 4)?).ok()?;
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u16::from_str_radix(digits, 16).ok()
}

/// **`message` with every `\u` escape of half a surrogate pair that has no other half, and every
/// byte that is not UTF-8, made U+FFFD** (#2745): as `String::from_utf16_lossy` reads a string
/// holding such a half. A leading half followed by a trailing one is a whole pair and is kept; a
/// backslash escaped by another (`\\ud83d`) is text, not an escape.
pub(crate) fn repaired(message: &[u8]) -> Vec<u8> {
    const REPLACEMENT: &[u8] = b"\\ufffd";
    let leading = |unit: u16| (0xD800..0xDC00).contains(&unit);
    let trailing = |unit: u16| (0xDC00..0xE000).contains(&unit);
    let text = String::from_utf8_lossy(message);
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut in_string = false;
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        if !in_string || byte != b'\\' {
            if byte == b'"' {
                in_string = !in_string;
            }
            out.push(byte);
            at += 1;
            continue;
        }
        // An escape inside a string.
        let unit = (bytes.get(at + 1) == Some(&b'u'))
            .then(|| unit_at(bytes, at + 2))
            .flatten();
        match unit {
            Some(unit) if leading(unit) => {
                let pair = bytes.get(at + 6..at + 8) == Some(br"\u".as_slice())
                    && unit_at(bytes, at + 8).is_some_and(trailing);
                if pair {
                    out.extend_from_slice(&bytes[at..at + 12]);
                    at += 12;
                } else {
                    out.extend_from_slice(REPLACEMENT);
                    at += 6;
                }
            }
            Some(unit) if trailing(unit) => {
                out.extend_from_slice(REPLACEMENT);
                at += 6;
            }
            _ => {
                // Any other escape: the backslash and what it escapes, as they are.
                out.push(byte);
                if let Some(&escaped) = bytes.get(at + 1) {
                    out.push(escaped);
                }
                at += 2;
            }
        }
    }
    out
}

/// Reads messages until the pipe ends or carries something that is not one, then fails every
/// waiting call and refuses every later one, keeping why it stopped.
/// [DEBUG-4472] Probe only.
pub(crate) fn probe_trace(way: &str, bytes: &[u8]) {
    use std::io::Write as _;
    let Some(path) = std::env::var_os("PURLIS_PROBE_TRACE") else {
        return;
    };
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(260)]).replace('\n', " ");
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{at} {way} {text}");
    }
}

fn read_until_ended(from_engine: impl Read, shared: &Shared, events: &SyncSender<Event>) {
    let mut frames = Frames::new(from_engine);
    loop {
        let frame = match frames.next_message(|head| most_for(shared, head)) {
            None => {
                close(shared, Closed::PipeEnded);
                break;
            }
            Some(Err(why)) if why.kind() == io::ErrorKind::InvalidData => {
                close(shared, Closed::NotDevTools { method: None });
                break;
            }
            Some(Err(why)) => {
                close(shared, Closed::ReadFailed(why.kind()));
                break;
            }
            Some(Ok(frame)) => frame,
        };
        let message = match frame {
            Frame::Whole(message) => {
                probe_trace("<-", &message);
                message
            }
            Frame::Over { head, tail } if too_large(shared, &head, &tail) => continue,
            Frame::Over { .. } => break,
        };
        if message.len() > most_for(shared, &message) {
            let tail = &message[message.len().saturating_sub(TAIL_BYTES)..];
            if too_large(shared, &message[..message.len().min(HEAD_BYTES)], tail) {
                continue;
            }
            break;
        }
        let Some(mut read) = parsed(&message) else {
            close(
                shared,
                Closed::NotDevTools {
                    method: method_of_unread(shared, &message),
                },
            );
            break;
        };
        match read.get("id").and_then(Value::as_u64) {
            Some(id) => {
                let outcome = match read.remove("error") {
                    Some(error) => Err(error),
                    None => Ok(read.remove("result").unwrap_or(Value::Null)),
                };
                let waiting = lock(&shared.pending).as_mut().and_then(|p| p.remove(&id));
                if let Some(waiting) = waiting {
                    let answer = outcome.map_err(|error| CallError::Engine {
                        code: error
                            .get("code")
                            .and_then(Value::as_i64)
                            .unwrap_or_default(),
                        message: error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    });
                    let _ = waiting.answer.try_send(answer);
                } else {
                    tapped_answer(shared, id, outcome);
                }
            }
            None => {
                let Some(Value::String(method)) = read.remove("method") else {
                    close(shared, Closed::NotDevTools { method: None });
                    break;
                };
                let event = Event {
                    method,
                    params: read.remove("params").unwrap_or(Value::Null),
                    session_id: read
                        .remove("sessionId")
                        .and_then(|s| s.as_str().map(str::to_owned)),
                };
                if event.method == "Target.detachedFromTarget"
                    && let Some(gone) = event.params["sessionId"].as_str()
                {
                    detached(shared, gone);
                }
                // A tapped session's messages are the tap's, its console's included (#2028).
                let Some(event) = tapped_event(shared, event) else {
                    continue;
                };
                // What a page writes to its console nobody here reads: never queued, so it
                // never takes the place of an event the driver needs (#2138).
                if UNREAD_EVENTS.contains(&event.method.as_str()) {
                    continue;
                }
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

/// Hands the answer to the connection's call `id` to the tap that sent it: false where no tap
/// did.
fn tapped_answer(shared: &Shared, id: u64, outcome: Result<Value, Value>) -> bool {
    let mut taps = lock(&shared.taps);
    let Some(at) = taps.iter().position(|one| one.calls.contains_key(&id)) else {
        return false;
    };
    let own = taps[at].calls.remove(&id).unwrap_or_default();
    if !(taps[at].sink)(Tapped::Answer { id: own, outcome }) {
        taps.remove(at);
    }
    true
}

/// Hands `event` to the tap whose session it is on, or whose session it says detached, keeping
/// the tap's sessions as the engine attaches and detaches them under it: the event back where no
/// tap takes it.
fn tapped_event(shared: &Shared, event: Event) -> Option<Event> {
    let mut taps = lock(&shared.taps);
    let gone = (event.method == "Target.detachedFromTarget")
        .then(|| event.params["sessionId"].as_str().map(str::to_owned))
        .flatten();
    let at = taps.iter().position(|one| {
        event
            .session_id
            .as_ref()
            .is_some_and(|session| one.sessions.contains(session))
            || gone
                .as_ref()
                .is_some_and(|gone| one.sessions.contains(gone))
    });
    let Some(at) = at else {
        return Some(event);
    };
    if event.method == "Target.attachedToTarget"
        && let Some(under) = event.params["sessionId"].as_str()
    {
        taps[at].sessions.insert(under.to_owned());
    }
    if let Some(gone) = &gone {
        taps[at].sessions.remove(gone);
    }
    if !(taps[at].sink)(Tapped::Event(event)) || taps[at].sessions.is_empty() {
        taps.remove(at);
    }
    None
}

/// The engine detached session `gone`: every call still waiting on it is refused now, as the
/// engine refuses one to a session it does not have (#3081).
fn detached(shared: &Shared, gone: &str) {
    let waiting: Vec<Waiting> = match lock(&shared.pending).as_mut() {
        Some(pending) => {
            let ids: Vec<u64> = pending
                .iter()
                .filter(|(_, one)| one.session.as_deref() == Some(gone))
                .map(|(id, _)| *id)
                .collect();
            ids.iter().filter_map(|id| pending.remove(id)).collect()
        }
        None => Vec::new(),
    };
    for one in waiting {
        let _ = one.answer.try_send(Err(CallError::Engine {
            code: NO_SUCH_SESSION,
            message: format!(
                "{} was not answered: the session it was made on detached",
                one.method
            ),
        }));
    }
}

/// A message read whole, or one read past because it went past its bound.
enum Frame {
    Whole(Vec<u8>),
    /// Past its bound: only its first [`HEAD_BYTES`] were kept.
    Over {
        head: Vec<u8>,
        /// Its last [`TAIL_BYTES`], where an event names its session.
        tail: Vec<u8>,
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
        let mut tail = Vec::new();
        let keep = |tail: &mut Vec<u8>, read: &[u8]| {
            tail.extend_from_slice(read);
            let cut = tail.len().saturating_sub(TAIL_BYTES);
            tail.drain(..cut);
        };
        keep(&mut tail, &self.buffer);
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
                keep(&mut tail, &self.buffer[..at]);
                self.buffer.drain(..=at);
                return Some(Ok(Frame::Over { head, tail }));
            }
            keep(&mut tail, &self.buffer);
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
