// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Carrying the browser's answer to a login on a runner** (#4044, spec #4023). Codex's default
//! login, and opencode's ChatGPT one, end with the browser sent to `localhost:1455` on the
//! machine that runs the login, which a browser on this Mac cannot reach on a runner.
//!
//! **One redirect is carried, never a port.** [`Forward`] listens on this Mac's loopback alone,
//! both of its addresses, because a browser may try either for `localhost`, and both are this
//! sign-in's or it does not start. It takes only a `GET` of [`PATH`], addressed to this machine's
//! own name, with the `state` that sign-in's page printed: any other request is answered here and
//! goes no further, which keeps a page in the browser from reaching the runner through it. The
//! first redirect that fits closes it before it is carried, so it is carried once, and a sign-in
//! that ends closes it. A program holding either address is never asked to leave: the port counts
//! as taken and the sign-in says so. (One listening on every address is not holding these two:
//! the sign-in's own bind is the more specific, and takes this machine's loopback while it runs.)
//!
//! **Nothing comes back but whether it worked.** On the runner, [`deliver`] asks the login's own
//! listener on the runner's loopback, at this same port and path, with a bare request of what the
//! browser sent. Codex's answer redirects to its own page, with the account's values in the
//! address, and ends only once that page is asked for. That page is asked for by its path alone, which is all it
//! needs, so the address is not read, not sent again, and never seen by the browser: the Mac is
//! told only whether it worked, and shows a page of its own. Nothing is kept on either side,
//! nothing here is logged, and nothing is opened on the runner for this: the only listener there
//! is the login's own. It connects to nothing unless a login purlis holds there waits with the
//! redirect's own `state` ([`waiting`]), once: a login that ended, or another sign-in's, gets
//! nothing, and neither does what else listens on that port.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::waiting;

/// The port Codex's and opencode's ChatGPT logins listen on, by their own code.
pub const PORT: u16 = 1455;

/// Where their logins send the browser back to.
pub const PATH: &str = "/auth/callback";

/// Where Codex's sends the browser next, on the runner alone.
const NEXT_PATH: &str = "/success";

/// The most of a redirect's request carried, and of a browser's request read.
const MOST_TARGET: usize = 8 * 1024;
const MOST_HEAD: usize = 16 * 1024;

/// How long one browser connection has to say what it asks, and how many are read at once: a
/// connection that says nothing costs one of these for that long, and the redirect waits for one.
const ASKED_WITHIN: Duration = Duration::from_secs(2);
const MOST_READ_AT_ONCE: usize = 32;

/// How often a [`Forward`] looks for a connection, and for being closed.
const LOOKED_EVERY: Duration = Duration::from_millis(50);

/// How long the login on the runner has to take the connection, and to answer it: it signs in
/// with its vendor before it does. Both of its two asks fit in [`CARRIED_WITHIN`].
const REACHED_WITHIN: Duration = Duration::from_secs(2);
const ANSWERED_WITHIN: Duration = Duration::from_secs(15);

/// How long a desktop waits for its runner to say how [`deliver`] went.
pub const CARRIED_WITHIN: Duration = Duration::from_secs(40);

/// What a redirect with no login to take it answers.
const NO_LOGIN_WAITING: &str = "no sign-in is waiting on the runner";

/// **Carries one redirect's request target to the runner**: to its [`deliver`], over the link,
/// answering whether the login there took it.
pub type Carry = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

/// What the browser is shown once the redirect was carried: no script, and nothing of what it
/// asked.
const CARRIED: &str = "<!doctype html><meta charset=utf-8><title>purlis</title>\
                       <p>Approved. Go back to purlis, which is finishing the sign-in.";
const NOT_CARRIED: &str = "<!doctype html><meta charset=utf-8><title>purlis</title>\
                           <p>That did not reach the runner. Go back to purlis to try again.";

/// **Listens for one sign-in's redirect on this Mac's loopback** until it is carried or this is
/// dropped, which closes it and waits for it to be closed.
#[derive(Debug)]
pub struct Forward {
    port: u16,
    closed: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Forward {
    /// **Listens on `port`** (`0` lets the system choose, for a test) for the redirect with
    /// `state`: `carry` carries it, and `told` hears how that went. Fails as nothing was opened
    /// where either loopback address is taken.
    pub fn open(
        port: u16,
        state: &str,
        carry: Carry,
        told: impl FnOnce(Result<(), String>) + Send + 'static,
    ) -> io::Result<Self> {
        let listeners = listening(port)?;
        let port = listeners[0].local_addr()?.port();
        let closed = Arc::new(AtomicBool::new(false));
        let thread = {
            let (closed, state) = (Arc::clone(&closed), state.to_owned());
            std::thread::spawn(move || {
                let Some((mut browser, target)) = redirect(listeners, port, &state, &closed) else {
                    return;
                };
                let Some(carried) = carried_while_open(&closed, &carry, &target) else {
                    return;
                };
                answer(&mut browser, carried.is_ok());
                told(carried);
            })
        };
        Ok(Self {
            port,
            closed,
            thread: Some(thread),
        })
    }

    /// The port it listens on.
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for Forward {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// `target` carried by `carry`, unless the sign-in ended since it was read: `None`, and nothing
/// goes to the runner.
fn carried_while_open(
    closed: &AtomicBool,
    carry: &Carry,
    target: &str,
) -> Option<Result<(), String>> {
    (!closed.load(Ordering::SeqCst)).then(|| carry(target))
}

/// Listeners on both of this machine's loopback addresses, the second on the port the first
/// was given. One this machine has no such address for is left out.
fn listening(port: u16) -> io::Result<Vec<TcpListener>> {
    let first = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))?;
    let port = first.local_addr()?.port();
    let mut listeners = vec![first];
    match TcpListener::bind(SocketAddr::from((Ipv6Addr::LOCALHOST, port))) {
        Ok(second) => listeners.push(second),
        // No such address here, or no IPv6 at all: only then is it left out. Any other refusal
        // ends the forward, so no other listener may take the redirect there.
        Err(why) if no_such_address(&why) => {}
        Err(why) => return Err(why),
    }
    for listener in &listeners {
        listener.set_nonblocking(true)?;
    }
    Ok(listeners)
}

/// Whether `why` says this machine has no IPv6 loopback to listen on.
fn no_such_address(why: &io::Error) -> bool {
    #[cfg(unix)]
    let no_ipv6 = why.raw_os_error() == Some(libc::EAFNOSUPPORT);
    #[cfg(not(unix))]
    let no_ipv6 = false;
    why.kind() == io::ErrorKind::AddrNotAvailable || no_ipv6
}

/// Answers every connection until one brings the redirect with `state`: that connection, and
/// what it asked, with the port closed. `None` once `closed`. Each is read on a thread of its own,
/// up to [`MOST_READ_AT_ONCE`], from each listener in turn, so no connection holds up the redirect.
fn redirect(
    listeners: Vec<TcpListener>,
    port: u16,
    state: &str,
    closed: &AtomicBool,
) -> Option<(TcpStream, String)> {
    let reading = Arc::new(AtomicUsize::new(0));
    let (found, redirects) = channel();
    loop {
        if closed.load(Ordering::SeqCst) {
            return None;
        }
        if let Ok(redirect) = redirects.try_recv() {
            return Some(redirect);
        }
        let mut accepted = false;
        for listener in &listeners {
            if reading.load(Ordering::SeqCst) < MOST_READ_AT_ONCE
                && let Ok((browser, _)) = listener.accept()
            {
                accepted = true;
                reading.fetch_add(1, Ordering::SeqCst);
                let (reading, found) = (Arc::clone(&reading), found.clone());
                let state = state.to_owned();
                std::thread::spawn(move || {
                    read_one(browser, port, &state, &found);
                    reading.fetch_sub(1, Ordering::SeqCst);
                });
            }
        }
        if !accepted {
            std::thread::sleep(LOOKED_EVERY);
        }
    }
}

/// Reads one connection: the redirect goes to `found`, and anything else is answered here.
fn read_one(mut browser: TcpStream, port: u16, state: &str, found: &Sender<(TcpStream, String)>) {
    match head_of(&mut browser).and_then(|head| redirect_in(&head, port, state)) {
        Some(target) => {
            let _ = found.send((browser, target));
        }
        None => refuse(&mut browser),
    }
}

/// What the browser sent, up to the end of its headers: `None` where it does not say that within
/// [`ASKED_WITHIN`] or [`MOST_HEAD`].
fn head_of(browser: &mut TcpStream) -> Option<String> {
    let until = Instant::now() + ASKED_WITHIN;
    browser.set_nonblocking(false).ok()?;
    browser.set_write_timeout(Some(ASKED_WITHIN)).ok()?;
    let mut head = Vec::new();
    let mut piece = [0u8; 2048];
    while !head.windows(4).any(|end| end == b"\r\n\r\n") {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() || head.len() > MOST_HEAD {
            return None;
        }
        browser.set_read_timeout(Some(left)).ok()?;
        match browser.read(&mut piece).ok()? {
            0 => return None,
            read => head.extend_from_slice(&piece[..read]),
        }
    }
    String::from_utf8(head).ok()
}

/// **The redirect's request target in `head`**, where it is this sign-in's: a `GET` of [`PATH`]
/// with `state` once, addressed to this machine by its own name on `port`. A page on another
/// site cannot name a host of this machine's and read the answer, and cannot know the state.
fn redirect_in(head: &str, port: u16, state: &str) -> Option<String> {
    let mut lines = head.split("\r\n");
    let mut request = lines.next()?.split(' ');
    let (method, target, version) = (request.next()?, request.next()?, request.next()?);
    if method != "GET" || !version.starts_with("HTTP/1.") || request.next().is_some() {
        return None;
    }
    let mut hosts = lines
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.eq_ignore_ascii_case("host"))
        .map(|(_, host)| host.trim());
    let host = hosts.next()?;
    let own = [
        format!("localhost:{port}"),
        format!("127.0.0.1:{port}"),
        format!("[::1]:{port}"),
    ];
    if hosts.next().is_some() || !own.iter().any(|name| name == host) {
        return None;
    }
    (is_a_redirect(target) && same(state_in(target)?, state)).then(|| target.to_owned())
}

/// Whether `target` is the only thing carried: [`PATH`] and a query, in the characters a query is
/// written in, so nothing in it can read as another line of a request.
pub fn is_a_redirect(target: &str) -> bool {
    target.len() <= MOST_TARGET
        && target
            .strip_prefix(PATH)
            .and_then(|rest| rest.strip_prefix('?'))
            .is_some_and(|query| query.bytes().all(in_a_query))
}

fn in_a_query(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-._~%+=&/:,;@!$()*".contains(&byte)
}

/// The value of `target`'s one `state`; `None` with none, or more than one.
fn state_in(target: &str) -> Option<&str> {
    let (_, query) = target.split_once('?')?;
    let mut states = query
        .split('&')
        .filter_map(|pair| pair.strip_prefix("state="));
    let state = states.next()?;
    states.next().is_none().then_some(state)
}

/// Whether `a` and `b` are the same. Of two the same length, it takes the same time wherever they
/// differ; two of different lengths answer at once, which tells only the length.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |differ, (x, y)| differ | (x ^ y))
            == 0
}

fn refuse(browser: &mut TcpStream) {
    respond(browser, "404 Not Found", "");
}

fn answer(browser: &mut TcpStream, carried: bool) {
    if carried {
        respond(browser, "200 OK", CARRIED);
    } else {
        respond(browser, "502 Bad Gateway", NOT_CARRIED);
    }
}

fn respond(browser: &mut TcpStream, status: &str, page: &str) {
    let _ = write!(
        browser,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nContent-Security-Policy: default-src 'none'\r\n\
         Cache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n{page}",
        page.len()
    );
}

/// **Hands the redirect `target` to the login waiting for it on this runner's loopback**, at
/// [`PORT`]: how a runner's host carries it out. Answers only whether the login took it.
pub fn deliver(target: &str) -> Result<(), String> {
    let kept_in = waiting::kept_here().ok_or_else(|| NO_LOGIN_WAITING.to_owned())?;
    deliver_waiting_in(&kept_in, target)
}

/// [`deliver`], with what logins wait for recorded in `kept_in` ([`waiting`]).
pub fn deliver_waiting_in(kept_in: &Path, target: &str) -> Result<(), String> {
    deliver_on(kept_in, PORT, target)
}

fn deliver_on(kept_in: &Path, port: u16, target: &str) -> Result<(), String> {
    if !is_a_redirect(target) {
        return Err("that is not a sign-in's redirect, so it was not passed on".to_owned());
    }
    // Before any connection: no other login, or what else listens here, hears of it.
    if !state_in(target).is_some_and(|state| waiting::take(kept_in, state)) {
        return Err(NO_LOGIN_WAITING.to_owned());
    }
    match asked(port, target)? {
        Answered::Took => Ok(()),
        Answered::Next => match asked(port, NEXT_PATH)? {
            Answered::Took => Ok(()),
            _ => Err("the sign-in on the runner did not finish".to_owned()),
        },
        Answered::Refused(status) => Err(format!(
            "the sign-in on the runner refused the browser's answer (status {status})"
        )),
    }
}

/// How the login answered.
#[derive(Debug, PartialEq, Eq)]
enum Answered {
    Took,
    /// Go on to its next page, [`NEXT_PATH`], on its own listener.
    Next,
    Refused(u16),
}

/// Asks the login on `port` for `target`, on one connection of its own.
fn asked(port: u16, target: &str) -> Result<Answered, String> {
    let mut login = [
        SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        SocketAddr::from((Ipv6Addr::LOCALHOST, port)),
    ]
    .iter()
    .find_map(|at| TcpStream::connect_timeout(at, REACHED_WITHIN).ok())
    .ok_or_else(|| NO_LOGIN_WAITING.to_owned())?;
    let cut_off = |_| "the sign-in on the runner stopped answering".to_owned();
    login
        .set_write_timeout(Some(REACHED_WITHIN))
        .map_err(cut_off)?;
    write!(
        login,
        "GET {target} HTTP/1.1\r\nHost: localhost:{port}\r\nConnection: close\r\n\r\n"
    )
    .map_err(cut_off)?;
    let until = Instant::now() + ANSWERED_WITHIN;
    let mut head = Vec::new();
    let mut piece = [0u8; 4096];
    while !head.windows(4).any(|end| end == b"\r\n\r\n") {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() || head.len() > MOST_HEAD {
            return Err("the sign-in on the runner stopped answering".to_owned());
        }
        login.set_read_timeout(Some(left)).map_err(cut_off)?;
        match login.read(&mut piece).map_err(cut_off)? {
            0 => break,
            read => head.extend_from_slice(&piece[..read]),
        }
    }
    answered(&String::from_utf8_lossy(&head), port)
        .ok_or_else(|| "the sign-in on the runner answered in a way purlis cannot read".to_owned())
}

/// What the head of a login's answer says: a redirect is followed only to its own [`NEXT_PATH`],
/// on the listener it is on.
fn answered(head: &str, port: u16) -> Option<Answered> {
    let mut lines = head.split("\r\n");
    let status: u16 = lines.next()?.split(' ').nth(1)?.parse().ok()?;
    match status {
        200..=299 => Some(Answered::Took),
        300..=399 => {
            let location = lines
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("location"))?
                .1
                .trim();
            // Codex 0.163.0 writes the machine's address here, as other logins may write its name.
            let target = [
                format!("http://127.0.0.1:{port}"),
                format!("http://localhost:{port}"),
            ]
            .iter()
            .find_map(|own| location.strip_prefix(own.as_str()))
            .unwrap_or(location);
            let path = target.split('?').next().unwrap_or(target);
            Some(if path == NEXT_PATH {
                Answered::Next
            } else {
                Answered::Refused(status)
            })
        }
        _ => Some(Answered::Refused(status)),
    }
}

#[cfg(test)]
mod tests;
