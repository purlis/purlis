// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The fixture site, served to a real engine as its reach proxy would carry it, and the real
//! engine to drive it with.
//!
//! The engine sends every request to its reach proxy, so the site is served by a small proxy
//! of the test's own on a loopback port: it answers `GET http://<host>/<path>` from
//! `tests/fixtures/site/<host>/<path>`, whatever the host, and reaches nothing else. Some
//! addresses are its own: `/slow-data` answers after a while (late-loading data);
//! `/redirect-to-file` and `/redirect-to-app` redirect to a `file:` address and to an app's own
//! scheme; and the download page's (#3747): `/export`, the report as an attachment under a name
//! of the header's, and `/big-export` and `/big-stream`, [`BIG`] bytes sent slowly, with their
//! size stated up front and not. `/verify-code` is a site's 2FA verifier (#5118): it holds
//! [`TWO_FACTOR_SEED`] on its server, as a site does, and accepts the code it makes now, a step
//! either side. `/stays-open`, with anything after it, answers only after [`STAYS_OPEN`]: a
//! request still open when the call that set it off answers (#5310). `/cached-once` is a page a
//! session earlier in the browser left in its HTTP cache (#5272): the first answer is its settings
//! page, cached for long, and any later one the site's form, which is what a load that passes the
//! cache gets.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use purlis_browser_engine::drive::{Browser, Judge, Limits};
use purlis_browser_engine::launch::{Launch, ReachProxy};
use purlis_browser_engine::machine::machine_chrome;
use purlis_browser_engine::process::{Engine, Keeper};

/// The seed `/verify-code` checks codes against: RFC 6238's SHA-1 test seed. The site's alone;
/// no page of it holds the seed.
pub const TWO_FACTOR_SEED: &[u8] = b"12345678901234567890";

/// The six-digit TOTP code [`TWO_FACTOR_SEED`] makes at `unix` (RFC 6238: SHA-1, 30 seconds),
/// worked out here as a site's verifier does, apart from purlis's own.
pub fn two_factor_code_at(unix: u64) -> String {
    use hmac::{KeyInit, Mac};
    let mut mac = hmac::Hmac::<sha1::Sha1>::new_from_slice(TWO_FACTOR_SEED).unwrap();
    mac.update(&(unix / 30).to_be_bytes());
    let hash = mac.finalize().into_bytes();
    let at = usize::from(hash[19] & 0x0f);
    let bits =
        u32::from_be_bytes([hash[at], hash[at + 1], hash[at + 2], hash[at + 3]]) & 0x7fff_ffff;
    format!("{:06}", bits % 1_000_000)
}

/// Seconds since 1970, by this machine's clock.
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// How long `/slow-data` takes to answer.
pub const SLOW: Duration = Duration::from_millis(1500);

/// How long `/stays-open` takes to answer: longer than a test that reads its page waits.
pub const STAYS_OPEN: Duration = Duration::from_secs(30);

/// The name `/export`'s `Content-Disposition` gives the report, which its address does not.
pub const EXPORTED_AS: &str = "q3-report.csv";

/// How many bytes `/big-export` and `/big-stream` send, a piece at a time.
pub const BIG: usize = 4 * 1024 * 1024;

/// One piece of [`BIG`], and how long the site waits after each: the whole takes some 6
/// seconds, so a download purlis cancels is cancelled while it still downloads.
const PIECE: usize = 64 * 1024;
const BETWEEN_PIECES: Duration = Duration::from_millis(100);

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
    /// Every request it was sent, as `METHOD host/path`, in order (#5152).
    sent: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    /// The hosts it carries alone, as a reach proxy held by a fill does (#5230): empty, all of them.
    held_to: Held,
}

/// The hosts the fixture site carries alone, where it is held (#5230).
pub type Held = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

impl Site {
    /// Whether any request reached it for `host`.
    pub fn heard_from(&self, host: &str) -> bool {
        self.heard.lock().unwrap().iter().any(|one| one == host)
    }

    /// What holds it to some hosts, as a fill's holder holds a reach proxy (#5230): a request for
    /// any other is hung up on, unheard, and shows the engine's own page, as a refused tunnel does.
    pub fn held_to(&self) -> Held {
        std::sync::Arc::clone(&self.held_to)
    }

    /// How many requests reached it for `host` (#5230).
    pub fn sent_count(&self, host: &str) -> usize {
        self.heard
            .lock()
            .unwrap()
            .iter()
            .filter(|one| *one == host)
            .count()
    }

    /// Whether it was sent `request`, written `METHOD host/path` (#5152).
    pub fn was_sent(&self, request: &str) -> bool {
        self.sent.lock().unwrap().iter().any(|one| one == request)
    }

    /// Every request it was sent for `host`, as `METHOD host/path`, in order: what a failed
    /// test says reached a site.
    pub fn sent_to(&self, host: &str) -> Vec<String> {
        let prefix = format!(" {host}/");
        self.sent
            .lock()
            .unwrap()
            .iter()
            .filter(|one| one.contains(&prefix))
            .cloned()
            .collect()
    }
}

/// Serves the fixture site on a loopback port of its own.
pub fn serve() -> Site {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/site");
    let heard = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sent = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let held_to: Held = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let hearing = std::sync::Arc::clone(&heard);
    let sending = std::sync::Arc::clone(&sent);
    let holding = std::sync::Arc::clone(&held_to);
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let root = root.clone();
            let hearing = std::sync::Arc::clone(&hearing);
            let sending = std::sync::Arc::clone(&sending);
            let holding = std::sync::Arc::clone(&holding);
            thread::spawn(move || answer(stream, &root, &hearing, &sending, &holding));
        }
    });
    Site {
        address,
        heard,
        sent,
        held_to,
    }
}

fn answer(
    stream: TcpStream,
    root: &Path,
    heard: &std::sync::Mutex<Vec<String>>,
    sent: &std::sync::Mutex<Vec<String>>,
    held_to: &std::sync::Mutex<Vec<String>>,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut first = String::new();
    if reader.read_line(&mut first).is_err() {
        return;
    }
    let mut length = 0;
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) | Err(_) => break,
            Ok(_) if header == "\r\n" || header == "\n" => break,
            Ok(_) => {
                if let Some((name, value)) = header.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse().unwrap_or(0);
                }
            }
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
    // Held to other hosts (#5230): hung up on, and never heard.
    {
        let held = held_to.lock().unwrap();
        if !held.is_empty() && !held.iter().any(|one| one == host) {
            return;
        }
    }
    heard.lock().unwrap().push(host.to_owned());
    if let Some(rest) = target.strip_prefix("http://") {
        let without_query = rest.split(['?', '#']).next().unwrap_or_default();
        sent.lock()
            .unwrap()
            .push(format!("{method} {without_query}"));
    }
    let mut out = stream;
    // The sign-in host's forms post (#5152): their body is read past, and the page is served.
    let posts_a_form = method == "POST" && host == "auth.fixture.test";
    if posts_a_form {
        let mut body = vec![0; length];
        let _ = std::io::Read::read_exact(&mut reader, &mut body);
    }
    if method != "GET" && !posts_a_form {
        let _ = respond(&mut out, "405 Method Not Allowed", "text/plain", &[], b"");
        return;
    }
    // A proxy is sent the whole address: http://host/path.
    let Some(rest) = target.strip_prefix("http://") else {
        let _ = respond(&mut out, "400 Bad Request", "text/plain", &[], b"");
        return;
    };
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let query = path.split_once('?').map_or("", |(_, query)| query);
    let path = path.split(['?', '#']).next().unwrap_or("");
    if path == "stays-open" || path.starts_with("stays-open/") {
        thread::sleep(STAYS_OPEN);
        let _ = respond(&mut out, "200 OK", "application/json", &[], b"{}");
        return;
    }
    match path {
        "cached-once" => {
            let first = !sent
                .lock()
                .unwrap()
                .iter()
                .rev()
                .skip(1)
                .any(|one| one.ends_with("/cached-once"));
            let _ = if first {
                let body = b"<!doctype html><title>Settings</title><h1>Settings</h1>\
                    <form onsubmit=\"event.preventDefault();this.remove();\
                    document.querySelector('h1').textContent='Kept for the earlier session'\">\
                    <label>Token <input type=password id=password></label></form>";
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                     Cache-Control: private, max-age=600\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                out.write_all(head.as_bytes())
                    .and_then(|()| out.write_all(body))
                    .and_then(|()| out.flush())
            } else {
                let body = b"<!doctype html><title>A site</title><h1>Sign in</h1>\
                    <form action=account.html method=get>\
                    <label>Password <input type=password name=password></label></form>";
                respond(&mut out, "200 OK", "text/html", &[], body)
            };
        }
        // A site's 2FA verifier (#5118): the code the seed makes now, a step either side.
        "verify-code" => {
            let code = query
                .split('&')
                .find_map(|pair| pair.strip_prefix("code="))
                .unwrap_or_default();
            let now = unix_now();
            let accepted = [now.saturating_sub(30), now, now + 30]
                .iter()
                .any(|at| two_factor_code_at(*at) == code);
            let said: &[u8] = if accepted {
                b"<!doctype html><title>Verified</title><h1>Code accepted</h1>"
            } else {
                b"<!doctype html><title>Not verified</title><h1>Code refused</h1>"
            };
            let _ = respond(&mut out, "200 OK", "text/html; charset=utf-8", &[], said);
        }
        // A server that hangs up, which shows the engine's own page (#5230).
        "hangs-up" => {}
        // A server's redirect to the address the query names, as a sign-in's is (#5230).
        "redirect" => {
            // Written %23 where a form's action would take it for that address's own fragment.
            let to = query
                .strip_prefix("to=")
                .unwrap_or_default()
                .replace("%23", "#");
            let _ = respond(
                &mut out,
                "302 Found",
                "text/plain",
                &[("Location", &to)],
                b"",
            );
        }
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
        // The report, as a site sends a file to save: an attachment, named by the header.
        "export" => {
            let body = std::fs::read(root.join(host).join("report.csv")).unwrap_or_default();
            let named = format!("attachment; filename=\"{EXPORTED_AS}\"");
            let _ = respond(
                &mut out,
                "200 OK",
                "text/csv",
                &[("Content-Disposition", named.as_str())],
                &body,
            );
        }
        "big-export" => {
            let _ = slowly(&mut out, Some(BIG), "big.bin");
        }
        "big-stream" => {
            let _ = slowly(&mut out, None, "big-stream.bin");
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

/// [`BIG`] bytes as an attachment named `name`, a [`PIECE`] at a time, its length stated up front
/// where `stated` is, else read until the connection closes. Ends as soon as the engine stops
/// reading, as a cancelled download does.
fn slowly(out: &mut TcpStream, stated: Option<usize>, name: &str) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Disposition: \
         attachment; filename=\"{name}\"\r\nCache-Control: no-store\r\nConnection: close\r\n"
    );
    if let Some(length) = stated {
        head.push_str(&format!("Content-Length: {length}\r\n"));
    }
    head.push_str("\r\n");
    out.write_all(head.as_bytes())?;
    let piece = vec![b'x'; PIECE];
    for _ in 0..BIG / PIECE {
        out.write_all(&piece)?;
        out.flush()?;
        thread::sleep(BETWEEN_PIECES);
    }
    Ok(())
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
