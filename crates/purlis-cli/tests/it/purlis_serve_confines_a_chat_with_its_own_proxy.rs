// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis serve`, the runner image's session host, starts a chat's confinement with its own
//! proxy (#2217): the window's plan, handed over the session protocol, is started there with the
//! code the app's own host runs, and the proxy refuses a host the plan does not list.
//!
//! It binds a socket and the proxy's ports, which a sandboxed agent cannot: it first runs on CI.
//! Linux only, as `purlis_serve_is_this_users_one_session_host` is.

#![cfg(target_os = "linux")]

use crate::support;

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Stdio};
use std::time::Duration;

use purlis_session_protocol::auth::{Credential, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{
    self, Answer, Beside, Client, Command, Confine, ConfinePlan, Layer, Listed,
};

const PROJECT: &str = "01J9PRJCT00000000000000000";

/// Ends the host this test started, by its pid, however the test ends.
struct Ends(Child);

impl Drop for Ends {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `purlis serve` with its config, data and runtime homes in `dir`, once it says it is ready.
fn serving(dir: &Path) -> Ends {
    for folder in ["run", "config", "data"] {
        std::fs::create_dir_all(dir.join(folder)).unwrap();
    }
    std::fs::set_permissions(dir.join("run"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut serve = support::purlis();
    serve
        .arg("serve")
        .env("PURLIS_CONFIG_HOME", dir.join("config"))
        // Its event log and purlis's git hooks, which it writes as it starts (#2447).
        .env("PURLIS_DATA_HOME", dir.join("data"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("XDG_RUNTIME_DIR", dir.join("run"))
        .env("TMPDIR", dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut host = Ends(serve.spawn().expect("purlis serve starts"));
    let mut line = String::new();
    BufReader::new(host.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    if line.trim_end() != "ready" {
        let _ = host.0.kill();
        let mut said = String::new();
        let _ = host.0.stderr.take().unwrap().read_to_string(&mut said);
        panic!("the host did not say it was ready: {line:?}\n{said}");
    }
    host
}

/// The status line the proxy on `port` answers a tunnel to `target` with.
fn tunnel_through(port: u16, target: &str) -> String {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("the proxy");
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    stream
        .write_all(format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n").as_bytes())
        .unwrap();
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).unwrap();
    line.trim_end().to_owned()
}

#[test]
fn purlis_serve_starts_a_chats_confinement_with_its_own_proxy() {
    let dir = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let _host = serving(dir.path());
    // Where the host of this config home listens (#2114).
    let socket = purlis_core::hostsocket::here_for(
        Some(dir.path().join("run").as_os_str()),
        Some(&dir.path().join("config")),
    );
    let credential = Credential::read(
        &purlis_core::machine::charterd(&dir.path().join("config")),
        Scope::LocalUi,
    )
    .expect("the window's credential");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        let stream = tokio::net::UnixStream::connect(&socket)
            .await
            .expect("the host's socket");
        let link = link::connect(stream, session::speaks(), Scope::LocalUi, &credential)
            .await
            .expect("admitted as the window");
        let (client, _views, _events) = Client::new(link);
        client
            .hold_project(PROJECT, project.path().to_str().unwrap())
            .await
            .expect("held");

        let confined = client
            .call(Command::Confine(Box::new(Confine {
                confinement: "01J9CNFNMNT0000000000000000".into(),
                project: PROJECT.into(),
                plan: ConfinePlan {
                    listed: vec![Listed {
                        host: "api.anthropic.com".into(),
                        layer: Layer::Open,
                    }],
                    never: Vec::new(),
                    withdrawn: Vec::new(),
                    anything: false,
                    tunnel_ports: vec![443],
                    plain_ports: vec![80],
                    any_port: false,
                    beside: Beside::Wrapped,
                    given: Vec::new(),
                },
                asks: false,
            })))
            .await;

        let confined = match confined.expect("purlis serve confines it") {
            Answer::Confined(confined) => confined,
            other => panic!("answered {other:?}"),
        };
        assert_ne!(confined.http, 0, "its proxy listens");
        // Tried while the link that made it is open: the confinement ends with it.
        let port = confined.http;
        let refused =
            tokio::task::spawn_blocking(move || tunnel_through(port, "refused.invalid:443"))
                .await
                .unwrap();
        assert_eq!(refused, "HTTP/1.1 403 Forbidden");
    });
}
