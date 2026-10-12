// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A private repository cloned and pushed to by the git the window runs, against a forge
//! stand-in over HTTP that demands credentials, with nothing but purlis's helper to give them.
//!
//! The stand-in is git's own `http-backend` behind a few lines of HTTP/1.1 that answer `401`
//! to any request without the account's login and token. It listens on this machine's loopback,
//! which a test build alone lets the helper answer over plain HTTP ([`super::super::secure`]).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use base64::Engine as _;

use super::*;
use crate::testgit;

/// The forge stand-in: a bare repository under `root`, served at `http://<host>/private.git`.
struct Forge {
    _root: tempfile::TempDir,
    bare: PathBuf,
    host: String,
    /// Every request's `Authorization` header, or `None` for a request without one.
    seen: Arc<Mutex<Vec<Option<String>>>>,
}

impl Forge {
    /// A stand-in holding one commit on `main`, that takes `login` with `token` and nothing else.
    fn holding_one_commit(login: &str, token: &str) -> Forge {
        let root = tempfile::tempdir().unwrap();
        let seed = root.path().join("seed");
        std::fs::create_dir(&seed).unwrap();
        let ok = |run: git::Run| assert!(run.ok(), "{run:?}");
        ok(testgit::run(
            &seed,
            &["-c", "init.defaultBranch=main", "init"],
        ));
        ok(testgit::run(
            &seed,
            &[
                "-c",
                "user.name=Seed",
                "-c",
                "user.email=seed@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-m",
                "first",
            ],
        ));
        ok(testgit::run(
            root.path(),
            &["clone", "--bare", "--", "seed", "private.git"],
        ));
        let bare = root.path().join("private.git");

        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let host = listener.local_addr().unwrap().to_string();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let wanted = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{login}:{token}"))
        );
        let (served, at, records) = (root.path().to_path_buf(), login.to_string(), seen.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                answer(stream, &served, &at, &wanted, &records);
            }
        });
        Forge {
            _root: root,
            bare,
            host,
            seen,
        }
    }

    fn url(&self) -> String {
        format!("http://{}/private.git", self.host)
    }

    fn subject_on_main(&self) -> String {
        let run = testgit::run(
            self.bare.parent().unwrap(),
            &[
                "--git-dir",
                &self.bare.display().to_string(),
                "log",
                "-1",
                "--format=%s",
                "main",
            ],
        );
        run.out.trim().to_string()
    }
}

/// One request: `401` without the wanted credentials, else `git http-backend`'s answer.
fn answer(
    stream: TcpStream,
    root: &Path,
    login: &str,
    wanted: &str,
    seen: &Mutex<Vec<Option<String>>>,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut first = String::new();
    if reader.read_line(&mut first).is_err() || first.trim().is_empty() {
        return;
    }
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();
    let mut headers: Vec<(String, String)> = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        }
    }
    let header = |name: &str| {
        headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    };
    let body = if header("transfer-encoding").is_some_and(|v| v.contains("chunked")) {
        unchunked(&mut reader)
    } else {
        let length: usize = header("content-length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        body
    };
    let authorization = header("authorization");
    seen.lock().unwrap().push(authorization.clone());
    let mut stream = stream;
    if authorization.as_deref() != Some(wanted) {
        let _ = stream.write_all(
            b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"stand-in\"\r\n\
              Content-Length: 0\r\nConnection: close\r\n\r\n",
        );
        return;
    }
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let mut backend = Command::new("git");
    backend
        .arg("http-backend")
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin")
        .env("HOME", root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_PROJECT_ROOT", root)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("REQUEST_METHOD", &method)
        .env("PATH_INFO", path)
        .env("QUERY_STRING", query)
        .env("CONTENT_LENGTH", body.len().to_string())
        .env("REMOTE_USER", login)
        .env("REMOTE_ADDR", "127.0.0.1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(kind) = header("content-type") {
        backend.env("CONTENT_TYPE", kind);
    }
    if let Some(encoding) = header("content-encoding") {
        backend.env("HTTP_CONTENT_ENCODING", encoding);
    }
    let mut child = crate::forklock::spawn(&mut backend).expect("git http-backend runs");
    let mut input = child.stdin.take().unwrap();
    let feeding = std::thread::spawn(move || {
        let _ = input.write_all(&body);
    });
    let out = child.wait_with_output().unwrap();
    let _ = feeding.join();
    let cgi = out.stdout;
    let split = cgi
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|at| (at, 4))
        .or_else(|| cgi.windows(2).position(|w| w == b"\n\n").map(|at| (at, 2)));
    let (head, rest) = match split {
        Some((at, gap)) => (
            String::from_utf8_lossy(&cgi[..at]).into_owned(),
            &cgi[at + gap..],
        ),
        None => (String::new(), &cgi[..]),
    };
    let mut status = "200 OK".to_string();
    let mut reply = String::new();
    for line in head.lines() {
        match line.split_once(':') {
            Some((name, value)) if name.eq_ignore_ascii_case("status") => {
                status = value.trim().to_string();
            }
            Some(_) => {
                reply.push_str(line.trim_end());
                reply.push_str("\r\n");
            }
            None => {}
        }
    }
    let _ = stream.write_all(
        format!(
            "HTTP/1.1 {status}\r\n{reply}Content-Length: {}\r\nConnection: close\r\n\r\n",
            rest.len()
        )
        .as_bytes(),
    );
    let _ = stream.write_all(rest);
    let _ = stream.flush();
}

/// A chunked request body, put back together.
fn unchunked(reader: &mut impl BufRead) -> Vec<u8> {
    let mut body = Vec::new();
    loop {
        let mut size = String::new();
        reader.read_line(&mut size).unwrap();
        let size =
            usize::from_str_radix(size.trim().split(';').next().unwrap_or("0"), 16).unwrap_or(0);
        if size == 0 {
            let mut end = String::new();
            let _ = reader.read_line(&mut end);
            return body;
        }
        let mut chunk = vec![0; size + 2];
        reader.read_exact(&mut chunk).unwrap();
        body.extend_from_slice(&chunk[..size]);
    }
}

#[test]
fn a_private_repo_is_cloned_and_pushed_to_with_purlis_sign_in_and_no_forge_cli() {
    let forge = Forge::holding_one_commit("octocat", TOKEN);
    let machine = Machine::signed_in_on(&forge.host);
    let helper = Helper::new("");
    let on = crate::forge::Forge {
        kind: Kind::GitHub,
        host: forge.host.clone(),
    };
    let work = tempfile::tempdir().unwrap();
    let clone = work.path().join("private");
    let url = forge.url();
    // The stand-in speaks plain HTTP on the loopback, which the network rule refuses: the test
    // allows it on its own command line, after the rule.
    let http = "protocol.http.allow=always";

    let (cloned, named) = as_the_window(
        &HostScope::for_a_test(),
        &helper.exe,
        machine.config(),
        || {
            let named = helper_for(&on);
            let cloned = git::run_network(
                work.path(),
                Some(&named),
                &["-c", http, "clone", "--", &url, "private"],
            )
            .expect("git runs");
            (cloned, named)
        },
    );
    assert!(cloned.ok(), "the clone: {cloned:?}");
    assert!(
        !named.contains("auth git-credential"),
        "no forge CLI is asked: {named}"
    );

    let committed = testgit::run(
        &clone,
        &[
            "-c",
            "user.name=Person",
            "-c",
            "user.email=person@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-m",
            "from the window",
        ],
    );
    assert!(committed.ok(), "{committed:?}");
    let pushed = as_the_window(
        &HostScope::for_a_test(),
        &helper.exe,
        machine.config(),
        || {
            git::run_network(
                &clone,
                Some(&helper_for(&on)),
                &["-c", http, "push", &url, "HEAD:refs/heads/main"],
            )
            .expect("git runs")
        },
    );
    assert!(pushed.ok(), "the push: {pushed:?}");
    assert_eq!(forge.subject_on_main(), "from the window");

    let seen = forge.seen.lock().unwrap().clone();
    assert_eq!(
        seen.first(),
        Some(&None),
        "git asked without a credential first"
    );
    assert!(
        seen.iter().flatten().count() >= 2,
        "the clone and the push each sent the account's: {seen:?}"
    );
}
