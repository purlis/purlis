// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! An LFS server that takes uploads, on a loopback port, for the tests against a real git-lfs
//! (#4399). It asks for the stand-in helper's credentials, and each object it stores says whether
//! the ref a push would move had moved by then.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

/// What a credential helper answers here, by [`HELPER`], as HTTP Basic says it.
const SIGNED: &str = "Basic eDp5";

/// The stand-in for the forge's helper.
pub(crate) const HELPER: &str =
    "!f() { test \"$1\" = get && printf 'username=x\\npassword=y\\n'; }; f";

/// How the server takes an upload.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Taking {
    Everything,
    /// It answers each object of a batch with a 403 of its own.
    Nothing,
    /// It answers the batch, then reads nothing and says nothing.
    GoesQuiet,
}

/// The ref whose state each upload records: a bare repository's, and the ref in it.
#[derive(Clone)]
pub(crate) struct Watching {
    pub forge: PathBuf,
    pub reference: String,
}

fn has_ref(watching: &Watching) -> bool {
    std::process::Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", &watching.reference])
        .current_dir(&watching.forge)
        .output()
        .is_ok_and(|out| out.status.success())
}

#[derive(Default)]
struct Held {
    requests: AtomicU32,
    signed: AtomicU32,
    over: AtomicBool,
    /// Each object stored, with whether the watched ref was there as it arrived.
    stored: Mutex<HashMap<String, (Vec<u8>, bool)>>,
}

/// The server, for as long as it is held.
pub(crate) struct Uploads {
    port: u16,
    held: Arc<Held>,
}

impl Drop for Uploads {
    fn drop(&mut self) {
        self.held.over.store(true, Ordering::SeqCst);
    }
}

impl Uploads {
    pub(crate) fn serving(taking: Taking, watching: Option<Watching>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let held = Arc::new(Held::default());
        let serving = Arc::clone(&held);
        std::thread::spawn(move || {
            while !serving.over.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (held, watching) = (Arc::clone(&serving), watching.clone());
                        std::thread::spawn(move || {
                            let _ = answer(stream, port, &held, taking, watching.as_ref());
                        });
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
        });
        Self { port, held }
    }

    pub(crate) fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}/acme/widget.git/info/lfs", self.port)
    }

    pub(crate) fn requests(&self) -> u32 {
        self.held.requests.load(Ordering::SeqCst)
    }

    pub(crate) fn signed(&self) -> u32 {
        self.held.signed.load(Ordering::SeqCst)
    }

    /// The bytes it holds of `oid`, and whether the watched ref was there as they arrived.
    pub(crate) fn holds(&self, oid: &str) -> Option<(Vec<u8>, bool)> {
        self.held.stored.lock().unwrap().get(oid).cloned()
    }
}

fn answer(
    stream: TcpStream,
    port: u16,
    held: &Held,
    taking: Taking,
    watching: Option<&Watching>,
) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut stream = stream;
    loop {
        let mut request = String::new();
        if reader.read_line(&mut request)? == 0 {
            return Ok(());
        }
        held.requests.fetch_add(1, Ordering::SeqCst);
        let (mut length, mut signed) = (0usize, false);
        loop {
            let mut header = String::new();
            reader.read_line(&mut header)?;
            let header = header.trim();
            if header.is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':') {
                if name.eq_ignore_ascii_case("content-length") {
                    length = value.trim().parse().unwrap_or(0);
                }
                signed |= name.eq_ignore_ascii_case("authorization") && value.trim() == SIGNED;
            }
        }
        let path = request.split(' ').nth(1).unwrap_or_default().to_owned();
        if taking == Taking::GoesQuiet && request.starts_with("PUT") {
            while !held.over.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(20));
            }
            return Ok(());
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body)?;
        if path.ends_with("/objects/batch") {
            if !signed {
                write_json(
                    &mut stream,
                    "401 Unauthorized\r\nLFS-Authenticate: Basic realm=\"lfs\"",
                    &json!({"message": "Credentials needed"}),
                )?;
                continue;
            }
            held.signed.fetch_add(1, Ordering::SeqCst);
            let asked: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            let empty = Vec::new();
            let rows: Vec<Value> = asked["objects"]
                .as_array()
                .unwrap_or(&empty)
                .iter()
                .map(|object| {
                    let oid = object["oid"].as_str().unwrap_or_default();
                    let mut row =
                        json!({"oid": oid, "size": object["size"], "authenticated": true});
                    if taking == Taking::Nothing {
                        row["error"] = json!({"code": 403, "message": "not allowed"});
                    } else if !held.stored.lock().unwrap().contains_key(oid) {
                        row["actions"] = json!({"upload": {
                            "href": format!("http://127.0.0.1:{port}/objects/{oid}"),
                        }});
                    }
                    row
                })
                .collect();
            write_json(
                &mut stream,
                "200 OK",
                &json!({"transfer": "basic", "objects": rows}),
            )?;
            continue;
        }
        if request.starts_with("PUT")
            && let Some(oid) = path.rsplit('/').next()
        {
            let moved = watching.is_some_and(has_ref);
            held.stored
                .lock()
                .unwrap()
                .insert(oid.to_owned(), (body, moved));
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")?;
            continue;
        }
        stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")?;
    }
}

fn write_json(stream: &mut TcpStream, status: &str, said: &Value) -> std::io::Result<()> {
    let said = said.to_string();
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/vnd.git-lfs+json\r\n\
         Content-Length: {}\r\n\r\n{said}",
        said.len()
    )
}

/// A file that a program started only by `.git/config` leaves, for a test to look for.
pub(crate) fn marker_in(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("the-{name}-ran"))
}
