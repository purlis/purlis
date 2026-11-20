// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The forge proxy's listener with a fake forge behind it and a client in front, on socket pairs:
//! no port is bound, so the whole pipeline (framing, the core's verdict, the upstream request, the
//! answer's scrub) runs wherever the tests do.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use purlis_core::forge::proxy::listener::{
    ChatCa, Dial, Limits, Upstream, UpstreamError, UpstreamRequest, UpstreamResponse, Wiring,
    serve_plain, serve_terminated,
};
use purlis_core::forge::proxy::{
    AllowedRepo, ChatId, Credential, CredentialError, CredentialRequest, CredentialSource,
    ForgeProxy, Kind, NoObjects, Sentinel, TokenKind,
};
use purlis_core::secrets::keyring::Secret;

/// The person's real token. No answer may carry it to the chat.
pub const REAL: &str = "REALforgeTOKEN-9d41c7";

pub struct Creds;

impl CredentialSource for Creds {
    fn credential_for(&self, _: &CredentialRequest) -> Result<Credential, CredentialError> {
        Ok(Credential {
            token_kind: TokenKind::Oauth,
            secret: Secret::new(REAL.to_string()),
        })
    }
}

/// One canned answer.
#[derive(Clone)]
pub struct Answer {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Answer {
    pub fn ok(body: &str) -> Self {
        Self {
            status: 200,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: body.as_bytes().to_vec(),
        }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }
}

/// A fake forge: answers in order, and keeps every request it was sent.
#[derive(Default)]
pub struct Fake {
    pub seen: Mutex<Vec<UpstreamRequest>>,
    answers: Mutex<VecDeque<Answer>>,
}

impl Fake {
    pub fn answer(&self, answer: Answer) {
        self.answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(answer);
    }

    pub fn requests(&self) -> Vec<UpstreamRequest> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Upstream for Fake {
    fn send(&self, request: UpstreamRequest) -> Result<UpstreamResponse, UpstreamError> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request);
        let answer = self
            .answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .ok_or_else(|| UpstreamError("the fake forge has no answer left".into()))?;
        Ok(UpstreamResponse {
            status: answer.status,
            headers: answer.headers,
            body: Box::new(std::io::Cursor::new(answer.body)),
        })
    }
}

pub struct NoDial;

impl Dial for NoDial {
    fn dial(&self, _: &str, _: u16) -> std::io::Result<TcpStream> {
        Err(std::io::Error::other("no tunnel in this test"))
    }
}

pub struct World {
    pub proxy: Arc<ForgeProxy>,
    pub chat: ChatId,
    pub sentinel: Sentinel,
    pub fake: Arc<Fake>,
    pub wiring: Wiring,
}

impl World {
    /// One chat with a repo on github.com and one on gitlab.com.
    pub fn new() -> Self {
        Self::with_repos(vec![
            AllowedRepo::new(Kind::GitHub, "github.com", 443, "acme/app", "main")
                .unwrap()
                .with_node_id("R_acme_app"),
            AllowedRepo::new(Kind::GitLab, "gitlab.com", 443, "group/proj", "main")
                .unwrap()
                .with_project_id(42),
        ])
    }

    pub fn with_repos(repos: Vec<AllowedRepo>) -> Self {
        let proxy = Arc::new(ForgeProxy::new(Box::new(Creds), Box::new(NoObjects)));
        let chat = ChatId::new("chat-listener");
        let advertised: Vec<(String, String)> = repos
            .iter()
            .map(|r| (r.host().to_string(), r.path().to_string()))
            .collect();
        let sentinel = proxy.enrol(&chat, repos);
        for (host, path) in &advertised {
            proxy.note_advertised(&chat, host, path, vec![]);
        }
        let fake = Arc::new(Fake::default());
        let wiring = Wiring {
            chat: chat.clone(),
            proxy: Arc::clone(&proxy),
            upstream: fake.clone(),
            dial: Arc::new(NoDial),
            limits: Limits {
                io_timeout: Duration::from_secs(10),
                ..Limits::default()
            },
            storage: Default::default(),
            observe: None,
        };
        Self {
            proxy,
            chat,
            sentinel,
            fake,
            wiring,
        }
    }
}

/// An answer the chat read.
#[derive(Debug)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Read what the listener sent: a status line, headers, and a body framed by length or `chunked`.
pub fn parse_reply(raw: &[u8]) -> Reply {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("no head in {:?}", String::from_utf8_lossy(raw)));
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|s| s.parse().ok())
        .expect("a status");
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_string(), v.trim().to_string()))
        .collect();
    let mut body = raw[split + 4..].to_vec();
    let chunked = headers
        .iter()
        .any(|(n, v)| n.eq_ignore_ascii_case("transfer-encoding") && v == "chunked");
    if chunked {
        let mut out = Vec::new();
        let mut rest = &body[..];
        loop {
            let at = rest
                .windows(2)
                .position(|w| w == b"\r\n")
                .expect("a chunk line");
            let size =
                usize::from_str_radix(std::str::from_utf8(&rest[..at]).unwrap(), 16).unwrap();
            rest = &rest[at + 2..];
            if size == 0 {
                break;
            }
            out.extend_from_slice(&rest[..size]);
            rest = &rest[size + 2..];
        }
        body = out;
    }
    Reply {
        status,
        headers,
        body,
    }
}

/// Send `request` to the listener's plain door, as `gh` on its unix socket does, and read what it
/// answers until it closes.
pub fn plain(world: &World, request: &[u8]) -> Reply {
    parse_reply(&plain_raw(world, request))
}

pub fn plain_raw(world: &World, request: &[u8]) -> Vec<u8> {
    let (mut client, server) = UnixStream::pair().unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut out = Vec::new();
    std::thread::scope(|scope| {
        scope.spawn(|| serve_plain(&world.wiring, server));
        // A listener that has already answered and closed is a write that fails, not a bug.
        let _ = client.write_all(request);
        // The request is all there is: a head that never ends reads as the end of the stream.
        let _ = client.shutdown(std::net::Shutdown::Write);
        let _ = client.read_to_end(&mut out);
    });
    out
}

/// The client half of a TLS connection to the listener's terminating door: it trusts the chat's
/// authority and asks for `sni` by name.
pub fn client_config(ca_pem: &str) -> Arc<rustls::ClientConfig> {
    let body: String = ca_pem.lines().filter(|l| !l.starts_with("-----")).collect();
    let der = data_encoding::BASE64
        .decode(body.as_bytes())
        .expect("a PEM body");
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(der))
        .expect("a certificate");
    Arc::new(
        rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth(),
    )
}

/// A request inside the TLS the proxy terminates for the `CONNECT` target `host:port`, with the
/// client asking for `sni`. `Err` is a handshake that did not complete.
pub fn tls(
    world: &World,
    ca: &ChatCa,
    host: &str,
    port: u16,
    sni: &str,
    request: &[u8],
) -> Result<Reply, String> {
    tls_with(world, ca, host, port, sni, |_| {}, request)
}

/// [`tls`] with the client's settings changed first: its ALPN list, or whether it sends SNI.
pub fn tls_with(
    world: &World,
    ca: &ChatCa,
    host: &str,
    port: u16,
    sni: &str,
    tweak: impl FnOnce(&mut rustls::ClientConfig),
    request: &[u8],
) -> Result<Reply, String> {
    let (client, server) = UnixStream::pair().unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut config = client_config(ca.certificate_pem());
    tweak(Arc::get_mut(&mut config).expect("a config of our own"));
    let name = rustls::pki_types::ServerName::try_from(sni.to_string()).unwrap();
    let connection = rustls::ClientConnection::new(config, name).unwrap();
    let mut stream = rustls::StreamOwned::new(connection, client);
    let mut out = Vec::new();
    let result = std::thread::scope(|scope| {
        scope.spawn(|| serve_terminated(&world.wiring, ca, host, port, server));
        let wrote = stream.write_all(request).and_then(|()| stream.flush());
        match wrote {
            Ok(()) => {
                let _ = stream.read_to_end(&mut out);
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    });
    result.map(|()| parse_reply(&out))
}

/// `Authorization: token <sentinel>`, as `gh` sends it.
pub fn gh_get(world: &World, target: &str) -> Vec<u8> {
    format!(
        "GET {target} HTTP/1.1\r\nHost: api.github.localhost\r\nAuthorization: token {}\r\nAccept: application/json\r\n\r\n",
        world.sentinel.expose()
    )
    .into_bytes()
}

pub fn basic(user: &str, password: &str) -> String {
    format!(
        "Basic {}",
        data_encoding::BASE64.encode(format!("{user}:{password}").as_bytes())
    )
}

/// One pkt-line.
pub fn pkt(text: &str) -> String {
    format!("{:04x}{text}", text.len() + 4)
}
