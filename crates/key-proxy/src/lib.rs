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
//! [`Route`], the token taken out and the run's [`Key`] put in the header the declaration names,
//! its body passed through as it came and the answer's body back as it came, streamed or not:
//! nothing translated, nothing added (R-042, R-045). A listener started with no route refuses
//! every request. Every refusal is in the Messages error shape, 400 with `x-should-retry: false`
//! (D6640-C2 §2; its final shape is T1a-26's), and every request is recorded. The header strip,
//! `Host`, upstream TLS and the outbound proxy are T1a-60's; the route here is reached over plain
//! HTTP.
//!
//! **The token** never leaves this crate but in [`Started`], to the start's caller: it is in no
//! line [`Heard::say`] is told, and no [`Record`], which carries no header. The listener's copy
//! lives in memory only, held by its accept loop, and is dropped with it at [`KeyProxy::stop`]
//! (R-065).

use std::collections::HashMap;
use std::convert::Infallible;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::header::{AUTHORIZATION, HeaderValue};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{HeaderMap, Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use subtle::ConstantTimeEq;
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;
use tokio::task::{JoinHandle, JoinSet};

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

/// What a request is answered when its route's upstream could not be reached.
const UNREACHED: &str = r#"{"type":"error","error":{"type":"api_error","message":"purlis: the key proxy could not reach its route's upstream"}}"#;

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

/// **Where a listener sends what it serves**: the upstream's address, reached over plain HTTP
/// (upstream TLS is T1a-60's), and the run's key.
#[derive(Debug, Clone)]
pub struct Route {
    pub upstream: SocketAddr,
    pub key: Key,
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
    listening: Mutex<HashMap<String, Listening>>,
}

struct Listening {
    /// The accept loop, which owns every connection it accepted: aborting it ends them all.
    accepting: JoinHandle<()>,
    heard: Heard,
}

impl KeyProxy {
    /// A key proxy with its own runtime of [`WORKER_THREADS`] threads named [`THREAD_NAME`],
    /// running no listener yet.
    pub fn new() -> io::Result<KeyProxy> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(WORKER_THREADS)
            .thread_name(THREAD_NAME)
            .enable_io()
            .enable_time()
            .build()?;
        Ok(KeyProxy {
            runtime: Some(runtime),
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
        Some(route) => match forward(request, route).await {
            Ok(response) => response.map(BodyExt::boxed),
            Err(_) => fixed(StatusCode::BAD_GATEWAY, UNREACHED, false),
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

/// Takes the token out of `headers` and puts `key` in the header its declaration names (R-042):
/// every other header is left as the harness sent it.
fn swap(headers: &mut HeaderMap, key: &Key) {
    headers.remove(AUTHORIZATION);
    let (name, value) = match key {
        Key::XApiKey(key) => ("x-api-key", key.clone()),
        Key::Bearer(key) => ("authorization", format!("Bearer {key}")),
    };
    let mut value = HeaderValue::from_str(&value).unwrap_or_else(|_| HeaderValue::from_static(""));
    value.set_sensitive(true);
    headers.insert(name, value);
}

/// Sends `request` to `route`'s upstream with the key swapped in, on a connection of its own,
/// and hands back its answer as it comes: neither body is read, changed or added to (R-042,
/// R-045).
async fn forward(
    mut request: Request<Incoming>,
    route: &Route,
) -> Result<Response<Incoming>, Box<dyn std::error::Error + Send + Sync>> {
    swap(request.headers_mut(), &route.key);
    let stream = TcpStream::connect(route.upstream).await?;
    let (mut sender, connection) =
        hyper::client::conn::http1::handshake(TokioIo::new(stream)).await?;
    // Ends when the answer has been carried, or the harness let go of it.
    tokio::spawn(connection);
    Ok(sender.send_request(request).await?)
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

/// An answer of the key proxy's own, in the Messages error shape.
fn fixed(status: StatusCode, body: &'static str, close: bool) -> Response<Body> {
    let mut response = Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("x-should-retry", "false");
    if close {
        response = response.header("connection", "close");
    }
    response
        .body(
            Full::new(Bytes::from_static(body.as_bytes()))
                .map_err(|never| match never {})
                .boxed(),
        )
        .expect("a response of fixed parts")
}

#[cfg(test)]
mod tests;
