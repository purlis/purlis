// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the tests start a run's key proxy through** (D-3): a [`StartPath`], and the desktop's,
//! [`Desktop`]: the window's `StartKeyProxy` to its session host, which starts the real key
//! proxy through the binary's trait. Here that trait is the test's own: it hands the key proxy
//! the route the test named for the run, or the one it resolves at the start from [`Sources`],
//! a declaration store and your keys that count their reads (T1a-08), with the stand-in key (the
//! real start's wiring is T1a-17's, its key T1a-61's). A start path is made with the [`Reach`] its key proxy reaches
//! upstreams through (T1a-60): [`plain_reach`] for a plain-HTTP provider on loopback, or one the
//! test made, with the fake provider's test root in the trust store, its name reached on
//! loopback and the outbound proxy the test names in place of the system's.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use fake_harness::model::{Provider, Recording, TestRoot};
use http_body_util::{BodyExt, Full};
use hyper::{HeaderMap, Request};
use hyper_util::rt::TokioIo;
use purlis_core::model_provider::{self, Format, built_ins};
use purlis_core::model_route;
use purlis_core::seam::{self, Kind, RecordingCredentials, RecordingFiles, Target};
use purlis_core::secrets::keyring::{FileStore, STUB_FILE, Secret};
use purlis_core::yourkeys::{self, mark, start};
use purlis_core::upstream::{Direct, TestReach, trust};
use purlis_host::standin::StandIns;
use purlis_host::{
    Compose, Ended, Hosting, KeyProxyStarted, KeyProxyTell, KeyProxyTold, ProxiesKeys, Record,
    ToCompose,
};
use purlis_key_proxy::{Heard, Key, KeyProxy, Matcher, Origin, Reach, Route};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Answer, Client, Command, Events};
use purlis_terminal::session::Spec;
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};
use ulid::Ulid;

/// How long a test waits on the key proxy or the host before it fails.
pub const PATIENCE: Duration = Duration::from_secs(10);

/// The stand-in for the run's key, which the fake provider must be handed in the token's place.
pub const STAND_IN_KEY: &str = "stand-in-for-the-runs-key";

/// **How a run's key proxy is started and stopped**, and what its host wrote and raised: the
/// parameter every test here takes, so each runs through the desktop's start and, from T1a-66,
/// the runner's.
pub(crate) trait StartPath {
    /// Starts `run`'s key proxy with `route` as its route: its port and token.
    async fn start(&self, run: &str, route: Route) -> (u16, String);

    /// Starts `run`'s key proxy on the route the binary's trait resolves from `sources` when
    /// the start reaches it, as a real start resolves its profile's: its port and token.
    async fn start_from(&self, run: &str, sources: Arc<Sources>) -> (u16, String);

    /// How many requests have arrived on `run`'s listener: the route-used signal.
    fn arrivals(&self, run: &str) -> usize;

    /// Stops `run`'s key proxy, as its run ends.
    async fn stop(&self, run: &str);

    /// What the host raised for the window since this was last asked: each event it pushed,
    /// as its variant's name, and the `kind` of each line it wrote in the event log.
    async fn raised(&mut self) -> Vec<String>;

    /// Everything the host wrote, as text: the files it wrote, the lines of its event log and
    /// every line the key proxy told it.
    fn written(&self) -> Vec<String>;
}

/// A route to `provider` over plain HTTP, with the stand-in key in `x-api-key`, as the Anthropic
/// built-in names.
pub fn route_to(provider: &Provider) -> Route {
    Route {
        origin: Origin::http("127.0.0.1", provider.port()),
        bound: anthropic(),
        key: Key::XApiKey(STAND_IN_KEY.to_owned()),
    }
}

/// The Anthropic built-in's Messages route for Claude Code (T1a-08).
pub fn anthropic() -> model_route::Route {
    let declaration = built_ins()
        .iter()
        .find(|declaration| declaration.id == "anthropic")
        .expect("the Anthropic built-in ships");
    model_route::Route::bind("claude-anthropic", declaration, Format::AnthropicMessages)
        .expect("a Messages endpoint")
}

/// **Where a run's route comes from at its start** (R-077, R-479): a declaration store holding
/// one declaration of the person's own, your keys holding the stand-in key, and the plain-HTTP
/// port on loopback each declared origin is served at; each read of the store and of your keys
/// counted.
pub struct Sources {
    declaration: Mutex<String>,
    served_at: HashMap<String, u16>,
    your_keys: YourKeys,
    pub declaration_reads: AtomicUsize,
    /// How many times the entry's item had been read when the last start's read ended.
    pub key_reads: AtomicUsize,
}

/// **Your keys, on a machine of the test's own**: a config home with the stand-in key added
/// through the window's write and marked for the Anthropic built-in, on the fenced stub keyring
/// behind the recording vault seam (T1a-69), so each read of its item is counted.
struct YourKeys {
    config: tempfile::TempDir,
    record: seam::Record,
    credentials: RecordingCredentials,
    entry: Ulid,
}

impl YourKeys {
    fn new() -> YourKeys {
        let config = tempfile::tempdir().expect("a config home");
        let record = seam::Record::default();
        let credentials = RecordingCredentials::new(
            Box::new(FileStore::at(config.path().join(STUB_FILE))),
            record.clone(),
        );
        let entry = match yourkeys::add(
            config.path(),
            &credentials,
            yourkeys::Product::anthropic(),
            yourkeys::Checked::Passed,
            STAND_IN_KEY,
        ) {
            Ok(yourkeys::Saved::Added(entry)) => entry,
            other => panic!("the stand-in key was not added: {other:?}"),
        };
        let declaration = built_ins()
            .iter()
            .find(|declaration| declaration.id == "anthropic")
            .expect("the Anthropic built-in ships");
        mark::mark(
            config.path(),
            &credentials,
            entry,
            &mark::Binding::of(declaration),
        )
        .unwrap_or_else(|why| panic!("the stand-in key was not marked: {why}"));
        record.clear();
        YourKeys {
            config,
            record,
            credentials,
            entry,
        }
    }

    /// The start's one read, through core.
    fn read(&self) -> String {
        let files = RecordingFiles::new(self.record.clone());
        match start::read_once(self.config.path(), &files, &self.credentials, self.entry) {
            start::Read::Key(value, _) => Secret::into_inner(value),
            start::Read::Locked(_) => panic!("the stub keychain is never locked"),
            start::Read::Refused(why) => panic!("the start was refused: {why}"),
        }
    }

    /// How many times the entry's item has been read.
    fn reads(&self) -> usize {
        let item = Target::Entry {
            service: yourkeys::SERVICE.to_owned(),
            account: self.entry.to_string(),
        };
        self.record
            .ops()
            .iter()
            .filter(|op| op.kind == Kind::CredentialRead && op.target == item)
            .count()
    }
}

impl Sources {
    /// A store holding a Messages declaration whose base is `base`, each origin in `served_at`
    /// served on that loopback port.
    pub fn new(base: &str, served_at: &[(&str, u16)]) -> Arc<Sources> {
        Arc::new(Sources {
            declaration: Mutex::new(Sources::text(base)),
            served_at: served_at
                .iter()
                .map(|(origin, port)| ((*origin).to_owned(), *port))
                .collect(),
            your_keys: YourKeys::new(),
            declaration_reads: AtomicUsize::new(0),
            key_reads: AtomicUsize::new(0),
        })
    }

    fn text(base: &str) -> String {
        format!(
            "id = \"gateway\"\nname = \"A gateway\"\nkind = \"router\"\n\n[[endpoint]]\n\
             format = \"anthropic-messages\"\nbase = \"{base}\"\nauth = \"x-api-key\"\n"
        )
    }

    /// The person edits the declaration: its base is now `base`.
    pub fn edit(&self, base: &str) {
        *self.declaration.lock().unwrap() = Sources::text(base);
    }

    /// How many times your keys' entry has been read, by the recording vault seam.
    pub fn your_keys_reads(&self) -> usize {
        self.your_keys.reads()
    }

    /// The route a start resolves: the declaration read once, bound, and the key read once.
    fn route(&self) -> Route {
        self.declaration_reads.fetch_add(1, Ordering::SeqCst);
        let text = self.declaration.lock().unwrap().clone();
        let declaration = model_provider::parse(&text, &model_provider::From::Yours, &[], "g.toml")
            .expect("the store holds a declaration");
        let bound = model_route::Route::bind("mine", &declaration, Format::AnthropicMessages)
            .expect("a Messages endpoint");
        let key = self.your_keys.read();
        self.key_reads
            .store(self.your_keys.reads(), Ordering::SeqCst);
        Route {
            origin: Origin::http("127.0.0.1", self.served_at[bound.base.origin()]),
            bound,
            key: Key::XApiKey(key),
        }
    }
}

/// The name the TLS tests' upstream is declared by, served by the fake provider on loopback.
pub const UPSTREAM: &str = "api.anthropic.com";

/// A route to [`UPSTREAM`] over TLS on its own port, 443, with the stand-in key in `x-api-key`.
pub fn route_to_upstream() -> Route {
    Route {
        origin: Origin::https(UPSTREAM, 443),
        bound: anthropic(),
        key: Key::XApiKey(STAND_IN_KEY.to_owned()),
    }
}

/// How a key proxy reaches a plain-HTTP provider on loopback: no outbound proxy, whatever this
/// machine's settings name.
pub fn plain_reach() -> Reach {
    Reach {
        outbound: Arc::new(Direct::new()),
        tls: Arc::new(trust::client_config().expect("the OS trust store")),
        proxies: Matcher::builder().build(),
    }
}

/// How a key proxy reaches [`UPSTREAM`] when it is served at `served`: its name resolves there,
/// `trusted` (the fake provider's test root, or none) sits in the trust-store seam beside the
/// OS's own roots, and `proxies` stands for the proxy variables in `purlisd`'s environment.
pub fn reach(served: std::net::SocketAddr, trusted: Option<&TestRoot>, proxies: Matcher) -> Reach {
    let tls = match trusted {
        Some(root) => trust::trusting_test_root(root.certificate()),
        None => trust::client_config(),
    };
    Reach {
        outbound: Arc::new(Direct::with_test_reach(TestReach::new(UPSTREAM, served))),
        tls: Arc::new(tls.expect("a TLS client")),
        proxies,
    }
}

/// A fake provider for [`UPSTREAM`] over TLS under a fresh test root, answering Messages: the
/// provider and its root.
pub fn tls_provider() -> (Provider, TestRoot) {
    let root = TestRoot::generate(&[UPSTREAM]);
    let provider = Provider::builder()
        .answer("/v1/messages", Recording::message(MESSAGE))
        .tls(&root)
        .start()
        .expect("the fake provider serves");
    (provider, root)
}

/// A fake provider answering Messages with a non-streamed message.
pub fn provider() -> Provider {
    Provider::builder()
        .answer("/v1/messages", Recording::message(MESSAGE))
        .start()
        .expect("the fake provider serves")
}

/// A non-streamed Messages answer.
pub const MESSAGE: &str = r#"{"id":"msg_1","type":"message","role":"assistant","content":[{"type":"text","text":"hé"}],"model":"claude-test","stop_reason":"end_turn","usage":{"input_tokens":3,"output_tokens":1}}"#;

/// What the key proxy answered the harness.
#[derive(Debug)]
pub struct Answered {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

/// Sends a `POST` of `body` to `path` on `port`, carrying `token` as the bearer credential when
/// there is one, as Claude Code sets it from `ANTHROPIC_AUTH_TOKEN`; or why it got no answer.
pub async fn try_send(
    port: u16,
    token: Option<&str>,
    path: &str,
    body: &[u8],
) -> Result<Answered, String> {
    try_send_with(port, token, path, body, &[]).await
}

/// [`try_send`], with `extra` headers beside the token as the harness set them; a `host` among
/// them in place of the listener's own.
pub async fn try_send_with(
    port: u16,
    token: Option<&str>,
    path: &str,
    body: &[u8],
    extra: &[(&str, &str)],
) -> Result<Answered, String> {
    let stream = tokio::time::timeout(
        PATIENCE,
        tokio::net::TcpStream::connect(("127.0.0.1", port)),
    )
    .await
    .map_err(|_| "no connection in time".to_owned())?
    .map_err(|why| format!("no connection: {why}"))?;
    let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .map_err(|why| why.to_string())?;
    tokio::spawn(connection);
    let mut request = Request::post(path)
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01");
    if !extra
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("host"))
    {
        request = request.header("host", format!("127.0.0.1:{port}"));
    }
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    for (name, value) in extra {
        request = request.header(*name, *value);
    }
    let request = request
        .body(Full::new(Bytes::copy_from_slice(body)))
        .expect("a request of fixed parts");
    let answered = tokio::time::timeout(PATIENCE, async {
        let response = sender.send_request(request).await?;
        let (parts, body) = response.into_parts();
        let body = body.collect().await?.to_bytes().to_vec();
        Ok::<_, hyper::Error>(Answered {
            status: parts.status.as_u16(),
            headers: parts.headers,
            body,
        })
    })
    .await
    .map_err(|_| "no answer in time".to_owned())?;
    answered.map_err(|why| why.to_string())
}

/// Sends `method` on `target` to `port` as written, the run's `token` as the bearer credential
/// when there is one, an empty body, and the connection closed after the answer: the request a
/// client library would refuse to write, read to the connection's end.
pub async fn raw(port: u16, method: &str, target: &str, token: Option<&str>) -> Answered {
    let mut stream = tokio::time::timeout(
        PATIENCE,
        tokio::net::TcpStream::connect(("127.0.0.1", port)),
    )
    .await
    .expect("connected in time")
    .expect("the listener accepts");
    let credential = token
        .map(|token| format!("authorization: Bearer {token}\r\n"))
        .unwrap_or_default();
    let head = format!(
        "{method} {target} HTTP/1.1\r\nhost: 127.0.0.1:{port}\r\n{credential}\
         content-length: 0\r\nconnection: close\r\n\r\n"
    );
    stream.write_all(head.as_bytes()).await.unwrap();
    let mut got = Vec::new();
    tokio::time::timeout(PATIENCE, stream.read_to_end(&mut got))
        .await
        .expect("answered in time")
        .expect("read to its end");
    let text = String::from_utf8_lossy(&got);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("no answer's head in {text:?}"));
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("no status line in {head:?}"));
    let mut headers = HeaderMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.append(
                hyper::header::HeaderName::from_bytes(name.trim().as_bytes()).unwrap(),
                value.trim().parse().unwrap(),
            );
        }
    }
    Answered {
        status,
        headers,
        body: body.as_bytes().to_vec(),
    }
}

/// [`try_send`], answered.
pub async fn send(port: u16, token: Option<&str>, path: &str, body: &[u8]) -> Answered {
    try_send(port, token, path, body)
        .await
        .expect("the key proxy answers")
}

/// [`try_send_with`], answered.
pub async fn send_with(
    port: u16,
    token: Option<&str>,
    path: &str,
    body: &[u8],
    extra: &[(&str, &str)],
) -> Answered {
    try_send_with(port, token, path, body, extra)
        .await
        .expect("the key proxy answers")
}

/// The `error.message` of an answer in the Messages error shape.
pub fn message(answered: &Answered) -> String {
    let body: serde_json::Value =
        serde_json::from_slice(&answered.body).expect("an answer in the Messages error shape");
    body["error"]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("no error message in {body}"))
        .to_owned()
}

/// Every value of `name` in a request the fake provider read, in any letter case.
pub fn headers_named<'a>(request: &'a fake_harness::model::Request, name: &str) -> Vec<&'a str> {
    request
        .headers
        .iter()
        .filter(|(sent, _)| sent.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
        .collect()
}

/// The value of `name` in a request the fake provider read, if it was sent.
pub fn header<'a>(request: &'a fake_harness::model::Request, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(sent, _)| sent.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// The binary's event log, kept in memory.
#[derive(Clone, Default)]
struct Kept(Arc<Mutex<Vec<serde_json::Value>>>);

impl Record for Kept {
    fn record(&self, entry: serde_json::Value) -> Result<(), String> {
        self.0.lock().unwrap().push(entry);
        Ok(())
    }

    fn ended(&self, _ended: &Ended<'_>) {}
}

/// The binary's trait, the test's own: the real key proxy, given the route the test named for
/// each run, keeping every line it tells the host.
#[derive(Clone)]
struct RoutesTheTestNamed {
    proxy: Arc<KeyProxy>,
    routes: Arc<Mutex<HashMap<String, Named>>>,
    told: Arc<Mutex<Vec<String>>>,
    arrivals: Arc<Mutex<HashMap<String, usize>>>,
}

/// The route a test named for a run: a value, or what [`Sources`] resolve at the start.
#[derive(Clone)]
enum Named {
    Route(Route),
    From(Arc<Sources>),
}

impl ProxiesKeys for RoutesTheTestNamed {
    fn start(&self, run: &str, tell: KeyProxyTell) -> Result<KeyProxyStarted, String> {
        let route = self
            .routes
            .lock()
            .unwrap()
            .get(run)
            .cloned()
            .map(|named| match named {
                Named::Route(route) => route,
                Named::From(sources) => sources.route(),
            });
        let (said, recorded) = (Arc::clone(&self.told), Arc::clone(&self.told));
        let (arrivals, arrived_on) = (Arc::clone(&self.arrivals), run.to_owned());
        let (tell_said, tell_recorded) = (Arc::clone(&tell), tell);
        let started = self
            .proxy
            .start(
                run,
                route,
                Heard {
                    say: Arc::new(move |line| {
                        let told = KeyProxyTold::Said(line.to_string());
                        said.lock().unwrap().push(format!("{told:?}"));
                        tell_said(told);
                    }),
                    record: Arc::new(move |record| {
                        let told = KeyProxyTold::Record {
                            method: record.method.clone(),
                            path: record.path.clone(),
                            status: record.status,
                        };
                        recorded.lock().unwrap().push(format!("{told:?}"));
                        tell_recorded(told);
                    }),
                    arrived: Arc::new(move || {
                        *arrivals
                            .lock()
                            .unwrap()
                            .entry(arrived_on.clone())
                            .or_default() += 1;
                    }),
                },
            )
            .map_err(|why| why.to_string())?;
        Ok(KeyProxyStarted {
            port: started.port,
            token: started.token.reveal().to_string(),
        })
    }

    fn stop(&self, run: &str) -> Result<(), String> {
        if self.proxy.stop(run) {
            Ok(())
        } else {
            Err("no key proxy runs for that run".into())
        }
    }
}

/// **The desktop's start**: the window's `StartKeyProxy` and `StopKeyProxy` to its session host,
/// which keeps its files in a folder of its own and its event log in memory.
pub struct Desktop {
    window: Client,
    events: Events,
    proxies: RoutesTheTestNamed,
    log: Kept,
    folder: tempfile::TempDir,
}

impl Desktop {
    /// The desktop's start, its key proxy reaching a plain-HTTP provider ([`plain_reach`]).
    pub async fn new() -> Desktop {
        Desktop::reaching(plain_reach()).await
    }

    /// The desktop's start, its key proxy reaching upstreams through `reach`.
    pub async fn reaching(reach: Reach) -> Desktop {
        let folder = tempfile::tempdir().unwrap();
        let proxies = RoutesTheTestNamed {
            proxy: Arc::new(KeyProxy::reaching(reach).expect("its runtime starts")),
            routes: Arc::default(),
            told: Arc::default(),
            arrivals: Arc::default(),
        };
        let log = Kept::default();
        let hosting = Hosting::running_with_its_switch_in(folder.path(), Echo, StandIns::default())
            .recording_with(log.clone())
            .proxying_keys_with(proxies.clone());
        let (a, b) = duplex(64 * 1024);
        let (client, served) = tokio::join!(
            link::connect(
                a,
                session::speaks(),
                Scope::LocalUi,
                HELD.of(Scope::LocalUi)
            ),
            link::serve_any(b, session::speaks(), &HELD)
        );
        tokio::spawn(session::serve(served.unwrap(), hosting, None));
        let (window, _views, events) = Client::new(client.unwrap());
        window
            .subscribe(Default::default())
            .await
            .expect("subscribed");
        Desktop {
            window,
            events,
            proxies,
            log,
            folder,
        }
    }
}

impl Desktop {
    async fn start_named(&self, run: &str, named: Named) -> (u16, String) {
        self.proxies
            .routes
            .lock()
            .unwrap()
            .insert(run.to_owned(), named);
        let answer = self
            .window
            .call(Command::StartKeyProxy { run: run.into() })
            .await
            .expect("the start is answered");
        let Answer::KeyProxyStarted { port, token } = answer else {
            panic!("not a key proxy's start: {answer:?}");
        };
        (port, token.0)
    }
}

impl StartPath for Desktop {
    async fn start(&self, run: &str, route: Route) -> (u16, String) {
        self.start_named(run, Named::Route(route)).await
    }

    async fn start_from(&self, run: &str, sources: Arc<Sources>) -> (u16, String) {
        self.start_named(run, Named::From(sources)).await
    }

    fn arrivals(&self, run: &str) -> usize {
        self.proxies
            .arrivals
            .lock()
            .unwrap()
            .get(run)
            .copied()
            .unwrap_or_default()
    }

    async fn stop(&self, run: &str) {
        self.window
            .call(Command::StopKeyProxy { run: run.into() })
            .await
            .expect("the stop is answered");
    }

    async fn raised(&mut self) -> Vec<String> {
        let mut raised = Vec::new();
        while let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(300), self.events.recv()).await
        {
            let shown = format!("{event:?}");
            let name = shown
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .find(|word| !word.is_empty())
                .unwrap_or_default()
                .to_owned();
            raised.push(format!("pushed {name}: {shown}"));
        }
        for entry in self.log.0.lock().unwrap().drain(..) {
            raised.push(format!("logged {}: {entry}", entry["kind"]));
        }
        raised
    }

    fn written(&self) -> Vec<String> {
        let mut written = Vec::new();
        files_in(self.folder.path(), &mut written);
        written.extend(self.log.0.lock().unwrap().iter().map(ToString::to_string));
        written.extend(self.proxies.told.lock().unwrap().iter().cloned());
        written
    }
}

/// The contents of every file under `folder`, as text.
fn files_in(folder: &Path, into: &mut Vec<String>) {
    for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_in(&path, into);
        } else if let Ok(bytes) = std::fs::read(&path) {
            into.push(format!(
                "{}: {}",
                path.display(),
                String::from_utf8_lossy(&bytes)
            ));
        }
    }
}
