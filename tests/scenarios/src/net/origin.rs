// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A local test origin, plain HTTP or HTTPS, recording every request.

use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use rustls::ServerConnection;

use super::TestCa;

/// The header every origin's answer carries, naming its variant: purlis's own refusals never
/// carry it, so a site's 403 is told from purlis's.
pub const ORIGIN_HEADER: &str = "x-purlis-test-origin";

/// The body of [`Variant::Forbidden`]'s own 403.
pub const FORBIDDEN_BODY: &str = "the test origin's own 403";

/// The most a request's head may take.
const HEAD_AT_MOST: usize = 64 * 1024;

/// How long a client has to send its head, and a write may wait.
pub(super) const PATIENCE: Duration = Duration::from_secs(10);

/// **What an origin answers.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Variant {
    /// `200`, its body naming the path asked for.
    Plain,
    /// [`Variant::Plain`], after waiting this long.
    Slow(Duration),
    /// `302` to this URL: another `.test` origin's.
    RedirectTo(String),
    /// Its own `403` ([`FORBIDDEN_BODY`]), never purlis's.
    Forbidden,
}

impl Variant {
    fn name(&self) -> &'static str {
        match self {
            Variant::Plain => "plain",
            Variant::Slow(_) => "slow",
            Variant::RedirectTo(_) => "redirect",
            Variant::Forbidden => "forbidden",
        }
    }
}

/// **How an origin speaks**: plain HTTP, or HTTPS under a certificate from the test CA for
/// `names`.
pub enum Scheme<'a> {
    Http,
    Https {
        ca: &'a TestCa,
        names: &'a [&'a str],
    },
}

/// **One request an origin got.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    /// The `Host` header, where one was sent.
    pub host: Option<String>,
    /// The TLS server name the client sent, for an HTTPS origin.
    pub sni: Option<String>,
    pub method: String,
    pub path: String,
    /// Every header, in order, names lower case.
    pub headers: Vec<(String, String)>,
}

/// **A local test origin** ([module docs](self)). It stops listening when dropped.
pub struct Origin {
    addr: SocketAddr,
    seen: Arc<Mutex<Vec<Seen>>>,
    stop: Arc<AtomicBool>,
}

impl std::fmt::Debug for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Origin")
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

impl Origin {
    /// A plain-HTTP [`Variant::Plain`] origin on the loopback interface.
    pub fn http() -> io::Result<Self> {
        Self::start(Scheme::Http, Variant::Plain, IpAddr::from([127, 0, 0, 1]))
    }

    /// **An origin speaking `scheme`, answering as `variant`, on a new port of `at`.**
    pub fn start(scheme: Scheme<'_>, variant: Variant, at: IpAddr) -> io::Result<Self> {
        let tls = tls_for(&scheme)?;
        let listener = TcpListener::bind((at, 0))?;
        let addr = listener.local_addr()?;
        let seen: Arc<Mutex<Vec<Seen>>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let (recording, stopping) = (Arc::clone(&seen), Arc::clone(&stop));
        std::thread::Builder::new()
            .name("scenario-origin".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    if stopping.load(Ordering::SeqCst) {
                        return;
                    }
                    let Ok(stream) = stream else { continue };
                    let (tls, seen, variant) =
                        (tls.clone(), Arc::clone(&recording), variant.clone());
                    let _ = std::thread::Builder::new()
                        .name("scenario-origin-conn".into())
                        .spawn(move || {
                            let _ = serve(stream, tls, &seen, &variant);
                        });
                }
            })?;
        Ok(Self { addr, seen, stop })
    }

    /// Where it listens: what the resolver seam maps a name to.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Every request it got so far, in order.
    pub fn requests(&self) -> Vec<Seen> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for Origin {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wakes the accept, which then sees the stop.
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200));
    }
}

/// The TLS configuration an origin of `scheme` serves, or `None` for plain HTTP.
pub(super) fn tls_for(scheme: &Scheme<'_>) -> io::Result<Option<Arc<rustls::ServerConfig>>> {
    let Scheme::Https { ca, names } = scheme else {
        return Ok(None);
    };
    let (chain, key) = ca.leaf(names)?;
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(io::Error::other)?
    .with_no_client_auth()
    .with_single_cert(chain, key)
    .map_err(io::Error::other)?;
    Ok(Some(Arc::new(config)))
}

/// Serves one connection: one request, answered, then closed.
fn serve(
    stream: TcpStream,
    tls: Option<Arc<rustls::ServerConfig>>,
    seen: &Mutex<Vec<Seen>>,
    variant: &Variant,
) -> io::Result<()> {
    stream.set_read_timeout(Some(PATIENCE))?;
    stream.set_write_timeout(Some(PATIENCE))?;
    match tls {
        None => answer(stream, None, seen, variant),
        Some(config) => {
            let mut conn = ServerConnection::new(config).map_err(io::Error::other)?;
            let mut stream = stream;
            while conn.is_handshaking() {
                conn.complete_io(&mut stream)?;
            }
            let sni = conn.server_name().map(str::to_owned);
            answer(rustls::StreamOwned::new(conn, stream), sni, seen, variant)
        }
    }
}

fn answer(
    mut stream: impl Read + Write,
    sni: Option<String>,
    seen: &Mutex<Vec<Seen>>,
    variant: &Variant,
) -> io::Result<()> {
    let head = read_head(&mut stream)?;
    let Some(request) = parse(&head, sni) else {
        return stream.write_all(b"HTTP/1.1 400 Bad Request\r\nconnection: close\r\n\r\n");
    };
    let path = request.path.clone();
    seen.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(request);
    let (status, extra, body) = match variant {
        Variant::Plain => ("200 OK", String::new(), format!("origin saw {path}")),
        Variant::Slow(wait) => {
            std::thread::sleep(*wait);
            ("200 OK", String::new(), format!("origin saw {path}"))
        }
        Variant::RedirectTo(to) => (
            "302 Found",
            format!("location: {to}\r\n"),
            format!("moved to {to}"),
        ),
        Variant::Forbidden => ("403 Forbidden", String::new(), FORBIDDEN_BODY.to_owned()),
    };
    let reply = format!(
        "HTTP/1.1 {status}\r\n{ORIGIN_HEADER}: {}\r\n{extra}content-type: text/plain\r\n\
         content-length: {}\r\nconnection: close\r\n\r\n{body}",
        variant.name(),
        body.len()
    );
    stream.write_all(reply.as_bytes())?;
    stream.flush()
}

pub(super) fn read_head(stream: &mut impl Read) -> io::Result<String> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() >= HEAD_AT_MOST || stream.read(&mut byte)? == 0 {
            break;
        }
        head.push(byte[0]);
    }
    Ok(String::from_utf8_lossy(&head).into_owned())
}

pub(super) fn parse(head: &str, sni: Option<String>) -> Option<Seen> {
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let (method, path) = (first.next()?.to_owned(), first.next()?.to_owned());
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let host = headers
        .iter()
        .find(|(name, _)| name == "host")
        .map(|(_, value)| value.clone());
    Some(Seen {
        host,
        sni,
        method,
        path,
        headers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_head_is_recorded_with_its_host_path_and_headers() {
        let seen = parse(
            "GET /a?b HTTP/1.1\r\nHost: api.example.test\r\nX-Probe: 1\r\n\r\n",
            Some("api.example.test".into()),
        )
        .expect("a head");
        assert_eq!(seen.method, "GET");
        assert_eq!(seen.path, "/a?b");
        assert_eq!(seen.host.as_deref(), Some("api.example.test"));
        assert_eq!(seen.sni.as_deref(), Some("api.example.test"));
        assert!(seen.headers.contains(&("x-probe".into(), "1".into())));
    }
}
