// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **purlis's key proxy** (V36's shared passthrough crate, R-053): a listener per run, on
//! `127.0.0.1`, that the run's harness sends its model calls to, with a 256-bit token of its own.
//!
//! **Its own runtime** (R-054). A [`KeyProxy`] owns a tokio runtime of [`WORKER_THREADS`]
//! threads named [`THREAD_NAME`], and every listener and connection runs there, never on the
//! runtime of whoever called [`KeyProxy::start`]: in `purlisd` that is the host's current-thread
//! runtime, which a slow request must never hold up. It is linked by the desktop's binary and the
//! runner's, never by the session host's crate, which starts it through a trait the binary
//! supplies (`purlis_host::ProxiesKeys`).
//!
//! **hyper's own parser** (R-059, D6640-C2 §1). Each connection is hyper 1's `http1` server
//! connection, with the builder settings the spike found ([`HEAD_TIMEOUT`], [`MOST_HEADERS`],
//! [`MOST_BUFFERED`]): there is no framing parser of purlis's own. A request hyper refuses (two
//! `Content-Length` values that disagree, an obs-fold, a head too large) never reaches the
//! service; one with both length headers is framed by `Transfer-Encoding` alone and its
//! connection closed after the answer. The thin checks on what hyper hands the service are
//! T1a-08's and T1a-43's.
//!
//! **What it answers** (T1a-05). A request must carry its run's token as the bearer credential,
//! the header a harness sets from `ANTHROPIC_AUTH_TOKEN`, compared in constant time (R-063); one
//! without it is refused and the listener goes on (R-064). One with it is sent to its listener's
//! [`Route`], its body passed through as it came and the answer's body back as it came, streamed
//! or not: nothing translated, nothing added (R-042, R-045). A listener started with no route
//! refuses every request. Every refusal is in the Messages error shape, 400 with
//! `x-should-retry: false` (D6640-C2 §2; its final shape is T1a-26's), and every request is
//! recorded.
//!
//! **What changes on the way** (T1a-60). Every header a credential may come in
//! ([`CREDENTIAL_HEADERS`], in any letter case) is taken out before the run's [`Key`] goes into
//! the one its declaration names (R-097), and `Host` is the upstream origin's, whatever the
//! harness sent (R-100). A key that can't be a header value is refused, never sent empty.
//!
//! **How the upstream is reached** (T1a-60, R-056). Through the [`Reach`] the key proxy was made
//! with: purlis-core's [`Outbound`] connects, off the reactor, to the upstream or to the outbound
//! proxy hyper-util's [`Matcher`] names for it; through a proxy the call is a `CONNECT` tunnel,
//! never a redirect; and an `https` origin is reached over TLS verified against the OS trust
//! store by purlis-core's `upstream::trust` client, under the declared name. A failure to reach
//! it is a 502 in the Messages error shape whose `purlis:` message names the cause (W81).
//!
//! **The token** never leaves this crate but in [`Started`], to the start's caller: it is in no
//! line [`Heard::say`] is told, and no [`Record`], which carries no header. The listener's copy
//! lives in memory only, held by its accept loop, and is dropped with it at [`KeyProxy::stop`]
//! (R-065).

use std::collections::HashMap;
use std::convert::Infallible;
use std::io;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, Full};
use hyper::body::Incoming;
use hyper::header::{AUTHORIZATION, HOST, HeaderName, HeaderValue, PROXY_AUTHORIZATION};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::upgrade::Upgraded;
use hyper::{HeaderMap, Request, Response, StatusCode, Uri};
use hyper_util::client::proxy::matcher::Intercept;
pub use hyper_util::client::proxy::matcher::Matcher;
use hyper_util::rt::{TokioIo, TokioTimer};
use purlis_core::upstream::{Destination, Direct, HostNotFound, Outbound, trust};
use purlis_core::window_copy::{self, line};
use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use subtle::ConstantTimeEq;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;
use tokio::task::{JoinHandle, JoinSet};
use tokio_rustls::TlsConnector;

/// The name of each of the key proxy's runtime threads (R-054).
pub const THREAD_NAME: &str = "purlis-key-proxy";

/// How many worker threads its runtime has (R-054).
pub const WORKER_THREADS: usize = 2;

/// How long a request's head may take to arrive (design §3.3, the spike's P10).
pub const HEAD_TIMEOUT: Duration = Duration::from_secs(30);

/// The most headers a request may carry: past it hyper answers 431 (the spike's knob).
pub const MOST_HEADERS: usize = 100;

/// The most bytes a connection buffers while it reads: hyper's own default, named here so that a
/// change is a choice. A head past it is answered 431.
pub const MOST_BUFFERED: usize = 8192 + 4096 * 100;

/// How many random bytes a run's token is: 256 bits.
const TOKEN_BYTES: usize = 32;

/// How long the accept loop rests after the operating system refused it a connection (out of
/// descriptors, say), so a refusal that lasts never spins a thread.
const AFTER_A_FAILED_ACCEPT: Duration = Duration::from_millis(100);

/// What a request to a listener with no route is answered: Anthropic's error shape.
const NOT_ROUTED: &str = r#"{"type":"error","error":{"type":"invalid_request_error","message":"purlis: this key proxy has no route for a request yet"}}"#;

/// What a request without its run's token is answered (R-064): Anthropic's error shape.
const NO_TOKEN: &str = r#"{"type":"error","error":{"type":"invalid_request_error","message":"purlis: this request does not carry its run's token"}}"#;

/// What a request is answered when its run's key can't be a header value: refused, never sent
/// with an empty credential.
const NOT_A_HEADER_VALUE: &str = r#"{"type":"error","error":{"type":"invalid_request_error","message":"purlis: this run's key can't be sent in a header"}}"#;

/// What a request is answered when its route's origin can't be a `Host` header.
const NOT_A_HOST: &str = r#"{"type":"error","error":{"type":"invalid_request_error","message":"purlis: this run's route names an origin that can't be sent as Host"}}"#;

/// **Every header a credential may come in**, each taken out of what the harness sent, in any
/// letter case, before the run's key goes into the one its declaration names (R-097). The
/// declarations' own auth headers (`x-api-key` and `authorization`) are among them.
pub const CREDENTIAL_HEADERS: [&str; 6] = [
    "authorization",
    "proxy-authorization",
    "x-api-key",
    "api-key",
    "x-goog-api-key",
    "cookie",
];

/// What the key proxy answers the harness with: a refusal of its own, or the upstream's body.
type Body = BoxBody<Bytes, hyper::Error>;

/// One line of the network record: a request the key proxy answered, and how. Never a header or
/// the query, so never the token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub run: String,
    pub method: String,
    pub path: String,
    pub status: u16,
}

/// What the key proxy tells the host, through callbacks the host supplies with each start. Each
/// is called on one of the key proxy's own threads.
#[derive(Clone)]
pub struct Heard {
    /// A line for the host's log.
    pub say: Arc<dyn Fn(&str) + Send + Sync>,
    /// A line of the network record.
    pub record: Arc<dyn Fn(&Record) + Send + Sync>,
}

/// A run's token: never written out by `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    fn mint() -> io::Result<Token> {
        let mut bytes = [0u8; TOKEN_BYTES];
        getrandom::fill(&mut bytes).map_err(io::Error::other)?;
        Ok(Token(bytes.iter().map(|b| format!("{b:02x}")).collect()))
    }

    /// The token itself, for the start's caller alone.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(..)")
    }
}

/// **The run's key**, and the header its declaration puts it in. Never written out by `Debug`.
#[derive(Clone)]
pub enum Key {
    /// In `x-api-key` (a declaration's `auth = "x-api-key"`).
    XApiKey(String),
    /// In `Authorization: Bearer` (a declaration's `auth = "bearer"`).
    Bearer(String),
}

impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Key::XApiKey(_) => "Key::XApiKey(..)",
            Key::Bearer(_) => "Key::Bearer(..)",
        })
    }
}

/// How an upstream origin is spoken to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    /// Plain HTTP: a loopback server's, or a test's.
    Http,
    /// TLS, verified against the OS trust store under the origin's name (R-056).
    Https,
}

/// **An upstream origin**: its scheme, its host name as the route names it, and its port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub scheme: Scheme,
    pub host: String,
    pub port: u16,
}

impl Origin {
    /// `https://host:port`.
    pub fn https(host: &str, port: u16) -> Origin {
        Origin {
            scheme: Scheme::Https,
            host: host.to_owned(),
            port,
        }
    }

    /// `http://host:port`.
    pub fn http(host: &str, port: u16) -> Origin {
        Origin {
            scheme: Scheme::Http,
            ..Origin::https(host, port)
        }
    }

    /// The host, in brackets when it is an IPv6 address.
    fn bracketed(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        }
    }

    /// The host, with `:port` where the port isn't its scheme's own: what `Host` carries.
    fn authority(&self) -> String {
        let own = match self.scheme {
            Scheme::Http => 80,
            Scheme::Https => 443,
        };
        if self.port == own {
            self.bracketed()
        } else {
            format!("{}:{}", self.bracketed(), self.port)
        }
    }

    /// The origin as a URI, for the outbound proxy's matcher.
    fn uri(&self) -> Option<Uri> {
        let scheme = match self.scheme {
            Scheme::Http => "http",
            Scheme::Https => "https",
        };
        format!("{scheme}://{}/", self.authority()).parse().ok()
    }
}

/// **Where a listener sends what it serves**: the upstream origin, and the run's key.
#[derive(Debug, Clone)]
pub struct Route {
    pub origin: Origin,
    pub key: Key,
}

/// **How the key proxy reaches its upstreams**, one for the process (R-056): what connects, the
/// TLS client every upstream is verified with, and which outbound proxy a connection goes
/// through.
pub struct Reach {
    /// What makes each connection, to the upstream or to the outbound proxy in its place.
    pub outbound: Arc<dyn Outbound>,
    /// The one TLS client config, which verifies against the trust store.
    pub tls: Arc<ClientConfig>,
    /// Which outbound proxy, if any, an origin is reached through.
    pub proxies: Matcher,
}

impl Reach {
    /// **What every build that ships reaches upstreams with**: the system's resolver; the OS
    /// trust store, through `rustls-platform-verifier`; and the outbound proxy the system names,
    /// read once here by `Matcher::from_system()` from the proxy variables in this process's
    /// environment (`HTTPS_PROXY`, `HTTP_PROXY`, `ALL_PROXY`, `NO_PROXY` and their lower-case
    /// forms), then the OS's proxy settings. No PAC file is read (C-16).
    pub fn system() -> io::Result<Reach> {
        Ok(Reach {
            outbound: Arc::new(Direct::new()),
            tls: Arc::new(trust::client_config().map_err(io::Error::other)?),
            proxies: Matcher::from_system(),
        })
    }
}

/// A listener started for a run.
#[derive(Debug)]
pub struct Started {
    pub port: u16,
    pub token: Token,
}

/// The key proxy: its runtime and the listeners it runs, by run. Dropping it ends them all.
pub struct KeyProxy {
    /// Always there until it is dropped, when it is shut down without waiting: a key proxy may
    /// be dropped on another runtime, where waiting is not allowed.
    runtime: Option<Runtime>,
    reach: Arc<Reach>,
    listening: Mutex<HashMap<String, Listening>>,
}

struct Listening {
    /// The accept loop, which owns every connection it accepted: aborting it ends them all.
    accepting: JoinHandle<()>,
    heard: Heard,
}

impl KeyProxy {
    /// A key proxy with its own runtime of [`WORKER_THREADS`] threads named [`THREAD_NAME`],
    /// reaching upstreams as every build that ships does ([`Reach::system`]), running no
    /// listener yet.
    pub fn new() -> io::Result<KeyProxy> {
        KeyProxy::reaching(Reach::system()?)
    }

    /// [`KeyProxy::new`], reaching upstreams through `reach`.
    pub fn reaching(reach: Reach) -> io::Result<KeyProxy> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(WORKER_THREADS)
            .thread_name(THREAD_NAME)
            .enable_io()
            .enable_time()
            .build()?;
        Ok(KeyProxy {
            runtime: Some(runtime),
            reach: Arc::new(reach),
            listening: Mutex::new(HashMap::new()),
        })
    }

    fn runtime(&self) -> &Runtime {
        self.runtime
            .as_ref()
            .expect("held until the key proxy is dropped")
    }

    /// Starts a listener for `run` on `127.0.0.1`, on a port the system picks, with a fresh
    /// token, sending what carries the token to `route` (refusing everything with none), telling
    /// `heard` what it does. Binds and returns at once: the listener runs on the key proxy's own
    /// runtime, whatever runtime this is called from. Refused where one runs for `run` already.
    pub fn start(&self, run: &str, route: Option<Route>, heard: Heard) -> io::Result<Started> {
        let mut listening = self
            .listening
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if listening.contains_key(run) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "a key proxy runs for this run already",
            ));
        }
        let bound = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        bound.set_nonblocking(true)?;
        let port = bound.local_addr()?.port();
        let token = Token::mint()?;
        let listener = {
            let _inside = self.runtime().enter();
            TcpListener::from_std(bound)?
        };
        let serving = Arc::new(Serving {
            run: run.to_string(),
            token: token.clone(),
            route,
            reach: Arc::clone(&self.reach),
            heard: heard.clone(),
        });
        let accepting = self.runtime().spawn(accept(listener, serving));
        (heard.say)(&format!("listening for run {run} on 127.0.0.1:{port}"));
        listening.insert(run.to_string(), Listening { accepting, heard });
        Ok(Started { port, token })
    }

    /// Stops `run`'s listener and every connection on it: whether one ran.
    pub fn stop(&self, run: &str) -> bool {
        let stopped = self
            .listening
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(run);
        match stopped {
            Some(listening) => {
                listening.accepting.abort();
                (listening.heard.say)(&format!("stopped the listener for run {run}"));
                true
            }
            None => false,
        }
    }
}

impl Drop for KeyProxy {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// What a listener's connections are served with. Its token is the host's one copy beside the
/// start's answer, dropped with the accept loop that holds it (R-065).
struct Serving {
    run: String,
    token: Token,
    route: Option<Route>,
    reach: Arc<Reach>,
    heard: Heard,
}

/// Accepts connections on `listener` until aborted, serving each on the key proxy's runtime and
/// owning it, so that an abort ends them all.
async fn accept(listener: TcpListener, serving: Arc<Serving>) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    connections.spawn(serve(stream, Arc::clone(&serving)));
                }
                Err(why) => {
                    let run = &serving.run;
                    (serving.heard.say)(&format!("the listener for run {run} could not accept: {why}"));
                    tokio::time::sleep(AFTER_A_FAILED_ACCEPT).await;
                }
            },
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
        }
    }
}

/// Serves one connection on hyper's own `http1` server connection (R-059).
async fn serve(stream: TcpStream, serving: Arc<Serving>) {
    let mut builder = http1::Builder::new();
    builder
        .timer(TokioTimer::new())
        .header_read_timeout(HEAD_TIMEOUT)
        .max_headers(MOST_HEADERS)
        .max_buf_size(MOST_BUFFERED);
    let service = service_fn(move |request| answer(request, Arc::clone(&serving)));
    // A connection hyper refused was answered by hyper where it could be; one that broke has
    // no one left to answer. Neither is the host's to hear of.
    let _ = builder
        .serve_connection(TokioIo::new(stream), service)
        .await;
}

/// Answers one request hyper parsed: sent to the route when it carries the run's token and there
/// is one, refused otherwise, and recorded either way.
async fn answer(
    request: Request<Incoming>,
    serving: Arc<Serving>,
) -> Result<Response<Body>, Infallible> {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let response = match &serving.route {
        _ if !carries(&serving.token, request.headers()) => refuse(request, NO_TOKEN).await,
        None => refuse(request, NOT_ROUTED).await,
        Some(route) => match forward(request, route, &serving.reach).await {
            Ok(response) => response.map(BodyExt::boxed),
            Err(Failed::Refused(why)) => fixed(StatusCode::BAD_REQUEST, why, false),
            Err(Failed::Unreached(why)) => unreached(why, &route.origin),
        },
    };
    (serving.heard.record)(&Record {
        run: serving.run.clone(),
        method,
        path,
        status: response.status().as_u16(),
    });
    Ok(response)
}

/// Whether `headers` carry `token` as their one bearer credential, compared in constant time
/// (R-063).
fn carries(token: &Token, headers: &HeaderMap) -> bool {
    let mut credentials = headers.get_all(AUTHORIZATION).iter();
    let (Some(credential), None) = (credentials.next(), credentials.next()) else {
        return false;
    };
    credential
        .as_bytes()
        .strip_prefix(b"Bearer ")
        .is_some_and(|presented| bool::from(presented.ct_eq(token.0.as_bytes())))
}

/// Takes every credential the harness sent out of `headers` ([`CREDENTIAL_HEADERS`], the token
/// among them) before `route`'s key goes into the one header its declaration names (R-097), and
/// sets `Host` to the upstream origin's (R-100): every other header is left as the harness sent
/// it. A key or an origin that can't be a header value is refused, and `headers` then carry no
/// credential at all.
fn swap(headers: &mut HeaderMap, route: &Route) -> Result<(), &'static str> {
    for name in CREDENTIAL_HEADERS {
        headers.remove(name);
    }
    let host = HeaderValue::from_str(&route.origin.authority()).map_err(|_| NOT_A_HOST)?;
    let (name, value) = match &route.key {
        Key::XApiKey(key) => (HeaderName::from_static("x-api-key"), key.clone()),
        Key::Bearer(key) => (AUTHORIZATION, format!("Bearer {key}")),
    };
    let mut value = HeaderValue::from_str(&value).map_err(|_| NOT_A_HEADER_VALUE)?;
    value.set_sensitive(true);
    headers.insert(HOST, host);
    headers.insert(name, value);
    Ok(())
}

/// Why a request was not carried to its upstream.
#[derive(Debug)]
enum Failed {
    /// Refused before anything was sent, with the answer's body.
    Refused(&'static str),
    /// The upstream could not be reached.
    Unreached(Unreached),
}

/// **Why an upstream could not be reached**, each named by its W81 line.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Unreached {
    /// The upstream's name resolved to no address.
    HostNotFound,
    /// The connect to the upstream failed (no route, refused).
    NoConnection,
    /// Its certificate did not verify against the trust store under the declared name.
    Untrusted,
    /// The outbound proxy could not be reached, or refused the tunnel (a 407 among them).
    ProxyRefused,
    /// The connection broke once made.
    Reset,
    /// The system names an outbound proxy of a scheme other than `http` (`https`, `socks5`,
    /// `socks5h`, ...), which purlis doesn't speak: refused before anything is sent to it, its
    /// credential above all.
    UnsupportedProxy(String),
}

impl Unreached {
    /// What a failed TLS handshake was: an untrusted certificate where rustls says so.
    fn of_tls(why: &io::Error) -> Unreached {
        let untrusted = why
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<rustls::Error>())
            .is_some_and(|inner| matches!(inner, rustls::Error::InvalidCertificate(_)));
        if untrusted {
            Unreached::Untrusted
        } else {
            Unreached::NoConnection
        }
    }

    /// The `purlis:` message naming it, for `host`.
    fn message(&self, host: &str) -> String {
        let (said, scheme) = match self {
            Unreached::HostNotFound => (line::W81_HOST_NOT_FOUND, ""),
            Unreached::NoConnection => (line::W81_NO_CONNECTION, ""),
            Unreached::Untrusted => (line::W81_UNTRUSTED_CERTIFICATE, ""),
            Unreached::ProxyRefused => (line::W81_PROXY_REFUSED, ""),
            Unreached::Reset => (line::W81_RESET, ""),
            Unreached::UnsupportedProxy(scheme) => (line::W81_UNSUPPORTED_PROXY, scheme.as_str()),
        };
        format!(
            "purlis: {}",
            window_copy::fill(said, &[("host", host), ("scheme", scheme)])
        )
    }
}

/// Where a connection to `origin` goes: straight there, or through the outbound proxy `proxies`
/// name for it, with the proxy's credentials where its address carried them. A loopback
/// upstream always goes direct, never through the proxy (design §3.3). Only an `http` proxy is
/// spoken to, with a `CONNECT`; one of any other scheme (`https`, `socks5`, `socks5h`, ...) is
/// refused here, before anything, its credential above all, is sent to it.
fn destination(
    origin: &Origin,
    proxies: &Matcher,
) -> Result<(Destination, Option<Intercept>), Unreached> {
    let loopback = origin.host.eq_ignore_ascii_case("localhost")
        || origin
            .host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    let intercept = origin
        .uri()
        .filter(|_| !loopback)
        .and_then(|uri| proxies.intercept(&uri));
    let Some(intercept) = intercept else {
        return Ok((Destination::direct(&origin.host, origin.port), None));
    };
    let uri = intercept.uri();
    let scheme = uri.scheme_str().unwrap_or_default();
    if scheme != "http" {
        return Err(Unreached::UnsupportedProxy(scheme.to_owned()));
    }
    let proxy = uri
        .host()
        .map(|host| format!("{host}:{}", uri.port_u16().unwrap_or(80)))
        .ok_or(Unreached::ProxyRefused)?;
    Ok((
        Destination::through(&origin.host, origin.port, &proxy),
        Some(intercept),
    ))
}

/// The `CONNECT` that asks the outbound proxy for a tunnel to `origin`, carrying a
/// `Proxy-Authorization` only where the proxy's address in the system's settings carried one:
/// purlis never sends a proxy credential it wasn't given.
fn connect_request(origin: &Origin, proxy: &Intercept) -> Option<Request<Empty<Bytes>>> {
    let authority = format!("{}:{}", origin.bracketed(), origin.port);
    let mut request = Request::connect(authority.as_str()).header(HOST, authority.as_str());
    if let Some(credential) = proxy.basic_auth() {
        request = request.header(PROXY_AUTHORIZATION, credential.clone());
    }
    request.body(Empty::new()).ok()
}

/// A connection's byte stream, whichever layers it went through.
trait Wire: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Wire for T {}

/// Asks the outbound proxy on `stream` for a tunnel to `origin` (a `CONNECT`, never followed as
/// a redirect): the tunnel, or the proxy's refusal.
async fn tunnel(
    stream: TcpStream,
    origin: &Origin,
    proxy: &Intercept,
) -> Result<Upgraded, Unreached> {
    let request = connect_request(origin, proxy).ok_or(Unreached::ProxyRefused)?;
    let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .map_err(|_| Unreached::ProxyRefused)?;
    tokio::spawn(connection.with_upgrades());
    let response = sender
        .send_request(request)
        .await
        .map_err(|_| Unreached::ProxyRefused)?;
    if !response.status().is_success() {
        return Err(Unreached::ProxyRefused);
    }
    hyper::upgrade::on(response)
        .await
        .map_err(|_| Unreached::ProxyRefused)
}

/// A connection to `origin` through `reach`: made by its outbound off the reactor, through the
/// outbound proxy's tunnel where one is named, and under TLS for an `https` origin.
async fn connect(origin: &Origin, reach: &Reach) -> Result<Box<dyn Wire>, Unreached> {
    let (destination, proxy) = destination(origin, &reach.proxies)?;
    let failed = if proxy.is_some() {
        Unreached::ProxyRefused
    } else {
        Unreached::NoConnection
    };
    let outbound = Arc::clone(&reach.outbound);
    let stream = tokio::task::spawn_blocking(move || outbound.connect(&destination))
        .await
        .map_err(|_| failed.clone())?
        .map_err(|why| {
            if proxy.is_none() && HostNotFound::is(&why) {
                Unreached::HostNotFound
            } else {
                failed.clone()
            }
        })?;
    stream.set_nonblocking(true).map_err(|_| failed.clone())?;
    let stream = TcpStream::from_std(stream).map_err(|_| failed)?;
    let _ = stream.set_nodelay(true);
    let wire: Box<dyn Wire> = match &proxy {
        Some(proxy) => Box::new(TokioIo::new(tunnel(stream, origin, proxy).await?)),
        None => Box::new(stream),
    };
    match origin.scheme {
        Scheme::Http => Ok(wire),
        Scheme::Https => {
            let name =
                ServerName::try_from(origin.host.clone()).map_err(|_| Unreached::NoConnection)?;
            let tls = TlsConnector::from(Arc::clone(&reach.tls))
                .connect(name, wire)
                .await
                .map_err(|why| Unreached::of_tls(&why))?;
            Ok(Box::new(tls))
        }
    }
}

/// Sends `request` to `route`'s upstream with the credentials and `Host` swapped, on a
/// connection of its own, and hands back its answer as it comes: neither body is read, changed
/// or added to (R-042, R-045).
async fn forward(
    mut request: Request<Incoming>,
    route: &Route,
    reach: &Reach,
) -> Result<Response<Incoming>, Failed> {
    swap(request.headers_mut(), route).map_err(Failed::Refused)?;
    let wire = connect(&route.origin, reach)
        .await
        .map_err(Failed::Unreached)?;
    let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(wire))
        .await
        .map_err(|_| Failed::Unreached(Unreached::Reset))?;
    // Ends when the answer has been carried, or the harness let go of it.
    tokio::spawn(connection);
    sender
        .send_request(request)
        .await
        .map_err(|_| Failed::Unreached(Unreached::Reset))
}

/// The 502 for an upstream that could not be reached, in the Messages error shape, its `purlis:`
/// message naming the cause; no `x-should-retry: false`, so the harness retries as it would
/// against the provider (design §3.3).
fn unreached(why: Unreached, origin: &Origin) -> Response<Body> {
    let body = serde_json::json!({
        "type": "error",
        "error": {"type": "api_error", "message": why.message(&origin.host)},
    });
    answer_of(
        StatusCode::BAD_GATEWAY,
        Bytes::from(body.to_string()),
        false,
        false,
    )
}

/// Refuses `request` with `why`, reading its body to its end and letting go of it first, so the
/// connection is left where hyper can carry on; one that could not be read whole closes it.
async fn refuse(request: Request<Incoming>, why: &'static str) -> Response<Body> {
    let mut body = request.into_body();
    let mut whole = true;
    while let Some(frame) = body.frame().await {
        if frame.is_err() {
            whole = false;
            break;
        }
    }
    fixed(StatusCode::BAD_REQUEST, why, !whole)
}

/// A refusal of the key proxy's own, in the Messages error shape.
fn fixed(status: StatusCode, body: &'static str, close: bool) -> Response<Body> {
    answer_of(status, Bytes::from_static(body.as_bytes()), true, close)
}

/// An answer of the key proxy's own: `body` as JSON, told not to retry where `final_`.
fn answer_of(status: StatusCode, body: Bytes, final_: bool, close: bool) -> Response<Body> {
    let mut response = Response::builder()
        .status(status)
        .header("content-type", "application/json");
    if final_ {
        response = response.header("x-should-retry", "false");
    }
    if close {
        response = response.header("connection", "close");
    }
    response
        .body(Full::new(body).map_err(|never| match never {}).boxed())
        .expect("a response of fixed parts")
}

#[cfg(test)]
mod tests;
