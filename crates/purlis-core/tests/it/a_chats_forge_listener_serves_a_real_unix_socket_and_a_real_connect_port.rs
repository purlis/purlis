// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A2 (ADR 0128 D3a to D3c): the two doors on real sockets. A unix socket in a
//! private folder, closed to a lax one; a loopback port that takes `CONNECT`, terminates a recorded
//! host under the chat's authority, tunnels a download host untouched and refuses everything else.
//!
//! These bind sockets, which a sandbox may not: they run in CI.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::Duration;

use purlis_core::forge::proxy::listener::{Dial, ForgeListener};
use purlis_core::forge::proxy::{EndpointKind, Refusal};
use purlis_core::sandbox::egress::handoff::{self, Secret};
use purlis_core::sandbox::egress::{DoorError, ForgeRoute, open_door};

use crate::support::forge_listener::{Answer, World, basic, client_config, gh_get, parse_reply};

fn short_dir() -> tempfile::TempDir {
    // A socket's path is short on macOS. The folder is made private, as the app's own is: the
    // listener refuses one that is not, and a temporary folder is made with the umask's mode.
    let dir = tempfile::Builder::new()
        .prefix("fp")
        .tempdir_in("/tmp")
        .expect("a folder in /tmp");
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    dir
}

#[test]
fn gh_reaches_the_forge_through_a_unix_socket_that_is_the_apps_alone() {
    purlis_core::unsteered!();
    let w = World::new();
    let dir = short_dir();
    let listener = ForgeListener::serve_socket(w.wiring.clone(), dir.path()).unwrap();
    let EndpointKind::UnixSocket(path) = listener.kind().clone() else {
        panic!("a socket");
    };
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    assert_eq!(std::fs::metadata(&path).unwrap().uid(), nix_euid());
    assert_eq!(
        std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o077,
        0
    );

    w.fake.answer(Answer::ok(r#"{"login":"octocat"}"#));
    let mut client = UnixStream::connect(&path).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    client.write_all(&gh_get(&w, "/user")).unwrap();
    let mut raw = Vec::new();
    let _ = client.read_to_end(&mut raw);
    let reply = parse_reply(&raw);
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.text(), r#"{"login":"octocat"}"#);

    drop(listener);
    assert!(!path.exists(), "the socket goes with the listener");
    assert!(UnixStream::connect(&path).is_err());
}

fn nix_euid() -> u32 {
    purlis_same_user::Uid::effective().as_raw()
}

#[test]
fn a_folder_that_is_open_to_others_or_a_link_is_refused_not_repaired() {
    purlis_core::unsteered!();
    let w = World::new();
    let dir = short_dir();
    let lax = dir.path().join("lax");
    std::fs::create_dir(&lax).unwrap();
    std::fs::set_permissions(&lax, std::fs::Permissions::from_mode(0o755)).unwrap();
    let refused = ForgeListener::serve_socket(w.wiring.clone(), &lax)
        .err()
        .expect("refused");
    assert_eq!(
        refused.kind(),
        std::io::ErrorKind::PermissionDenied,
        "{refused}"
    );
    assert_eq!(
        std::fs::metadata(&lax).unwrap().permissions().mode() & 0o777,
        0o755,
        "not repaired"
    );
    assert_eq!(
        std::fs::read_dir(&lax).unwrap().count(),
        0,
        "no socket was made in it"
    );

    let group = dir.path().join("group");
    std::fs::create_dir(&group).unwrap();
    std::fs::set_permissions(&group, std::fs::Permissions::from_mode(0o770)).unwrap();
    assert!(ForgeListener::serve_socket(w.wiring.clone(), &group).is_err());

    let real = dir.path().join("real");
    std::fs::create_dir(&real).unwrap();
    std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o700)).unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(
        ForgeListener::serve_socket(w.wiring.clone(), &link).is_err(),
        "a link to a folder is not the folder"
    );

    let fine = dir.path().join("fine");
    let listener = ForgeListener::serve_socket(w.wiring.clone(), &fine).unwrap();
    assert_eq!(
        std::fs::metadata(&fine).unwrap().permissions().mode() & 0o777,
        0o700
    );
    drop(listener);
}

/// What the tunnel dials in place of the download host: a local server that echoes.
struct ToEcho(std::net::SocketAddr);

impl Dial for ToEcho {
    fn dial(&self, host: &str, port: u16) -> std::io::Result<TcpStream> {
        assert_eq!((host, port), ("codeload.github.com", 443));
        TcpStream::connect(self.0)
    }
}

/// `line` (a `CONNECT` with its head's last blank line) as the chat's egress proxy sends it: with
/// the proof of `secret` before that line.
fn proven(secret: &Secret, line: &str) -> String {
    let (proof, _) = secret.ask().unwrap();
    let head = line
        .strip_suffix("\r\n")
        .expect("a head ends in a blank line");
    format!("{head}{}: {proof}\r\n\r\n", handoff::HEADER)
}

fn connect(port: std::net::SocketAddr, secret: &Secret, line: &str) -> (TcpStream, String) {
    connect_raw(port, &proven(secret, line))
}

fn connect_raw(port: std::net::SocketAddr, line: &str) -> (TcpStream, String) {
    let mut stream = TcpStream::connect(port).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream.write_all(line.as_bytes()).unwrap();
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).unwrap_or(0) == 0 {
            break;
        }
        head.push(byte[0]);
    }
    (stream, String::from_utf8_lossy(&head).into_owned())
}

#[test]
fn connect_terminates_a_recorded_host_tunnels_a_download_host_and_refuses_the_rest() {
    purlis_core::unsteered!();
    let echo = TcpListener::bind("127.0.0.1:0").unwrap();
    let echo_at = echo.local_addr().unwrap();
    std::thread::spawn(move || {
        while let Ok((mut c, _)) = echo.accept() {
            std::thread::spawn(move || {
                let mut buf = [0u8; 256];
                while let Ok(n) = c.read(&mut buf) {
                    if n == 0 || c.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
            });
        }
    });

    let mut w = World::new();
    w.wiring.dial = Arc::new(ToEcho(echo_at));
    let listener = ForgeListener::serve_connect(w.wiring.clone()).unwrap();
    let EndpointKind::Loopback(port) = listener.kind().clone() else {
        panic!("a port");
    };
    assert!(port.ip().is_loopback());
    let ca_pem = listener
        .ca_certificate_pem()
        .expect("an authority")
        .to_string();
    assert!(!ca_pem.contains("PRIVATE KEY"));

    // A recorded host: terminated under the chat's authority, and the request is the core's.
    w.fake.answer(Answer::ok(r#"{"login":"octocat"}"#));
    let (stream, head) = connect(
        port,
        &w.wiring.handoff,
        "CONNECT api.github.com:443 HTTP/1.1\r\nHost: api.github.com:443\r\n\r\n",
    );
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    let name = rustls::pki_types::ServerName::try_from("api.github.com".to_string()).unwrap();
    let connection = rustls::ClientConnection::new(client_config(&ca_pem), name).unwrap();
    let mut tls = rustls::StreamOwned::new(connection, stream);
    let request = format!(
        "GET /user HTTP/1.1\r\nHost: api.github.com\r\nAuthorization: {}\r\n\r\n",
        basic("x-access-token", w.sentinel.expose())
    );
    tls.write_all(request.as_bytes()).unwrap();
    let mut raw = Vec::new();
    let _ = tls.read_to_end(&mut raw);
    let reply = parse_reply(&raw);
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(w.fake.requests()[0].url, "https://api.github.com/user");

    // A download host: a tunnel, never terminated. The bytes are the client's own, both ways.
    let (mut tunnel, head) = connect(
        port,
        &w.wiring.handoff,
        "CONNECT codeload.github.com:443 HTTP/1.1\r\nHost: codeload.github.com:443\r\n\r\n",
    );
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    tunnel.write_all(b"not TLS at all").unwrap();
    let mut echoed = [0u8; 14];
    tunnel.read_exact(&mut echoed).unwrap();
    assert_eq!(&echoed, b"not TLS at all");

    // Everything else: refused, and the chat's audit says so.
    for line in [
        "CONNECT evil.example:443 HTTP/1.1\r\n\r\n",
        "CONNECT github.com:8443 HTTP/1.1\r\n\r\n",
        "CONNECT codeload.github.com:80 HTTP/1.1\r\n\r\n",
        "CONNECT 127.0.0.1:443 HTTP/1.1\r\n\r\n",
        "CONNECT gitlab.com:443 HTTP/1.1\r\n\r\n",
        "CONNECT [::1]:443 HTTP/1.1\r\n\r\n",
        "CONNECT github.com HTTP/1.1\r\n\r\n",
    ] {
        let (_, head) = connect(port, &w.wiring.handoff, line);
        let status = head.split(' ').nth(1).unwrap_or_default();
        if line.starts_with("CONNECT gitlab.com") {
            // The chat has a repo there: a recorded host is terminated.
            assert_eq!(status, "200", "{line}: {head}");
        } else {
            assert!(status == "403" || status == "400", "{line}: {head}");
        }
    }
    let refused = w
        .proxy
        .audit(&w.chat)
        .into_iter()
        .filter(|r| {
            matches!(
                r.verdict,
                purlis_core::forge::proxy::Verdict::Refused(Refusal::HostNotAllowed)
            )
        })
        .count();
    assert!(refused >= 3, "{refused}");

    // A request that is not a CONNECT is not served on this port.
    let mut plain = TcpStream::connect(port).unwrap();
    plain
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    plain
        .write_all(b"GET http://github.com/ HTTP/1.1\r\nHost: github.com\r\n\r\n")
        .unwrap();
    let mut raw = Vec::new();
    let _ = plain.read_to_end(&mut raw);
    assert_eq!(parse_reply(&raw).status, 405);

    drop(listener);
    assert!(TcpStream::connect_timeout(&port, Duration::from_millis(500)).is_err());
}

#[test]
fn a_client_that_speaks_before_the_connect_is_answered_is_closed() {
    purlis_core::unsteered!();
    let w = World::new();
    let listener = ForgeListener::serve_connect(w.wiring.clone()).unwrap();
    let EndpointKind::Loopback(port) = listener.kind().clone() else {
        panic!("a port");
    };
    let mut stream = TcpStream::connect(port).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .write_all(
            format!(
                "{}\x16\x03\x01early bytes",
                proven(&w.wiring.handoff, "CONNECT github.com:443 HTTP/1.1\r\n\r\n")
            )
            .as_bytes(),
        )
        .unwrap();
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    assert!(raw.is_empty(), "{:?}", String::from_utf8_lossy(&raw));
}

#[test]
fn the_door_carries_nothing_for_a_connect_that_does_not_prove_the_runs_secret() {
    purlis_core::unsteered!();
    let w = World::new();
    let listener = ForgeListener::serve_connect(w.wiring.clone()).unwrap();
    let EndpointKind::Loopback(port) = listener.kind().clone() else {
        panic!("a port");
    };
    let line = "CONNECT api.github.com:443 HTTP/1.1\r\nHost: api.github.com:443\r\n\r\n";
    let other = Secret::random().unwrap();

    for sent in [line.to_owned(), proven(&other, line)] {
        let (_, head) = connect_raw(port, &sent);
        assert!(head.starts_with("HTTP/1.1 407"), "{head}");
        assert!(
            handoff::header_of(&head, handoff::REPLY_HEADER).is_none(),
            "an unproven connection is given no proof of the door: {head}"
        );
    }
    assert!(
        w.proxy.audit(&w.chat).is_empty(),
        "nothing reached the core's verdict"
    );
}

#[test]
fn every_answer_of_the_door_proves_the_door_to_the_nonce_it_was_asked() {
    purlis_core::unsteered!();
    let w = World::new();
    let listener = ForgeListener::serve_connect(w.wiring.clone()).unwrap();
    let EndpointKind::Loopback(port) = listener.kind().clone() else {
        panic!("a port");
    };
    for line in [
        "CONNECT api.github.com:443 HTTP/1.1\r\nHost: api.github.com:443\r\n\r\n",
        "CONNECT evil.example:443 HTTP/1.1\r\n\r\n",
    ] {
        let (proof, nonce) = w.wiring.handoff.ask().unwrap();
        let head = line.strip_suffix("\r\n").unwrap();
        let (_, answer) = connect_raw(port, &format!("{head}{}: {proof}\r\n\r\n", handoff::HEADER));

        let said = handoff::header_of(&answer, handoff::REPLY_HEADER).expect("a proof");
        assert!(w.wiring.handoff.answers(&nonce, said), "{answer}");
    }
}

#[test]
fn the_proxy_splices_nothing_to_a_port_that_answers_without_the_doors_proof() {
    purlis_core::unsteered!();
    // A port the run gave up, and something else bound: it answers a CONNECT as a door would.
    let impostor = TcpListener::bind("127.0.0.1:0").unwrap();
    let at = impostor.local_addr().unwrap();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let heard = seen.clone();
    std::thread::spawn(move || {
        if let Ok((mut c, _)) = impostor.accept() {
            let mut buf = [0u8; 1024];
            let n = c.read(&mut buf).unwrap_or(0);
            heard.lock().unwrap().extend_from_slice(&buf[..n]);
            let _ = c.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n");
            let _ = c.flush();
            std::thread::sleep(Duration::from_millis(500));
        }
    });
    let secret = Secret::random().unwrap();
    let route = ForgeRoute {
        addr: at,
        handoff: secret.clone(),
        hosts: vec!["github.com".to_owned()],
    };

    let opened = open_door(&route, "github.com", 443, b"", Duration::from_secs(5));

    assert_eq!(opened.err(), Some(DoorError::NotTheDoor));
    let sent = String::from_utf8_lossy(&seen.lock().unwrap()).into_owned();
    assert!(
        !sent.contains(secret.expose()),
        "only a proof was sent, never the secret: {sent}"
    );
}

#[test]
fn the_proxy_opens_the_real_door_with_its_proof_and_gives_the_chat_the_doors_head() {
    purlis_core::unsteered!();
    let w = World::new();
    let listener = ForgeListener::serve_connect(w.wiring.clone()).unwrap();
    let EndpointKind::Loopback(door) = listener.kind().clone() else {
        panic!("a port");
    };
    let route = ForgeRoute {
        addr: door,
        handoff: w.wiring.handoff.clone(),
        hosts: vec!["api.github.com".to_owned()],
    };

    let (_, head) = open_door(&route, "api.github.com", 443, b"", Duration::from_secs(5))
        .expect("the door proves itself");
    let head = String::from_utf8_lossy(&head).into_owned();

    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
}
