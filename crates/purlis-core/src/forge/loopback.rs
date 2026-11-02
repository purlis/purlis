// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The loopback listener a browser sign-in's redirect comes back to (ADR 0077 §1, RFC 8252
//! §7.3): `http://127.0.0.1:<port>/purlis/signin`, on the first unused of the three ports the
//! application registered.
//!
//! **It is a network listener, so it is as small as one can be.**
//!
//! - **Loopback only.** It binds `127.0.0.1`, never every address, and drops a connection whose
//!   peer is not a loopback address without answering it.
//! - **One sign-in, one redirect.** It lives for one sign-in. The first request that carries
//!   that sign-in's `state` ([`super::pkce::read_redirect`]) is the redirect; the listener
//!   answers it and is closed, so no second request is ever read. It also closes when the
//!   person cancels and when the time allowed runs out.
//! - **A request that is not the redirect changes nothing.** Another path, another method, a
//!   `Host` that is not the listener's own address, or no `state`: each is refused with a fixed
//!   answer, and the sign-in goes on waiting. After [`MOST_STRANGERS`] of them the sign-in stops
//!   with a sentence saying so, so nothing can keep the listener answering for long.
//! - **It reflects nothing.** Every answer is one of three fixed pages. Nothing from the
//!   request is put in an answer, in an error or in a log: not the query, not a header.
//! - **It reads little, briefly.** One request head of at most [`MOST_HEAD`] bytes, within
//!   [`PER_REQUEST`]; a body is never read.
//!
//! **If all three ports are taken**, the sign-in stops with a sentence that names them. It never
//! falls back to another port: the forge would refuse a redirect address the application did
//! not register, and a port chosen at run time is one nobody reviewed.
//!
//! **What another program on the machine can do to it.** It can take one of the ports first:
//! the listener then binds the next, and the browser is sent back to the port purlis bound, not
//! the squatter's. It can take all three: the sign-in fails as above. It can call the listener:
//! without the `state` that is refused. And a code that leaks on the way is useless without the
//! verifier ([`super::pkce`]). So nothing here depends on the port being secret or free.
//!
//! The logic is written over [`Incoming`] and [`Conn`], so a test drives it with no socket.
//! [`bind`] gives the real one.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::pkce::{Code, Declined, Redirected, State, read_redirect};

/// The most one request's head may be. A redirect's is a few hundred bytes; a browser's
/// headers bring it to a kilobyte or two.
pub const MOST_HEAD: usize = 8 * 1024;

/// How long one connection has to send its request head.
pub const PER_REQUEST: Duration = Duration::from_secs(3);

/// How many requests that are not the redirect a sign-in answers before it stops.
pub const MOST_STRANGERS: usize = 32;

/// How long the listener rests between looks for a connection, a cancel and the deadline.
const BETWEEN_LOOKS: Duration = Duration::from_millis(50);

/// One connection to the listener.
pub trait Conn: Read + Write {
    /// From now on a read or a write gives up after `wait`.
    fn give_up_after(&mut self, wait: Duration) -> std::io::Result<()>;
}

/// The connections made to the listener, without waiting for one.
pub trait Incoming {
    /// The next connection and its peer's address, or `None` when nobody is connecting now.
    fn next(&mut self) -> std::io::Result<Option<(Box<dyn Conn>, SocketAddr)>>;
}

impl Conn for TcpStream {
    fn give_up_after(&mut self, wait: Duration) -> std::io::Result<()> {
        // A zero timeout means "never" to the socket.
        let wait = wait.max(Duration::from_millis(1));
        self.set_read_timeout(Some(wait))?;
        self.set_write_timeout(Some(wait))
    }
}

impl Incoming for TcpListener {
    fn next(&mut self) -> std::io::Result<Option<(Box<dyn Conn>, SocketAddr)>> {
        match self.accept() {
            Ok((stream, peer)) => {
                stream.set_nonblocking(false)?;
                Ok(Some((Box::new(stream), peer)))
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// The listener of one sign-in, bound and not yet waited on.
pub struct Listening {
    incoming: Box<dyn Incoming + Send>,
    port: u16,
}

impl Listening {
    /// The port it is bound on: the one the redirect address must name.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// A listener over `incoming`, as if bound on `port`. Only a test build has it.
    #[cfg(any(test, feature = "fenced"))]
    pub fn over(incoming: Box<dyn Incoming + Send>, port: u16) -> Listening {
        Listening { incoming, port }
    }
}

impl std::fmt::Debug for Listening {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Listening({})", self.port)
    }
}

/// Binds the first of `ports` that `bind_one` can take, in order. With none, a sentence naming
/// them all; never another port.
pub fn bind_first<L>(
    ports: &[u16],
    bind_one: &dyn Fn(u16) -> std::io::Result<L>,
) -> Result<(u16, L), String> {
    let mut other_than_in_use = None;
    for &port in ports {
        match bind_one(port) {
            Ok(bound) => return Ok((port, bound)),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {}
            Err(e) => other_than_in_use = Some(e),
        }
    }
    let named = match ports {
        [] => String::new(),
        [one] => one.to_string(),
        [most @ .., last] => format!(
            "{} and {last}",
            most.iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    Err(match other_than_in_use {
        None => format!(
            "purlis could not take the browser's answer: ports {named} on this machine are all \
             in use by other software, and the sign-in can come back on no other. Close what is \
             using one of them and sign in again, or sign in with a token instead."
        ),
        Some(e) => format!(
            "purlis could not listen on this machine for the browser's answer (127.0.0.1, ports \
             {named}): {e}. Sign in with a token instead."
        ),
    })
}

/// **A listener on a port the system picks** on `127.0.0.1`: for a redirect address made at run
/// time, which no registration lists, as a GitHub App's manifest names its own.
pub fn bind_any() -> Result<Listening, String> {
    let bound = (|| {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        Ok::<_, std::io::Error>((listener, port))
    })();
    let (listener, port) = bound.map_err(|e| {
        format!("purlis could not listen on this machine for the browser's answer: {e}.")
    })?;
    Ok(Listening {
        incoming: Box::new(listener),
        port,
    })
}

/// The real listener: the first unused of `ports` on `127.0.0.1`.
pub fn bind(ports: &[u16]) -> Result<Listening, String> {
    let (port, listener) = bind_first(ports, &|port| {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))?;
        listener.set_nonblocking(true)?;
        Ok(listener)
    })?;
    Ok(Listening {
        incoming: Box::new(listener),
        port,
    })
}

/// Why a listener stopped without a code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    /// The time allowed ran out with no redirect.
    TimedOut,
    /// The person cancelled.
    Cancelled,
    /// The sign-in's own redirect said no.
    Declined(Declined),
    /// Too many requests that were not the redirect.
    Strangers,
    /// The listener itself failed.
    Failed,
}

/// The clock a listener waits by: now, and resting for a while.
pub struct Clock<'a> {
    pub now: &'a dyn Fn() -> Instant,
    pub rest: &'a dyn Fn(Duration),
}

impl Clock<'static> {
    /// The system's clock.
    pub fn system() -> Clock<'static> {
        Clock {
            now: &Instant::now,
            rest: &std::thread::sleep,
        }
    }
}

/// The page the browser shows once the redirect is taken. Fixed text; nothing of the request.
const DONE_PAGE: &str = "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
<title>purlis</title></head><body><h1>You can return to purlis</h1>\
<p>The sign-in is finishing there. You can close this tab.</p></body></html>\n";

/// The page for a redirect that said no.
const DECLINED_PAGE: &str = "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
<title>purlis</title></head><body><h1>The sign-in did not finish</h1>\
<p>Return to purlis to try again. You can close this tab.</p></body></html>\n";

/// The page for anything else.
const NOTHING_PAGE: &str = "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
<title>purlis</title></head><body><h1>Nothing to see here</h1>\
<p>This address only takes the answer to a sign-in started in purlis.</p></body></html>\n";

/// The content policy of every page but one that must post a form on.
const NOTHING_ALLOWED: &str = "default-src 'none'";

/// One fixed answer: the status line's words and the page.
fn respond(conn: &mut dyn Conn, status: &str, page: &str) {
    respond_under(conn, status, NOTHING_ALLOWED, page);
}

/// [`respond`], the page under the content policy `policy`.
fn respond_under(conn: &mut dyn Conn, status: &str, policy: &str, page: &str) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nContent-Security-Policy: {policy}\r\n\
         X-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\n\
         Connection: close\r\n\r\n",
        page.len()
    );
    // A browser that went away gets nothing, and that changes nothing here.
    let _ = conn
        .write_all(head.as_bytes())
        .and_then(|()| conn.write_all(page.as_bytes()))
        .and_then(|()| conn.flush());
}

/// The request head, read up to its blank line: `None` when it does not end within
/// [`MOST_HEAD`] bytes or before `until`.
fn head(conn: &mut dyn Conn, until: Instant, now: &dyn Fn() -> Instant) -> Option<Vec<u8>> {
    let mut read = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        if read.windows(4).any(|w| w == b"\r\n\r\n") {
            return Some(read);
        }
        let left = until.checked_duration_since(now())?;
        if left.is_zero() || read.len() >= MOST_HEAD {
            return None;
        }
        conn.give_up_after(left).ok()?;
        let room = (MOST_HEAD - read.len()).min(chunk.len());
        match conn.read(&mut chunk[..room]) {
            Ok(0) | Err(_) => return None,
            Ok(n) => read.extend_from_slice(&chunk[..n]),
        }
    }
}

/// The request target of a `GET` whose `Host` is this listener's own address, or the status to
/// refuse it with.
fn target_of(head: &[u8], port: u16) -> Result<&str, &'static str> {
    let text = std::str::from_utf8(head).map_err(|_| "400 Bad Request")?;
    let mut lines = text.split("\r\n");
    let mut words = lines.next().unwrap_or("").split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (words.next(), words.next(), words.next(), words.next())
    else {
        return Err("400 Bad Request");
    };
    if !matches!(version, "HTTP/1.1" | "HTTP/1.0") || !target.starts_with('/') {
        return Err("400 Bad Request");
    }
    if method != "GET" {
        return Err("405 Method Not Allowed");
    }
    // The browser was sent to this exact address, so that is the `Host` it names. A page that
    // reached the listener under another name (a host name made to resolve here) is refused.
    let own = format!("127.0.0.1:{port}");
    let mut hosts = lines
        .take_while(|line| !line.is_empty())
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.eq_ignore_ascii_case("host"))
        .map(|(_, value)| value.trim());
    match (hosts.next(), hosts.next()) {
        (Some(host), None) if host == own => Ok(target),
        _ => Err("400 Bad Request"),
    }
}

/// What a request's target was to the flow listening.
pub enum Heard<T> {
    /// Not this flow's: refused with the fixed page, and it counts against [`MOST_STRANGERS`].
    NotOurs,
    /// A page for the browser, and the flow goes on waiting: `policy` is its content policy.
    Page { policy: String, page: String },
    /// The redirect it waited for: answered with `page`, and the listener closes.
    Took { value: T, page: &'static str },
    /// The redirect said no: answered with `page`, and the listener closes.
    Stop { why: Stopped, page: &'static str },
}

/// Reads and answers one connection.
fn serve<T>(
    conn: &mut dyn Conn,
    peer: SocketAddr,
    port: u16,
    until: Instant,
    now: &dyn Fn() -> Instant,
    read: &mut dyn FnMut(&str) -> Heard<T>,
) -> Heard<T> {
    if !peer.ip().is_loopback() {
        // Not answered at all: the listener is bound to the loopback, so this cannot be the
        // person's browser.
        return Heard::NotOurs;
    }
    let Some(head) = head(conn, until, now) else {
        respond(conn, "400 Bad Request", NOTHING_PAGE);
        return Heard::NotOurs;
    };
    let target = match target_of(&head, port) {
        Ok(target) => target,
        Err(status) => {
            respond(conn, status, NOTHING_PAGE);
            return Heard::NotOurs;
        }
    };
    let heard = read(target);
    match &heard {
        Heard::NotOurs => respond(conn, "404 Not Found", NOTHING_PAGE),
        Heard::Page { policy, page } => respond_under(conn, "200 OK", policy, page),
        Heard::Took { page, .. } | Heard::Stop { page, .. } => respond(conn, "200 OK", page),
    }
    heard
}

/// Waits for this sign-in's redirect on `listening` and gives its code, then closes the
/// listener: it is taken by value, so it is gone when this returns, whatever the outcome.
///
/// It stops without a code when `allowed` has passed, when `cancelled` is set, when the
/// redirect itself said no, and after [`MOST_STRANGERS`] requests that were not the redirect.
pub fn take_the_redirect(
    listening: Listening,
    state: &State,
    allowed: Duration,
    cancelled: &AtomicBool,
    clock: &Clock<'_>,
) -> Result<Code, Stopped> {
    take_with(
        listening,
        allowed,
        cancelled,
        clock,
        &mut |target| match read_redirect(target, state) {
            Redirected::Code(code) => Heard::Took {
                value: code,
                page: DONE_PAGE,
            },
            Redirected::Declined(why) => Heard::Stop {
                why: Stopped::Declined(why),
                page: DECLINED_PAGE,
            },
            Redirected::NotOurs => Heard::NotOurs,
        },
    )
}

/// **The listener's one loop**: every request that passes its checks goes to `read`, and what
/// `read` hears decides the answer. Taken by value, so the listener is gone when this returns.
pub fn take_with<T>(
    listening: Listening,
    allowed: Duration,
    cancelled: &AtomicBool,
    clock: &Clock<'_>,
    read: &mut dyn FnMut(&str) -> Heard<T>,
) -> Result<T, Stopped> {
    let Listening { mut incoming, port } = listening;
    let deadline = (clock.now)() + allowed;
    let mut strangers = 0;
    loop {
        if cancelled.load(Ordering::SeqCst) {
            return Err(Stopped::Cancelled);
        }
        let now = (clock.now)();
        if now >= deadline {
            return Err(Stopped::TimedOut);
        }
        match incoming.next() {
            Ok(Some((mut conn, peer))) => {
                let until = (now + PER_REQUEST).min(deadline);
                match serve(conn.as_mut(), peer, port, until, clock.now, read) {
                    Heard::Took { value, .. } => return Ok(value),
                    Heard::Stop { why, .. } => return Err(why),
                    Heard::Page { .. } => {}
                    Heard::NotOurs => {
                        strangers += 1;
                        if strangers >= MOST_STRANGERS {
                            return Err(Stopped::Strangers);
                        }
                    }
                }
            }
            Ok(None) => (clock.rest)(BETWEEN_LOOKS),
            Err(_) => return Err(Stopped::Failed),
        }
    }
}

/// Stand-ins for a connection and for the connections made, for this module's tests and the
/// flow's ([`super::pkce`]).
#[cfg(test)]
pub(super) mod standin {
    use std::collections::VecDeque;
    use std::io::{Read, Write};
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::{Conn, Incoming};

    /// What a stand-in connection was sent, shared with the test.
    pub type Written = Arc<Mutex<Vec<u8>>>;

    /// A connection that sends `request` and records the answer.
    pub struct Scripted {
        pub request: std::io::Cursor<Vec<u8>>,
        pub written: Written,
        /// Set when anything was read from it.
        pub touched: Arc<AtomicBool>,
        /// When set, a read gives one byte and a second passes, for ever.
        pub stalls: Option<Arc<Mutex<Duration>>>,
    }

    impl Read for Scripted {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.touched.store(true, Ordering::SeqCst);
            if let Some(passed) = &self.stalls {
                *passed.lock().unwrap() += Duration::from_secs(1);
                buf[0] = b'x';
                return Ok(1);
            }
            self.request.read(buf)
        }
    }

    impl Write for Scripted {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.written.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Conn for Scripted {
        fn give_up_after(&mut self, _: Duration) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// One queued connection, as the test reads it back.
    pub struct Visit {
        written: Written,
        touched: Arc<AtomicBool>,
    }

    impl Visit {
        pub fn answer(&self) -> String {
            String::from_utf8(self.written.lock().unwrap().clone()).unwrap()
        }
        pub fn was_read(&self) -> bool {
            self.touched.load(Ordering::SeqCst)
        }
    }

    /// The connections waiting on a stand-in listener.
    #[derive(Default)]
    pub struct Queue {
        pub waiting: VecDeque<(Box<dyn Conn + Send>, SocketAddr)>,
        pub fails: bool,
    }

    impl Queue {
        pub fn visit_from(&mut self, peer: &str, request: &str) -> Visit {
            let written = Written::default();
            let touched = Arc::new(AtomicBool::new(false));
            self.waiting.push_back((
                Box::new(Scripted {
                    request: std::io::Cursor::new(request.as_bytes().to_vec()),
                    written: written.clone(),
                    touched: touched.clone(),
                    stalls: None,
                }),
                peer.parse().unwrap(),
            ));
            Visit { written, touched }
        }

        pub fn visit(&mut self, request: &str) -> Visit {
            self.visit_from("127.0.0.1:50000", request)
        }
    }

    impl Incoming for Queue {
        fn next(&mut self) -> std::io::Result<Option<(Box<dyn Conn>, SocketAddr)>> {
            if self.fails {
                return Err(std::io::Error::other("the listener broke"));
            }
            Ok(self
                .waiting
                .pop_front()
                .map(|(conn, peer)| (conn as Box<dyn Conn>, peer)))
        }
    }

    /// A queue the test goes on adding to while the listener waits on it.
    impl Incoming for Arc<Mutex<Queue>> {
        fn next(&mut self) -> std::io::Result<Option<(Box<dyn Conn>, SocketAddr)>> {
            self.lock().unwrap().next()
        }
    }

    /// A browser's `GET` of `target` on the listener at `port`.
    pub fn get(target: &str, port: u16) -> String {
        format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: text/html\r\n\r\n")
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::{Arc, Mutex};

    use super::standin::{Queue, Scripted, Written};
    use super::*;

    const PORT: u16 = 47823;

    fn get(target: &str) -> String {
        standin::get(target, PORT)
    }

    /// Waits on `queue` for the state `s-1`, by a clock that moves only when the listener rests.
    fn waited(queue: Queue, cancelled: &AtomicBool) -> Result<Code, Stopped> {
        let start = Instant::now();
        let passed = Cell::new(Duration::ZERO);
        let now = || start + passed.get();
        let rest = |wait: Duration| passed.set(passed.get() + wait);
        take_the_redirect(
            Listening::over(Box::new(queue), PORT),
            &State::for_a_test("s-1"),
            Duration::from_secs(300),
            cancelled,
            &Clock {
                now: &now,
                rest: &rest,
            },
        )
    }

    fn no_cancel() -> AtomicBool {
        AtomicBool::new(false)
    }

    #[test]
    fn the_redirect_gives_its_code_and_the_browser_a_fixed_page() {
        let mut queue = Queue::default();
        let visit = queue.visit(&get("/purlis/signin?code=c0de-from-the-forge&state=s-1"));
        let code = waited(queue, &no_cancel()).unwrap();
        assert_eq!(code.expose(), "c0de-from-the-forge");
        let answer = visit.answer();
        assert!(answer.starts_with("HTTP/1.1 200 OK\r\n"), "{answer}");
        assert!(answer.contains("You can return to purlis"));
        for header in [
            "Cache-Control: no-store",
            "Content-Security-Policy: default-src 'none'",
            "X-Content-Type-Options: nosniff",
            "Referrer-Policy: no-referrer",
            "Connection: close",
        ] {
            assert!(answer.contains(header), "{header}");
        }
    }

    #[test]
    fn no_answer_repeats_anything_the_request_said() {
        // A marker in every part of a request a page could reflect: the path, the query's names
        // and values, a header's name and its value.
        let marked = |target: &str| {
            format!(
                "GET {target} HTTP/1.1\r\nHost: 127.0.0.1:{PORT}\r\nX-Marker-Zq9: marker-zq9\r\n\
                 Referer: http://marker-zq9.example/\r\n\r\n"
            )
        };
        let mut queue = Queue::default();
        let visits = [
            queue.visit(&marked("/marker-zq9?marker-zq9=marker-zq9")),
            queue.visit(&marked("/purlis/signin?code=marker-zq9&state=marker-zq9")),
            queue.visit(&marked(
                "/purlis/signin?marker-zq9=1&state=s-1&error=marker-zq9",
            )),
        ];
        assert_eq!(
            waited(queue, &no_cancel()).unwrap_err(),
            Stopped::Declined(Declined::ByTheForge)
        );
        let mut queue = Queue::default();
        let taken = queue.visit(&marked(
            "/purlis/signin?code=marker-zq9&state=s-1&x=marker-zq9",
        ));
        waited(queue, &no_cancel()).unwrap();
        for visit in visits.iter().chain([&taken]) {
            let answer = visit.answer();
            assert!(!answer.is_empty());
            assert!(!answer.to_ascii_lowercase().contains("zq9"), "{answer}");
        }
        // And the three pages are the only bodies there are.
        for visit in visits.iter().chain([&taken]) {
            let answer = visit.answer();
            let body = answer.split_once("\r\n\r\n").unwrap().1;
            assert!([DONE_PAGE, DECLINED_PAGE, NOTHING_PAGE].contains(&body));
        }
    }

    #[test]
    fn a_second_request_is_never_read_once_the_redirect_is_taken() {
        let mut queue = Queue::default();
        queue.visit(&get("/purlis/signin?code=first&state=s-1"));
        let second = queue.visit(&get("/purlis/signin?code=second&state=s-1"));
        let code = waited(queue, &no_cancel()).unwrap();
        assert_eq!(code.expose(), "first");
        assert!(!second.was_read());
        assert_eq!(second.answer(), "");
    }

    #[test]
    fn a_peer_that_is_not_the_loopback_is_dropped_unread_and_unanswered() {
        for peer in ["192.168.1.20:50000", "10.0.0.7:4000", "[2001:db8::1]:50000"] {
            let mut queue = Queue::default();
            let stranger = queue.visit_from(peer, &get("/purlis/signin?code=theirs&state=s-1"));
            let own = queue.visit(&get("/purlis/signin?code=ours&state=s-1"));
            let code = waited(queue, &no_cancel()).unwrap();
            assert_eq!(code.expose(), "ours", "{peer}");
            assert!(!stranger.was_read(), "{peer}");
            assert_eq!(stranger.answer(), "", "{peer}");
            assert!(own.was_read());
        }
        // The loopback's own addresses are taken.
        for peer in ["127.0.0.1:1", "127.8.9.10:50000", "[::1]:50000"] {
            let mut queue = Queue::default();
            queue.visit_from(peer, &get("/purlis/signin?code=ours&state=s-1"));
            assert!(waited(queue, &no_cancel()).is_ok(), "{peer}");
        }
    }

    #[test]
    fn a_request_that_is_not_the_redirect_is_refused_and_the_sign_in_goes_on() {
        let mut queue = Queue::default();
        let refused = [
            (queue.visit(&get("/favicon.ico")), "404 Not Found"),
            (queue.visit(&get("/purlis/signin?code=guess&state=wrong")), "404 Not Found"),
            (queue.visit(&get("/purlis/signin?code=guess")), "404 Not Found"),
            (
                queue.visit(&format!(
                    "POST /purlis/signin?code=c&state=s-1 HTTP/1.1\r\nHost: 127.0.0.1:{PORT}\r\n\r\n"
                )),
                "405 Method Not Allowed",
            ),
            // Reached under another name, or with no name, or two.
            (
                queue.visit(
                    "GET /purlis/signin?code=c&state=s-1 HTTP/1.1\r\nHost: rebound.example:47823\r\n\r\n",
                ),
                "400 Bad Request",
            ),
            (
                queue.visit("GET /purlis/signin?code=c&state=s-1 HTTP/1.1\r\nHost: localhost:47823\r\n\r\n"),
                "400 Bad Request",
            ),
            (
                queue.visit("GET /purlis/signin?code=c&state=s-1 HTTP/1.1\r\nHost: 127.0.0.1:47824\r\n\r\n"),
                "400 Bad Request",
            ),
            (
                queue.visit("GET /purlis/signin?code=c&state=s-1 HTTP/1.1\r\n\r\n"),
                "400 Bad Request",
            ),
            (
                queue.visit(&format!(
                    "GET /purlis/signin?code=c&state=s-1 HTTP/1.1\r\nHost: 127.0.0.1:{PORT}\r\n\
                     Host: 127.0.0.1:{PORT}\r\n\r\n"
                )),
                "400 Bad Request",
            ),
            // Not a request, an absolute target, and one that never ends its head.
            (queue.visit("\r\n\r\n"), "400 Bad Request"),
            (
                queue.visit(&format!(
                    "GET http://127.0.0.1:{PORT}/purlis/signin?code=c&state=s-1 HTTP/1.1\r\n\
                     Host: 127.0.0.1:{PORT}\r\n\r\n"
                )),
                "400 Bad Request",
            ),
            (queue.visit("GET /purlis/signin?code=c&state=s-1 HTTP/1.1\r\n"), "400 Bad Request"),
            (queue.visit(""), "400 Bad Request"),
        ];
        let own = queue.visit(&get("/purlis/signin?code=the-real-one&state=s-1"));
        let code = waited(queue, &no_cancel()).unwrap();
        assert_eq!(code.expose(), "the-real-one");
        for (visit, status) in &refused {
            let answer = visit.answer();
            assert!(
                answer.starts_with(&format!("HTTP/1.1 {status}\r\n")),
                "wanted {status}: {answer}"
            );
            assert!(answer.ends_with(NOTHING_PAGE));
        }
        assert!(own.answer().contains("You can return to purlis"));
    }

    #[test]
    fn a_head_longer_than_the_most_is_refused_without_reading_on() {
        let mut queue = Queue::default();
        let long = format!(
            "GET /purlis/signin?code=c&state=s-1 HTTP/1.1\r\nHost: 127.0.0.1:{PORT}\r\nX-Pad: {}\r\n\r\n",
            "p".repeat(MOST_HEAD)
        );
        let visit = queue.visit(&long);
        assert_eq!(waited(queue, &no_cancel()).unwrap_err(), Stopped::TimedOut);
        assert!(visit.answer().starts_with("HTTP/1.1 400 Bad Request\r\n"));
    }

    #[test]
    fn a_connection_that_drips_is_cut_off_and_the_sign_in_goes_on() {
        let start = Instant::now();
        let passed = Arc::new(Mutex::new(Duration::ZERO));
        let written = Written::default();
        let mut queue = Queue::default();
        queue.waiting.push_back((
            Box::new(Scripted {
                request: std::io::Cursor::new(Vec::new()),
                written: written.clone(),
                touched: Arc::new(AtomicBool::new(false)),
                stalls: Some(passed.clone()),
            }),
            "127.0.0.1:50000".parse().unwrap(),
        ));
        let own = queue.visit(&get("/purlis/signin?code=after-the-drip&state=s-1"));
        let now = || start + *passed.lock().unwrap();
        let rest = |wait: Duration| *passed.lock().unwrap() += wait;
        let code = take_the_redirect(
            Listening::over(Box::new(queue), PORT),
            &State::for_a_test("s-1"),
            Duration::from_secs(300),
            &no_cancel(),
            &Clock {
                now: &now,
                rest: &rest,
            },
        )
        .unwrap();
        assert_eq!(code.expose(), "after-the-drip");
        // The dripper was given the per-request time and no more.
        assert_eq!(*passed.lock().unwrap(), PER_REQUEST);
        assert!(
            String::from_utf8(written.lock().unwrap().clone())
                .unwrap()
                .starts_with("HTTP/1.1 400 Bad Request\r\n")
        );
        assert!(own.was_read());
    }

    #[test]
    fn the_person_saying_no_at_the_forge_ends_the_sign_in() {
        let mut queue = Queue::default();
        let visit = queue.visit(&get("/purlis/signin?error=access_denied&state=s-1"));
        let after = queue.visit(&get("/purlis/signin?code=late&state=s-1"));
        assert_eq!(
            waited(queue, &no_cancel()).unwrap_err(),
            Stopped::Declined(Declined::ByThePerson)
        );
        assert!(visit.answer().contains("The sign-in did not finish"));
        assert!(!after.was_read());
    }

    #[test]
    fn nobody_coming_back_stops_it_when_the_time_allowed_is_up() {
        assert_eq!(
            waited(Queue::default(), &no_cancel()).unwrap_err(),
            Stopped::TimedOut
        );
    }

    #[test]
    fn a_cancel_stops_it_before_anything_is_read() {
        let mut queue = Queue::default();
        let visit = queue.visit(&get("/purlis/signin?code=c0de&state=s-1"));
        assert_eq!(
            waited(queue, &AtomicBool::new(true)).unwrap_err(),
            Stopped::Cancelled
        );
        assert!(!visit.was_read());
    }

    #[test]
    fn too_many_requests_that_are_not_the_redirect_stop_it() {
        let mut queue = Queue::default();
        for _ in 0..MOST_STRANGERS {
            queue.visit(&get("/purlis/signin?code=guess&state=wrong"));
        }
        let late = queue.visit(&get("/purlis/signin?code=c0de&state=s-1"));
        assert_eq!(waited(queue, &no_cancel()).unwrap_err(), Stopped::Strangers);
        assert!(!late.was_read());
    }

    #[test]
    fn a_listener_that_breaks_stops_it() {
        let queue = Queue {
            fails: true,
            ..Queue::default()
        };
        assert_eq!(waited(queue, &no_cancel()).unwrap_err(), Stopped::Failed);
    }

    fn in_use() -> std::io::Error {
        std::io::Error::from(std::io::ErrorKind::AddrInUse)
    }

    #[test]
    fn the_first_unused_port_is_bound_in_the_registered_order() {
        let tried = Mutex::new(Vec::new());
        let bind_one = |port: u16| {
            tried.lock().unwrap().push(port);
            if port == 47823 {
                Err(in_use())
            } else {
                Ok(port)
            }
        };
        assert_eq!(
            bind_first(&[47823, 47824, 47825], &bind_one).unwrap(),
            (47824, 47824)
        );
        assert_eq!(*tried.lock().unwrap(), [47823, 47824]);
    }

    #[test]
    fn with_all_three_ports_taken_it_says_so_and_tries_no_other() {
        let tried = Mutex::new(Vec::new());
        let bind_one = |port: u16| -> std::io::Result<()> {
            tried.lock().unwrap().push(port);
            Err(in_use())
        };
        let why = bind_first(&[47823, 47824, 47825], &bind_one).unwrap_err();
        assert_eq!(*tried.lock().unwrap(), [47823, 47824, 47825]);
        assert_eq!(
            why,
            "purlis could not take the browser's answer: ports 47823, 47824 and 47825 on this \
             machine are all in use by other software, and the sign-in can come back on no \
             other. Close what is using one of them and sign in again, or sign in with a token \
             instead."
        );
    }

    #[test]
    fn a_machine_that_refuses_the_listener_is_told_why_and_offered_the_token() {
        let bind_one = |_: u16| -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Operation not permitted",
            ))
        };
        let why = bind_first(&[47823, 47824, 47825], &bind_one).unwrap_err();
        assert!(
            why.contains("127.0.0.1, ports 47823, 47824 and 47825"),
            "{why}"
        );
        assert!(why.contains("Operation not permitted"), "{why}");
        assert!(why.ends_with("Sign in with a token instead."), "{why}");
    }
}
