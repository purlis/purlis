// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Git LFS stage against a real git-lfs** (#4162): [`clone`] itself, with nothing standing
//! in for `git lfs pull`, fetching from an LFS server in this process on a loopback port. The
//! repository's own `.lfsconfig` names the server, and the server asks for the clone's helper's
//! credentials, so what is proved is what a person's clone does.
//!
//! `#[ignore]`d everywhere but the CI job `real git-lfs`, which installs it and runs these by
//! name. Run by hand on a machine with none they say so and pass; with `CI` set they fail, so a
//! job that lost git-lfs is red and never a skip. The stall limit is shortened to seconds: a
//! download a line of git-lfs's meter apart, and one that stops, are the real thing either way.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use super::*;

/// git-lfs is here, and the filter a person's `git lfs install` writes is in git's config.
/// `false`, having said so, where this is not CI and it is not.
fn real_git_lfs() -> bool {
    let probe = tempfile::tempdir().unwrap();
    let version = git::run(probe.path(), &["lfs", "version"], git::READ).ok();
    let filter = git::run(
        probe.path(),
        &["config", "--get-regexp", r"^filter\.lfs\.(process|smudge)$"],
        git::READ,
    )
    .ok();
    let here = version.as_ref().is_some_and(git::Run::ok)
        && filter
            .as_ref()
            .is_some_and(|run| run.ok() && !run.out.trim().is_empty());
    if !here {
        assert!(
            std::env::var_os("CI").is_none(),
            "CI must run these against a real git-lfs: {version:?}, {filter:?}"
        );
        eprintln!("no git-lfs configured here: not run");
    } else if let Some(version) = version {
        eprintln!("real git-lfs: {}", version.out.trim());
    }
    here
}

/// How the server sends a download.
#[derive(Clone, Copy)]
enum Sending {
    AtOnce,
    /// A chunk, then a wait, to the end.
    Slowly {
        gap: Duration,
    },
    /// `chunks` chunks, then nothing, with the connection held open.
    Stops {
        chunks: usize,
    },
    /// Nothing at all: the batch call itself is never answered.
    NoAnswer,
}

const CHUNK: usize = 64 * 1024;

#[derive(Clone)]
struct Object {
    oid: String,
    bytes: Vec<u8>,
}

impl Object {
    fn of(chunks: usize, seed: u8) -> Self {
        let bytes: Vec<u8> = (0..chunks * CHUNK)
            .map(|at| seed ^ u8::try_from(at % 251).unwrap())
            .collect();
        Self {
            oid: crate::extension::hex(&Sha256::digest(&bytes)),
            bytes,
        }
    }

    fn pointer(&self) -> String {
        format!(
            "version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize {}\n",
            self.oid,
            self.bytes.len()
        )
    }
}

#[derive(Default)]
struct Asked {
    batches: AtomicU32,
    signed_batches: AtomicU32,
    downloads: AtomicU32,
    over: AtomicBool,
}

/// An LFS server on a loopback port, for as long as it is held.
struct LfsServer {
    port: u16,
    asked: Arc<Asked>,
}

impl Drop for LfsServer {
    fn drop(&mut self) {
        self.asked.over.store(true, Ordering::SeqCst);
    }
}

impl LfsServer {
    fn serving(objects: Vec<Object>, sending: Sending) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let asked = Arc::new(Asked::default());
        let objects = Arc::new(objects);
        let serving = Arc::clone(&asked);
        std::thread::spawn(move || {
            while !serving.over.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (objects, asked) = (Arc::clone(&objects), Arc::clone(&serving));
                        std::thread::spawn(move || {
                            let _ = answer(stream, port, &objects, &asked, sending);
                        });
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
        });
        Self { port, asked }
    }

    fn batches(&self) -> u32 {
        self.asked.batches.load(Ordering::SeqCst)
    }
}

fn hold_until_over(asked: &Asked) {
    while !asked.over.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// "x:y", the stand-in helper's answer, as HTTP Basic says it.
const SIGNED: &str = "Basic eDp5";

fn answer(
    stream: TcpStream,
    port: u16,
    objects: &[Object],
    asked: &Asked,
    sending: Sending,
) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut stream = stream;
    loop {
        let mut request = String::new();
        if reader.read_line(&mut request)? == 0 {
            return Ok(());
        }
        let mut length = 0usize;
        let mut signed = false;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header)?;
            let header = header.trim();
            if header.is_empty() {
                break;
            }
            let lower = header.to_ascii_lowercase();
            if let Some(value) = lower.strip_prefix("content-length:") {
                length = value.trim().parse().unwrap_or(0);
            }
            signed |= header.split_once(':').is_some_and(|(name, value)| {
                name.eq_ignore_ascii_case("authorization") && value.trim() == SIGNED
            });
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body)?;
        let path = request.split(' ').nth(1).unwrap_or_default().to_owned();
        if path.ends_with("/objects/batch") {
            asked.batches.fetch_add(1, Ordering::SeqCst);
            if !signed {
                let said = br#"{"message":"Credentials needed"}"#;
                write!(
                    stream,
                    "HTTP/1.1 401 Unauthorized\r\nLFS-Authenticate: Basic realm=\"lfs\"\r\n\
                     Content-Type: application/vnd.git-lfs+json\r\nContent-Length: {}\r\n\r\n",
                    said.len()
                )?;
                stream.write_all(said)?;
                continue;
            }
            asked.signed_batches.fetch_add(1, Ordering::SeqCst);
            if matches!(sending, Sending::NoAnswer) {
                hold_until_over(asked);
                return Ok(());
            }
            let rows: Vec<Value> = objects
                .iter()
                .map(|object| {
                    json!({
                        "oid": object.oid,
                        "size": object.bytes.len(),
                        "authenticated": true,
                        "actions": {"download": {
                            "href": format!("http://127.0.0.1:{port}/objects/{}", object.oid),
                        }},
                    })
                })
                .collect();
            let said = json!({"transfer": "basic", "objects": rows}).to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.git-lfs+json\r\n\
                 Content-Length: {}\r\n\r\n{said}",
                said.len()
            )?;
            continue;
        }
        let Some(object) = objects
            .iter()
            .find(|object| path.ends_with(&format!("/objects/{}", object.oid)))
        else {
            stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")?;
            continue;
        };
        asked.downloads.fetch_add(1, Ordering::SeqCst);
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\n\
             Content-Length: {}\r\n\r\n",
            object.bytes.len()
        )?;
        for (at, chunk) in object.bytes.chunks(CHUNK).enumerate() {
            match sending {
                Sending::Stops { chunks } if at == chunks => {
                    stream.flush()?;
                    hold_until_over(asked);
                    return Ok(());
                }
                Sending::Slowly { gap } if at > 0 => std::thread::sleep(gap),
                _ => {}
            }
            stream.write_all(chunk)?;
            stream.flush()?;
        }
    }
}

/// The credential helper the server's sign-in is answered by, as a clone's route names one.
const HELPER: &str = "!f() { test \"$1\" = get && printf 'username=x\\npassword=y\\n'; }; f";

impl World {
    /// [`World::holding`], with `objects` Git LFS files `file0`.. in `name`, and the repo's own
    /// `.lfsconfig` naming `server`.
    fn holding_real_large_files(name: &str, objects: &[&Object], server: &LfsServer) -> Self {
        let lfsconfig = format!(
            "[lfs]\n\turl = http://127.0.0.1:{}/acme/{name}.git/info/lfs\n",
            server.port
        );
        let pointers: Vec<(String, String)> = objects
            .iter()
            .enumerate()
            .map(|(at, object)| (format!("file{at}.bin"), object.pointer()))
            .collect();
        let mut files: Vec<(&str, &str)> = vec![(".lfsconfig", &lfsconfig)];
        files.extend(
            pointers
                .iter()
                .map(|(at, text)| (at.as_str(), text.as_str())),
        );
        Self::holding_seeded(name, &files, "*.bin filter=lfs diff=lfs merge=lfs -text\n")
    }

    /// A clone of `name` through the real stage, as the window runs one, with the stall limit
    /// `stall`, the helper where `signed`, and what it told the window.
    fn real_clone(
        &self,
        name: &str,
        stall: Duration,
        signed: bool,
    ) -> (u8, Vec<Say>, Vec<(Stage, u8)>, Duration) {
        let seen = Mutex::new(Vec::new());
        let heard = |stage: Stage, percent: u8| seen.lock().unwrap().push((stage, percent));
        let off = AtomicBool::new(false);
        let mut watch = self.watching(&heard, &off, stall, None);
        watch.stand_in.helper = signed.then(|| HELPER.to_owned());
        let started = Instant::now();
        let (code, said) = self.clone_as_the_window(&[name], &watch);
        let took = started.elapsed();
        (code, said, seen.into_inner().unwrap(), took)
    }
}

/// What the stall limit is held to here: long enough for a runner under load, short enough that
/// git-lfs's own 30 s activity timeout never answers first.
const STALL: Duration = Duration::from_secs(4);

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_lands_each_file_as_its_content_signed_by_the_clones_helper() {
    if !real_git_lfs() {
        return;
    }
    let (first, second) = (Object::of(16, 1), Object::of(4, 2));
    let server = LfsServer::serving(vec![first.clone(), second.clone()], Sending::AtOnce);
    let world = World::holding_real_large_files("widget", &[&first, &second], &server);

    let (code, said, seen, _) = world.real_clone("widget", Duration::from_secs(60), true);

    assert_eq!(code, 0, "{said:?}");
    let dest = world.dest("widget");
    assert!(
        std::fs::read(dest.join("file0.bin")).unwrap() == first.bytes,
        "file0 is not its content"
    );
    assert!(
        std::fs::read(dest.join("file1.bin")).unwrap() == second.bytes,
        "file1 is not its content"
    );
    assert!(
        server.asked.signed_batches.load(Ordering::SeqCst) >= 1,
        "never signed"
    );
    assert_eq!(server.asked.downloads.load(Ordering::SeqCst), 2);
    assert!(!world.hidden("widget").exists());
    // Nothing reads as changed: the files are what git expects of a checkout.
    // In the person's own git, with nothing of purlis's between.
    let plain = |args: &[&str]| {
        let mut command = std::process::Command::new("git");
        command.args(args).current_dir(&dest);
        let out = crate::forklock::output(&mut command).unwrap();
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    };
    let status = plain(&["status", "--porcelain"]);
    assert!(status.trim().is_empty(), "{status}");
    // And in purlis's own, which its dirty checks ask.
    let ours = git::run(&dest, &["status", "--porcelain"], git::READ).unwrap();
    if !ours.out.trim().is_empty() {
        eprintln!(
            ".git/config: {}",
            std::fs::read_to_string(dest.join(".git/config")).unwrap()
        );
        eprintln!("listed: {}", plain(&["config", "--list", "--show-scope"]));
        for args in [
            &["-c", "core.hooksPath=/dev/null", "status", "--porcelain"][..],
            &["-c", "core.fsmonitor=false", "status", "--porcelain"][..],
            &["-c", "diff.ignoreSubmodules=dirty", "status", "--porcelain"][..],
            &[
                "-c",
                "status.submoduleSummary=false",
                "status",
                "--porcelain",
            ][..],
            &["-c", "filter.lfs.process=", "status", "--porcelain"][..],
        ] {
            eprintln!("{args:?}: {}", plain(args));
        }
        let held = git::run(
            &dest,
            &["hash-object", "--path=file0.bin", "file0.bin"],
            git::READ,
        );
        eprintln!("hash-object: {held:?}");
        let on = git::run(
            &dest,
            &["-c", "trace.x=1", "var", "GIT_CONFIG_PARAMETERS"],
            git::READ,
        );
        eprintln!("var: {on:?}");
    }
    assert!(ours.ok() && ours.out.trim().is_empty(), "{ours:?}");
    // The percent never goes back, across git's stages and git-lfs's own, and the window says it.
    let percents: Vec<u8> = seen.iter().map(|(_, percent)| *percent).collect();
    assert!(percents.is_sorted(), "{seen:?}");
    assert!(
        seen.iter().any(|(stage, _)| *stage == Stage::LargeFiles),
        "{seen:?}"
    );
    assert_eq!(seen.last(), Some(&(Stage::LargeFiles, 100)), "{seen:?}");
    assert_eq!(Stage::LargeFiles.words(), "Downloading large files");
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_downloading_slowly_past_the_stall_limit_is_not_stopped() {
    if !real_git_lfs() {
        return;
    }
    // 12 s in all, three stall limits, a chunk every half second.
    let object = Object::of(24, 3);
    let server = LfsServer::serving(
        vec![object.clone()],
        Sending::Slowly {
            gap: Duration::from_millis(500),
        },
    );
    let world = World::holding_real_large_files("widget", &[&object], &server);

    let (code, said, seen, took) = world.real_clone("widget", STALL, true);

    assert_eq!(code, 0, "{said:?}");
    assert!(took > STALL * 2, "it never outlasted the limit: {took:?}");
    assert!(std::fs::read(world.dest("widget").join("file0.bin")).unwrap() == object.bytes);
    let percents: Vec<u8> = seen.iter().map(|(_, percent)| *percent).collect();
    assert!(percents.is_sorted(), "{seen:?}");
}

/// A clone of a server that sends as `sending` is stopped at the limit, in LFS's words.
fn stalls(sending: Sending) {
    let object = Object::of(24, 4);
    let server = LfsServer::serving(vec![object.clone()], sending);
    let world = World::holding_real_large_files("widget", &[&object], &server);

    let (code, said, seen, took) = world.real_clone("widget", STALL, true);

    assert_eq!(code, 1, "{said:?}");
    let text = words(&said);
    assert!(
        text.contains("stalled") && text.contains("Git LFS"),
        "{text}"
    );
    // git-lfs's own 30 s timeout did not answer for it.
    assert!(took < Duration::from_secs(25), "{took:?}");
    assert!(!world.dest("widget").exists(), "{said:?}");
    assert!(!world.hidden("widget").exists(), "{said:?}");
    assert!(
        seen.iter().any(|(stage, _)| *stage == Stage::LargeFiles),
        "{seen:?}"
    );
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_whose_download_stops_is_stopped_at_the_stall_limit() {
    if real_git_lfs() {
        stalls(Sending::Stops { chunks: 4 });
    }
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_whose_server_never_answers_is_stopped_at_the_stall_limit() {
    if real_git_lfs() {
        stalls(Sending::NoAnswer);
    }
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_with_no_credentials_for_its_server_fails_and_leaves_nothing() {
    if !real_git_lfs() {
        return;
    }
    let object = Object::of(1, 5);
    let server = LfsServer::serving(vec![object.clone()], Sending::AtOnce);
    let world = World::holding_real_large_files("widget", &[&object], &server);

    let (code, said, _, took) = world.real_clone("widget", Duration::from_secs(60), false);

    assert_eq!(code, 1, "{said:?}");
    assert!(
        took < Duration::from_secs(30),
        "it waited on a prompt: {took:?}"
    );
    assert_eq!(server.asked.downloads.load(Ordering::SeqCst), 0);
    assert!(!world.dest("widget").exists(), "{said:?}");
    assert!(!world.hidden("widget").exists(), "{said:?}");
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn a_repo_with_no_large_files_never_meets_real_git_lfs() {
    if !real_git_lfs() {
        return;
    }
    let server = LfsServer::serving(Vec::new(), Sending::AtOnce);
    let world = World::holding(&["widget"]);

    let (code, said, seen, _) = world.real_clone("widget", Duration::from_secs(60), true);

    assert_eq!(code, 0, "{said:?}");
    assert_eq!(server.batches(), 0);
    assert!(world.dest("widget").join("README.md").is_file());
    assert!(
        !seen.iter().any(|(stage, _)| *stage == Stage::LargeFiles),
        "{seen:?}"
    );
    assert_eq!(
        seen.last().map(|(_, percent)| *percent),
        Some(100),
        "{seen:?}"
    );
}
