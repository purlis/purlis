// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The fixture site, served to a real engine as its reach proxy would carry it, and the real
//! engine to drive it with.
//!
//! The engine sends every request to its reach proxy, so the site is served by a small proxy
//! of the test's own on a loopback port: it answers `GET http://<host>/<path>` from
//! `tests/fixtures/site/<host>/<path>`, whatever the host, and reaches nothing else. Three
//! addresses are its own: `/slow-data` answers after a while (late-loading data), and
//! `/redirect-to-file` and `/redirect-to-app` redirect to a `file:` address and to an app's own
//! scheme.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use purlis_browser_engine::drive::{Browser, Judge, Limits};
use purlis_browser_engine::launch::{Launch, ReachProxy};
use purlis_browser_engine::machine::machine_chrome;
use purlis_browser_engine::process::{Engine, Keeper};

/// How long `/slow-data` takes to answer.
pub const SLOW: Duration = Duration::from_millis(1500);

/// The engine to run, or why tests of a real engine do not run here.
pub fn engine_program() -> Option<PathBuf> {
    if let Some(named) = std::env::var_os("PURLIS_TEST_ENGINE") {
        return Some(PathBuf::from(named));
    }
    if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true") {
        let found = machine_chrome();
        assert!(
            found.is_some(),
            "CI runs these tests against the runner's Chrome, and none was found"
        );
        return found;
    }
    eprintln!(
        "not run: no engine here. Set PURLIS_TEST_ENGINE to a Chrome executable to run this \
         (a sandboxed agent cannot start one; CI runs it against the runner's Chrome)."
    );
    None
}

/// On CI, a failing test also says why as an annotation of its job (a workflow command on its
/// output), so the reason is read beside the run without its log.
fn say_failures_on_ci() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    if std::env::var("GITHUB_ACTIONS").as_deref() != Ok("true") {
        return;
    }
    ONCE.call_once(|| {
        let before = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let test = std::thread::current().name().unwrap_or("a test").to_owned();
            let mut said: String = info.to_string().chars().take(6000).collect();
            said = said
                .replace('%', "%25")
                .replace('\r', "%0D")
                .replace('\n', "%0A");
            // Written to the process's own output, past the test harness's capture, so a run
            // under `cargo test` is read for it as a run under nextest is.
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "::error title={test}::{said}");
            let _ = out.flush();
            before(info);
        }));
    });
}

/// The fixture site, served until the test ends.
pub struct Site {
    pub address: SocketAddr,
    /// The host of every request it was sent, in order (#2018).
    heard: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl Site {
    /// Whether any request reached it for `host`.
    pub fn heard_from(&self, host: &str) -> bool {
        self.heard.lock().unwrap().iter().any(|one| one == host)
    }
}

/// Serves the fixture site on a loopback port of its own.
pub fn serve() -> Site {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/site");
    let heard = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let hearing = std::sync::Arc::clone(&heard);
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let root = root.clone();
            let hearing = std::sync::Arc::clone(&hearing);
            thread::spawn(move || answer(stream, &root, &hearing));
        }
    });
    Site { address, heard }
}

fn answer(stream: TcpStream, root: &Path, heard: &std::sync::Mutex<Vec<String>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
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
    let mut words = first.split_whitespace();
    let (method, target) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
    // Whatever it was sent, a tunnel's target or a plain request's host.
    let host = target
        .strip_prefix("http://")
        .unwrap_or(target)
        .split(['/', ':'])
        .next()
        .unwrap_or_default();
    heard.lock().unwrap().push(host.to_owned());
    let mut out = stream;
    if method != "GET" {
        let _ = respond(&mut out, "405 Method Not Allowed", "text/plain", &[], b"");
        return;
    }
    // A proxy is sent the whole address: http://host/path.
    let Some(rest) = target.strip_prefix("http://") else {
        let _ = respond(&mut out, "400 Bad Request", "text/plain", &[], b"");
        return;
    };
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let path = path.split(['?', '#']).next().unwrap_or("");
    match path {
        "slow-data" => {
            thread::sleep(SLOW);
            let _ = respond(
                &mut out,
                "200 OK",
                "application/json",
                &[],
                br#"{"answer":42}"#,
            );
        }
        // A sign-in that keeps: a cookie that outlives the browser (#2021).
        "sign-in-kept" => {
            let _ = respond(
                &mut out,
                "200 OK",
                "text/html; charset=utf-8",
                &[(
                    "Set-Cookie",
                    "kept=fixture-session; Max-Age=3600; Path=/; SameSite=Lax",
                )],
                b"<!doctype html><title>Signed in</title><h1>Signed in</h1>",
            );
        }
        "redirect-to-file" => {
            let _ = respond(
                &mut out,
                "302 Found",
                "text/plain",
                &[("Location", "file:///etc/hosts")],
                b"",
            );
        }
        "redirect-to-app" => {
            let _ = respond(
                &mut out,
                "302 Found",
                "text/plain",
                &[("Location", "purlis-test-app://open")],
                b"",
            );
        }
        _ => {
            let name = if path.is_empty() { "index.html" } else { path };
            let file = root.join(host).join(name);
            let inside = !name.contains("..") && !host.contains("..") && !host.contains('/');
            match std::fs::read(&file) {
                Ok(body) if inside => {
                    let kind = if name.ends_with(".html") {
                        "text/html; charset=utf-8"
                    } else {
                        "application/octet-stream"
                    };
                    let _ = respond(&mut out, "200 OK", kind, &[], &body);
                }
                _ => {
                    let _ = respond(&mut out, "404 Not Found", "text/plain", &[], b"not here");
                }
            }
        }
    }
}

fn respond(
    out: &mut TcpStream,
    status: &str,
    kind: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    out.write_all(head.as_bytes())?;
    out.write_all(body)?;
    out.flush()
}

/// The fixture site and a real engine whose reach proxy it is, driven as a [`Browser`], or
/// `None` where no engine runs (and then nothing listens). Keep the folder and the site for as
/// long as the browser.
pub fn started() -> Option<(Browser, tempfile::TempDir, Site)> {
    started_with(None)
}

/// [`started`], every request of its pages told to `judge` (#2018).
pub fn started_judged(judge: Judge) -> Option<(Browser, tempfile::TempDir, Site)> {
    started_with(Some(judge))
}

fn started_with(judge: Option<Judge>) -> Option<(Browser, tempfile::TempDir, Site)> {
    let data = tempfile::tempdir().unwrap();
    let (browser, site) = started_on(&data.path().join("browser"), judge)?;
    Some((browser, data, site))
}

/// [`started`], its limits those `limits` makes of the usual ones (#3277).
#[allow(dead_code)]
pub fn started_limited(
    limits: impl FnOnce(Limits) -> Limits,
) -> Option<(Browser, tempfile::TempDir, Site)> {
    let data = tempfile::tempdir().unwrap();
    let (browser, site) = started_on_limited(&data.path().join("browser"), None, limits)?;
    Some((browser, data, site))
}

/// A real engine on the data folder `folder`, as a browser opened from what an earlier one
/// kept there starts (#2021), with the fixture site as its reach proxy, or `None` where no
/// engine runs.
pub fn started_on(folder: &Path, judge: Option<Judge>) -> Option<(Browser, Site)> {
    started_on_limited(folder, judge, |limits| limits)
}

/// [`started_on`], its limits those `limits` makes of the usual ones.
fn started_on_limited(
    folder: &Path,
    judge: Option<Judge>,
    limits: impl FnOnce(Limits) -> Limits,
) -> Option<(Browser, Site)> {
    let program = engine_program()?;
    say_failures_on_ci();
    let site = serve();
    let launch = Launch::new(
        &super::fixture::chosen(&program),
        folder.to_path_buf(),
        ReachProxy(site.address),
    );
    let keeper = Keeper::program(env!("CARGO_BIN_EXE_purlis-engine-keeper"));
    let engine = Engine::start(&launch, &keeper).unwrap_or_else(|why| {
        let log = std::fs::read_to_string(folder.join("engine.log")).unwrap_or_default();
        panic!("{why}\nthe engine's log:\n{log}")
    });
    let limits = limits(Limits {
        settle_within: Duration::from_secs(20),
        ..Limits::default()
    });
    let browser = match judge {
        Some(judge) => Browser::judged(engine, limits, judge),
        None => Browser::new(engine, limits),
    };
    Some((browser.unwrap(), site))
}
