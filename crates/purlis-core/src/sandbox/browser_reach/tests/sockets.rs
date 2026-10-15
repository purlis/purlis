// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser's reach proxy over real sockets: a named local port is carried by its literal
//! address, anything else local is refused before a connection is made, and a proxy that is
//! gone carries nothing. These bind sockets, so they first run on CI.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use super::super::{Lane, NewSites, ReachProxy};
use crate::sandbox::reach::{By, Reach};

/// A server on this machine's loopback, and its port.
fn a_server() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
    let port = listener.local_addr().expect("an address").port();
    (listener, port)
}

/// A proxy whose chat's reach lists `port` of this machine as a service on this Mac, by each
/// name a chat's proxy carries it by, telling `new_sites` each new site.
fn a_proxy(port: u16, new_sites: NewSites) -> ReachProxy {
    let reach = Reach::of(
        crate::sandbox::localport::reach_entries(port)
            .into_iter()
            .map(|entry| (entry, By::Chat))
            .collect(),
    );
    ReachProxy::start(Lane::of(reach), new_sites).expect("a reach proxy")
}

fn to(proxy: &ReachProxy) -> TcpStream {
    let stream = TcpStream::connect(proxy.address()).expect("the proxy");
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .expect("a timeout");
    stream
}

/// The status line the proxy answers `request` with, the rest of its head read.
fn status(stream: &mut TcpStream, request: &str) -> String {
    stream.write_all(request.as_bytes()).expect("sent");
    let mut reader = BufReader::new(stream.try_clone().expect("a clone"));
    let mut line = String::new();
    reader.read_line(&mut line).expect("a status line");
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).expect("a header");
        if header == "\r\n" || header.is_empty() {
            break;
        }
    }
    line.trim_end().to_owned()
}

/// Whether anything connected to `listener` in the time a refusal takes.
fn nobody_connected(listener: &TcpListener) -> bool {
    listener.set_nonblocking(true).expect("non-blocking");
    std::thread::sleep(Duration::from_millis(200));
    listener.accept().is_err()
}

#[test]
fn a_named_local_port_is_tunnelled_to_by_its_literal_address() {
    let (listener, port) = a_server();
    let echo = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("the proxy's connection");
        let mut buf = [0u8; 4];
        stream.read_exact(&mut buf).expect("four bytes");
        stream.write_all(&buf).expect("echoed");
    });
    let proxy = a_proxy(port, NewSites::refusing());
    let mut client = to(&proxy);
    assert_eq!(
        status(
            &mut client,
            &format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n")
        ),
        "HTTP/1.1 200 Connection established"
    );
    client.write_all(b"ping").expect("sent");
    let mut back = [0u8; 4];
    client.read_exact(&mut back).expect("echoed");
    assert_eq!(&back, b"ping");
    echo.join().expect("the server");
}

#[test]
fn a_plain_request_to_a_named_local_port_is_carried_by_its_literal_address() {
    let (listener, port) = a_server();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("the proxy's connection");
        let mut reader = BufReader::new(stream.try_clone().expect("a clone"));
        let mut line = String::new();
        reader.read_line(&mut line).expect("a request line");
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
            .expect("answered");
        line
    });
    let proxy = a_proxy(port, NewSites::refusing());
    let mut client = to(&proxy);
    assert_eq!(
        status(
            &mut client,
            &format!("GET http://127.0.0.1:{port}/app HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n")
        ),
        "HTTP/1.1 204 No Content"
    );
    assert_eq!(server.join().expect("the server"), "GET /app HTTP/1.1\r\n");
}

#[test]
fn a_name_for_a_named_local_port_is_refused_and_never_connected_to() {
    let (listener, port) = a_server();
    let proxy = a_proxy(port, NewSites::refusing());
    for host in ["localhost", "this-mac.internal"] {
        let mut client = to(&proxy);
        assert_eq!(
            status(
                &mut client,
                &format!("CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n\r\n")
            ),
            "HTTP/1.1 403 Forbidden",
            "{host}"
        );
    }
    let mut client = to(&proxy);
    assert_eq!(
        status(
            &mut client,
            &format!("GET http://localhost:{port}/ HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n")
        ),
        "HTTP/1.1 403 Forbidden"
    );
    assert!(nobody_connected(&listener));
}

#[test]
fn a_private_a_metadata_and_a_loopback_address_are_refused() {
    let (listener, port) = a_server();
    let (unnamed, other) = a_server();
    let proxy = a_proxy(port, NewSites::refusing());
    for request in [
        "CONNECT 10.0.0.5:443 HTTP/1.1\r\n\r\n".to_owned(),
        "CONNECT 192.168.1.1:443 HTTP/1.1\r\n\r\n".to_owned(),
        "GET http://169.254.169.254/latest/meta-data/ HTTP/1.1\r\nHost: 169.254.169.254\r\n\r\n"
            .to_owned(),
        "CONNECT [fd00:ec2::254]:80 HTTP/1.1\r\n\r\n".to_owned(),
        format!("CONNECT 127.0.0.1:{other} HTTP/1.1\r\n\r\n"),
        format!("CONNECT 127.0.0.2:{port} HTTP/1.1\r\n\r\n"),
        format!("CONNECT [::ffff:127.0.0.1]:{port} HTTP/1.1\r\n\r\n"),
        format!("CONNECT 2130706433:{port} HTTP/1.1\r\n\r\n"),
    ] {
        let mut client = to(&proxy);
        assert_eq!(
            status(&mut client, &request),
            "HTTP/1.1 403 Forbidden",
            "{request}"
        );
    }
    assert!(nobody_connected(&listener));
    assert!(nobody_connected(&unnamed));
}

#[test]
fn a_new_site_is_told_and_refused_by_default() {
    let told = Arc::new(AtomicUsize::new(0));
    let telling = Arc::clone(&told);
    let new_sites = NewSites::new(move |site| {
        assert_eq!(
            (site.host.as_str(), site.port),
            ("new.example.invalid", 443)
        );
        telling.fetch_add(1, Ordering::SeqCst);
        false
    });
    let proxy = a_proxy(5173, new_sites);
    let mut client = to(&proxy);
    assert_eq!(
        status(
            &mut client,
            "CONNECT new.example.invalid:443 HTTP/1.1\r\nHost: new.example.invalid:443\r\n\r\n"
        ),
        "HTTP/1.1 403 Forbidden"
    );
    assert_eq!(told.load(Ordering::SeqCst), 1);
}

#[test]
fn a_request_that_is_not_http_is_refused() {
    let proxy = a_proxy(5173, NewSites::refusing());
    let mut client = to(&proxy);
    assert_eq!(
        status(&mut client, "HELLO\r\n\r\n"),
        "HTTP/1.1 400 Bad Request"
    );
}

#[test]
fn with_the_proxy_gone_nothing_is_carried() {
    let (listener, port) = a_server();
    let proxy = a_proxy(port, NewSites::refusing());
    let address = proxy.address();
    drop(proxy);
    // The engine sends every request to this address and resolves no name of its own, so
    // with nothing listening there every load fails.
    let refused = (0..20).all(|_| {
        let gone = TcpStream::connect_timeout(&address, Duration::from_secs(1)).is_err();
        if !gone {
            std::thread::sleep(Duration::from_millis(50));
        }
        gone
    });
    assert!(refused, "something still listens at the proxy's address");
    assert!(nobody_connected(&listener));
}
