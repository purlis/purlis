// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis serve` runs this user's session host (#1997): it says it is ready once its socket
//! under the runtime directory is this user's alone and its scopes' credentials are minted into
//! the machine store, and a second one finds it serving and leaves it be.
//!
//! It binds a socket, which a sandboxed agent cannot: it first runs on CI. Linux only: on macOS
//! the socket is in this user's own temporary directory, which a test cannot move aside, and a
//! window running there would already hold it.

#![cfg(target_os = "linux")]

use crate::support;

use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Stdio};

/// `purlis serve` with its config, data and runtime homes in `dir`.
fn serve(dir: &Path) -> std::process::Command {
    let mut serve = support::purlis();
    serve
        .arg("serve")
        .env("PURLIS_CONFIG_HOME", dir.join("config"))
        // Its event log and purlis's git hooks, which it writes as it starts (#2447).
        .env("PURLIS_DATA_HOME", dir.join("data"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("XDG_RUNTIME_DIR", dir.join("run"))
        // A temporary directory the process inherits does not move the socket. It is the
        // test's own folder, which the build's fence already holds the run to.
        .env("TMPDIR", dir)
        .stdin(Stdio::null());
    serve
}

/// Ends the host this test started, by its pid, however the test ends.
struct Ends(Child);

impl Drop for Ends {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn purlis_serve_listens_under_the_runtime_directory_and_a_second_one_leaves_it_serving() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("run")).unwrap();
    std::fs::set_permissions(
        dir.path().join("run"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("config")).unwrap();

    let mut first = Ends(
        serve(dir.path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("purlis serve starts"),
    );
    let mut line = String::new();
    BufReader::new(first.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    if line.trim_end() != "ready" {
        let _ = first.0.kill();
        let mut said = String::new();
        let _ = std::io::Read::read_to_string(&mut first.0.stderr.take().unwrap(), &mut said);
        panic!("the host did not say it was ready: {line:?}\n{said}");
    }

    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    // Its config home is one of the test's own, so its socket has a name of its own (#2114).
    let socket = purlis_core::hostsocket::here_for(
        Some(dir.path().join("run").as_os_str()),
        Some(&dir.path().join("config")),
    );
    assert_eq!(
        socket.parent(),
        Some(dir.path().join("run").join("purlis").as_path())
    );
    assert_ne!(
        socket.file_name(),
        Some(std::ffi::OsStr::new("purlisd.sock"))
    );
    assert_eq!(mode(&socket), 0o600, "{}", socket.display());
    assert_eq!(mode(socket.parent().unwrap()), 0o700);
    let mut beside: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    beside.sort();
    assert_eq!(
        beside,
        // The homes the test pinned there, and nothing else.
        ["config", "data", "run"],
        "nothing went in the inherited temporary directory"
    );
    let credentials = purlis_core::machine::charterd(&dir.path().join("config"));
    let minted = std::fs::read_to_string(credentials.join("local-ui")).unwrap();
    assert_eq!(mode(&credentials.join("local-ui")), 0o600);

    let second = serve(dir.path())
        .output()
        .expect("a second purlis serve runs");
    assert_eq!(second.status.code(), Some(3), "{second:?}");
    assert!(
        String::from_utf8_lossy(&second.stderr).contains("already serves this user"),
        "{second:?}"
    );
    assert_eq!(
        std::fs::read_to_string(credentials.join("local-ui")).unwrap(),
        minted,
        "the second host left the first one's credentials alone"
    );
    assert!(
        matches!(first.0.try_wait(), Ok(None)),
        "the first host still serves"
    );
}
