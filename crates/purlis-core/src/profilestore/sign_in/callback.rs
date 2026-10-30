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
//! that ends closes it. A person's own program on the port is never asked to leave: the port
//! counts as taken and the sign-in says so.
//!
//! **Nothing comes back but whether it worked.** On the runner, [`deliver`] asks the login's own
//! listener on the runner's loopback, at this same port and path, with a bare request of what the
//! browser sent. Codex's answer redirects to its own page, with a token in the address, so that
//! redirect is followed on the runner and the browser never sees it: the Mac is told only whether
//! it worked, and shows a page of its own. Nothing is kept on either side, nothing here is logged,
//! and nothing is opened on the runner for this: the only listener there is the login's own.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The port Codex's and opencode's ChatGPT logins listen on, by their own code.
pub const PORT: u16 = 1455;

/// Where their logins send the browser back to.
pub const PATH: &str = "/auth/callback";

/// Where Codex's sends the browser next, on the runner alone.
const NEXT_PATH: &str = "/success";

/// The most of a redirect's request carried, and of a browser's request read.
const MOST_TARGET: usize = 8 * 1024;
const MOST_HEAD: usize = 16 * 1024;

/// How long one browser connection has to say what it asks.
const ASKED_WITHIN: Duration = Duration::from_secs(3);

/// How often a [`Forward`] looks for a connection, and for being closed.
const LOOKED_EVERY: Duration = Duration::from_millis(50);

/// How long the login on the runner has to take the connection, and to answer it: it signs in
/// with its vendor before it does. Inside how long a desktop waits for its runner.
const REACHED_WITHIN: Duration = Duration::from_secs(2);
const ANSWERED_WITHIN: Duration = Duration::from_secs(15);

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
                let carried = carry(&target);
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

/// Listeners on both of this machine's loopback addresses, the second on the port the first
/// was given. One this machine has no such address for is left out.
fn listening(port: u16) -> io::Result<Vec<TcpListener>> {
    let first = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))?;
    let port = first.local_addr()?.port();
    let mut listeners = vec![first];
    match TcpListener::bind(SocketAddr::from((Ipv6Addr::LOCALHOST, port))) {
        Ok(second) => listeners.push(second),
        Err(why) if why.kind() == io::ErrorKind::AddrNotAvailable => {}
        Err(why) => return Err(why),
    }
    for listener in &listeners {
        listener.set_nonblocking(true)?;
    }
    Ok(listeners)
}

/// Answers every connection until one brings the redirect with `state`: that connection, and
/// what it asked, with the port closed. `None` once `closed`.
fn redirect(
    listeners: Vec<TcpListener>,
    port: u16,
    state: &str,
    closed: &AtomicBool,
) -> Option<(TcpStream, String)> {
    loop {
        if closed.load(Ordering::SeqCst) {
            return None;
        }
        let Some(mut browser) = listeners
            .iter()
            .find_map(|listener| listener.accept().ok())
            .map(|(stream, _)| stream)
        else {
            std::thread::sleep(LOOKED_EVERY);
            continue;
        };
        match head_of(&mut browser).and_then(|head| redirect_in(&head, port, state)) {
            Some(target) => return Some((browser, target)),
            None => refuse(&mut browser),
        }
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

/// Whether `a` and `b` are the same, in the same time however much of them is.
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

/// **Hands the redirect `target` to the login listening on this runner's loopback**, at [`PORT`]:
/// how a runner's host carries it out. Answers only whether the login took it.
pub fn deliver(target: &str) -> Result<(), String> {
    deliver_on(PORT, target)
}

fn deliver_on(port: u16, target: &str) -> Result<(), String> {
    if !is_a_redirect(target) {
        return Err("that is not a sign-in's redirect, so it was not passed on".to_owned());
    }
    match asked(port, target)? {
        Answered::Took => Ok(()),
        Answered::Next(next) => match asked(port, &next)? {
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
    /// Go on to this request target, on the login's own listener.
    Next(String),
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
    .ok_or_else(|| "no sign-in is waiting on the runner".to_owned())?;
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
            let own = format!("http://localhost:{port}");
            let target = location.strip_prefix(own.as_str()).unwrap_or(location);
            let is_next = (target == NEXT_PATH || target.starts_with("/success?"))
                && target.bytes().all(|byte| in_a_query(byte) || byte == b'?');
            Some(if is_next {
                Answered::Next(target.to_owned())
            } else {
                Answered::Refused(status)
            })
        }
        _ => Some(Answered::Refused(status)),
    }
}

#[cfg(test)]
mod tests;
