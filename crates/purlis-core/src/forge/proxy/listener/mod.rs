// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The forge proxy's listener** (one login, slice A2; ADR 0128 D3a to D3c, D3g): the part that
//! takes a chat's connection, reads one request, asks [`ForgeProxy`] and sends what it allows to
//! the forge, behind the `forge-wiring` feature.
//!
//! Two doors, both made for one chat:
//!
//! - **a unix socket** ([`ForgeListener::serve_socket`]) for `gh`: plain HTTP, the forge named by
//!   `Host` (`github.localhost` and its API host stand for the chat's one GitHub host). The folder
//!   is mode 0700, the socket 0600 and the app's, an existing folder that is not is refused, the
//!   umask is held while the socket is bound, and where the platform gives a connection's peer
//!   credentials a peer of another user is closed unread ([`UnixEndpoint`](super::UnixEndpoint));
//! - **`CONNECT` on a loopback port** ([`ForgeListener::serve_connect`]) for `git` and `glab`
//!   through `HTTPS_PROXY`: a forge host the chat recorded is **terminated** with the chat's own
//!   authority ([`ChatCa`]: memory-only key, critical name constraints, a leaf for those hosts
//!   alone), a download-only host (`codeload`, `release-assets`, `objects`) is **tunnelled
//!   untouched**, and everything else is refused. The `CONNECT` is read by
//!   [`serve_connect_stream`], which ADR 0067's egress port calls with a connection it has read the
//!   `CONNECT` line of when that port fronts this route.
//!
//! **One request per connection, strict framing** ([`http`]); **the upstream is strict TLS with the
//! system's roots**, ignores the chat's environment and never trusts the chat's authority
//! ([`upstream`]); **an answer is read before the chat gets it** ([`scrub`]): the real credential in
//! it is never sent, a credentialed clone URL is cut down, `Location` and `Link` are written again
//! for the chat, and a redirect is never followed with a credential ([`serve`]). **SNI, the
//! `CONNECT` target and `Host` must be one name**: the leaf is made for the target and handed only
//! to a client that asks for it by name, and the core refuses a `Host` that differs.
//!
//! **Tier** (ADR 0069): memory only. The authority's key, its leaves, the sentinels and the audit
//! are held by one running app and gone with it; the one thing that leaves memory is the
//! authority's *certificate* ([`ChatCa::certificate_pem`]), which the caller writes to the chat's
//! own folder (`docs/plane-format.md`, "The forge proxy's per-chat folder", Machine tier,
//! rebuildable). The socket is a file for as long as the listener lives.
//!
//! **The seam to slice B.** [`Wiring`] is plain values: the chat's id, the shared core, the
//! upstream and the dialer. Where slice B's route inputs (`forgewire`) name the chat's repos and
//! its folder, they are [`ForgeProxy::enrol`]'s and the caller's `dir`, and nothing here reads
//! more of them.

mod ca;
mod gitsay;
mod http;
mod lfs;
mod scrub;
mod serve;
mod upstream;

use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub use ca::{CaError, ChatCa};
pub use lfs::LfsStorage;
pub use upstream::{StrictUpstream, Upstream, UpstreamError, UpstreamRequest, UpstreamResponse};

use self::http::{Framed, HeadError, message, write_answer, write_head};
use self::serve::{Route, serve_one};
use super::{ChatEndpoint, ChatId, ConnectVerdict, EndpointKind, ForgeProxy, LoopbackEndpoint};
use crate::sandbox::egress::handoff::{self, Secret};

/// What a connection is read under.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// A request's head.
    pub head: usize,
    /// A JSON or form body, and the body of a read.
    pub body: usize,
    /// A push, whose ref commands are read before any of it goes on.
    pub push: usize,
    /// How long a read or a write may wait on a client.
    pub io_timeout: Duration,
    /// How long one request may take to be read, and one answer to be sent, however steadily the
    /// client sends or reads: the idle timeout alone lets a client that drips a byte a minute
    /// hold a connection (and one of the few slots) for ever.
    pub total: Duration,
    /// Connections served at once; one more is closed.
    pub connections: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            head: 64 * 1024,
            body: 1 << 20,
            push: 32 << 20,
            io_timeout: Duration::from_secs(60),
            total: Duration::from_secs(10 * 60),
            connections: 64,
        }
    }
}

/// How the listener opens a tunnel: a name to a connection. [`PublicDial`] is the app's; a test
/// puts a fake forge's address in its place.
pub trait Dial: Send + Sync {
    fn dial(&self, host: &str, port: u16) -> io::Result<TcpStream>;
}

/// Dials a host's public addresses and no other: a name that resolves to this machine, a private
/// network or a link-local address is not a way to turn the proxy on the person's network.
#[derive(Debug, Default, Clone, Copy)]
pub struct PublicDial;

impl Dial for PublicDial {
    fn dial(&self, host: &str, port: u16) -> io::Result<TcpStream> {
        let mut last = io::Error::new(io::ErrorKind::NotFound, "the name has no public address");
        for address in (host, port).to_socket_addrs()? {
            if !is_public(address.ip()) {
                continue;
            }
            match TcpStream::connect_timeout(&address, Duration::from_secs(15)) {
                Ok(stream) => return Ok(stream),
                Err(e) => last = e,
            }
        }
        Err(last)
    }
}

/// Whether an address is one on the public internet.
#[must_use]
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
                || v4.is_multicast()
                || o[0] == 0
                // 100.64.0.0/10, carrier-grade NAT
                || (o[0] == 100 && (o[1] & 0xc0) == 64)
                // 192.0.0.0/24, IETF protocol assignments
                || (o[0] == 192 && o[1] == 0 && o[2] == 0)
                // 198.18.0.0/15, benchmarking
                || (o[0] == 198 && (o[1] & 0xfe) == 18)
                || o[0] >= 240)
        }
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_public(IpAddr::V4(v4)),
            None => {
                let first = v6.segments()[0];
                !(v6.is_loopback()
                    || v6.is_unspecified()
                    || v6.is_multicast()
                    // fc00::/7 unique local, fe80::/10 link-local
                    || (first & 0xfe00) == 0xfc00
                    || (first & 0xffc0) == 0xfe80)
            }
        },
    }
}

/// Sees every request a listener reads, with its body, before the core decides it. For a test or
/// a capture of what a tool sent; the app sets none. What it is given is what the chat sent, which
/// holds no credential of the person's.
pub trait Observe: Send + Sync {
    fn request(&self, head: &super::RequestHead, body: &[u8]);
}

/// One ref a push moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushUpdate {
    pub old: String,
    pub new: String,
    pub name: String,
}

/// A push as the listener read it, before any of it goes to the forge: the repo, the refs it moves
/// and the pack it sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushRead {
    pub host: String,
    /// The port the push goes to: a repo confirmed on `host:443` is not the one on `host:8443`.
    pub port: u16,
    /// The repo's path as the proxy recorded it (`owner/name`).
    pub repo: String,
    pub updates: Vec<PushUpdate>,
    /// The packfile after the commands (empty for a push that only deletes).
    pub pack: Vec<u8>,
}

/// What a [`PushGate`] says of a push.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushVerdict {
    /// Nothing the person must see in it: send it on.
    Forward,
    /// Not sent, and not asked: the sentence says why.
    Refuse(String),
    /// Not sent: an ask was raised for the person, and the sentence names it.
    Held(String),
}

/// **The check of a push's objects** (ADR 0128 D8.3), made by the app, which alone holds the forge
/// and a place for the pack. The listener asks it before it forwards a push, and forwards only on
/// [`PushVerdict::Forward`]: a push that changes CI files is never sent on by this door.
pub trait PushGate: Send + Sync {
    fn check(&self, chat: &ChatId, push: &PushRead) -> PushVerdict;
}

/// Everything one chat's listener needs, as plain values.
#[derive(Clone)]
pub struct Wiring {
    /// The chat the listener is bound for: the only source of a connection's chat.
    pub chat: ChatId,
    pub proxy: Arc<ForgeProxy>,
    pub upstream: Arc<dyn Upstream>,
    pub dial: Arc<dyn Dial>,
    pub limits: Limits,
    /// The storage addresses this chat's LFS batches handed the listener.
    pub storage: Arc<LfsStorage>,
    /// See every request read ([`Observe`]); none in the app.
    pub observe: Option<Arc<dyn Observe>>,
    /// The run's hand-off secret: the door answers a `CONNECT` only when it proves this
    /// ([`crate::sandbox::egress::handoff`]), and proves it back.
    pub handoff: Secret,
    /// The check of a push's objects ([`PushGate`]), **always one**: a listener with no gate would
    /// forward every push the core allowed, CI files and all.
    pub push: Arc<dyn PushGate>,
}

impl Wiring {
    /// The app's wiring: strict TLS to the forge with the system's roots, public addresses only.
    #[must_use]
    pub fn new(
        chat: ChatId,
        proxy: Arc<ForgeProxy>,
        push: Arc<dyn PushGate>,
        handoff: Secret,
    ) -> Self {
        Self {
            chat,
            proxy,
            upstream: Arc::new(StrictUpstream::system_roots()),
            dial: Arc::new(PublicDial),
            limits: Limits::default(),
            storage: Arc::new(LfsStorage::default()),
            observe: None,
            handoff,
            push,
        }
    }
}

/// A gate that lets every push through, for a test of something else than the gate.
#[cfg(any(test, feature = "fenced"))]
pub struct ForwardAll;

#[cfg(any(test, feature = "fenced"))]
impl PushGate for ForwardAll {
    fn check(&self, _: &ChatId, _: &PushRead) -> PushVerdict {
        PushVerdict::Forward
    }
}

/// A running listener. Dropping it closes the door and removes the socket.
pub struct ForgeListener {
    kind: EndpointKind,
    ca_pem: Option<String>,
    shutdown: Arc<AtomicBool>,
    wake: Wake,
    thread: Option<JoinHandle<()>>,
}

#[derive(Clone)]
enum Wake {
    #[cfg(unix)]
    Unix(std::path::PathBuf),
    Tcp(SocketAddr),
}

impl ForgeListener {
    /// Where it listens, for the chat's environment: the socket `gh`'s `http_unix_socket` names, or
    /// the loopback port that is `HTTPS_PROXY`.
    pub fn kind(&self) -> &EndpointKind {
        &self.kind
    }

    /// The chat's authority's certificate, for the file git's `http.<host>.sslCAInfo` and glab's
    /// `GLAB_CA_CERT` name. `None` on the unix socket, which has no TLS.
    pub fn ca_certificate_pem(&self) -> Option<&str> {
        self.ca_pem.as_deref()
    }

    /// Serve `gh` on a unix socket in `dir` (see the module's note on what `dir` must be).
    #[cfg(unix)]
    pub fn serve_socket(wiring: Wiring, dir: &std::path::Path) -> io::Result<Self> {
        let endpoint = super::UnixEndpoint::bind(&wiring.chat, dir)?;
        let kind = endpoint.kind();
        let wake = Wake::Unix(endpoint.path().to_path_buf());
        let shutdown = Arc::new(AtomicBool::new(false));
        let active = Arc::new(AtomicUsize::new(0));
        let flag = Arc::clone(&shutdown);
        let thread = std::thread::Builder::new()
            .name("forge-proxy-socket".into())
            .spawn(move || {
                loop {
                    let accepted = endpoint.accept_same_user();
                    if flag.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok((_, stream)) = accepted else {
                        std::thread::sleep(Duration::from_millis(50));
                        continue;
                    };
                    let timeout = Some(wiring.limits.io_timeout);
                    let _ = stream.set_read_timeout(timeout);
                    let _ = stream.set_write_timeout(timeout);
                    spawn_connection(&wiring, &active, move |w| {
                        let mut stream = stream;
                        serve_one(w, &mut stream, &Route::Plain);
                    });
                }
            })?;
        Ok(Self {
            kind,
            ca_pem: None,
            shutdown,
            wake,
            thread: Some(thread),
        })
    }

    /// Serve `git` and `glab` on a loopback port, with an authority made for the chat's recorded
    /// hosts ([`ForgeProxy::terminable_hosts`], as enrolled at this moment).
    pub fn serve_connect(wiring: Wiring) -> io::Result<Self> {
        let hosts = wiring.proxy.terminable_hosts(&wiring.chat);
        let ca = Arc::new(ChatCa::new(&hosts).map_err(io::Error::other)?);
        let endpoint = LoopbackEndpoint::bind(&wiring.chat)?;
        let kind = endpoint.kind();
        let EndpointKind::Loopback(address) = kind.clone() else {
            return Err(io::Error::other("a loopback endpoint is not on loopback"));
        };
        let ca_pem = ca.certificate_pem().to_string();
        let shutdown = Arc::new(AtomicBool::new(false));
        let active = Arc::new(AtomicUsize::new(0));
        let flag = Arc::clone(&shutdown);
        let thread = std::thread::Builder::new()
            .name("forge-proxy-connect".into())
            .spawn(move || {
                loop {
                    let accepted = endpoint.accept();
                    if flag.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok((_, stream)) = accepted else {
                        std::thread::sleep(Duration::from_millis(50));
                        continue;
                    };
                    let timeout = Some(wiring.limits.io_timeout);
                    let _ = stream.set_read_timeout(timeout);
                    let _ = stream.set_write_timeout(timeout);
                    let ca = Arc::clone(&ca);
                    spawn_connection(&wiring, &active, move |w| {
                        serve_connect_stream(w, &ca, stream);
                    });
                }
            })?;
        Ok(Self {
            kind,
            ca_pem: Some(ca_pem),
            shutdown,
            wake: Wake::Tcp(address),
            thread: Some(thread),
        })
    }
}

impl Drop for ForgeListener {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // The accept loop is blocked in `accept`: one connection of our own wakes it. If none can
        // be made (the socket's folder is already gone), the loop cannot be woken, and waiting for
        // it would wait for ever: it is left, and ends with the process.
        let woke = match &self.wake {
            #[cfg(unix)]
            Wake::Unix(path) => std::os::unix::net::UnixStream::connect(path).is_ok(),
            Wake::Tcp(address) => TcpStream::connect_timeout(
                &SocketAddr::from((Ipv4Addr::LOCALHOST, address.port())),
                Duration::from_secs(1),
            )
            .is_ok(),
        };
        if let Some(thread) = self.thread.take()
            && woke
        {
            let _ = thread.join();
        }
    }
}

/// **One run's whole route** (one login, slice K): the unix socket for `gh` and the `CONNECT` door
/// for `git` and `glab`, bound for one chat of one run, with the authority's certificate written
/// beside the socket. Dropping it closes both doors and removes the socket.
#[cfg(unix)]
pub struct RunRoute {
    _socket: ForgeListener,
    _connect: ForgeListener,
    /// The socket `gh` is pointed at.
    pub socket: std::path::PathBuf,
    /// The `CONNECT` door on loopback, which the chat's egress proxy hands a forge connection to.
    pub door: SocketAddr,
    /// The authority's certificate (public), for git's `sslCAInfo` and glab's `GLAB_CA_CERT`.
    pub ca_cert: std::path::PathBuf,
    /// The hosts the door terminates.
    pub hosts: Vec<String>,
}

#[cfg(unix)]
impl RunRoute {
    /// The route of `wiring`'s chat, its socket in `dir` (a folder that is the app's alone, which
    /// [`ForgeListener::serve_socket`] checks) and its certificate there too.
    pub fn open(wiring: Wiring, dir: &std::path::Path) -> io::Result<Self> {
        let hosts = wiring.proxy.terminable_hosts(&wiring.chat);
        let socket = ForgeListener::serve_socket(wiring.clone(), dir)?;
        let EndpointKind::UnixSocket(path) = socket.kind().clone() else {
            return Err(io::Error::other("a socket endpoint is not a socket"));
        };
        let connect = ForgeListener::serve_connect(wiring)?;
        let EndpointKind::Loopback(door) = connect.kind().clone() else {
            return Err(io::Error::other("a loopback endpoint is not on loopback"));
        };
        let pem = connect
            .ca_certificate_pem()
            .ok_or_else(|| io::Error::other("the door made no authority"))?;
        let ca_cert = dir.join("ca.pem");
        write_public(&ca_cert, pem.as_bytes())?;
        Ok(Self {
            _socket: socket,
            _connect: connect,
            socket: path,
            door,
            ca_cert,
            hosts,
        })
    }
}

/// Writes a certificate (public) as a new file the app owns: `create_new` never follows a link.
#[cfg(unix)]
fn write_public(path: &std::path::Path, bytes: &[u8]) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let _ = std::fs::remove_file(path);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(path)?;
    file.write_all(bytes)
}

/// Run `serve` on a thread of its own, unless the listener is already serving as many connections
/// as it is allowed.
fn spawn_connection(
    wiring: &Wiring,
    active: &Arc<AtomicUsize>,
    serve: impl FnOnce(&Wiring) + Send + 'static,
) {
    if active.fetch_add(1, Ordering::SeqCst) >= wiring.limits.connections {
        active.fetch_sub(1, Ordering::SeqCst);
        return;
    }
    let wiring = wiring.clone();
    let slot = Arc::clone(active);
    let spawned = std::thread::Builder::new()
        .name("forge-proxy-conn".into())
        .spawn(move || {
            // A panic in a handler ends this connection and nothing else: the slot comes back
            // either way, so panics never add up to a listener that serves no one (#7589).
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| serve(&wiring)));
            slot.fetch_sub(1, Ordering::SeqCst);
        });
    if spawned.is_err() {
        // Out of threads: the closure never ran, so the slot is given back here.
        active.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The door's `200`, carrying its proof of the connection's nonce.
fn established(out: &mut TcpStream, proof: &str) -> io::Result<()> {
    write!(
        out,
        "HTTP/1.1 200 Connection Established\r\n{}: {proof}\r\n\r\n",
        handoff::REPLY_HEADER
    )?;
    out.flush()
}

/// [`write_answer`], carrying the door's proof of the connection's nonce.
fn answer_proving(out: &mut TcpStream, status: u16, body: &str, proof: &str) -> io::Result<()> {
    let headers = vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        ("Content-Length".to_string(), body.len().to_string()),
        (handoff::REPLY_HEADER.to_string(), proof.to_string()),
    ];
    write_head(out, status, &headers)?;
    out.write_all(body.as_bytes())?;
    out.flush()
}

/// One connection on the `CONNECT` door: the `CONNECT` line, the core's verdict on its host, and
/// then a terminated request or a tunnel.
pub fn serve_connect_stream(wiring: &Wiring, ca: &ChatCa, stream: TcpStream) {
    let mut framed = Framed::new(stream).with_deadline(Instant::now() + wiring.limits.total);
    let head = match framed.read_head(wiring.limits.head) {
        Ok(head) => head,
        Err(HeadError::Closed | HeadError::Io) => return,
        Err(_) => {
            let _ = write_answer(
                &mut framed.stream,
                400,
                &message("not a request purlis reads"),
            );
            return;
        }
    };
    if !head.method.eq_ignore_ascii_case("CONNECT") {
        let _ = write_answer(
            &mut framed.stream,
            405,
            &message("this port carries CONNECT to a forge and nothing else"),
        );
        return;
    }
    // **Only the chat's own egress proxy comes in by this door** ([`handoff`]): a connection that
    // does not prove the run's secret is answered with nothing that proves the door, and carried
    // nowhere.
    let target = authority(&head.target);
    let proven = target.as_ref().and_then(|(host, port)| {
        head.headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(handoff::HEADER))
            .and_then(|(_, value)| wiring.handoff.accept(value, host, *port))
    });
    let Some(nonce) = proven else {
        let _ = write_answer(
            &mut framed.stream,
            407,
            &message("this port takes the chat's own egress proxy and nothing else"),
        );
        return;
    };
    let proof = wiring.handoff.reply(&nonce);
    let Some((host, port)) = authority(&head.target) else {
        let _ = answer_proving(
            &mut framed.stream,
            400,
            &message("not a host and port"),
            &proof,
        );
        return;
    };
    // The client waits for the answer before it speaks; one that did not has sent bytes the
    // tunnel would lose, and is refused.
    let Some(mut client) = framed.into_stream() else {
        return;
    };
    match wiring.proxy.connect(&wiring.chat, &host, port) {
        ConnectVerdict::Refuse(refusal) => {
            let said = if refusal == super::Refusal::Pinned {
                wiring.proxy.pin_said(&host, port)
            } else {
                format!("purlis refused this connection: {refusal:?}")
            };
            let _ = answer_proving(&mut client, 403, &message(&said), &proof);
        }
        ConnectVerdict::Terminate { host } => {
            if established(&mut client, &proof).is_err() {
                return;
            }
            serve_terminated(wiring, ca, &host, port, client);
        }
        ConnectVerdict::Tunnel { host } => {
            let upstream = match wiring.dial.dial(&host, port) {
                Ok(upstream) => upstream,
                Err(e) => {
                    let _ = answer_proving(
                        &mut client,
                        502,
                        &message(&format!("purlis could not reach {host}: {e}")),
                        &proof,
                    );
                    return;
                }
            };
            if established(&mut client, &proof).is_err() {
                return;
            }
            tunnel(client, upstream, wiring.limits.io_timeout);
        }
    }
}

/// Serve one plain-HTTP request on a connection the caller accepted: `gh`'s, on the chat's unix
/// socket. The forge is named by `Host`. The connection is closed after the answer.
pub fn serve_plain<S: Read + Write>(wiring: &Wiring, mut stream: S) {
    serve_one(wiring, &mut stream, &Route::Plain);
}

/// Serve one request inside the TLS the proxy terminates for `host:port`, on a connection the
/// caller has answered `200` to the `CONNECT` of. A client that does not ask for `host` by name
/// gets no certificate, and `false` is no certificate for this host at all.
pub fn serve_terminated<S: Read + Write>(
    wiring: &Wiring,
    ca: &ChatCa,
    host: &str,
    port: u16,
    stream: S,
) -> bool {
    let Some(config) = ca.server_config_for(host) else {
        return false;
    };
    let Ok(connection) = rustls::ServerConnection::new(config) else {
        return false;
    };
    let mut tls = rustls::StreamOwned::new(connection, http::Gated(stream));
    // The handshake first, under a bound of its own: a client that drips one into a connection
    // would otherwise be inside a single read of it for as long as it liked.
    let shaken = {
        let _scope = http::Scope::new(wiring.limits.io_timeout);
        tls.conn.complete_io(&mut tls.sock).is_ok()
    };
    if shaken {
        serve_one(wiring, &mut tls, &Route::Tls { host, port });
    }
    // Say goodbye, and only write: a flush would also wait to read, from a client that may have
    // nothing more to say (a handshake that failed leaves one silent, not gone).
    tls.conn.send_close_notify();
    while tls.conn.wants_write() {
        if tls.conn.write_tls(&mut tls.sock).is_err() {
            break;
        }
    }
    true
}

/// `host:port`, the way a `CONNECT` names it: no scheme, no path, no userinfo, no bracketed
/// address, and a port.
fn authority(target: &str) -> Option<(String, u16)> {
    let (host, port) = target.rsplit_once(':')?;
    let port: u16 = port.parse().ok().filter(|p| *p != 0)?;
    (!host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-'))
    .then(|| (host.to_string(), port))
}

/// Carry bytes both ways, untouched, until either side ends or goes quiet.
fn tunnel(client: TcpStream, upstream: TcpStream, idle: Duration) {
    let _ = upstream.set_read_timeout(Some(idle));
    let _ = upstream.set_write_timeout(Some(idle));
    let (Ok(mut client_read), Ok(mut upstream_write)) = (client.try_clone(), upstream.try_clone())
    else {
        return;
    };
    let forward = std::thread::spawn(move || {
        let _ = io::copy(&mut client_read, &mut upstream_write);
        let _ = upstream_write.shutdown(std::net::Shutdown::Write);
    });
    let (mut upstream_read, mut client_write) = (upstream, client);
    let _ = io::copy(&mut upstream_read, &mut client_write);
    let _ = client_write.shutdown(std::net::Shutdown::Write);
    let _ = forward.join();
}
