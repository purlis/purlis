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
    ForgeProxy, ForgeRepoId, Kind, NoObjects, Sentinel, TokenKind,
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
            account: None,
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

/// The plane's project id the recording world's chat belongs to.
pub const PROJECT_ID: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S2P";
/// The branches a [`World::asking_in`] repo's advertisement lists: the ones a test may be asked
/// about, and two the ask must not say (a name that is not ASCII, one that starts with `-`).
pub const ASKING_ADVERTISES: &[&str] = &[
    "main", "theirs", "other", "b1", "b2", "b3", "b4", "b5", "naïve", "-dash",
];

/// The session [`World::asking_in`]'s chat is.
pub const SESSION: u32 = 7;
/// The forge's id of `acme/app` in [`World`].
pub const ACME_APP_ID: u64 = 41;

impl World {
    /// One chat with a repo on github.com and one on gitlab.com.
    pub fn new() -> Self {
        Self::with_repos(Self::default_repos())
    }

    pub fn with_repos(repos: Vec<AllowedRepo>) -> Self {
        Self::with_repos_and_pin(repos, Arc::new(purlis_core::forge::pin::NoPin))
    }

    /// [`Self::new`]'s repos, the proxy asking `pin` on every request.
    pub fn with_pin(pin: Arc<dyn purlis_core::forge::pin::Pin>) -> Self {
        Self::with_repos_and_pin(Self::default_repos(), pin)
    }

    fn default_repos() -> Vec<AllowedRepo> {
        vec![
            AllowedRepo::new(Kind::GitHub, "github.com", 443, "acme/app", "main")
                .unwrap()
                .with_node_id("R_acme_app")
                .with_forge_id(ForgeRepoId::for_a_test(41)),
            AllowedRepo::new(Kind::GitLab, "gitlab.com", 443, "group/proj", "main")
                .unwrap()
                .with_forge_id(ForgeRepoId::for_a_test(42)),
        ]
    }

    /// [`Self::new`]'s repos, the proxy asking `stance` how `guards.merge` runs on each merge.
    pub fn with_merge_stance(
        stance: Arc<dyn purlis_core::forge::mergestance::MergeStance>,
    ) -> Self {
        Self::build(
            Self::default_repos(),
            Arc::new(purlis_core::forge::pin::NoPin),
            Some(stance),
            None,
            None,
            None,
        )
    }

    pub fn with_repos_and_pin(
        repos: Vec<AllowedRepo>,
        pin: Arc<dyn purlis_core::forge::pin::Pin>,
    ) -> Self {
        Self::build(repos, pin, None, None, None, None)
    }

    /// [`Self::new`]'s repos, with each branch the forge confirms the chat made recorded in the
    /// config root `config`, as the app does ([`purlis_core::forge::createdbranches`]).
    pub fn recording_to(config: std::path::PathBuf) -> Self {
        Self::build(
            Self::default_repos(),
            Arc::new(purlis_core::forge::pin::NoPin),
            None,
            Some(config),
            None,
            None,
        )
    }

    /// [`Self::recording_to`] over `repos` in place of [`Self::new`]'s.
    pub fn recording_repos(repos: Vec<AllowedRepo>, config: std::path::PathBuf) -> Self {
        Self::build(
            repos,
            Arc::new(purlis_core::forge::pin::NoPin),
            None,
            Some(config),
            None,
            None,
        )
    }

    /// [`Self::recording_to`], where the chat may also move a branch a chat it dispatched made,
    /// by the dispatch records of the project at `project`, as the app reads them.
    pub fn recording_with_dispatches(
        config: std::path::PathBuf,
        project: std::path::PathBuf,
    ) -> Self {
        Self::build(
            Self::default_repos(),
            Arc::new(purlis_core::forge::pin::NoPin),
            None,
            Some(config),
            Some(project),
            None,
        )
    }

    /// [`Self::recording_with_dispatches`], the repos also protecting `names` exactly, as the app
    /// wires a project's `[risk] protected_branches` (`forgeroute`'s `repos`).
    pub fn recording_with_dispatches_protecting(
        config: std::path::PathBuf,
        project: std::path::PathBuf,
        names: &[&str],
    ) -> Self {
        let repos = Self::default_repos()
            .into_iter()
            .map(|repo| repo.with_protected_branches(names))
            .collect();
        Self::build(
            repos,
            Arc::new(purlis_core::forge::pin::NoPin),
            None,
            Some(config),
            Some(project),
            None,
        )
    }

    /// [`Self::recording_to`], where an update of a branch the chat has no record of raises the
    /// ask to let it push, in the project at `project`, as the app wires it
    /// ([`purlis_core::windowonly::PushAsks`]): the chat is session [`SESSION`] of that project.
    pub fn asking_in(config: std::path::PathBuf, project: std::path::PathBuf) -> Self {
        Self::build(
            Self::default_repos(),
            Arc::new(purlis_core::forge::pin::NoPin),
            None,
            Some(config),
            None,
            Some(project),
        )
    }

    fn build(
        repos: Vec<AllowedRepo>,
        pin: Arc<dyn purlis_core::forge::pin::Pin>,
        stance: Option<Arc<dyn purlis_core::forge::mergestance::MergeStance>>,
        record_to: Option<std::path::PathBuf>,
        dispatches: Option<std::path::PathBuf>,
        asking: Option<std::path::PathBuf>,
    ) -> Self {
        let mut proxy = ForgeProxy::new(Box::new(Creds), Box::new(NoObjects)).with_pin(pin);
        if let Some(stance) = stance {
            proxy = proxy.with_merge_stance(stance);
        }
        if let Some(config) = record_to {
            let forgets = config.clone();
            proxy = proxy.with_gone(Box::new(move |gone| {
                purlis_core::forge::createdbranches::forget(&forgets, gone)
                    .expect("the record ends");
            }));
            let tips = config.clone();
            proxy = proxy.with_tips(std::sync::Arc::new(move |host, repo_id, branch| {
                purlis_core::forge::createdbranches::tip_for(
                    &tips,
                    PROJECT_ID,
                    "chat-listener",
                    host,
                    repo_id,
                    branch,
                    &|owner| {
                        dispatches.as_ref().is_some_and(|project| {
                            purlis_core::dispatchrecord::lineage_allows(
                                project,
                                "chat-listener",
                                owner,
                                branch,
                            )
                        })
                    },
                )
            }));
            proxy = proxy.with_created(Box::new(move |created| {
                purlis_core::forge::createdbranches::record_via(
                    &config,
                    PROJECT_ID,
                    "chat-listener",
                    created,
                    Some(purlis_core::forge::createdbranches::Via::Route),
                )
                .expect("the record is written");
            }));
        }
        let asking_world = asking.is_some();
        if let Some(root) = asking {
            proxy = proxy.with_branch_ask(Box::new(purlis_core::windowonly::PushAsks {
                root,
                chat: "chat-listener".to_owned(),
                session: Box::new(|| Some(SESSION)),
                owner_of: Box::new(|_, _, _| None),
            }));
        }
        let proxy = Arc::new(proxy);
        let chat = ChatId::new("chat-listener");
        let advertised: Vec<(String, String)> = repos
            .iter()
            .map(|r| (r.host().to_string(), r.path().to_string()))
            .collect();
        let sentinel = proxy.enrol(&chat, repos);
        for (host, path) in &advertised {
            let listed = if asking_world {
                ASKING_ADVERTISES.iter().map(|b| (*b).to_owned()).collect()
            } else {
                vec![]
            };
            proxy.note_advertised(&chat, host, path, listed);
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
            handoff: purlis_core::sandbox::egress::handoff::Secret::random().unwrap(),
            push: Arc::new(purlis_core::forge::proxy::listener::ForwardAll),
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

/// **What a chat's egress proxy does for a forge host's `CONNECT`**, for a client that cannot prove
/// the hand-off secret itself (a real `git`): a loopback relay that opens the door as the proxy does
/// ([`purlis_core::sandbox::egress::open_door`], so the proof and the door's proof back are the
/// shipped ones) and then carries bytes both ways. Returns the relay's port.
#[allow(dead_code)]
pub fn relay_to_door(
    door: std::net::SocketAddr,
    handoff: purlis_core::sandbox::egress::handoff::Secret,
) -> u16 {
    use purlis_core::sandbox::egress::{ForgeRoute, open_door};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for client in listener.incoming().flatten() {
            let handoff = handoff.clone();
            std::thread::spawn(move || {
                let mut client = client;
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    if client.read(&mut byte).unwrap_or(0) == 0 {
                        return;
                    }
                    head.push(byte[0]);
                }
                let head = String::from_utf8_lossy(&head).into_owned();
                let Some(target) = head.split_whitespace().nth(1) else {
                    return;
                };
                let Some((host, at)) = target.rsplit_once(':') else {
                    return;
                };
                let Ok(at) = at.parse::<u16>() else { return };
                let route = ForgeRoute {
                    addr: door,
                    handoff,
                    hosts: vec![host.to_owned()],
                };
                let Ok((upstream, answered)) =
                    open_door(&route, host, at, &[], Duration::from_secs(10))
                else {
                    return;
                };
                if client.write_all(&answered).is_err() {
                    return;
                }
                let (Ok(mut client_out), Ok(mut upstream_in)) =
                    (client.try_clone(), upstream.try_clone())
                else {
                    return;
                };
                let (mut client_in, mut upstream_out) = (client, upstream);
                std::thread::spawn(move || {
                    let _ = std::io::copy(&mut upstream_in, &mut client_out);
                    let _ = client_out.shutdown(std::net::Shutdown::Both);
                });
                let _ = std::io::copy(&mut client_in, &mut upstream_out);
                let _ = upstream_out.shutdown(std::net::Shutdown::Both);
            });
        }
    });
    port
}
