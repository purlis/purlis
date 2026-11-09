// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A wrapped chat's ssh is carried by its route from inside its own sandbox** (#5897, the
//! follow-up to #3536): Seatbelt on macOS, purlis's own wrap (`purlis _sandbox`) on Linux.
//!
//! The route's `ProxyCommand` is `<purlis> _ssh-proxy <socks port> %h %p`, so ssh inside the
//! chat's sandbox has to run the `purlis` binary and that binary has to reach the chat's SOCKS
//! port on the loopback interface. `a_chats_ssh_reaches_a_listed_host_through_its_route` runs
//! ssh through the route outside any sandbox. This test runs it under the chat's real line:
//! an opencode chat's, as [`sandbox::Applied::line`] makes it, with the confinement's proxy, temp
//! folder and route, and the route's folder first on the `PATH` as a start puts it
//! (`hoststart`). The shell inside runs the system's `/usr/bin/ssh` and the `purlis` this build
//! made. The server answers the way an ssh server opens, and ssh's `SSH-2.0-` line reaching it
//! shows the connection was carried.
//!
//! What the route refuses stays refused from inside: a port the proxy does not list, and a
//! direct ssh to the listed port that does not go through the route, which the sandbox
//! refuses. So the route is still the only way out.
//!
//! The proxy here is [`sandbox::Confinement::proxy_only`]'s, serving one exact loopback address
//! and port. A project's hosts never name this machine ([`purlis_core::sandbox::hosts`]), and
//! what the proxy decides is not what is under test here; what the sandbox lets through is.
//!
//! It binds ports and applies a sandbox, which a sandboxed agent cannot do, so it first runs on
//! CI. On Linux the `rust` job runs it. On macOS it needs `sandbox-exec`, so it runs in the
//! `studio-confinement-macos` job (a ready pull request that touches the route, and the merge
//! queue).

#![cfg(any(target_os = "macos", target_os = "linux"))]

use crate::support;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use purlis_core::harness::Harness;
use purlis_core::sandbox::egress::Serving;
use purlis_core::sandbox::reach::Reach;
use purlis_core::sandbox::{self, At, Confinement, Decided, Machine, Os, Words};

/// Set in the child this test re-runs itself in, where `PURLIS_BINARY` names this build's
/// `purlis`: the route's `ProxyCommand` runs the binary [`sandbox::tools::purlis_binary`] finds,
/// and none sits beside a test binary.
const IN_THE_CHILD: &str = "SSH_ROUTE_SANDBOXED_CHILD";

/// This test's libtest path, which the re-run names.
const THIS: &str = "a_chats_ssh_route_carries_from_inside_its_sandbox::a_wrapped_chats_ssh_is_carried_by_its_route_from_inside_its_sandbox_and_nothing_else_is";

/// The system's ssh, which a route's `ssh` runs.
const SSH: &str = "/usr/bin/ssh";

/// What every ssh here is told: ask nobody anything, and trust no host key file.
const QUIET: &str = "-o BatchMode=yes -o StrictHostKeyChecking=no \
                     -o UserKnownHostsFile=/dev/null -o ConnectTimeout=20";

#[cfg(target_os = "macos")]
const OS: Os = Os::MacOs;
#[cfg(target_os = "linux")]
const OS: Os = Os::Linux;

/// A server on the loopback interface that opens as an ssh server does, and tells the test the
/// first line each client sends.
fn an_ssh_server() -> (u16, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
    let port = listener.local_addr().expect("an address").port();
    let (told, heard) = mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let told = told.clone();
            std::thread::spawn(move || {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
                let _ = stream.write_all(b"SSH-2.0-purlis_sandboxed_route_test\r\n");
                let mut line = String::new();
                let _ = BufReader::new(stream).read_line(&mut line);
                let _ = told.send(line);
            });
        }
    });
    (port, heard)
}

/// Whether this process can apply a Seatbelt profile: not where it is already sandboxed.
#[cfg(target_os = "macos")]
fn can_sandbox() -> bool {
    purlis_core::forklock::status(std::process::Command::new("/usr/bin/sandbox-exec").args([
        "-p",
        "(version 1)(allow default)",
        "/usr/bin/true",
    ]))
    .is_ok_and(|status| status.success())
}

/// Whether this kernel has the Landlock purlis's Linux sandbox needs. Never a skip: on Linux
/// the sandbox is the product, so a kernel without it fails the test, saying so.
#[cfg(target_os = "linux")]
fn can_sandbox() -> bool {
    assert!(
        purlis_linux_sandbox::abi() >= purlis_linux_sandbox::MIN_ABI,
        "this kernel's Landlock is version {}, and purlis's Linux sandbox needs {} (Linux 6.12 or \
         later)",
        purlis_linux_sandbox::abi(),
        purlis_linux_sandbox::MIN_ABI
    );
    true
}

/// A chat's sandbox: an opencode chat's, decided for a project that runs every chat sandboxed.
fn a_wrapped_chat(out: &Path) -> (sandbox::Applied, PathBuf) {
    let root = out.join("project");
    let home = out.join("home");
    let bin = out.join("bin");
    for dir in [&root, &home, &bin] {
        std::fs::create_dir_all(dir).expect("a folder");
    }
    std::fs::write(root.join("purlis.toml"), "[sandbox]\nmode = \"on\"\n").expect("the manifest");
    let machine = Machine {
        env: purlis_core::secrets::Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(home),
        os: OS,
    };
    let decided = sandbox::decide(Harness::Opencode, &root, &machine, &|_| true, None, None)
        .expect("decided")
        .expect("a sandbox in force");
    let Decided::Sandboxed(applied) = decided else {
        panic!("not sandboxed: {decided:?}");
    };
    (applied, root)
}

/// `script`, run by `/bin/sh` as the chat's own program, under the chat's whole line: its
/// sandbox, its environment, and its route first on its `PATH`.
fn in_the_chat(
    applied: &sandbox::Applied,
    confinement: &Confinement,
    cwd: &Path,
    script: &str,
) -> std::process::Output {
    let purlis = Path::new(support::PURLIS)
        .canonicalize()
        .expect("purlis's real path");
    let line = applied
        .line(
            Words {
                program: "/bin/sh".to_owned(),
                command: vec!["-c".to_owned(), script.to_owned()],
                armed: Vec::new(),
                charters: Vec::new(),
            },
            &At {
                cwd: Some(cwd),
                confinement: Some(confinement),
                purlis: Some(&purlis),
                ..At::default()
            },
        )
        .expect("the chat's line");
    let route = applied
        .ssh_route(Some(confinement))
        .expect("the chat's ssh route");
    support::command(&line.program)
        .args(&line.args)
        .current_dir(cwd)
        .envs(line.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .env("PATH", route.on_path("/usr/bin:/bin"))
        .output()
        .expect("the chat's line ran")
}

/// What `ran` said, for a failed assertion.
fn said(ran: &std::process::Output) -> String {
    format!(
        "{:?}\nstdout: {}\nstderr: {}",
        ran.status,
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    )
}

#[test]
fn a_wrapped_chats_ssh_is_carried_by_its_route_from_inside_its_sandbox_and_nothing_else_is() {
    purlis_core::unsteered!();
    if std::env::var_os(IN_THE_CHILD).is_none() {
        purlis_core::testrun::rerun(
            &[THIS],
            &[
                (IN_THE_CHILD, std::ffi::OsStr::new("1")),
                ("PURLIS_BINARY", std::ffi::OsStr::new(support::PURLIS)),
            ],
        );
        return;
    }
    if !can_sandbox() {
        // On CI nothing sandboxes the test, so a skip there is a failure.
        assert!(
            std::env::var_os("CI").is_none(),
            "this process cannot apply a Seatbelt profile on CI"
        );
        let _ = writeln!(
            std::io::stderr(),
            "skipped: this process is already sandboxed, so no second profile applies"
        );
        return;
    }
    assert!(
        Path::new(SSH).is_file(),
        "{SSH} is missing: install the OpenSSH client (openssh-client, openssh-clients)"
    );
    let (listed, heard) = an_ssh_server();
    let (unlisted, heard_unlisted) = an_ssh_server();
    let scratch = tempfile::tempdir().expect("a folder");
    let out = scratch.path().canonicalize().expect("a real folder");
    let (applied, root) = a_wrapped_chat(&out);
    let confinement = Confinement::proxy_only(Serving::of(Reach::open(vec![format!(
        "127.0.0.1:{listed}"
    )])))
    .expect("the chat's proxy");
    let route = applied
        .ssh_route(Some(&confinement))
        .expect("the chat's ssh route");
    let config = std::fs::read_to_string(route.config()).expect("its config");
    assert!(
        config.contains(&format!("_ssh-proxy {} %h %p", confinement.socks_port())),
        "the route runs purlis as its proxy:\n{config}"
    );

    // A bare `ssh` in the chat is the route's, and reaches the listed host through it.
    let carried = in_the_chat(
        &applied,
        &confinement,
        &root,
        &format!("command -v ssh; ssh {QUIET} -p {listed} 127.0.0.1 true"),
    );
    let line = heard
        .recv_timeout(Duration::from_secs(10))
        .unwrap_or_default();
    assert!(
        line.starts_with("SSH-2.0-"),
        "ssh's identification reached the host through the route from inside the sandbox: \
         {line:?}\n{}",
        said(&carried)
    );
    assert!(
        String::from_utf8_lossy(&carried.stdout)
            .lines()
            .next()
            .is_some_and(|ssh| Path::new(ssh) == route.bin().join("ssh")),
        "the chat's ssh is its route's: {}",
        said(&carried)
    );

    // A port the route does not list: refused by the proxy, and the chat is told why.
    let refused = in_the_chat(
        &applied,
        &confinement,
        &root,
        &format!("ssh {QUIET} -p {unlisted} 127.0.0.1 true"),
    );
    assert!(
        !refused.status.success(),
        "ssh connected: {}",
        said(&refused)
    );
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("may not reach"),
        "the chat is told why: {}",
        said(&refused)
    );

    // The listed port, straight from the chat and not through the route: the sandbox refuses
    // the connection, so the route stays the only way out.
    let direct = in_the_chat(
        &applied,
        &confinement,
        &root,
        &format!("{SSH} -F /dev/null {QUIET} -p {listed} 127.0.0.1 true"),
    );
    assert!(
        !direct.status.success(),
        "ssh connected around the route: {}",
        said(&direct)
    );
    assert!(
        heard.recv_timeout(Duration::from_secs(1)).is_err(),
        "a connection around the route reached the host: {}",
        said(&direct)
    );
    assert!(
        heard_unlisted.recv_timeout(Duration::from_secs(1)).is_err(),
        "the unlisted host was connected to"
    );
}
