// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's browser, end to end on a real engine** (#2243, #2294, spec #2008): every hop at
//! once, as the person runs it, the person taking the browser over included. Each hop has its
//! own tests against a stand-in for the next; here none is stood in for.
//!
//! - **The chat** is the repo's fake harness, run in a real `purlis serve` as the window runs
//!   its chats there by default ([`crate::hostedsessions::chats_in_the_host`]). It starts
//!   `purlis mcp` as its own child, as a harness starts its MCP servers, and hands that server's
//!   stdio to the test over a socket (`fake-harness --mcp-relay`): the test speaks to the tool
//!   server as the model does, and the server's lines to the hook socket come from inside the
//!   chat, which is what the chat's hook channel admits.
//! - **The window** is this process: its planes, its asks and its browsers
//!   ([`crate::browsers::WindowBrowsers`]) over its link to the host, as the app wires them,
//!   with no Tauri window. The person's presses are the functions the window's commands call.
//! - **The browser** is the session host's: the pinned Chrome for Testing, downloaded into the
//!   host's data home on first use and checked as on any machine, behind its reach proxy and
//!   its judge.
//! - **The site** is the engine's fixture site, served on this machine's loopback by a plain
//!   HTTP server of the test's own, which the browser reaches as a browser site the person
//!   allowed (`127.0.0.1:<port>`).
//!
//! It runs on Linux, as the behaviour suite's host process does (`crate::behaviour`): there each
//! test gives its host a runtime directory, a config home and a data home of its own. It runs
//! on CI, which downloads the engine; elsewhere it runs when `PURLIS_TEST_PINNED_ENGINE` is set
//! (a sandboxed agent can start no engine).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde_json::{Value, json};

use crate::browsers::WindowBrowsers;
use crate::planes::{Held, Planes};
use crate::sessionhost::SessionHost;

/// How long anything here waits: the first browser's start downloads the engine.
const PATIENCE: Duration = Duration::from_secs(180);

/// The variable that runs these tests off CI, on a machine that may download and start the
/// pinned engine.
const RUN_HERE: &str = "PURLIS_TEST_PINNED_ENGINE";

/// The login the project's persona declares, and what its vault holds for it.
const LOGIN: &str = "shop-login";
const USER: &str = "fixture-user-7f3a";
const PASSWORD: &str = "kept-0ut-9d2c";

/// Whether a real engine runs here, and why not when it does not.
pub(crate) fn a_real_engine_runs_here() -> bool {
    if !cfg!(target_os = "linux") {
        tracing::warn!(
            "not run: a session host process of a test's own runs on Linux alone (see \
             `behaviour::in_a_host_process_at`)"
        );
        return false;
    }
    if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true")
        || std::env::var_os(RUN_HERE).is_some()
    {
        return true;
    }
    tracing::warn!(
        "not run: no engine here. Set {RUN_HERE} to download and start the pinned engine (a \
         sandboxed agent cannot start one; CI runs it)."
    );
    false
}

// ---- the window's log ----

/// Every line the window's log is given in this process, from any thread: the window's own and
/// what its session host says on its standard error.
pub(crate) fn the_log() -> Arc<Mutex<Vec<String>>> {
    static LOG: OnceLock<Arc<Mutex<Vec<String>>>> = OnceLock::new();
    LOG.get_or_init(|| {
        let lines = Arc::new(Mutex::new(Vec::new()));
        // Once per process: nextest runs each test in a process of its own.
        let _ = tracing::subscriber::set_global_default(Hearing(Arc::clone(&lines)));
        lines
    })
    .clone()
}

/// Whether the window's log has a line holding each of `words` yet, waiting for it.
pub(crate) fn logged(all: &[&str]) -> Option<String> {
    let log = the_log();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let found = log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|line| all.iter().all(|words| line.contains(words)))
            .cloned();
        if found.is_some() || Instant::now() > deadline {
            return found;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A subscriber that keeps each event's message.
struct Hearing(Arc<Mutex<Vec<String>>>);

struct Message(String);

impl tracing::field::Visit for Message {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }
}

impl tracing::Subscriber for Hearing {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let mut message = Message(String::new());
        event.record(&mut message);
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(message.0);
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

// ---- the fixture site ----

/// The engine's fixture site, served as a web server serves it on a loopback port of its own,
/// with the path of every request it was sent.
pub(crate) struct Site {
    pub(crate) port: u16,
    heard: Arc<Mutex<Vec<String>>>,
}

impl Site {
    pub(crate) fn serve() -> Site {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let port = listener.local_addr().expect("its address").port();
        let heard: Arc<Mutex<Vec<String>>> = Arc::default();
        let hearing = Arc::clone(&heard);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let hearing = Arc::clone(&hearing);
                std::thread::spawn(move || answer(stream, &hearing));
            }
        });
        Site { port, heard }
    }

    /// `path` on this site, as the browser is sent to it.
    pub(crate) fn at(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}/{path}", self.port)
    }

    /// The browser site the person allows for it.
    pub(crate) fn site(&self) -> purlis_core::sandbox::browser_reach::Site {
        purlis_core::sandbox::browser_reach::Site::of("127.0.0.1", self.port)
    }

    /// The paths it was sent, in order: its pages, not the icon an engine asks for by itself.
    pub(crate) fn heard(&self) -> Vec<String> {
        self.heard
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|path| *path != "favicon.ico")
            .cloned()
            .collect()
    }
}

/// The fixture site's own folder.
fn fixture_site() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/browser-engine/tests/fixtures/site/fixture.test")
}

fn answer(stream: std::net::TcpStream, heard: &Mutex<Vec<String>>) {
    let mut reader = BufReader::new(stream.try_clone().expect("the stream twice"));
    let mut first = String::new();
    if reader.read_line(&mut first).is_err() {
        return;
    }
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) | Err(_) => break,
            Ok(_) if header == "\r\n" || header == "\n" => break,
            Ok(_) => {}
        }
    }
    // An origin's request names its path; one a proxy hands on may name the whole address.
    let target = first.split_whitespace().nth(1).unwrap_or_default();
    let path = target
        .strip_prefix("http://")
        .map_or(target, |rest| rest.find('/').map_or("/", |at| &rest[at..]));
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let name = path.trim_start_matches('/');
    heard
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(name.to_owned());
    let found = (!name.is_empty() && !name.contains("..") && !name.contains('/'))
        .then(|| std::fs::read(fixture_site().join(name)).ok())
        .flatten();
    let (status, body) = match found {
        Some(body) => ("200 OK", body),
        None => ("404 Not Found", b"not here".to_vec()),
    };
    let mut out = stream;
    let _ = write!(
        out,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = out.write_all(&body);
    let _ = out.flush();
}

// ---- the session host, the window and the project ----

/// A real `purlis serve` in a home of its own, which the window started and ends.
pub(crate) struct Host {
    pub(crate) session: SessionHost,
    /// Its data home, where its engines and browsers' folders are.
    pub(crate) data: PathBuf,
    /// Its config home, where this machine's choices are kept.
    pub(crate) config: PathBuf,
    _home: tempfile::TempDir,
}

impl Host {
    /// A host whose data home is its user's default, `~/.local/share/purlis`, its home the
    /// test's: where the profile the `.deb` installs names its engine (#2835; on CI, whose homes
    /// are under the temporary folder, as `ci.yml` tells AppArmor).
    pub(crate) fn serving() -> Host {
        Host::serving_with(true)
    }

    /// A host whose data home is a folder the person named elsewhere: no profile the `.deb`
    /// installs names its engine.
    pub(crate) fn serving_with_its_data_folder_elsewhere() -> Host {
        Host::serving_with(false)
    }

    fn serving_with(default_data_home: bool) -> Host {
        let home = tempfile::tempdir().expect("the host's home");
        let crate::behaviour::HostHome {
            place,
            mut inherited,
            ..
        } = crate::behaviour::a_host_home(home.path());
        let mut data = home.path().join("data");
        if default_data_home {
            let named = |name: &std::ffi::OsString, var: &str| {
                purlis_core::envvar::same(&name.to_string_lossy(), var)
            };
            inherited.retain(|(name, _)| {
                !named(name, purlis_core::datahome::HOME_VAR)
                    && !named(name, "XDG_DATA_HOME")
                    && !named(name, "HOME")
            });
            inherited.push(("HOME".into(), home.path().as_os_str().to_owned()));
            data = purlis_core::names::DATA_HOME.folder_at(&home.path().join(".local/share"));
        }
        let purlis = crate::behaviour::built_beside("purlis", "purlis-cli");
        let session = SessionHost::default();
        session.start_at(place, move || {
            use std::os::unix::process::CommandExt as _;
            let mut command = std::process::Command::new(&purlis);
            command
                .arg("serve")
                .env_clear()
                .envs(inherited.iter().cloned())
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .process_group(0);
            Ok(command)
        });
        session
            .connection()
            .wait(Duration::from_secs(60))
            .expect("the window connects to purlis serve");
        Host {
            session,
            data,
            config: home.path().join("config"),
            _home: home,
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.session.end();
    }
}

/// The window: its planes over its link to the host, its chats run there and their browsers
/// too, holding one project.
pub(crate) struct Window {
    pub(crate) planes: Planes,
    pub(crate) held: Arc<Held>,
    pub(crate) browsers: Arc<WindowBrowsers>,
    /// The chats' browsers as the window drives them, with how many acts are in flight.
    pub(crate) acting: Arc<Acting>,
    pub(crate) root: PathBuf,
    /// Ended after the planes let go, before the folders it serves from go.
    pub(crate) host: Host,
    _project: tempfile::TempDir,
}

impl Window {
    /// A window holding a project whose persona `devops` declares the browser login
    /// [`LOGIN`] on `login`'s site, from a vault of a plain file, allowed on this machine; and
    /// whose browsers may reach each of `allowed`, allowed by the person on this machine.
    pub(crate) fn holding(login: &Site, allowed: &[&Site]) -> Window {
        Window::holding_on(Host::serving, login, allowed)
    }

    /// [`Window::holding`], its session host as `host` starts it.
    pub(crate) fn holding_on(host: fn() -> Host, login: &Site, allowed: &[&Site]) -> Window {
        the_log();
        let project = tempfile::tempdir().expect("the project's folder");
        let root = std::fs::canonicalize(project.path()).expect("its path");
        std::fs::write(
            purlis_core::names::manifest(&root),
            format!(
                "schema = 1\n\n[sandbox]\nmode = \"on\"\n\n[sandbox.personas.devops]\ntools = {{ \
                 {LOGIN} = {{ vault = \"team\", browser = \"{}\", user = \"USER\", key = \
                 \"PASSWORD\" }} }}\n",
                login.at("login.html")
            ),
        )
        .expect("its file");
        std::fs::write(
            root.join("vaults.json"),
            json!({ "vaults": {
                "team": {"provider": "plain-file", "config": {"file": "team.json"}, "persona": "devops"},
            }})
            .to_string(),
        )
        .expect("its vaults");
        std::fs::write(
            root.join("team.json"),
            json!({ "USER": USER, "PASSWORD": PASSWORD }).to_string(),
        )
        .expect("its vault's file");
        // The person's Allows on this machine: the persona's login, and the browser sites.
        let devops = purlis_core::sandbox::persona::shown(&root)
            .into_iter()
            .find(|shown| shown.persona == "devops")
            .expect("devops declares its login");
        purlis_core::sandbox::local::allow_persona_hosts(&root, "devops", &devops.digest)
            .expect("its login allowed here");
        for site in allowed {
            purlis_core::sandbox::local::grant_browser_site(&root, &site.site())
                .expect("a browser site allowed here");
        }

        let host = host();
        let browsers = Arc::new(WindowBrowsers::new(host.session.connection()));
        let acting = Arc::new(Acting {
            browsers: Arc::clone(&browsers),
            in_flight: std::sync::atomic::AtomicUsize::new(0),
        });
        let planes = crate::hostedsessions::chats_in_the_host(
            Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None),
            &host.session.connection(),
        )
        .driving_browsers_with(Arc::clone(&acting) as Arc<dyn crate::chatbrowsers::Driver>);
        let held = planes
            .held(&planes.open(&root))
            .expect("the project is held");
        Window {
            planes,
            held,
            browsers,
            acting,
            root,
            host,
            _project: project,
        }
    }

    /// A chat as persona `devops`, the fake harness, and its tool server.
    pub(crate) fn a_chat(&self) -> (u32, Tools) {
        let folder = tempfile::tempdir().expect("the relay's folder");
        let socket = folder.path().join("relay.sock");
        let listener = UnixListener::bind(&socket).expect("the relay's socket");
        let purlis = crate::behaviour::built_beside("purlis", "purlis-cli");
        let harness = crate::behaviour::built_beside("fake-harness", "fake-harness");
        let chat = purlis_core::reopen::Chat {
            program: harness.display().to_string(),
            args: vec![
                "--mcp-relay".to_owned(),
                socket.display().to_string(),
                "--mcp-server".to_owned(),
                purlis.display().to_string(),
                "mcp".to_owned(),
            ],
            name: "fake".to_owned(),
            persona: Some("devops".to_owned()),
            cwd: Some(self.root.clone()),
            ..Default::default()
        };
        let session = self
            .held
            .chats()
            .start(
                &chat,
                purlis_core::engine::Size {
                    columns: 80,
                    rows: 24,
                },
            )
            .expect("the chat starts in the host");
        listener.set_nonblocking(true).expect("a socket to poll");
        let deadline = Instant::now() + Duration::from_secs(60);
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(why) if why.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "the chat's harness never started its tool server"
                    );
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(why) => panic!("the relay's socket: {why}"),
            }
        };
        stream.set_nonblocking(false).expect("a blocking stream");
        let mut tools = Tools::over(stream, folder);
        tools.initialize();
        (session, tools)
    }

    /// Chat `session`'s browser `b1`'s program in the session host.
    pub(crate) fn program_of(&self, session: u32) -> String {
        self.held
            .browsers()
            .which(session, None)
            .expect("the chat's browser")
            .1
    }
}

impl Window {
    /// Opens `address` in the chat's browser, which must answer: where it does not, with what
    /// every engine's own log says.
    pub(crate) fn opens(&self, tools: &mut Tools, address: &str) -> Said {
        let said = tools.call("browser_open", json!({ "address": address }));
        assert!(
            !said.error,
            "browser_open {address} was refused: {}\n{}",
            said.text,
            self.engine_logs()
        );
        said
    }

    /// What each engine's log in the host's data home says.
    pub(crate) fn engine_logs(&self) -> String {
        let folder = purlis_core::browsers::browsers_in(&self.host.data);
        let mut said = String::new();
        for entry in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.to_string_lossy().ends_with(".engine.log") {
                let log = std::fs::read_to_string(&path).unwrap_or_default();
                said.push_str(&format!("{}:\n{log}\n", path.display()));
            }
        }
        said
    }
}

/// **The window's browsers, as its chats drive them**, counting the acts in flight: what a test
/// waits on to take a browser over while a call of the agent's runs.
pub(crate) struct Acting {
    browsers: Arc<WindowBrowsers>,
    in_flight: std::sync::atomic::AtomicUsize,
}

impl Acting {
    /// How many acts of the chats' are being carried out now.
    pub(crate) fn in_flight(&self) -> usize {
        self.in_flight.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl crate::chatbrowsers::Driver for Acting {
    fn start(
        &self,
        root: &Path,
        reach: purlis_session_protocol::browsing::BrowserReach,
        kept: Option<purlis_session_protocol::browsing::KeptLogin>,
    ) -> Result<String, crate::chatbrowsers::NotStarted> {
        self.browsers.start(root, reach, kept)
    }

    fn act(
        &self,
        program: &str,
        call_id: &str,
        act: purlis_session_protocol::browsing::Act,
        reach: purlis_session_protocol::browsing::BrowserReach,
    ) -> Result<purlis_session_protocol::browsing::Browsed, crate::chatbrowsers::Refused> {
        use std::sync::atomic::Ordering;
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        let acted = self.browsers.act(program, call_id, act, reach);
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        acted
    }

    fn log_in(
        &self,
        program: &str,
        login: purlis_session_protocol::browsing::LogIn,
    ) -> Result<purlis_session_protocol::browsing::Browsed, crate::chatbrowsers::Refused> {
        self.browsers.log_in(program, login)
    }

    fn close(&self, program: &str) -> Result<(), String> {
        self.browsers.close(program)
    }

    fn stop(&self, program: &str) -> Result<(), String> {
        self.browsers.stop(program)
    }

    fn driven(&self) -> Result<Vec<String>, String> {
        self.browsers.driven()
    }

    fn note(&self, program: &str, note: &crate::chatbrowsers::Note) {
        self.browsers.note(program, note);
    }

    fn taken_back(&self, root: &Path, program: &str) {
        self.browsers.taken_back(root, program);
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.planes.let_go_of_all();
    }
}

// ---- the tool server, as the model speaks to it ----

/// What a tool call answered: in error or not, its text, and each image attached.
#[derive(Debug, Clone)]
pub(crate) struct Said {
    pub(crate) error: bool,
    pub(crate) text: String,
    pub(crate) images: Vec<Vec<u8>>,
}

/// The chat's tool server, `purlis mcp`, spoken to over the harness's relay.
pub(crate) struct Tools {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    next: u64,
    /// Everything every tool answered, to read for a value kept out.
    pub(crate) told: Vec<Said>,
    _folder: tempfile::TempDir,
}

impl Tools {
    fn over(stream: UnixStream, folder: tempfile::TempDir) -> Tools {
        stream
            .set_read_timeout(Some(PATIENCE))
            .expect("a bounded read");
        Tools {
            reader: BufReader::new(stream.try_clone().expect("the stream twice")),
            writer: stream,
            next: 1,
            told: Vec::new(),
            _folder: folder,
        }
    }

    fn send(&mut self, message: &Value) {
        writeln!(self.writer, "{message}").expect("the relay takes it");
        self.writer.flush().expect("flushed");
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.ask(method, params);
        self.answer_to(id)
    }

    /// Sends a request and answers its id, without waiting for its answer.
    fn ask(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next;
        self.next += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        id
    }

    /// The answer to request `id`.
    fn answer_to(&mut self, id: u64) -> Value {
        loop {
            let mut line = String::new();
            let read = self
                .reader
                .read_line(&mut line)
                .expect("the server answers");
            assert!(read > 0, "the tool server ended");
            let answer: Value = serde_json::from_str(&line)
                .unwrap_or_else(|why| panic!("not JSON-RPC ({why}): {line:?}"));
            // A notification or a request of the server's own is not the answer.
            if answer.get("id") == Some(&json!(id)) {
                return answer;
            }
        }
    }

    fn initialize(&mut self) {
        let hello = self.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "fake-harness", "version": "1" },
            }),
        );
        assert!(hello.get("result").is_some(), "{hello}");
        self.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    }

    /// Calls tool `name` with `arguments`, as the model does.
    pub(crate) fn call(&mut self, name: &str, arguments: Value) -> Said {
        let id = self.start(name, arguments);
        self.answered(name, id)
    }

    /// Calls tool `name` with `arguments` and leaves it in flight: its call's id.
    pub(crate) fn start(&mut self, name: &str, arguments: Value) -> u64 {
        self.ask(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )
    }

    /// What the call of tool `name` with id `id` answered.
    pub(crate) fn answered(&mut self, name: &str, id: u64) -> Said {
        let answer = self.answer_to(id);
        let result = answer
            .get("result")
            .unwrap_or_else(|| panic!("{name} was not answered: {answer}"));
        let mut said = Said {
            error: result["isError"].as_bool().unwrap_or(false),
            text: String::new(),
            images: Vec::new(),
        };
        for block in result["content"].as_array().into_iter().flatten() {
            match block["type"].as_str() {
                Some("text") => said
                    .text
                    .push_str(block["text"].as_str().unwrap_or_default()),
                Some("image") => said.images.push(
                    base64::engine::general_purpose::STANDARD
                        .decode(block["data"].as_str().unwrap_or_default())
                        .expect("an image in base64"),
                ),
                _ => {}
            }
        }
        self.told.push(said.clone());
        said
    }

    /// [`Tools::call`], which must not be refused.
    pub(crate) fn done(&mut self, name: &str, arguments: Value) -> Said {
        let said = self.call(name, arguments.clone());
        assert!(!said.error, "{name} {arguments} was refused: {}", said.text);
        said
    }
}

/// The reference on the first line of `text` that holds `words`.
pub(crate) fn ref_on(text: &str, words: &str) -> String {
    let line = text
        .lines()
        .find(|line| line.contains(words) && line.contains("[e"))
        .unwrap_or_else(|| panic!("no element with {words:?} in\n{text}"));
    let start = line.find("[e").expect("a reference") + 1;
    let end = line[start..].find(']').expect("its end") + start;
    line[start..end].to_owned()
}

/// How many pixels of a PNG are the fixture site's pure red, the colour its pages draw a value
/// kept out in.
pub(crate) fn red_pixels(png_bytes: &[u8]) -> usize {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().expect("a PNG");
    let mut buffer = vec![0; reader.output_buffer_size().expect("its size")];
    let info = reader.next_frame(&mut buffer).expect("its frame");
    let channels = info.color_type.samples();
    buffer[..info.line_size * info.height as usize]
        .chunks(channels)
        .filter(|pixel| pixel.len() >= 3 && pixel[0] > 200 && pixel[1] < 60 && pixel[2] < 60)
        .count()
}

/// Waits until `done`, at most [`PATIENCE`].
pub(crate) fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + PATIENCE;
    while !done() {
        assert!(Instant::now() < deadline, "{what} never happened");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Whether `bytes` are a PNG.
pub(crate) fn is_a_png(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
}

/// Runs `test`, an ignored test of this binary, again in a process of its own whose data home
/// and config home are the test's, as the app's are its person's: what the window reads there
/// is the test's, never this machine's.
pub(crate) fn in_homes_of_its_own(test: &str) {
    let homes = tempfile::tempdir().expect("the window's homes");
    let mut again = std::process::Command::new(std::env::current_exe().expect("this test"));
    again
        .args([
            "--exact",
            test,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(purlis_core::datahome::HOME_VAR, homes.path().join("data"))
        .env(purlis_core::machine::HOME_VAR, homes.path().join("config"));
    let ran = purlis_core::forklock::output(&mut again).expect("the test runs again");
    assert!(
        ran.status.success() && String::from_utf8_lossy(&ran.stdout).contains("1 passed"),
        "{test} did not pass in homes of its own:\n{}\n{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
}

// ---- #2243: a chat's browser tools, end to end ----

/// The acceptance lines of #2243 and #2016's, on one browser through every hop: the first call
/// raises the Ask; after Allow the tools open, read, find, click, type and screenshot the
/// fixture site; the proxy carries only what the person allowed and the judge holds a new site
/// back; a login purlis performs leaves no value in any answer, and the browser then reaches
/// the login's site alone; and closing it is said in the window's log, with how its engine
/// ended, its engine's log kept.
///
/// The window reads its data home and config home as the app does, from its environment, so
/// the test runs again in a process of its own whose homes are the test's (a login reads the
/// data home for a kept store, #2021).
#[test]
fn a_chats_browser_tools_drive_a_real_engine_through_every_hop() {
    if !a_real_engine_runs_here() {
        return;
    }
    in_homes_of_its_own(
        "browsers::tests::end_to_end::the_browser_tools_through_every_hop_in_homes_of_their_own",
    );
}

/// [`a_chats_browser_tools_drive_a_real_engine_through_every_hop`], in homes of its own.
#[test]
#[ignore = "run by a_chats_browser_tools_drive_a_real_engine_through_every_hop in homes of its own"]
fn the_browser_tools_through_every_hop_in_homes_of_their_own() {
    if !a_real_engine_runs_here() {
        return;
    }
    let site = Site::serve();
    let unlisted = Site::serve();
    let other = Site::serve();
    let window = Window::holding(&site, &[&site, &other]);
    let (chat, mut tools) = window.a_chat();

    // The first call raises the one Ask, and starts nothing.
    let first = tools.call("browser_snapshot", json!({}));
    assert!(first.error, "{first:?}");
    assert!(
        first
            .text
            .contains("purlis asked the person what this chat's browser may reach"),
        "{}",
        first.text
    );
    assert_eq!(window.held.browsers().waiting().len(), 1, "one Ask waits");
    assert!(site.heard().is_empty(), "nothing was opened");
    crate::chatbrowsers::allowed(&window.held, chat, false).expect("the person allows it");

    // Open, read, find, click.
    window.opens(&mut tools, &site.at("counter.html"));
    let snapshot = tools.done("browser_snapshot", json!({}));
    assert!(snapshot.text.contains("Count: 0"), "{}", snapshot.text);
    let found = tools.done("browser_find", json!({ "text": "Add one" }));
    let add = ref_on(&found.text, "button \"Add one\"");
    let clicked = tools.done("browser_click", json!({ "target": add, "changes": true }));
    assert!(clicked.text.contains("Count: 1"), "{}", clicked.text);

    // Type, and the page sees it.
    tools.done(
        "browser_navigate",
        json!({ "address": site.at("login-stays.html") }),
    );
    let field = ref_on(
        &tools
            .done("browser_find", json!({ "text": "Password" }))
            .text,
        "textbox \"Password\"",
    );
    tools.done(
        "browser_type",
        json!({ "fields": [{ "into": field, "text": "seven77" }] }),
    );
    let seen = tools.done(
        "browser_wait_for_text",
        json!({ "text": "holds 7 characters" }),
    );
    assert!(!seen.error, "{}", seen.text);

    // A screenshot is an image result.
    let shot = tools.done("browser_screenshot", json!({}));
    assert_eq!(shot.images.len(), 1, "{shot:?}");
    assert!(is_a_png(&shot.images[0]), "a PNG");

    // The proxy carries what the person allowed, and nothing else on this machine.
    let refused = tools.call(
        "browser_navigate",
        json!({ "address": unlisted.at("opened.html") }),
    );
    // Its page never came: the browser shows the engine's own page for a load that failed,
    // titled by its address, never the fixture's page titled "Opened".
    assert!(
        !refused.text.contains("\"Opened\""),
        "the proxy refused it: {}",
        refused.text
    );
    let shown = tools.done("browser_snapshot", json!({}));
    assert!(
        !shown.text.contains("The opened page"),
        "nothing of the page nobody allowed: {}",
        shown.text
    );
    assert!(
        unlisted.heard().is_empty(),
        "a port of this machine nobody allowed is never reached: {:?}",
        unlisted.heard()
    );
    tools.done(
        "browser_navigate",
        json!({ "address": other.at("opened.html") }),
    );
    assert_eq!(other.heard(), ["opened.html"], "an allowed site is reached");
    // An address the agent writes for a site new to the project is held back, and asks.
    let held_back = tools.call(
        "browser_navigate",
        json!({ "address": "https://example.com/" }),
    );
    assert!(held_back.text.contains("example.com"), "{held_back:?}");
    let waits = window.held.browsers().sites_waiting();
    assert!(
        waits.iter().any(|wait| wait.session == chat
            && wait
                .sites
                .iter()
                .any(|one| one.to_string() == "example.com:443")),
        "the chat's site waits on the person: {waits:?}"
    );

    // A login purlis performs: no value in its answer, nor in any read of the page it left,
    // the page's own "Show password" pressed.
    let logged_in = tools.done("browser_login", json!({ "login": LOGIN }));
    assert!(
        logged_in.text.contains("account.html")
            && logged_in.text.contains("the password field is gone"),
        "the login went through to the page it lands on: {}",
        logged_in.text
    );
    let show = ref_on(
        &tools
            .done("browser_find", json!({ "text": "Show password" }))
            .text,
        "button \"Show password\"",
    );
    tools.done("browser_click", json!({ "target": show, "changes": true }));
    tools.done("browser_snapshot", json!({}));
    tools.done("browser_find", json!({ "text": "Your password" }));
    let shown = tools.done("browser_screenshot", json!({}));
    assert_eq!(shown.images.len(), 1, "{shown:?}");
    // The page draws what was filled in the fixture's red: none of it is left to see.
    assert_eq!(
        red_pixels(&shown.images[0]),
        0,
        "what purlis filled is painted over"
    );
    for said in &tools.told {
        for value in [USER, PASSWORD] {
            assert!(
                !said.text.contains(value),
                "a value purlis filled reached the chat: {}",
                said.text
            );
        }
    }
    // The browser reaches the login's site alone from now on.
    tools.call(
        "browser_navigate",
        json!({ "address": other.at("opened.html") }),
    );
    assert_eq!(
        other.heard(),
        ["opened.html"],
        "a browser holding a login reaches nothing but its site"
    );

    // Closing it: the window's line, the host's, and the engine's own log kept.
    let program = window.program_of(chat);
    tools.done("browser_close", json!({}));
    assert!(
        logged(&[&program, "is ended by the window: its chat closed it"]).is_some(),
        "the window says why it ended the browser"
    );
    let host_said = logged(&["purlisd: browser", &program, "closed at its link's ask"])
        .expect("the host says how the browser ended");
    assert!(host_said.contains("engine's log is"), "{host_said}");
    assert!(
        logged(&["purlis: browser", &program, "ended, its engine's"]).is_some(),
        "the window says how the engine ended"
    );
    let engine_log = purlis_core::browsers::engine_log_of(
        &purlis_core::browsers::browsers_in(&window.host.data).join(&program),
    );
    let mode = {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::metadata(&engine_log)
            .unwrap_or_else(|why| panic!("{}: {why}", engine_log.display()))
            .permissions()
            .mode()
    };
    assert_eq!(mode & 0o777, 0o600, "the engine's log is this user's alone");
}

/// Keep off refuses the next call in words that say not to retry.
#[test]
fn a_chat_whose_browser_the_person_kept_off_is_refused_in_words() {
    if !a_real_engine_runs_here() {
        return;
    }
    let site = Site::serve();
    let window = Window::holding(&site, &[&site]);
    let (chat, mut tools) = window.a_chat();

    let first = tools.call(
        "browser_open",
        json!({ "address": site.at("counter.html") }),
    );
    assert!(first.error, "{first:?}");
    crate::chatbrowsers::kept_off(&window.held, chat, None).expect("the person keeps it off");

    let next = tools.call(
        "browser_open",
        json!({ "address": site.at("counter.html") }),
    );
    assert!(next.error, "{next:?}");
    assert!(
        next.text.contains(crate::chatbrowsers::KEPT_OFF),
        "{}",
        next.text
    );
    assert!(site.heard().is_empty(), "no browser opened anything");
}

// ---- #2835: where the engine cannot make its sandbox ----

/// Where AppArmor restricts user namespaces and no profile names the pinned engine (its data
/// folder elsewhere), the chat is told a browser is not available on this machine, never the
/// engine's crash; the person is offered the profile's command and this machine's own Chrome;
/// and once they choose it, the next call opens on it. Run in homes of its own, since the
/// window reads its data home for the command.
#[test]
fn where_the_engine_cannot_make_its_sandbox_the_chat_is_told_and_this_machines_chrome_is_offered() {
    if !a_real_engine_runs_here() {
        return;
    }
    in_homes_of_its_own(
        "browsers::tests::end_to_end::the_engines_sandbox_and_this_machines_chrome_in_homes_of_their_own",
    );
}

/// [`where_the_engine_cannot_make_its_sandbox_the_chat_is_told_and_this_machines_chrome_is_offered`],
/// in homes of its own.
#[test]
#[ignore = "run by where_the_engine_cannot_make_its_sandbox_the_chat_is_told_and_this_machines_chrome_is_offered"]
fn the_engines_sandbox_and_this_machines_chrome_in_homes_of_their_own() {
    if !a_real_engine_runs_here() {
        return;
    }
    if !purlis_core::browsers::sandboxing::Machine::here().restricts {
        tracing::warn!("not run: nothing restricts user namespaces on this machine");
        return;
    }
    let Some(chrome) = purlis_core::browsers::machine_chrome() else {
        panic!("CI runs this where the runner's own Chrome is installed, and none was found");
    };
    let site = Site::serve();
    let window = Window::holding_on(
        Host::serving_with_its_data_folder_elsewhere,
        &site,
        &[&site],
    );
    let (chat, mut tools) = window.a_chat();
    tools.call("browser_snapshot", json!({}));
    crate::chatbrowsers::allowed(&window.held, chat, false).expect("the person allows it");

    let refused = tools.call(
        "browser_open",
        json!({ "address": site.at("counter.html") }),
    );
    assert!(refused.error, "{refused:?}");
    assert!(
        refused.text.contains(crate::enginesandbox::NO_BROWSER_HERE),
        "{}",
        refused.text
    );
    for crash in ["No usable sandbox", "zygote", "pipe"] {
        assert!(!refused.text.contains(crash), "{}", refused.text);
    }
    assert!(site.heard().is_empty(), "nothing started");

    let notice = crate::enginesandbox::notice()
        .expect("the Notice reads")
        .expect("the Notice stands");
    let command = notice.command.expect("a command to add the profile");
    assert!(command.contains("sudo apparmor_parser -r"), "{command}");
    assert_eq!(notice.chrome, Some(chrome.display().to_string()));

    // The person uses this machine's Chrome: the next call opens on it.
    purlis_core::machine::set_machine_chrome(&window.host.config, true)
        .expect("their choice is kept");
    window.opens(&mut tools, &site.at("counter.html"));
    let read = tools.done("browser_snapshot", json!({}));
    assert!(read.text.contains("Count: 0"), "{}", read.text);
}

// ---- #2294: the person takes the browser over, end to end ----

/// What the person typed, letters alone, in words no page or answer says by itself.
const THEIR_NAME: &str = "zqvname";
const THEIR_PASSWORD: &str = "zqvpass";
const THEIR_ECHO: &str = "zqvecho";

/// The person's view of a chat's browser, as the window draws it: what its latest frame said it
/// shows (`p1:5`, its page and document), which their input names back.
pub(crate) struct Watching {
    latest: Arc<Mutex<Option<String>>>,
}

impl Watching {
    pub(crate) fn of(window: &Window, program: &str) -> Watching {
        let latest: Arc<Mutex<Option<String>>> = Arc::default();
        let drawn = Arc::clone(&latest);
        window
            .browsers
            .watch(
                program,
                crate::browsers::BrowserViewRole::Focused,
                Box::new(move |aim, _image| {
                    if aim.is_some() {
                        *drawn.lock().unwrap_or_else(PoisonError::into_inner) = aim;
                    }
                }),
                Box::new(|_| {}),
                Arc::new(|_| {}),
            )
            .expect("the person watches the browser");
        let watching = Watching { latest };
        watching.aim_other_than(None);
        watching
    }

    /// What the latest frame shows, once it is not `than`.
    pub(crate) fn aim_other_than(&self, than: Option<&str>) -> String {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let latest = self
                .latest
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            if let Some(aim) = latest.filter(|aim| Some(aim.as_str()) != than) {
                return aim;
            }
            assert!(
                Instant::now() < deadline,
                "no frame shows anything but {than:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// The person presses and lets go of `key`, aimed at `aim`: what became of each.
pub(crate) fn press(
    window: &Window,
    program: &str,
    aim: &str,
    key: &str,
    physical: &str,
    key_code: u32,
) -> Vec<crate::browsers::InputOutcome> {
    use crate::browsers::{BrowserInput, KeyAct};
    [KeyAct::Down, KeyAct::Up]
        .into_iter()
        .map(|action| {
            window
                .browsers
                .input(
                    program,
                    BrowserInput::Key {
                        aim: aim.to_owned(),
                        action,
                        key: key.to_owned(),
                        physical: physical.to_owned(),
                        key_code,
                        modifiers: 0,
                    },
                )
                .expect("the window reaches its host")
        })
        .collect()
}

/// The person types `letters` (a to z alone) where the focus is, each key taken.
pub(crate) fn types(window: &Window, program: &str, aim: &str, letters: &str) {
    for letter in letters.chars() {
        let upper = letter.to_ascii_uppercase();
        let outcomes = press(
            window,
            program,
            aim,
            &letter.to_string(),
            &format!("Key{upper}"),
            u32::from(upper),
        );
        assert!(
            outcomes
                .iter()
                .all(|outcome| *outcome == crate::browsers::InputOutcome::Taken),
            "{letter:?} did not reach the page: {outcomes:?}"
        );
    }
}

/// **The person takes `program` over while the agent's wait runs in the engine**, and what the
/// wait answered. A wait the window counts in flight may not have reached the session host yet,
/// and one that reaches it after the take-over is refused because the person drives, not
/// interrupted. Both answers are right, and only the second is the one asked for. So the person
/// hands the browser back and the wait starts again, given longer each time to reach the engine
/// first, until a wait is cut while it runs.
fn taken_over_during_a_wait(window: &Window, tools: &mut Tools, program: &str) -> Said {
    let mut reaching = Duration::ZERO;
    loop {
        let waiting = tools.start(
            "browser_wait_for_text",
            json!({ "text": "words this page never shows", "seconds": 60 }),
        );
        until("the wait to be in flight", || window.acting.in_flight() > 0);
        std::thread::sleep(reaching);
        window
            .browsers
            .take_over(program)
            .expect("the person takes it over");
        let answered = tools.answered("browser_wait_for_text", waiting);
        let came_after = answered
            .text
            .contains(purlis_host::A_BROWSER_THE_PERSON_DRIVES);
        if !came_after || reaching >= Duration::from_secs(5) {
            return answered;
        }
        window
            .browsers
            .hand_back(program)
            .expect("they hand it back to try again");
        reaching += Duration::from_millis(250);
    }
}

/// The acceptance lines of #2294 on a real engine: a wait in flight answers interrupted as the
/// person takes the browser over; every call of the agent's is refused while they drive; they
/// type into the fixture site's login form and its page that echoes, and hand it back; a
/// reference dealt before is refused after; no later snapshot, find or screenshot holds what
/// they typed, the form's "Show password" pressed or not; and input aimed at a document the
/// page has left is dropped.
#[test]
fn the_person_takes_a_real_browser_over_types_and_the_agent_never_sees_it() {
    if !a_real_engine_runs_here() {
        return;
    }
    let site = Site::serve();
    let window = Window::holding(&site, &[&site]);
    let (chat, mut tools) = window.a_chat();
    tools.call("browser_snapshot", json!({}));
    crate::chatbrowsers::allowed(&window.held, chat, false).expect("the person allows it");
    tools.done(
        "browser_open",
        json!({ "address": site.at("login-stays.html") }),
    );
    let program = window.program_of(chat);
    // The agent clicks a plain text field, and the person then takes over and types.
    let name_field = ref_on(
        &tools
            .done("browser_find", json!({ "text": "User name" }))
            .text,
        "textbox \"User name\"",
    );
    tools.done("browser_click", json!({ "target": name_field }));
    let watching = Watching::of(&window, &program);
    let form = watching.aim_other_than(None);

    // A wait in flight answers interrupted as the person takes over.
    let cut = taken_over_during_a_wait(&window, &mut tools, &program);
    assert!(cut.text.contains("it was interrupted"), "{cut:?}");

    // While they drive, every call of the agent's is refused in words.
    let refused = tools.call("browser_snapshot", json!({}));
    assert!(refused.error, "{refused:?}");
    assert!(
        refused
            .text
            .contains(purlis_host::A_BROWSER_THE_PERSON_DRIVES),
        "{}",
        refused.text
    );

    // They type their name where the agent clicked, move on with Tab, type a password.
    types(&window, &program, &form, THEIR_NAME);
    press(&window, &program, &form, "Tab", "Tab", 9);
    types(&window, &program, &form, THEIR_PASSWORD);
    window
        .browsers
        .hand_back(&program)
        .expect("they hand it back");

    // A reference dealt before the take-over is refused.
    let stale = tools.call("browser_click", json!({ "target": name_field }));
    assert!(stale.error, "{stale:?}");
    assert!(
        stale
            .text
            .contains("was dealt before the browser last changed hands"),
        "{}",
        stale.text
    );
    // What they typed reached the page, and no read the agent gets holds it, the password
    // shown by the page's toggle or not.
    let read = tools.done("browser_snapshot", json!({}));
    assert!(
        read.text.contains(&format!(
            "the password field holds {} characters",
            THEIR_PASSWORD.len()
        )),
        "their keys reached the page: {}",
        read.text
    );
    let form_shot = tools.done("browser_screenshot", json!({}));
    assert_eq!(form_shot.images.len(), 1, "{form_shot:?}");
    tools.call("browser_find", json!({ "text": "Password" }));
    let show = ref_on(
        &tools
            .done("browser_find", json!({ "text": "Show password" }))
            .text,
        "button \"Show password\"",
    );
    tools.done("browser_click", json!({ "target": show, "changes": true }));
    tools.done("browser_snapshot", json!({}));
    tools.call("browser_find", json!({ "text": "User name" }));
    let shown_shot = tools.done("browser_screenshot", json!({}));
    assert_eq!(shown_shot.images.len(), 1, "{shown_shot:?}");

    // The page that echoes what is typed.
    tools.done(
        "browser_navigate",
        json!({ "address": site.at("echo-hidden.html") }),
    );
    let echo_field = ref_on(
        &tools
            .done("browser_find", json!({ "text": "Say something" }))
            .text,
        "textbox \"Say something\"",
    );
    tools.done("browser_click", json!({ "target": echo_field }));
    let echo = watching.aim_other_than(Some(&form));
    window
        .browsers
        .take_over(&program)
        .expect("the person takes it over again");
    // Input aimed at the document the page has left is dropped.
    assert_eq!(
        press(&window, &program, &form, "x", "KeyX", 88),
        [
            crate::browsers::InputOutcome::Missed,
            crate::browsers::InputOutcome::Missed
        ],
        "input aimed at the form the page has left"
    );
    types(&window, &program, &echo, THEIR_ECHO);
    window
        .browsers
        .hand_back(&program)
        .expect("they hand it back");
    tools.done("browser_snapshot", json!({}));
    tools.call("browser_find", json!({ "text": "Say something" }));
    let echo_shot = tools.done("browser_screenshot", json!({}));
    assert_eq!(echo_shot.images.len(), 1, "{echo_shot:?}");
    // The page draws its echo of what was typed in the fixture's red: none of it is left to see.
    assert_eq!(
        red_pixels(&echo_shot.images[0]),
        0,
        "what the person typed is painted over wherever the page draws it"
    );

    for said in &tools.told {
        for typed in [THEIR_NAME, THEIR_PASSWORD, THEIR_ECHO] {
            assert!(
                !said.text.contains(typed),
                "what the person typed reached the agent: {}",
                said.text
            );
        }
    }
}
