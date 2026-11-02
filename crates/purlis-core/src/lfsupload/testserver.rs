// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! An LFS server that takes uploads, on a loopback port, for the tests against a real git-lfs
//! (#4399). It asks for the stand-in helper's credentials, and each object it stores says whether
//! the ref a push would move had moved by then.
//!
//! **And what every real git-lfs test shares** (#4516): whether git-lfs is here at all, a Git LFS
//! object, and a machine of a test's own ([`on_a_machine`]) with the person's own helper in its
//! `HOME` and a fake `gh` first on its `PATH`, for a test that asks what the product itself finds
//! there rather than what a stand-in was handed.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::worktree::git;

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
    let mut command = std::process::Command::new("git");
    command
        .args(["rev-parse", "--verify", "--quiet", &watching.reference])
        .current_dir(&watching.forge);
    crate::forklock::output(&mut command).is_ok_and(|out| out.status.success())
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

/// git-lfs is here, and the filter a person's `git lfs install` writes is in git's config.
/// `false`, having said so, where this is not CI and it is not.
pub(crate) fn real_git_lfs() -> bool {
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
        let _ = writeln!(std::io::stderr(), "no git-lfs configured here: not run");
    } else if let Some(version) = version {
        let _ = writeln!(std::io::stderr(), "real git-lfs: {}", version.out.trim());
    }
    here
}

/// How much of a Git LFS object [`Object::of`] counts in.
pub(crate) const CHUNK: usize = 64 * 1024;

/// A Git LFS object: what a pointer names, and the bytes it stands for.
#[derive(Clone)]
pub(crate) struct Object {
    pub oid: String,
    pub bytes: Vec<u8>,
}

impl Object {
    pub(crate) fn of(chunks: usize, seed: u8) -> Self {
        let bytes: Vec<u8> = (0..chunks * CHUNK)
            .map(|at| seed ^ u8::try_from(at % 251).unwrap())
            .collect();
        Self {
            oid: crate::extension::hex(&Sha256::digest(&bytes)),
            bytes,
        }
    }

    pub(crate) fn pointer(&self) -> String {
        format!(
            "version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize {}\n",
            self.oid,
            self.bytes.len()
        )
    }
}

/// What tells a child of the test binary that it runs on a [`ThisMachine`], and where that is.
const ON_A_MACHINE: &str = "REAL_LFS_MACHINE";

/// **A machine of a test's own**, whose `HOME` and `PATH` the product really reads: `HOME`'s git
/// config has the filter `git lfs install` writes and a helper of the person's own, and the first
/// `gh` on `PATH` is a fake, which writes down each time it ran and answers `x:y`, the login the
/// upload server wants. A variable a test sets needs `unsafe`, so these are a child's
/// ([`on_a_machine`]).
pub(crate) struct ThisMachine {
    top: PathBuf,
}

impl ThisMachine {
    fn made_at(top: &Path) -> Self {
        let machine = Self {
            top: top.to_path_buf(),
        };
        std::fs::create_dir_all(machine.home()).unwrap();
        std::fs::create_dir_all(machine.bin()).unwrap();
        let persons = format!(
            "!f() {{ touch '{}'; test \"$1\" = get && printf 'username=x\\npassword=y\\n'; }}; f",
            machine.persons_helper_mark().display()
        );
        let config = machine.home().join(".gitconfig");
        for (key, value) in [
            ("credential.helper", persons.as_str()),
            ("filter.lfs.clean", "git-lfs clean -- %f"),
            ("filter.lfs.smudge", "git-lfs smudge -- %f"),
            ("filter.lfs.process", "git-lfs filter-process"),
            ("filter.lfs.required", "true"),
            ("user.name", "Person"),
            ("user.email", "person@example.invalid"),
            ("commit.gpgsign", "false"),
        ] {
            let out = crate::forklock::output(
                std::process::Command::new("git")
                    .arg("config")
                    .arg("--file")
                    .arg(&config)
                    .args([key, value]),
            )
            .unwrap();
            assert!(out.status.success(), "{key}: {out:?}");
        }
        stand_in::program(
            &machine.bin(),
            "gh",
            &format!(
                "#!/bin/sh\n\
                 asked=''\n\
                 while IFS= read -r line && [ -n \"$line\" ]; do\n\
                 case \"$line\" in host=*) asked=\"$line\" ;; esac\n\
                 done\n\
                 printf '%s %s\\n' \"$*\" \"$asked\" >> '{}'\n\
                 test \"$3\" = get && printf 'username=x\\npassword=y\\n'\n\
                 exit 0\n",
                machine.gh_record().display()
            ),
        );
        stand_in::program(
            &machine.bin(),
            "purlis-app",
            &format!(
                "#!/bin/sh\n\
                 cat > /dev/null\n\
                 touch '{}'\n\
                 case \"$*\" in *get) printf 'username=x\\npassword=y\\n' ;; esac\n\
                 exit 0\n",
                machine.window_helper_mark().display()
            ),
        );
        machine
    }

    fn home(&self) -> PathBuf {
        self.top.join("home")
    }

    fn bin(&self) -> PathBuf {
        self.top.join("bin")
    }

    fn gh_record(&self) -> PathBuf {
        self.top.join("gh-ran")
    }

    fn persons_helper_mark(&self) -> PathBuf {
        self.top.join("the-persons-helper-ran")
    }

    fn window_helper_mark(&self) -> PathBuf {
        self.top.join("the-windows-helper-ran")
    }

    /// Each time the fake `gh` ran: its arguments, and the host it was asked about.
    pub(crate) fn gh_ran(&self) -> Vec<String> {
        std::fs::read_to_string(self.gh_record())
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// Whether any git asked the helper in the person's own config.
    pub(crate) fn persons_helper_ran(&self) -> bool {
        self.persons_helper_mark().exists()
    }

    /// The program that stands for the app, whose own helper purlis names for a git the window
    /// runs ([`crate::forge::gitcredential::as_the_window`]).
    pub(crate) fn window_exe(&self) -> PathBuf {
        self.bin().join("purlis-app")
    }

    /// Whether any git asked the window's own helper.
    pub(crate) fn window_helper_ran(&self) -> bool {
        self.window_helper_mark().exists()
    }

    /// The fake `gh` was asked for the upload's login on the server's host, and nothing else
    /// was: `gh` is the one credential, and the helper in the person's config is not.
    pub(crate) fn only_gh_signed(&self, server: &Uploads, said: &str) {
        assert!(server.signed() >= 1, "never signed: {said}");
        let host = format!("host=127.0.0.1:{}", server.port);
        let ran = self.gh_ran();
        assert!(
            ran.iter()
                .any(|one| one.starts_with("auth git-credential get") && one.ends_with(&host)),
            "the forge's CLI was never asked for {host}: {ran:?}: {said}"
        );
        assert!(!self.persons_helper_ran(), "{said}");
    }
}

/// A test's full path, from `module_path!()` and its own name.
pub(crate) fn named(module: &str, test: &str) -> String {
    let (_, within) = module.split_once("::").unwrap_or(("", module));
    format!("{within}::{test}")
}

/// `then`, on a [`ThisMachine`]: `test`, this `#[ignore]`d test's own full path ([`named`]), run
/// again in a child of the test binary whose `HOME` and `PATH` are that machine's. Where git-lfs
/// is not here it says so and passes; in CI that fails ([`real_git_lfs`]).
pub(crate) fn on_a_machine(test: &str, then: impl FnOnce(&ThisMachine)) {
    if let Some(top) = std::env::var_os(ON_A_MACHINE) {
        let machine = ThisMachine {
            top: PathBuf::from(top),
        };
        assert!(real_git_lfs(), "the machine has no filter");
        then(&machine);
        return;
    }
    if !real_git_lfs() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let top = std::fs::canonicalize(dir.path()).unwrap();
    let machine = ThisMachine::made_at(&top);
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(
        std::iter::once(machine.bin()).chain(std::env::split_paths(&inherited)),
    )
    .unwrap();
    let home = machine.home();
    let config = home.join(".config");
    crate::testrun::rerun_ignored(
        &[test],
        &[
            (ON_A_MACHINE, top.as_os_str()),
            ("HOME", home.as_os_str()),
            ("XDG_CONFIG_HOME", config.as_os_str()),
            ("PATH", path.as_os_str()),
        ],
    );
}

/// A file that a program started only by `.git/config` leaves, for a test to look for.
pub(crate) fn marker_in(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("the-{name}-ran"))
}
