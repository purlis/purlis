// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! purlis's loopback egress proxy (ADR 0067 §3 as amended 2026-10-10): what a harness purlis
//! wraps reaches the network through, where the harness has no proxy of its own.
//!
//! **One chat, one pair of ports** (#1664). Each chat purlis wraps gets its own proxy, made when
//! the chat starts and gone when it ends ([`Proxy`] stops listening when dropped): an HTTP port
//! ([`Proxy::port`]) and a SOCKS5 port ([`Proxy::socks_port`]). The wrap lets the chat connect to
//! those two ports on the loopback interface and nowhere else, so a program that ignores
//! `HTTPS_PROXY` reaches nothing, and one chat never reaches another's ports. **Which chat a
//! connection is from is which proxy it arrived on**: nothing a connection sends names a chat,
//! and nothing it sends is read as one.
//!
//! The HTTP port carries:
//!
//! - **`CONNECT host:443`**, the tunnel every HTTPS client asks a proxy for, to a listed host.
//!   The bytes inside the tunnel are the client's TLS, which the proxy does not read, so what
//!   host a TLS client names inside it (SNI) is the client's to say, as with any proxy that
//!   does not decrypt. There is no TLS interception and no certificate of purlis's own.
//! - **A plain request in absolute form** (`GET http://host/path`) to a listed host on port 80,
//!   sent on in origin form with `Connection: close`, so one connection reaches one host. Its
//!   one `Host` must name the same host, and its body must state its length: the body is
//!   carried and nothing after it, so a second request cannot ride the same connection.
//!
//! The SOCKS5 port carries a `CONNECT` with no authentication (RFC 1928), to a name or an
//! address, on the ports a tunnel is carried to. Every other method, command and address type
//! is refused with SOCKS's own reply.
//!
//! **The head is read strictly**, so the proxy and the host it reaches cannot read it two ways:
//! CRLF line ends only, one space between the request line's three parts, `HTTP/1.1` or
//! `HTTP/1.0`, header names of token characters with no space before the colon, no folded
//! header, at most one `Host` and one `Content-Length` (digits only), and no
//! `Transfer-Encoding`. A head that breaks any of these is refused.
//!
//! **What is carried is decided by the core decision module** ([`super::reach`]): by host and
//! port only, open, persona, allowed, ask or refused. Anything not carried is refused before a
//! connection is made: a host nothing lists (an ask, held while the person is asked where the
//! chat's proxy has a board of live asks, [`super::asks`], #1666, and refused where it has
//! none or nobody answered in time), a tunnel to any port but 443, a plain request to any port but 80, and
//! a request that is not one of the two above.
//!
//! **Bounded** ([`Limits`]): at most a few connections at once are served, over both ports
//! together; a head or a SOCKS greeting must arrive whole within a deadline; a connection that
//! carries nothing either way for long is closed; a write the other side does not take in that
//! time ends it; and a failing accept backs off rather than spinning. No buffer grows past a
//! fixed size.
//!
//! **The local-address check.** The proxy resolves each name itself, and a name is reached only
//! at the addresses it resolves to that a chat may reach ([`reachable`]): never this machine, a
//! link-local, multicast or broadcast address, or a cloud metadata service, checked at every
//! connect, so a name that resolves or is rebound there reaches nothing. A name that resolves
//! *only* there is refused as a Block. An address literal is held to the same check: the one
//! exception is that exact address and port, listed ([`super::reach::Reach::lists_exactly`]).
//! A listed name is a name, so an address literal is never carried unless a preset or a host
//! lists it, and a wildcard never matches one.
//!
//! **A host listed with a port** (`10.0.0.5:6443`, a project's or a person's, #1341) is carried
//! on that port alone, tunnel or plain; a host without one on the ports above.
//!
//! **What it refused is its own record** ([`Refusals`]): each host and port it refused because
//! no preset or host lists it, as the request named them, once each and at most
//! [`REFUSALS_KEPT`]. A brokered `secret exec` reads it to tell the asking chat which host its
//! command was refused (`crate::secrets::brokered`), and the app raises the chat's Block from
//! it: the proxy's word, never the command's.
//!
//! **Every connection is told** ([`Reached`]): each connection carried or refused, by host and
//! port and the decision's word, coalesced to a line per host and port a minute ([`Tally`]), so
//! the app keeps every connection in the network record without a chat being able to flood it.
//!
//! **A refusal is never held up by whoever is told it** (#1353). It is kept in the record at
//! once, then told by the proxy's teller, a thread of its own, while the client is answered:
//! a slow listener (the app's file write and window emit) neither delays the client's refusal
//! nor holds one of the proxy's few connection slots.

use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::asks::{Answer, Asks};
use super::internal::Internal;
use super::localport;
use super::reach::{Decision, Reach, Refused};

/// The port a tunnel is carried to: HTTPS's.
pub const TUNNEL_PORTS: [u16; 1] = [443];

/// The port a plain request is carried to: HTTP's.
pub const PLAIN_PORTS: [u16; 1] = [80];

/// The most distinct hosts a proxy's [`Refusals`] keeps: a command that tries a thousand hosts
/// is one pattern, not a thousand Notices.
pub const REFUSALS_KEPT: usize = 8;

/// The most hosts past [`REFUSALS_KEPT`] a proxy's [`Refusals`] counts (#1353), so it can say
/// how many more there were: each counted once, by a short digest of it, never kept by name.
pub const MORE_COUNTED_AT_MOST: usize = 1024;

/// The most a request's head may be before it is refused.
const HEAD_MAX: usize = 16 * 1024;

/// How long a client has to send its request, and a host to answer the connection.
pub(super) const PATIENCE: Duration = Duration::from_secs(30);

/// How long a failing accept waits before it tries again.
const BACKOFF: Duration = Duration::from_millis(100);

/// How long a stopping proxy waits for its teller to tell what it was handed (#1353).
const TELLER_PATIENCE: Duration = Duration::from_secs(2);

/// What one proxy takes on at once, and how long a connection may carry nothing.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Connections served at once; one more is answered `503` and closed.
    pub connections: usize,
    /// How long a connection may carry nothing in either direction before it is closed, and a
    /// write may wait for the other side to take it.
    pub idle: Duration,
    /// How long a client has to send its request's whole head.
    pub head: Duration,
}

/// What a chat's proxy takes on: a harness opens a handful of connections, and a model's
/// answer streams far more often than this.
pub const LIMITS: Limits = Limits {
    connections: 64,
    idle: Duration::from_secs(600),
    head: Duration::from_secs(30),
};

/// Whether `host` is one `listed` names: the same name, ignoring case and a final dot, or a
/// name under a listed `*.suffix` (never the suffix itself).
pub fn allows(listed: &[String], host: &str) -> bool {
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    listed.iter().any(|listed| {
        let listed = listed.to_ascii_lowercase();
        match listed.strip_prefix("*.") {
            // A wildcard names names, never an address literal (#1341).
            Some(_) if host.parse::<std::net::IpAddr>().is_ok() => false,
            Some(suffix) => host
                .strip_suffix(suffix)
                .is_some_and(|head| head.len() > 1 && head.ends_with('.')),
            None => host == listed,
        }
    })
}

/// Whether `host` on `port` is one `listed` names (#1341): an entry with a port (`host:port`,
/// `[v6]:port`) on that port alone, and one without on `defaults`, the ports the proxy carries
/// for what a preset lists. An IPv6 address is matched as an address, whatever its spelling.
pub fn allows_on(listed: &[String], host: &str, port: u16, defaults: &[u16]) -> bool {
    listed.iter().any(|entry| {
        let (name, held) = entry_parts(entry);
        let on = match held {
            Some(held) => held == port,
            None => defaults.contains(&port),
        };
        on && match (
            name.parse::<std::net::Ipv6Addr>(),
            host.parse::<std::net::Ipv6Addr>(),
        ) {
            (Ok(listed), Ok(asked)) => listed == asked,
            _ => allows(&[name.to_owned()], host),
        }
    })
}

/// A listed entry's name and the port it is held to, where it has one: `[v6]` and `[v6]:port`,
/// `name:port` with one colon, or the name alone.
pub(crate) fn entry_parts(entry: &str) -> (&str, Option<u16>) {
    if let Some(rest) = entry.strip_prefix('[')
        && let Some((inside, after)) = rest.split_once(']')
    {
        return (
            inside,
            after.strip_prefix(':').and_then(|port| port.parse().ok()),
        );
    }
    match entry.split_once(':') {
        Some((name, port)) if !port.contains(':') => match port.parse() {
            Ok(port) => (name, Some(port)),
            // Not a port: never a match, rather than the name on every port.
            Err(_) => ("", None),
        },
        _ => (entry, None),
    }
}

/// What is told each host and port a proxy refuses, the first time it refuses it.
pub type Told = Arc<dyn Fn(&str, u16) + Send + Sync + 'static>;

/// What is told each host and port a proxy refuses, saying whether anyone heard it (#1683):
/// `false` where nobody could be told yet, and the refusal is kept to be told again
/// ([`Refusals::tell_untold`]).
pub type Delivers = Arc<dyn Fn(&str, u16) -> bool + Send + Sync + 'static>;

/// **The hosts a proxy refused because nothing lists them**: each host and port once, in the
/// order refused, at most [`REFUSALS_KEPT`], and told to whoever is listening as it happens.
/// A request the proxy could not read, or one whose `Host` names another host, is not one: no
/// grant would let it through.
#[derive(Clone, Default)]
pub struct Refusals {
    kept: Kept,
    told: Option<Delivers>,
    /// Kept refusals nobody could be told yet (#1683), oldest first, shared by its clones.
    untold: Kept,
    /// Told each host and port refused by the local-address check (#1664), once each and at
    /// most [`REFUSALS_KEPT`]: never one a person could allow, so never kept in [`Self::refused`].
    local: Option<(Told, Kept)>,
    /// Each host and port refused past the kept ones, as a digest, at most
    /// [`MORE_COUNTED_AT_MOST`], shared by its clones (#1353).
    more: Arc<Mutex<std::collections::HashSet<u64>>>,
    /// Told each count past the kept ones as it grows (#1353), and the last count told, shared
    /// by its clones: what lets a Notice already up say "and N more".
    more_told: Option<(MoreTold, Arc<std::sync::atomic::AtomicUsize>)>,
}

/// Told how many hosts and ports past the kept ones a [`Refusals`] has counted, each time that
/// grows (#1353).
pub type MoreTold = Arc<dyn Fn(usize) + Send + Sync + 'static>;

/// Hosts and ports a [`Refusals`] has told, shared by its clones.
type Kept = Arc<Mutex<Vec<(String, u16)>>>;

impl std::fmt::Debug for Refusals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Refusals")
            .field("kept", &self.refused())
            .finish_non_exhaustive()
    }
}

impl Refusals {
    /// A record that tells `told` each refusal it keeps, as it keeps it.
    pub fn telling(told: Told) -> Self {
        Self::telling_once_heard(Arc::new(move |host: &str, port: u16| {
            told(host, port);
            true
        }))
    }

    /// A record that tells `told` each refusal it keeps, as it keeps it, and keeps one nobody
    /// could be told yet (`told` answered `false`) for [`Self::tell_untold`] (#1683): a
    /// refusal heard before the chat has its number, or before the project's hooks listen, is
    /// told once they do, not dropped.
    pub fn telling_once_heard(told: Delivers) -> Self {
        Self {
            kept: Arc::default(),
            told: Some(told),
            untold: Arc::default(),
            local: None,
            more: Arc::default(),
            more_told: None,
        }
    }

    /// This record, telling `told` the count of hosts and ports refused past the kept ones,
    /// each time it grows (#1353): never a host, which is not kept by name.
    #[must_use]
    pub fn telling_more(self, told: MoreTold) -> Self {
        Self {
            more_told: Some((told, Arc::default())),
            ..self
        }
    }

    /// Tells the count past the kept ones where it grew since it was last told: the
    /// listener's half of counting one more, run where [`Self::tell`] is.
    fn tell_more(&self) {
        let Some((told, at)) = &self.more_told else {
            return;
        };
        let now = self.more();
        if now > at.fetch_max(now, std::sync::atomic::Ordering::SeqCst) {
            told(now);
        }
    }

    /// Tells again each kept refusal nobody could be told yet, oldest first; one still not
    /// heard stays kept for the next call. Told under the same lock [`Self::heard`] tells
    /// under, so a refusal heard while the chat gets its number, or while its listener is set,
    /// is either told here or told by [`Self::heard`] itself, never left between them. `told`
    /// must therefore not call back into this record.
    pub fn tell_untold(&self) {
        let Some(told) = &self.told else { return };
        self.untold
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|(host, port)| !told(host, *port));
    }

    /// This record, telling `told` each host and port the local-address check refuses as well.
    #[must_use]
    pub fn telling_local(self, told: Told) -> Self {
        Self {
            local: Some((told, Arc::default())),
            ..self
        }
    }

    /// Tells `host` on `port` as refused by the local-address check, unless it was told already
    /// or [`REFUSALS_KEPT`] were.
    pub fn heard_local(&self, host: &str, port: u16) {
        if self.keep_local(host, port) {
            self.tell_local(host, port);
        }
    }

    /// Whether `host` on `port`, refused by the local-address check, is to be told: not told
    /// already, and fewer than [`REFUSALS_KEPT`] were.
    fn keep_local(&self, host: &str, port: u16) -> bool {
        self.local
            .as_ref()
            .is_some_and(|(_, seen)| once(seen, host, port))
    }

    /// Tells `host` on `port` as refused by the local-address check.
    fn tell_local(&self, host: &str, port: u16) {
        if let Some((told, _)) = &self.local {
            told(host, port);
        }
    }

    /// Keeps `host` on `port` as refused, and tells it, unless it is kept already or the
    /// record is full.
    pub fn heard(&self, host: &str, port: u16) {
        if self.keep(host, port) {
            self.tell(host, port);
        } else {
            self.tell_more();
        }
    }

    /// Keeps `host` on `port` as refused, and says whether it was new and there was room: the
    /// record's half of [`Self::heard`], which never waits on a listener.
    fn keep(&self, host: &str, port: u16) -> bool {
        match keeps(&self.kept, host, port) {
            Keeping::New => true,
            Keeping::Already => false,
            Keeping::Full => {
                let mut more = self
                    .more
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if more.len() < MORE_COUNTED_AT_MOST {
                    more.insert(digest_of(host, port));
                }
                false
            }
        }
    }

    /// Tells `host` on `port`, kept, as refused: the listener's half of [`Self::heard`]. One
    /// nobody could be told yet is kept untold ([`Self::tell_untold`]).
    fn tell(&self, host: &str, port: u16) {
        let Some(told) = &self.told else { return };
        // Told under the lock [`Self::tell_untold`] tells under (#1683).
        let mut untold = self
            .untold
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !told(host, port) {
            // Bounded as `kept` is: each host and port once, at most [`REFUSALS_KEPT`].
            untold.push((host.to_owned(), port));
        }
    }

    /// How many hosts and ports past the kept ones were refused (#1353), each once, at most
    /// [`MORE_COUNTED_AT_MOST`]: what a list of [`Self::refused`] says as "and N more".
    pub fn more(&self) -> usize {
        self.more
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    /// Each refused host and port, as one names it to a grant: `host:port`, an IPv6 address
    /// in brackets.
    pub fn refused(&self) -> Vec<String> {
        self.kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|(host, port)| host_and_port(host, *port))
            .collect()
    }
}

/// Keeps `host` on `port` in `kept`, and says whether it was new and there was room.
fn once(kept: &Mutex<Vec<(String, u16)>>, host: &str, port: u16) -> bool {
    keeps(kept, host, port) == Keeping::New
}

/// What [`keeps`] made of a host and port.
#[derive(Debug, PartialEq, Eq)]
enum Keeping {
    /// Kept now.
    New,
    /// Kept already.
    Already,
    /// Not kept, and new: the record was full.
    Full,
}

/// Keeps `host` on `port` in `kept`, where it is new and there is room.
fn keeps(kept: &Mutex<Vec<(String, u16)>>, host: &str, port: u16) -> Keeping {
    let mut kept = kept
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if kept
        .iter()
        .any(|(h, p)| *p == port && h.eq_ignore_ascii_case(host))
    {
        return Keeping::Already;
    }
    if kept.len() >= REFUSALS_KEPT {
        return Keeping::Full;
    }
    kept.push((host.to_owned(), port));
    Keeping::New
}

/// `host` on `port`, ignoring the case of the host, as a short digest: what counts a host past
/// the kept ones without keeping its name.
fn digest_of(host: &str, port: u16) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    host.to_ascii_lowercase().hash(&mut hasher);
    port.hash(&mut hasher);
    hasher.finish()
}

/// `host` on `port` as a grant names it: `host:port`, an IPv6 address in brackets.
pub fn host_and_port(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// What is told each connection a proxy carried or refused, coalesced by its [`Tally`]: the host
/// and port, the decision's word ([`super::reach::Decision::word`]: the layer that let it
/// through, or `ask` or `refused`), and how many connections the line stands for. A target of `None` stands for hosts past what one tally tells apart.
pub type Reached = Arc<dyn Fn(Option<&str>, &'static str, u64) + Send + Sync + 'static>;

/// How long one tally line stands for: a host and port is told once a [`TALLY_WINDOW`], with
/// how many connections went to it since it was last told.
pub const TALLY_WINDOW: Duration = Duration::from_secs(60);

/// How often a proxy's timer tells what has waited a [`TALLY_WINDOW`] (#1699).
pub const TALLY_TICK: Duration = Duration::from_secs(5);

/// The most hosts and ports one tally tells apart at once; connections past it are counted
/// together, with no target.
pub const TALLY_HOSTS: usize = 32;

/// **Every connection a proxy carried, coalesced** (#1664): the first to a host and port is told
/// at once; those after it in the same [`TALLY_WINDOW`] are counted and told as one line when
/// the window has passed (by the proxy's timer, [`TALLY_TICK`], or at the next connection,
/// whichever is first) or when the proxy stops. So a chat that
/// opens a thousand connections to its registry is a line or two a minute, never a thousand,
/// and the network record keeps every connection without a chat being able to turn it over.
#[derive(Debug, Default)]
pub struct Tally {
    seen: Vec<Seen>,
    /// Connections past [`TALLY_HOSTS`] since they were last told, per layer, and when they
    /// began.
    others: Vec<(&'static str, Instant, u64)>,
}

#[derive(Debug)]
struct Seen {
    target: String,
    by: &'static str,
    since: Instant,
    /// Connections since it was last told.
    untold: u64,
}

/// One line a [`Tally`] tells: a target (none for the hosts past what it tells apart), the
/// layer, and how many connections.
pub type Line = (Option<String>, &'static str, u64);

impl Tally {
    /// A connection to `target`, let through by `by`, at `now`: the lines to tell now.
    pub fn heard(&mut self, target: &str, by: &'static str, now: Instant) -> Vec<Line> {
        let mut out = self.due(now);
        if let Some(seen) = self
            .seen
            .iter_mut()
            .find(|seen| seen.by == by && seen.target.eq_ignore_ascii_case(target))
        {
            seen.untold += 1;
        } else if self.seen.len() < TALLY_HOSTS {
            self.seen.push(Seen {
                target: target.to_owned(),
                by,
                since: now,
                untold: 0,
            });
            out.push((Some(target.to_owned()), by, 1));
        } else if let Some((_, _, untold)) = self.others.iter_mut().find(|(layer, ..)| *layer == by)
        {
            *untold += 1;
        } else {
            self.others.push((by, now, 1));
        }
        out
    }

    /// **What has waited a window at `now`**, told: each host's count since it was last told,
    /// and the hosts past what it tells apart; a host told with nothing since is let go. What
    /// the proxy's timer asks every few seconds (#1699), so a burst's count is timed near its
    /// window's end, not at the next connection.
    pub fn due(&mut self, now: Instant) -> Vec<Line> {
        let mut out = Vec::new();
        self.seen.retain_mut(|seen| {
            if now.saturating_duration_since(seen.since) < TALLY_WINDOW {
                return true;
            }
            if seen.untold > 0 {
                out.push((Some(seen.target.clone()), seen.by, seen.untold));
            }
            false
        });
        self.others.retain(|(layer, since, untold)| {
            if now.saturating_duration_since(*since) < TALLY_WINDOW {
                return true;
            }
            out.push((None, layer, *untold));
            false
        });
        out
    }

    /// Everything not told yet, as the proxy stops.
    pub fn ended(&mut self) -> Vec<Line> {
        let mut out: Vec<Line> = self
            .seen
            .drain(..)
            .filter(|seen| seen.untold > 0)
            .map(|seen| (Some(seen.target), seen.by, seen.untold))
            .collect();
        out.extend(
            self.others
                .drain(..)
                .map(|(layer, _, untold)| (None, layer, untold)),
        );
        out
    }
}

/// What a proxy carries, and to whom it tells what it did.
#[derive(Clone)]
pub struct Serving {
    /// Each host the chat's sandbox lists, with its layer.
    pub reach: Reach,
    /// The ports a tunnel (HTTP `CONNECT`, SOCKS5) is carried to for a host listed without one.
    pub tunnel_ports: Vec<u16>,
    /// The ports a plain request is carried to for a host listed without one.
    pub plain_ports: Vec<u16>,
    /// Whether a host listed without a port is carried on every port instead (#1665): what a
    /// Claude Code chat's own proxy did, by name alone ([`Reach::decide_on_any_port`]).
    pub any_port: bool,
    pub limits: Limits,
    pub refusals: Refusals,
    /// Told each connection carried, coalesced ([`Tally`]); none tells nobody.
    pub reached: Option<Reached>,
    /// The chat's live asks (#1666): a connection to a host nothing lists is held while the
    /// person is asked. None refuses it at once, as where policy turns asking off.
    pub asks: Option<Arc<Asks>>,
    /// For a chat planned on another machine (#2445): the internal services the runner's
    /// policy lists, the only internal addresses its connections are carried to
    /// ([`super::internal`]). None holds no address to it.
    pub internal: Option<Internal>,
    /// For a chat planned on another machine (#2510, ruled 2026-10-18), or composed here by its
    /// names (#2590): the services of this machine, by port, the window allowed it alone and
    /// this machine's sandbox does not give, so it was started without them
    /// ([`super::plan::Plan::as_the_runner_starts_it`], [`super::plan::Plan::start_leaving_out`]).
    /// Where the proxy asks live, a connection to one on the loopback interface is held while
    /// the person is asked, by its loopback address, and carried once they allow it for this
    /// chat. Any other port of this machine is refused as before.
    pub left_out: Vec<u16>,
    /// **A confined extension's proxy** (ADR 0101 §3, #2777): a tunnel is carried only where the
    /// TLS ClientHello names the host the tunnel was asked for ([`hello_names`]); no address in
    /// a private or special-use range is reached, at any connect, after the name is looked up
    /// ([`super::hosts::special_use`]); no SOCKS request is carried; and its two ports are held
    /// for as long as purlis runs, never handed to another listener ([`Proxy`]'s drop), so
    /// nothing its program left running can reach another proxy at them.
    pub confined: bool,
}

impl Serving {
    /// A chat's proxy for `reach`: tunnels on [`TUNNEL_PORTS`], plain requests on
    /// [`PLAIN_PORTS`], within [`LIMITS`], keeping nothing.
    pub fn of(reach: Reach) -> Self {
        Self {
            reach,
            tunnel_ports: TUNNEL_PORTS.to_vec(),
            plain_ports: PLAIN_PORTS.to_vec(),
            any_port: false,
            limits: LIMITS,
            refusals: Refusals::default(),
            reached: None,
            asks: None,
            internal: None,
            left_out: Vec::new(),
            confined: false,
        }
    }

    /// **A confined extension's proxy for `hosts`** ([`Self::confined`]), refusing anything they
    /// do not name and asking nobody.
    pub fn confined(hosts: Vec<String>) -> Self {
        Self {
            confined: true,
            ..Self::of(Reach::open(hosts))
        }
    }
}

/// The URL a chat's `HTTPS_PROXY` and `HTTP_PROXY` name a proxy on `port` of the loopback
/// interface by, wherever the proxy runs (#1999).
pub fn url_of(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// A running proxy, on a pair of ports of the loopback interface: HTTP and SOCKS5. It stops
/// listening on both when dropped, and tells what its tally still holds; a tunnel already open
/// runs until either end closes it.
pub struct Proxy {
    http: SocketAddr,
    socks: SocketAddr,
    ports: Vec<u16>,
    stop: Arc<AtomicBool>,
    refusals: Refusals,
    allowed: Arc<Allowed>,
    /// The timer telling the tally's due lines (#1699), where anything is told them.
    ticking: Option<std::thread::JoinHandle<()>>,
    /// The teller's thread (#1353).
    telling: Option<std::thread::JoinHandle<()>>,
    /// Whether its ports stay bound and purlis's own once it stops ([`Serving::confined`]).
    keep: bool,
}

impl std::fmt::Debug for Proxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Proxy")
            .field("http", &self.http)
            .field("socks", &self.socks)
            .field("ports", &self.ports)
            .finish_non_exhaustive()
    }
}

impl Proxy {
    /// A proxy that tunnels to `hosts` on [`TUNNEL_PORTS`] and carries plain requests to them
    /// on [`PLAIN_PORTS`].
    pub fn start(hosts: Vec<String>) -> io::Result<Self> {
        Self::start_keeping(hosts, Refusals::default())
    }

    /// [`Self::start`], keeping what it refuses in `refusals`.
    pub fn start_keeping(hosts: Vec<String>, refusals: Refusals) -> io::Result<Self> {
        Self::serving(Serving {
            refusals,
            ..Serving::of(Reach::open(hosts))
        })
    }

    /// A proxy that carries `hosts` on `ports`, tunnels and plain requests alike.
    pub fn allowing(hosts: Vec<String>, ports: Vec<u16>) -> io::Result<Self> {
        Self::limited(hosts, ports, LIMITS)
    }

    /// A proxy that carries `hosts` on `ports`, tunnels and plain requests alike, within
    /// `limits`.
    pub fn limited(hosts: Vec<String>, ports: Vec<u16>, limits: Limits) -> io::Result<Self> {
        Self::serving(Serving {
            tunnel_ports: ports.clone(),
            plain_ports: ports,
            limits,
            ..Serving::of(Reach::open(hosts))
        })
    }

    /// **A chat's proxy**: two new ports on the loopback interface, serving `serving`.
    pub fn serving(serving: Serving) -> io::Result<Self> {
        // Never a port a chat's sandbox lets it reach as a service on this Mac, and each kept
        // as purlis's own while it listens, so no chat is allowed it (qw384).
        let http = super::localport::bind_loopback()?;
        let socks = match super::localport::bind_loopback() {
            Ok(socks) => socks,
            Err(err) => {
                super::localport::release(http.local_addr()?.port());
                return Err(err);
            }
        };
        let (http_addr, socks_addr) = (http.local_addr()?, socks.local_addr()?);
        let stop = Arc::new(AtomicBool::new(false));
        let ports: Vec<u16> = serving
            .tunnel_ports
            .iter()
            .chain(&serving.plain_ports)
            .copied()
            .collect();
        let limits = serving.limits;
        let allowed = Arc::new(Allowed {
            reach: serving.reach,
            tunnel_ports: serving.tunnel_ports,
            plain_ports: serving.plain_ports,
            any_port: serving.any_port,
            idle: limits.idle,
            head: limits.head,
            refusals: serving.refusals.clone(),
            reached: serving.reached,
            asks: serving.asks,
            internal: serving.internal,
            left_out: serving.left_out,
            confined: serving.confined,
            tally: Mutex::new(Tally::default()),
            teller: Teller::default(),
        });
        // The teller (#1353). Where its thread does not start, a refusal is told on the
        // connection's own, after the record keeps it, as before.
        let (handing, handed) = std::sync::mpsc::channel::<Job>();
        let telling = std::thread::Builder::new()
            .name("purlis-egress-teller".into())
            .spawn(move || {
                for job in handed {
                    job();
                }
            })
            .inspect_err(|err| {
                tracing::warn!("purlis: a chat proxy's teller did not start ({err})");
            })
            .ok();
        if telling.is_some() {
            *allowed
                .teller
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(handing);
        }
        // One count for both ports: the limit is the chat's, whichever port it uses.
        let open = Arc::new(AtomicUsize::new(0));
        let keep = serving.confined;
        listen(http, Speaks::Http, &allowed, &open, &stop, limits, keep)?;
        listen(socks, Speaks::Socks, &allowed, &open, &stop, limits, keep)?;
        // Without its timer, a count is told at the next connection or at the stop, as before.
        let ticking = if allowed.reached.is_some() {
            let allowed = Arc::clone(&allowed);
            let stop = Arc::clone(&stop);
            std::thread::Builder::new()
                .name("purlis-egress-tally".into())
                .spawn(move || {
                    loop {
                        std::thread::park_timeout(TALLY_TICK);
                        if stop.load(Ordering::SeqCst) {
                            return;
                        }
                        allowed.tell(|tally| tally.due(Instant::now()));
                    }
                })
                .inspect_err(|err| {
                    tracing::warn!("purlis: a chat proxy's tally timer did not start ({err})");
                })
                .ok()
        } else {
            None
        };
        Ok(Self {
            http: http_addr,
            socks: socks_addr,
            ports,
            stop,
            refusals: serving.refusals,
            allowed,
            ticking,
            telling,
            keep,
        })
    }

    /// Waits until the teller has told every refusal handed to it so far.
    #[cfg(test)]
    pub(crate) fn settled(&self) {
        let (done, waited) = std::sync::mpsc::channel::<()>();
        self.allowed.later(Box::new(move || {
            let _ = done.send(());
        }));
        let _ = waited.recv_timeout(Duration::from_secs(15));
    }

    /// What it has refused because nothing lists it.
    pub fn refusals(&self) -> &Refusals {
        &self.refusals
    }

    /// **`port` of this machine is carried no longer** (qw384b): its grant was removed while
    /// the chat runs ([`Reach::withdraw`]).
    pub fn withdraw_port(&self, port: u16) {
        self.allowed.reach.withdraw(port);
    }

    /// **`port` of this machine, withdrawn, is carried again as it was given** (#2625,
    /// [`Reach::give_back`]).
    pub fn give_back_port(&self, port: u16) {
        self.allowed.reach.give_back(port);
    }

    /// The loopback port its HTTP proxy listens on.
    pub fn port(&self) -> u16 {
        self.http.port()
    }

    /// The loopback port its SOCKS5 proxy listens on.
    pub fn socks_port(&self) -> u16 {
        self.socks.port()
    }

    /// The ports it carries.
    pub fn ports(&self) -> &[u16] {
        &self.ports
    }

    /// The URL a chat's `HTTPS_PROXY` and `HTTP_PROXY` name it by.
    pub fn url(&self) -> String {
        url_of(self.port())
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wakes each accept, which then sees the stop and closes its listener.
        for addr in [self.http, self.socks] {
            let _ = TcpStream::connect_timeout(&addr, Duration::from_secs(1));
        }
        if let Some(ticking) = self.ticking.take() {
            ticking.thread().unpark();
            let _ = ticking.join();
        }
        // What was refused is told before what the tally still holds, waiting a moment at
        // most: a listener stuck on a refusal never holds up a chat's end.
        self.allowed.teller.close();
        if let Some(telling) = self.telling.take() {
            let until = Instant::now() + TELLER_PATIENCE;
            while !telling.is_finished() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(10));
            }
            if telling.is_finished() {
                let _ = telling.join();
            }
        }
        self.allowed.tell(|tally| tally.ended());
        // A confined extension's ports are never handed out again in this run: its listeners are
        // kept ([`KEPT`]) and the ports stay purlis's own.
        if !self.keep {
            super::localport::release(self.http.port());
            super::localport::release(self.socks.port());
        }
    }
}

/// Which protocol a port speaks.
#[derive(Debug, Clone, Copy)]
enum Speaks {
    Http,
    Socks,
}

/// Accepts on `listener` until `stop`, serving each connection on a thread of its own while
/// fewer than `limits.connections` are open across the proxy's ports.
/// The listeners of every confined extension's proxy that has stopped, still bound: no other
/// listener of this machine is given their ports while purlis runs ([`Serving::confined`]).
static KEPT: Mutex<Vec<TcpListener>> = Mutex::new(Vec::new());

#[allow(
    clippy::too_many_arguments,
    reason = "each is one part of a port's service; `keep` is whether its port outlives it"
)]
fn listen(
    listener: TcpListener,
    speaks: Speaks,
    allowed: &Arc<Allowed>,
    open: &Arc<AtomicUsize>,
    stop: &Arc<AtomicBool>,
    limits: Limits,
    keep: bool,
) -> io::Result<()> {
    let (allowed, open, stopping) = (Arc::clone(allowed), Arc::clone(open), Arc::clone(stop));
    std::thread::Builder::new()
        .name("purlis-egress".into())
        .spawn(move || {
            for stream in listener.incoming() {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut stream) = stream else {
                    std::thread::sleep(BACKOFF);
                    continue;
                };
                if open.fetch_add(1, Ordering::SeqCst) >= limits.connections {
                    open.fetch_sub(1, Ordering::SeqCst);
                    let _ = stream.set_write_timeout(Some(BACKOFF));
                    match speaks {
                        Speaks::Http => answer(
                            &mut stream,
                            "503 Service Unavailable",
                            "purlis's egress proxy is carrying all the connections it takes",
                        ),
                        // Not knowing yet what the client asks, the greeting is answered with
                        // no acceptable method, and the connection closed.
                        Speaks::Socks => {
                            let _ = stream.write_all(&[SOCKS_VERSION, NO_METHOD]);
                            let _ = stream.shutdown(Shutdown::Both);
                        }
                    }
                    continue;
                }
                let held = Held(Arc::clone(&open));
                let allowed = Arc::clone(&allowed);
                let _ = std::thread::Builder::new()
                    .name("purlis-egress-conn".into())
                    .spawn(move || {
                        let _held = held;
                        match speaks {
                            Speaks::Http => serve(stream, &allowed),
                            Speaks::Socks => serve_socks(stream, &allowed),
                        }
                    });
            }
            if keep {
                KEPT.lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(listener);
            }
        })?;
    Ok(())
}

/// One connection being served, counted until it ends.
struct Held(Arc<AtomicUsize>);

impl Drop for Held {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

struct Allowed {
    reach: Reach,
    tunnel_ports: Vec<u16>,
    plain_ports: Vec<u16>,
    any_port: bool,
    idle: Duration,
    head: Duration,
    refusals: Refusals,
    reached: Option<Reached>,
    asks: Option<Arc<Asks>>,
    internal: Option<Internal>,
    left_out: Vec<u16>,
    confined: bool,
    tally: Mutex<Tally>,
    teller: Teller,
}

/// What the teller runs: telling one refusal.
type Job = Box<dyn FnOnce() + Send + 'static>;

/// **The proxy's teller** (#1353): where refusals are told, one after another, on a thread of
/// its own, so a slow listener never holds up a connection's answer. Gone once the proxy stops
/// (or where its thread did not start), a job is run where it was handed over, after the
/// answer.
#[derive(Default)]
struct Teller(Mutex<Option<std::sync::mpsc::Sender<Job>>>);

impl Teller {
    /// Hands `job` to the teller's thread, or back where there is none.
    fn hand(&self, job: Job) -> Result<(), Job> {
        match &*self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            Some(sender) => sender.send(job).map_err(|gone| gone.0),
            None => Err(job),
        }
    }

    /// Lets the teller's thread end once it has told what it was handed.
    fn close(&self) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
}

impl Allowed {
    /// Whether `host`, reached at `at`, is carried past what its runner's policy lists, for a
    /// chat planned on another machine (#2445): always, for any other.
    fn carries(&self, host: &str, at: SocketAddr, own: Option<&[super::hosts::Network]>) -> bool {
        // A confined extension reaches no private or special-use address, whatever a name
        // resolved to, at this connect (ADR 0101 §2).
        if self.confined && super::hosts::special_use(at.ip()) {
            return false;
        }
        self.internal
            .as_ref()
            .is_none_or(|internal| internal.carries(host, at, own))
    }

    /// The decision for a connection to `host` on `port`, a tunnel or not.
    fn decide(&self, host: &str, port: u16, tunnel: bool, own: &[std::net::IpAddr]) -> Decision {
        // A service on this Mac is reached by a name of this Mac too (qw384b), never looked up.
        let ports = if self.any_port {
            &[port][..]
        } else if tunnel {
            &self.tunnel_ports
        } else {
            &self.plain_ports
        };
        let decision = self.reach.decide_by_name(host, port, ports, own);
        // A service left out of this chat's plan (#2510): asked about, never refused as this
        // machine or as a name of it that is never a host, where the proxy asks live.
        let refused_here = matches!(
            decision,
            Decision::Ask | Decision::Refused(Refused::LocalAddress | Refused::NeverAHost(_))
        );
        if refused_here && self.asks.is_some() && self.left_out_at(host, port) {
            return Decision::Ask;
        }
        decision
    }

    /// **Whether `host` on `port` is a service left out of this chat's plan** (#2510): one of
    /// [`Serving::left_out`], on this machine's loopback by any name the proxy carries one by,
    /// not withdrawn, and never a port purlis's own proxies listen on.
    ///
    /// **The one way a proxy asks about a port of its machine's loopback.** `left_out` is
    /// filled only on a runner, for a plan made on another machine or one it composed for a
    /// chat started by its names ([`super::hosted::start`], [`super::plan::Plan::start_held`],
    /// [`super::plan::Plan::start_leaving_out`]); a proxy the window planned here never asks
    /// about one. The window relies on that: it raises every loopback ask as the runner's
    /// service (`chats::asked_as` in the app), never as one on the person's own machine.
    fn left_out_at(&self, host: &str, port: u16) -> bool {
        self.left_out.contains(&port)
            && localport::names_loopback(host, port)
            && !self.reach.withdrew(port)
            && localport::still_ports(&[port]) == [port]
    }

    /// Where a connection to `host` on `port` is made once a service left out of this chat's
    /// plan is allowed: its loopback address, never looked up. `None` for any other connection.
    fn pinned(&self, host: &str, port: u16) -> Option<SocketAddr> {
        if !self.left_out_at(host, port) {
            return None;
        }
        let named = host.trim_start_matches('[').trim_end_matches(']');
        let ip = match named.parse::<std::net::IpAddr>() {
            Ok(ip) => ip,
            Err(_) => std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        };
        Some(SocketAddr::new(ip, port))
    }

    /// Connects for `host` on `port`, carried by `decision`: at its pinned loopback address for
    /// a service left out of this chat's plan ([`Self::pinned`]), otherwise as
    /// [`connect_held`].
    fn connect(
        &self,
        host: &str,
        port: u16,
        own: &[std::net::IpAddr],
        read: Option<&[super::hosts::Network]>,
    ) -> Connected {
        match self.pinned(host, port) {
            Some(at) => {
                TcpStream::connect_timeout(&at, PATIENCE).map_or(Connected::Not, Connected::To)
            }
            None => connect_held(host, port, &self.reach, own, &|at| {
                self.carries(host, at, read)
            }),
        }
    }

    /// **The decision, once the person had their say** (#1666): an ask is held on the chat's
    /// board while the person is asked, and carried as an Allowed host at the scope they chose;
    /// refused where they kept it blocked or nobody answered in time. Either refusal is in the
    /// record's tally, and raises no second Block: the ask's Notice stays up. `None` is the
    /// decision as it was: carry it, or refuse it as [`Self::refused`] does.
    fn asked(
        self: &Arc<Self>,
        host: &str,
        port: u16,
        tunnel: bool,
        decision: Decision,
    ) -> Result<Decision, GaveUp> {
        let (Decision::Ask, Some(asks)) = (&decision, &self.asks) else {
            return Ok(decision);
        };
        let ports = if self.any_port {
            vec![port]
        } else if tunnel {
            self.tunnel_ports.clone()
        } else {
            self.plain_ports.clone()
        };
        // A service left out of this chat's plan is asked about by its loopback address,
        // whichever name the chat reached it by, so one Allow answers every spelling (#2510).
        let asked = if self.left_out_at(host, port) {
            super::tunnel::LOOPBACK
        } else {
            host
        };
        match asks.hold(asked, port, &ports) {
            Answer::Allowed(by) => Ok(Decision::by(by)),
            // The board is full, or no Notice can be raised: refused at once as before, with a
            // Block of its own.
            Answer::Busy | Answer::NobodyToAsk => Ok(decision),
            answer => {
                let (allowed, target, by) =
                    (Arc::clone(self), host_and_port(host, port), decision.word());
                self.later(Box::new(move || {
                    allowed.tell(|tally| tally.heard(&target, by, Instant::now()));
                }));
                Err(if answer == Answer::TimedOut {
                    GaveUp::TimedOut
                } else {
                    GaveUp::KeptBlocked
                })
            }
        }
    }

    /// Tells what `step` makes of the tally, outside its lock.
    fn tell(&self, step: impl FnOnce(&mut Tally) -> Vec<Line>) {
        let Some(reached) = &self.reached else { return };
        let lines = step(
            &mut self
                .tally
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        for (target, by, times) in lines {
            reached(target.as_deref(), by, times);
        }
    }

    /// A connection to `host` on `port` carried, by `decision`.
    fn carried(&self, host: &str, port: u16, decision: &Decision) {
        let target = host_and_port(host, port);
        self.tell(|tally| tally.heard(&target, decision.word(), Instant::now()));
    }

    /// The decision's refusal of `host` on `port`, told where it is told: an ask, or a host
    /// that could never be one, as nothing lists it; a local address as such. Counted in the
    /// tally too, so every refused connection is in the record, not only the first a Block is
    /// raised for.
    ///
    /// Kept in the proxy's record here, and told by the teller ([`Teller`], #1353): called
    /// before the client is answered, so the answer waits on no listener, and whoever reads the
    /// record after the answer finds the refusal in it.
    fn refused(self: &Arc<Self>, host: &str, port: u16, decision: &Decision) {
        let local = matches!(decision, Decision::Refused(Refused::LocalAddress));
        let new = if local {
            self.refusals.keep_local(host, port)
        } else {
            self.refusals.keep(host, port)
        };
        let (allowed, host, by) = (Arc::clone(self), host.to_owned(), decision.word());
        self.later(Box::new(move || {
            let target = host_and_port(&host, port);
            allowed.tell(|tally| tally.heard(&target, by, Instant::now()));
            match (new, local) {
                (false, true) => {}
                (false, false) => allowed.refusals.tell_more(),
                (true, true) => allowed.refusals.tell_local(&host, port),
                (true, false) => allowed.refusals.tell(&host, port),
            }
        }));
    }

    /// Runs `job` on the teller's thread, or here where there is none.
    fn later(&self, job: Job) {
        if let Err(job) = self.teller.hand(job) {
            job();
        }
    }
}

/// How a held connection ended without being carried (#1666).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GaveUp {
    TimedOut,
    KeptBlocked,
}

/// Why a held connection was refused, as the client is told: fixed words, so a chat reads what
/// to do next.
fn held_said(host: &str, port: u16, held: GaveUp) -> String {
    match held {
        GaveUp::TimedOut => format!(
            "purlis's sandbox does not allow {host}:{port} yet: the person was asked while \
             this connection waited, and nobody answered in time. The ask stays with them; once \
             they allow it, run the command again"
        ),
        GaveUp::KeptBlocked => format!(
            "purlis's sandbox does not allow {host}:{port}: the person kept it blocked. Do not \
             try it again unless they ask"
        ),
    }
}

/// Why a decision refused, as the client is told.
fn refusal_said(host: &str, port: u16, decision: &Decision) -> String {
    match decision {
        Decision::Refused(Refused::Policy) => format!(
            "purlis's sandbox does not allow {host}:{port}: an administrator's policy pins it as \
             never allowed"
        ),
        Decision::Refused(Refused::LocalAddress) => format!(
            "purlis's sandbox does not allow {host}:{port}: it is this machine, a link-local \
             address, a cloud metadata service, or, for a chat on a runner, an internal address \
             the runner's policy does not list"
        ),
        _ => format!(
            "purlis's sandbox does not allow {host}:{port}: no egress preset or host of this \
             project lists it"
        ),
    }
}

/// One client connection: its request is read, judged, and either carried or refused.
fn serve(mut client: TcpStream, allowed: &Arc<Allowed>) {
    let _ = client.set_write_timeout(Some(allowed.idle.min(PATIENCE)));
    let Some((head, rest)) = read_head(&mut client, allowed.head) else {
        answer(
            &mut client,
            "400 Bad Request",
            "purlis's egress proxy: no request",
        );
        return;
    };
    let Some(request) = Request::parse(&head) else {
        answer(
            &mut client,
            "400 Bad Request",
            "purlis's egress proxy carries CONNECT, and plain absolute-form requests whose \
             body states its length, only",
        );
        return;
    };
    if let Some(named) = &request.host_header
        && !same_host(named, &request.host, request.port)
    {
        answer(
            &mut client,
            "403 Forbidden",
            &format!(
                "purlis's egress proxy carries a request to {} only when its Host names it",
                request.host
            ),
        );
        return;
    }
    // This machine's networks, read once for the decision and the connect.
    let read = super::hosts::own_networks_read();
    let own: Vec<std::net::IpAddr> = read.iter().flatten().map(|one| one.address).collect();
    let tunnel = request.forward.is_none();
    let decision = allowed.decide(&request.host, request.port, tunnel, &own);
    let decision = match allowed.asked(&request.host, request.port, tunnel, decision) {
        Ok(decision) => decision,
        Err(held) => {
            answer(
                &mut client,
                "403 Forbidden",
                &held_said(&request.host, request.port, held),
            );
            return;
        }
    };
    if !decision.carries() {
        allowed.refused(&request.host, request.port, &decision);
        answer(
            &mut client,
            "403 Forbidden",
            &refusal_said(&request.host, request.port, &decision),
        );
        return;
    }
    let _ = client.set_write_timeout(Some(allowed.idle));
    let mut upstream = match allowed.connect(&request.host, request.port, &own, read.as_deref()) {
        Connected::To(upstream) => upstream,
        Connected::OnlyLocal => {
            let local = Decision::Refused(Refused::LocalAddress);
            allowed.refused(&request.host, request.port, &local);
            answer(
                &mut client,
                "403 Forbidden",
                &refusal_said(&request.host, request.port, &local),
            );
            return;
        }
        Connected::Not => {
            answer(
                &mut client,
                "502 Bad Gateway",
                &format!("purlis's egress proxy could not reach {}", request.host),
            );
            return;
        }
    };
    let _ = upstream.set_write_timeout(Some(allowed.idle));
    let sent = match &request.forward {
        None => client.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n"),
        Some(head) => upstream.write_all(head.as_bytes()),
    };
    if sent.is_err() {
        return;
    }
    // A confined extension's tunnel is carried only where its TLS ClientHello names the host
    // the tunnel was asked for: the hello is read here and sent on whole, never decrypted.
    let rest: Vec<u8> = if allowed.confined && request.forward.is_none() {
        match hello_naming(&mut client, &rest, &request.host, allowed.head) {
            Some(hello) => hello,
            None => {
                let named = Decision::Refused(Refused::NeverAHost(format!(
                    "the tunnel to {} did not name it in its TLS ClientHello",
                    request.host
                )));
                allowed.refused(&request.host, request.port, &named);
                let _ = client.shutdown(Shutdown::Both);
                let _ = upstream.shutdown(Shutdown::Both);
                return;
            }
        }
    } else {
        rest
    };
    allowed.carried(&request.host, request.port, &decision);
    // A plain request's body, and nothing after it; a tunnel's bytes, all of them.
    let up_to = request.body.unwrap_or(u64::MAX);
    let rest = &rest[..rest.len().min(usize::try_from(up_to).unwrap_or(usize::MAX))];
    if upstream.write_all(rest).is_err() {
        return;
    }
    splice(client, upstream, up_to - rest.len() as u64, allowed.idle);
}

/// SOCKS's version byte (RFC 1928).
const SOCKS_VERSION: u8 = 5;

/// The one method served: no authentication. A port is one chat's, so nothing a client sends
/// is asked to say whose it is.
const NO_AUTH: u8 = 0;

/// The method reply that refuses every method offered.
const NO_METHOD: u8 = 0xff;

/// SOCKS's reply codes the proxy answers with.
mod reply {
    pub const SUCCEEDED: u8 = 0;
    pub const NOT_ALLOWED: u8 = 2;
    pub const HOST_UNREACHABLE: u8 = 4;
    pub const COMMAND_NOT_SUPPORTED: u8 = 7;
    pub const ADDRESS_NOT_SUPPORTED: u8 = 8;
}

/// One SOCKS5 client connection: the greeting, one `CONNECT`, judged as a tunnel is, then
/// carried or refused with SOCKS's own reply. The whole of the greeting and the request must
/// arrive within the head's deadline, and neither is more than a few hundred bytes.
fn serve_socks(mut client: TcpStream, allowed: &Arc<Allowed>) {
    let _ = client.set_write_timeout(Some(allowed.idle.min(PATIENCE)));
    // A confined extension's proxy carries tunnels its HTTP port judges, the ClientHello among
    // what it judges ([`Serving::confined`]), and no SOCKS request.
    if allowed.confined {
        let _ = client.write_all(&[SOCKS_VERSION, NO_METHOD]);
        let _ = client.shutdown(Shutdown::Both);
        return;
    }
    let deadline = Instant::now() + allowed.head;
    let Some(target) = socks_request(&mut client, deadline) else {
        let _ = client.shutdown(Shutdown::Both);
        return;
    };
    let (host, port) = match target {
        Ok(target) => target,
        Err(code) => return socks_reply(&mut client, code),
    };
    let read = super::hosts::own_networks_read();
    let own: Vec<std::net::IpAddr> = read.iter().flatten().map(|one| one.address).collect();
    let decision = match allowed.asked(&host, port, true, allowed.decide(&host, port, true, &own)) {
        Ok(decision) => decision,
        Err(_) => return socks_reply(&mut client, reply::NOT_ALLOWED),
    };
    if !decision.carries() {
        allowed.refused(&host, port, &decision);
        return socks_reply(&mut client, reply::NOT_ALLOWED);
    }
    let _ = client.set_write_timeout(Some(allowed.idle));
    let upstream = match allowed.connect(&host, port, &own, read.as_deref()) {
        Connected::To(upstream) => upstream,
        Connected::OnlyLocal => {
            allowed.refused(&host, port, &Decision::Refused(Refused::LocalAddress));
            return socks_reply(&mut client, reply::NOT_ALLOWED);
        }
        Connected::Not => return socks_reply(&mut client, reply::HOST_UNREACHABLE),
    };
    let _ = upstream.set_write_timeout(Some(allowed.idle));
    if client.write_all(&socks_answer(reply::SUCCEEDED)).is_err() {
        return;
    }
    allowed.carried(&host, port, &decision);
    splice(client, upstream, u64::MAX, allowed.idle);
}

/// Answers a SOCKS request with `code` and closes the connection.
fn socks_reply(client: &mut TcpStream, code: u8) {
    let _ = client.write_all(&socks_answer(code));
    let _ = client.shutdown(Shutdown::Both);
}

/// SOCKS's reply with `code`. The bound address is not this proxy's to say: zeros, as many
/// proxies answer.
fn socks_answer(code: u8) -> [u8; 10] {
    [SOCKS_VERSION, code, 0, 1, 0, 0, 0, 0, 0, 0]
}

/// Reads the greeting, answers it, and reads the request: the host and port a `CONNECT` names,
/// or the reply code that refuses what it asked. `None` where the client said nothing a SOCKS5
/// client says, or not in time: the connection is closed with no reply.
fn socks_request(client: &mut TcpStream, deadline: Instant) -> Option<Result<(String, u16), u8>> {
    let [version, methods] = read_exact::<2>(client, deadline)?;
    if version != SOCKS_VERSION || methods == 0 {
        return None;
    }
    let mut offered = [0u8; 255];
    read_into(client, &mut offered[..usize::from(methods)], deadline)?;
    if !offered[..usize::from(methods)].contains(&NO_AUTH) {
        let _ = client.write_all(&[SOCKS_VERSION, NO_METHOD]);
        return None;
    }
    client.write_all(&[SOCKS_VERSION, NO_AUTH]).ok()?;
    let [version, command, _, kind] = read_exact::<4>(client, deadline)?;
    if version != SOCKS_VERSION {
        return None;
    }
    let host = match kind {
        1 => std::net::Ipv4Addr::from(read_exact::<4>(client, deadline)?).to_string(),
        3 => {
            let [len] = read_exact::<1>(client, deadline)?;
            let mut name = [0u8; 255];
            let name = &mut name[..usize::from(len)];
            read_into(client, name, deadline)?;
            match std::str::from_utf8(name) {
                Ok(name) if !name.is_empty() && name.bytes().all(|b| b.is_ascii_graphic()) => {
                    name.to_owned()
                }
                _ => return Some(Err(reply::ADDRESS_NOT_SUPPORTED)),
            }
        }
        4 => std::net::Ipv6Addr::from(read_exact::<16>(client, deadline)?).to_string(),
        _ => return Some(Err(reply::ADDRESS_NOT_SUPPORTED)),
    };
    let port = u16::from_be_bytes(read_exact::<2>(client, deadline)?);
    if command != 1 {
        return Some(Err(reply::COMMAND_NOT_SUPPORTED));
    }
    Some(Ok((host, port)))
}

/// `N` bytes from `client`, all of them before `deadline`.
fn read_exact<const N: usize>(client: &mut TcpStream, deadline: Instant) -> Option<[u8; N]> {
    let mut buf = [0u8; N];
    read_into(client, &mut buf, deadline)?;
    Some(buf)
}

/// Fills `buf` from `client` before `deadline`, however slowly each byte comes.
fn read_into(client: &mut TcpStream, buf: &mut [u8], deadline: Instant) -> Option<()> {
    let mut have = 0;
    while have < buf.len() {
        let left = deadline.checked_duration_since(Instant::now())?;
        if left.is_zero() || client.set_read_timeout(Some(left)).is_err() {
            return None;
        }
        match client.read(&mut buf[have..]) {
            Ok(0) => return None,
            Ok(n) => have += n,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
    Some(())
}

/// Whether a `Host` header's `named` is `host` on `port`.
pub(super) fn same_host(named: &str, host: &str, port: u16) -> bool {
    let (name, named_port) = match named.rsplit_once(':') {
        Some((name, port)) if !name.ends_with(']') || name.starts_with('[') => {
            (name, port.parse().ok())
        }
        _ => (named, None),
    };
    let name = name
        .strip_prefix('[')
        .and_then(|it| it.strip_suffix(']'))
        .unwrap_or(name);
    let name = name.strip_suffix('.').unwrap_or(name);
    let host = host.strip_suffix('.').unwrap_or(host);
    name.eq_ignore_ascii_case(host) && named_port.unwrap_or(80) == port
}

/// The request's head, as text, and whatever the client sent after it.
pub(super) fn read_head(client: &mut TcpStream, within: Duration) -> Option<(String, Vec<u8>)> {
    let deadline = Instant::now() + within;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        // The whole head within the deadline, however slowly each byte comes.
        let left = deadline.checked_duration_since(Instant::now())?;
        if left.is_zero() || client.set_read_timeout(Some(left)).is_err() {
            return None;
        }
        let n = client.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let rest = buf.split_off(end + 4);
            return String::from_utf8(buf).ok().map(|head| (head, rest));
        }
        if buf.len() > HEAD_MAX {
            return None;
        }
    }
}

/// A request the proxy can carry.
pub(super) struct Request {
    pub(super) host: String,
    pub(super) port: u16,
    /// For a plain request, the head sent on in its place; none for a tunnel.
    pub(super) forward: Option<String>,
    /// For a plain request, its `Host` header, where it has one.
    pub(super) host_header: Option<String>,
    /// For a plain request, its body's length; none for a tunnel.
    pub(super) body: Option<u64>,
}

impl Request {
    /// The request in `head`, which ends in its blank line, read strictly (see the module's
    /// docs): `None` for anything two parsers could read two ways.
    pub(super) fn parse(head: &str) -> Option<Self> {
        let lines: Vec<&str> = head.strip_suffix("\r\n\r\n")?.split("\r\n").collect();
        if lines.iter().any(|line| line.contains(['\r', '\n'])) {
            return None;
        }
        let parts: Vec<&str> = lines[0].split(' ').collect();
        let [method, target, version] = parts[..] else {
            return None;
        };
        if !matches!(version, "HTTP/1.1" | "HTTP/1.0")
            || method.is_empty()
            || !method.bytes().all(|b| b.is_ascii_uppercase())
        {
            return None;
        }
        let mut headers = Vec::new();
        for line in &lines[1..] {
            // A folded line, a name with a space before its colon, a line with no colon.
            let (name, value) = line.split_once(':')?;
            if name.is_empty() || !name.bytes().all(is_token) {
                return None;
            }
            let value = value.trim_matches([' ', '\t']);
            if value.bytes().any(|b| (b < 0x20 && b != b'\t') || b == 0x7f) {
                return None;
            }
            headers.push((name, value));
        }
        let named = |wanted: &str| -> Vec<&str> {
            headers
                .iter()
                .filter(|(name, _)| name.eq_ignore_ascii_case(wanted))
                .map(|(_, value)| *value)
                .collect()
        };
        let hosts = named("host");
        let lengths = named("content-length");
        if hosts.len() > 1 || lengths.len() > 1 || !named("transfer-encoding").is_empty() {
            return None;
        }
        if method == "CONNECT" {
            let (host, port) = host_port(target)?;
            return Some(Self {
                host,
                port,
                forward: None,
                // A tunnel's `Host` says nothing the target does not; the target decides.
                host_header: None,
                body: None,
            });
        }
        let [host_header] = hosts[..] else {
            return None;
        };
        let after = target.strip_prefix("http://")?;
        let (authority, path) = match after.find('/') {
            Some(at) => (&after[..at], &after[at..]),
            None => (after, "/"),
        };
        let (host, port) = match authority.rsplit_once(':') {
            Some(_) => host_port(authority)?,
            None => (authority.to_owned(), 80),
        };
        if host.is_empty() || authority.contains('@') {
            return None;
        }
        let body = match lengths[..] {
            [] => 0,
            [length] if !length.is_empty() && length.bytes().all(|b| b.is_ascii_digit()) => {
                length.parse().ok()?
            }
            _ => return None,
        };
        let mut forward = format!("{method} {path} {version}\r\n");
        for (name, value) in &headers {
            if [
                "connection",
                "proxy-connection",
                "proxy-authorization",
                "keep-alive",
            ]
            .iter()
            .any(|it| name.eq_ignore_ascii_case(it))
            {
                continue;
            }
            forward.push_str(&format!("{name}: {value}\r\n"));
        }
        forward.push_str("Connection: close\r\n\r\n");
        Some(Self {
            host,
            port,
            forward: Some(forward),
            host_header: Some(host_header.to_owned()),
            body: Some(body),
        })
    }
}

/// Whether `byte` may be in a header's name (RFC 9110's token).
fn is_token(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

/// `host:port`, with an IPv6 literal's brackets taken off.
fn host_port(authority: &str) -> Option<(String, u16)> {
    let (host, port) = authority.rsplit_once(':')?;
    let host = host
        .strip_prefix('[')
        .and_then(|it| it.strip_suffix(']'))
        .unwrap_or(host);
    if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let port = port.parse().ok()?;
    (!host.is_empty()).then(|| (host.to_owned(), port))
}

/// What a connect came to.
pub(crate) enum Connected {
    To(TcpStream),
    /// The name resolved, and only to addresses a chat never reaches: refused, as a Block.
    OnlyLocal,
    /// It did not resolve, or nothing answered.
    Not,
}

/// Connects to `host` on `port`, only at the addresses it resolves to now that a chat may reach
/// ([`reachable`]), checked at every connect so a name rebound to this machine or a metadata
/// service reaches nothing. An address literal was decided as itself
/// ([`super::reach::Reach::decide`]) and is held to the same check here.
pub(crate) fn connect(host: &str, port: u16, reach: &Reach, own: &[std::net::IpAddr]) -> Connected {
    connect_held(host, port, reach, own, &|_| true)
}

/// [`connect`], only at the addresses `carries` keeps besides (#2445): what a chat planned on
/// another machine reaches of its runner's networks ([`super::internal`]), held at every connect
/// since a name can resolve to another address later.
pub(crate) fn connect_held(
    host: &str,
    port: u16,
    reach: &Reach,
    own: &[std::net::IpAddr],
    carries: &dyn Fn(SocketAddr) -> bool,
) -> Connected {
    let resolve = |host: &str, port: u16| {
        (host, port)
            .to_socket_addrs()
            .map(Iterator::collect)
            .unwrap_or_default()
    };
    // Looked up once at most: what answered is remembered, so an empty answer tells apart from
    // one with only addresses a chat never reaches.
    let answered = std::cell::Cell::new(true);
    let once = |host: &str, port: u16| {
        let found: Vec<SocketAddr> = resolve(host, port);
        answered.set(!found.is_empty());
        found
    };
    let mut reached = addresses(host, port, reach, own, &once);
    reached.retain(|at| carries(*at));
    if reached.is_empty() {
        return if answered.get() {
            Connected::OnlyLocal
        } else {
            Connected::Not
        };
    }
    reached
        .into_iter()
        .find_map(|addr| TcpStream::connect_timeout(&addr, PATIENCE).ok())
        .map_or(Connected::Not, Connected::To)
}

/// **The addresses a connection to `host` on `port` is made at** (#2097 review): a name of this
/// Mac on a port the chat holds is pinned to this machine's loopback and never looked up
/// ([`Reach::address_of`]); any other name is looked up now, by `resolve`, and only the
/// addresses a chat may reach are kept ([`reachable`]).
pub fn addresses(
    host: &str,
    port: u16,
    reach: &Reach,
    own: &[std::net::IpAddr],
    resolve: &dyn Fn(&str, u16) -> Vec<SocketAddr>,
) -> Vec<SocketAddr> {
    match reach.address_of(host, port) {
        Some(pinned) => vec![pinned],
        None => reachable_from(&resolve(host, port), reach, own),
    }
}

/// The addresses `host` on `port` is reached at, as `resolve` answers: each one a chat may
/// reach ([`super::hosts::refused_address`], with `own` this machine's interface addresses),
/// and one it may not only where `reach` lists that exact address and port. An address literal
/// is held to it as a name is.
pub fn reachable(
    host: &str,
    port: u16,
    resolve: &dyn Fn(&str, u16) -> Vec<SocketAddr>,
    reach: &Reach,
    own: &[std::net::IpAddr],
) -> Vec<SocketAddr> {
    reachable_from(&resolve(host, port), reach, own)
}

fn reachable_from(
    resolved: &[SocketAddr],
    reach: &Reach,
    own: &[std::net::IpAddr],
) -> Vec<SocketAddr> {
    resolved
        .iter()
        .copied()
        .filter(|addr| {
            !super::hosts::refused_address(addr.ip(), own)
                || reach.lists_exactly(addr.ip(), addr.port())
        })
        .collect()
}

/// The TLS extension that carries an encrypted ClientHello (ECH).
const ENCRYPTED_CLIENT_HELLO: usize = 0xfe0d;

/// The most a TLS record may hold: 2^14 bytes of plaintext and the record's own overhead.
const MOST_TLS_RECORD: usize = (1 << 14) + 2048;

/// **The ClientHello a tunnel's client sent, where it names `host`**, read from `client` after
/// `already` (what arrived with the request's head) within `within`: the whole first TLS
/// record, returned to be sent on unchanged. `None` where it is no TLS handshake, does not hold
/// a whole ClientHello in its first record, names no server, or names another than `host`.
fn hello_naming(
    client: &mut TcpStream,
    already: &[u8],
    host: &str,
    within: Duration,
) -> Option<Vec<u8>> {
    let deadline = Instant::now() + within;
    let mut hello = already.to_vec();
    let mut chunk = [0u8; 4096];
    let whole = |hello: &[u8]| {
        (hello.len() >= 5).then(|| 5 + usize::from(u16::from_be_bytes([hello[3], hello[4]])))
    };
    loop {
        if let Some(record) = whole(&hello)
            && (hello[0] != 0x16 || record > MOST_TLS_RECORD || hello.len() >= record)
        {
            break;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || hello.len() > MOST_TLS_RECORD {
            return None;
        }
        let _ = client.set_read_timeout(Some(left));
        match client.read(&mut chunk) {
            Ok(0) | Err(_) => return None,
            Ok(n) => hello.extend_from_slice(&chunk[..n]),
        }
    }
    let record = whole(&hello)?;
    let named = hello_names(hello.get(..record)?)?;
    let wanted = host.trim_end_matches('.');
    named.eq_ignore_ascii_case(wanted).then_some(hello)
}

/// **The server name a TLS record holding a whole ClientHello names** (RFC 8446 §4.1.2, RFC
/// 6066 §3), read within the record's bounds, or `None` for anything else: one carrying an
/// encrypted ClientHello among them, whose real server name is inside it.
pub(crate) fn hello_names(record: &[u8]) -> Option<String> {
    let take = |at: &mut usize, n: usize, from: &[u8]| -> Option<std::ops::Range<usize>> {
        let range = *at..at.checked_add(n)?;
        from.get(range.clone())?;
        *at = range.end;
        Some(range)
    };
    let len16 = |from: &[u8], range: std::ops::Range<usize>| {
        usize::from(u16::from_be_bytes([
            from[range.start],
            from[range.start + 1],
        ]))
    };
    // The record: a handshake, its length within what was read.
    if record.first() != Some(&0x16) || record.len() < 5 {
        return None;
    }
    let body = record.get(5..5 + usize::from(u16::from_be_bytes([record[3], record[4]])))?;
    // The handshake: a ClientHello, whole in this record.
    if body.first() != Some(&1) || body.len() < 4 {
        return None;
    }
    let length = (usize::from(body[1]) << 16) | (usize::from(body[2]) << 8) | usize::from(body[3]);
    let hello = body.get(4..4 + length)?;
    let mut at = 0usize;
    take(&mut at, 2 + 32, hello)?;
    let session = take(&mut at, 1, hello)?;
    take(&mut at, usize::from(hello[session.start]), hello)?;
    let suites = take(&mut at, 2, hello)?;
    take(&mut at, len16(hello, suites), hello)?;
    let methods = take(&mut at, 1, hello)?;
    take(&mut at, usize::from(hello[methods.start]), hello)?;
    let extensions = take(&mut at, 2, hello)?;
    let extensions = take(&mut at, len16(hello, extensions), hello)?;
    let extensions = &hello[extensions];
    let mut at = 0usize;
    let mut named: Option<String> = None;
    while at < extensions.len() {
        let kind = take(&mut at, 2, extensions)?;
        let size = take(&mut at, 2, extensions)?;
        let data = take(&mut at, len16(extensions, size), extensions)?;
        // An encrypted ClientHello (ECH) names its real server inside, where the proxy does not
        // read: the outer name says nothing of where the tunnel goes, so none is carried.
        let kind = len16(extensions, kind);
        if kind == ENCRYPTED_CLIENT_HELLO {
            return None;
        }
        if kind != 0 {
            continue;
        }
        // One server_name extension, holding one host name.
        if named.is_some() {
            return None;
        }
        let data = &extensions[data];
        let mut inner = 0usize;
        let list = take(&mut inner, 2, data)?;
        let list = data.get(inner..inner + len16(data, list))?;
        let mut inner = 0usize;
        let kind = take(&mut inner, 1, list)?;
        if list[kind.start] != 0 {
            return None;
        }
        let size = take(&mut inner, 2, list)?;
        let name = take(&mut inner, len16(list, size), list)?;
        if inner != list.len() {
            return None;
        }
        let name = std::str::from_utf8(&list[name]).ok()?;
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_graphic()) {
            return None;
        }
        named = Some(name.trim_end_matches('.').to_owned());
    }
    named
}

pub(super) fn answer(client: &mut TcpStream, status: &str, why: &str) {
    let _ = write!(
        client,
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\n\
         Content-Length: {}\r\n\r\n{why}",
        why.len()
    );
    let _ = client.shutdown(Shutdown::Both);
}

/// Copies each way until either side ends, at most `up_to` more bytes from the client, and
/// closes both once nothing has moved either way for `idle`.
pub(crate) fn splice(client: TcpStream, upstream: TcpStream, up_to: u64, idle: Duration) {
    let (Ok(client_in), Ok(upstream_out)) = (client.try_clone(), upstream.try_clone()) else {
        return;
    };
    let moved = Arc::new(Mutex::new(Instant::now()));
    let up_moved = Arc::clone(&moved);
    let up = std::thread::Builder::new()
        .name("charter-egress-up".into())
        .spawn(move || {
            pump(client_in, &upstream_out, up_to, idle, &up_moved);
            let _ = upstream_out.shutdown(Shutdown::Write);
        });
    pump(upstream.try_clone().ok(), &client, u64::MAX, idle, &moved);
    // Either side done, or both idle: the whole connection ends.
    let _ = client.shutdown(Shutdown::Both);
    let _ = upstream.shutdown(Shutdown::Both);
    if let Ok(up) = up {
        let _ = up.join();
    }
}

/// Copies from `from` to `to` until `from` ends, `up_to` bytes have gone, or nothing has moved
/// either way (`moved`) for `idle`.
fn pump(
    from: impl Into<Option<TcpStream>>,
    mut to: &TcpStream,
    up_to: u64,
    idle: Duration,
    moved: &Mutex<Instant>,
) {
    let Some(mut from) = from.into() else { return };
    let tick = idle
        .min(Duration::from_secs(5))
        .max(Duration::from_millis(50));
    if from.set_read_timeout(Some(tick)).is_err() {
        return;
    }
    let mut left = up_to;
    let mut buf = [0u8; 16 * 1024];
    while left > 0 {
        let want = buf.len().min(usize::try_from(left).unwrap_or(usize::MAX));
        match from.read(&mut buf[..want]) {
            Ok(0) => return,
            Ok(n) => {
                if to.write_all(&buf[..n]).is_err() {
                    return;
                }
                left -= n as u64;
                *moved
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Instant::now();
            }
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                let last = *moved
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if last.elapsed() >= idle {
                    return;
                }
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return,
        }
    }
}

#[cfg(test)]
mod tests;
