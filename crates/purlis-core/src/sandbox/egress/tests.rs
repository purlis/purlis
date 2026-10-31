// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Charter's egress proxy, driven over real sockets: a listed host is reached, and anything
//! else is refused before a connection to it is made.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use super::{
    Limits, MORE_COUNTED_AT_MOST, Proxy, REFUSALS_KEPT, Refusals, allows, allows_on, reachable,
};
use crate::sandbox::reach::{By, Reach};

/// A reach that lists nothing exactly: no local address is let through.
fn none() -> Reach {
    Reach::default()
}

/// The listed host every carried test request goes to: the address itself, so a name that
/// also resolves to `::1`, where another program on the machine may listen, is never asked.
const LOOPBACK: &str = "127.0.0.1";

/// The test server's own address and port, listed exactly: the one way the local-address
/// check lets this machine be reached (#1664), so the proxy can be driven against a server here.
fn exactly(port: u16) -> String {
    format!("{LOOPBACK}:{port}")
}

/// A server on this machine that answers each connection by echoing what it is sent, and the
/// port it listens on.
fn an_echo_server() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
    let port = listener.local_addr().expect("an address").port();
    (listener, port)
}

/// The connection on `listener` whose first bytes are `first`: the proxy's. A port another
/// test of this binary let go of and asks again (one looking for a port nobody listens on)
/// may land here first, so any other connection is let go.
fn proxys(listener: &TcpListener, first: &[u8]) -> TcpStream {
    loop {
        let (stream, _) = listener.accept().expect("a connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .expect("a timeout");
        let mut seen = vec![0u8; first.len()];
        let mut have = 0;
        while have < first.len() {
            match stream.peek(&mut seen) {
                Ok(0) | Err(_) => break,
                Ok(n) => have = n,
            }
            if have < first.len() {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        if have >= first.len() && seen == first {
            return stream;
        }
    }
}

fn echo_once(listener: TcpListener) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut stream = proxys(&listener, b"ping");
        let mut buf = [0u8; 4];
        stream.read_exact(&mut buf).expect("four bytes");
        stream.write_all(&buf).expect("echoed");
        buf.to_vec()
    })
}

fn to(proxy: &Proxy) -> TcpStream {
    let stream = TcpStream::connect(("127.0.0.1", proxy.port())).expect("the proxy");
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .expect("a timeout");
    stream
}

/// The status line the proxy answers `request` with.
fn status(stream: &mut TcpStream, request: &str) -> String {
    stream.write_all(request.as_bytes()).expect("sent");
    let mut reader = BufReader::new(stream.try_clone().expect("a clone"));
    let mut line = String::new();
    reader.read_line(&mut line).expect("a status line");
    // The rest of the head, so what follows is the tunnel's.
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).expect("a header");
        if header == "\r\n" || header.is_empty() {
            break;
        }
    }
    line.trim_end().to_owned()
}

#[test]
fn a_listed_host_is_tunnelled_to() {
    let (listener, port) = an_echo_server();
    let echoed = echo_once(listener);
    let proxy = Proxy::allowing(vec![exactly(port)], vec![port]).expect("a proxy");
    let mut client = to(&proxy);
    let said = status(
        &mut client,
        &format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
    );
    assert_eq!(said, "HTTP/1.1 200 Connection established");
    client.write_all(b"ping").expect("through the tunnel");
    let mut back = [0u8; 4];
    client.read_exact(&mut back).expect("echoed back");
    assert_eq!(&back, b"ping");
    assert_eq!(echoed.join().expect("the server"), b"ping");
}

#[test]
fn an_unlisted_host_is_refused_and_never_connected_to() {
    let (listener, port) = an_echo_server();
    listener.set_nonblocking(true).expect("nonblocking");
    let proxy = Proxy::allowing(vec!["example.com".to_owned()], vec![port]).expect("a proxy");
    let mut client = to(&proxy);
    let said = status(
        &mut client,
        &format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
    );
    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        listener.accept().is_err(),
        "the proxy connected to a host it refused"
    );
}

#[test]
fn a_listed_host_on_a_port_charter_does_not_carry_is_refused() {
    let (listener, port) = an_echo_server();
    listener.set_nonblocking(true).expect("nonblocking");
    let proxy = Proxy::allowing(vec![LOOPBACK.to_owned()], vec![443]).expect("a proxy");
    let mut client = to(&proxy);
    let said = status(
        &mut client,
        &format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\n\r\n"),
    );
    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    std::thread::sleep(Duration::from_millis(200));
    assert!(listener.accept().is_err());
}

#[test]
fn a_plain_request_to_a_listed_host_is_carried_in_origin_form() {
    let (listener, port) = an_echo_server();
    let upstream = std::thread::spawn(move || {
        let stream = proxys(&listener, b"GET /simple/x");
        let mut reader = BufReader::new(stream.try_clone().expect("a clone"));
        let mut first = String::new();
        reader.read_line(&mut first).expect("a request line");
        // The whole head, so the close that follows is not a reset that loses the answer.
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).expect("a header");
            if header == "\r\n" || header.is_empty() {
                break;
            }
        }
        let mut stream = stream;
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
            .expect("answered");
        first
    });
    let proxy = Proxy::allowing(vec![exactly(port)], vec![port]).expect("a proxy");
    let mut client = to(&proxy);
    let said = status(
        &mut client,
        &format!("GET http://127.0.0.1:{port}/simple/x HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
    );
    assert_eq!(said, "HTTP/1.1 204 No Content");
    assert_eq!(
        upstream.join().expect("the server"),
        "GET /simple/x HTTP/1.1\r\n"
    );
}

#[test]
fn a_plain_request_to_an_unlisted_host_is_refused() {
    let proxy = Proxy::allowing(vec![LOOPBACK.to_owned()], vec![80]).expect("a proxy");
    let mut client = to(&proxy);
    let said = status(
        &mut client,
        "GET http://example.com/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
    );
    assert_eq!(said, "HTTP/1.1 403 Forbidden");
}

#[test]
fn a_request_that_is_not_http_is_refused() {
    let proxy = Proxy::allowing(vec![LOOPBACK.to_owned()], vec![80]).expect("a proxy");
    let mut client = to(&proxy);
    let said = status(&mut client, "\x16\x03\x01 not a proxy request\r\n\r\n");
    assert_eq!(said, "HTTP/1.1 400 Bad Request");
}

#[test]
fn the_production_proxy_tunnels_only_to_https_and_carries_plain_requests_only_to_http() {
    let proxy = Proxy::start(vec![LOOPBACK.to_owned()]).expect("a proxy");
    // Refused before any connection is tried, so nothing need listen on either port.
    let mut tunnel = to(&proxy);
    assert_eq!(
        status(
            &mut tunnel,
            "CONNECT 127.0.0.1:80 HTTP/1.1\r\nHost: 127.0.0.1:80\r\n\r\n"
        ),
        "HTTP/1.1 403 Forbidden"
    );
    let mut plain = to(&proxy);
    assert_eq!(
        status(
            &mut plain,
            "GET http://127.0.0.1:443/ HTTP/1.1\r\nHost: 127.0.0.1:443\r\n\r\n"
        ),
        "HTTP/1.1 403 Forbidden"
    );
}

#[test]
fn a_head_that_two_parsers_could_read_two_ways_is_refused() {
    // Each of these is read one way here and may be read another by the host it would reach:
    // a folded header, two Hosts, no Host, two lengths, a length that is not a number, a space
    // before a colon, a bare LF.
    let proxy = Proxy::allowing(vec![LOOPBACK.to_owned()], vec![80]).expect("a proxy");
    for head in [
        "GET http://127.0.0.1/ HTTP/1.1\r\nHost: 127.0.0.1\r\nX-A: 1\r\n folded\r\n\r\n",
        "GET http://127.0.0.1/ HTTP/1.1\r\nHost: 127.0.0.1\r\nHost: other.test\r\n\r\n",
        "GET http://127.0.0.1/ HTTP/1.1\r\nX-A: 1\r\n\r\n",
        "POST http://127.0.0.1/ HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\n",
        "POST http://127.0.0.1/ HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: +1\r\n\r\n",
        "GET http://127.0.0.1/ HTTP/1.1\r\nHost : 127.0.0.1\r\n\r\n",
        "GET http://127.0.0.1/ HTTP/1.1\nHost: 127.0.0.1\r\n\r\n",
        "GET  http://127.0.0.1/ HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
        "GET http://127.0.0.1/ HTTP/2.0\r\nHost: 127.0.0.1\r\n\r\n",
    ] {
        let mut client = to(&proxy);
        assert_eq!(
            status(&mut client, head),
            "HTTP/1.1 400 Bad Request",
            "{head:?}"
        );
    }
}

#[test]
fn a_head_that_comes_too_slowly_is_given_up_on() {
    let proxy = Proxy::limited(
        vec![LOOPBACK.to_owned()],
        vec![443],
        Limits {
            connections: 8,
            idle: Duration::from_secs(60),
            head: Duration::from_millis(400),
        },
    )
    .expect("a proxy");
    let mut client = to(&proxy);
    let began = std::time::Instant::now();
    // A byte at a time, each well inside a read's own patience.
    for byte in b"CONNECT 127.0.0.1:443 HTTP/1.1\r\n" {
        if client.write_all(&[*byte]).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        if began.elapsed() > Duration::from_secs(3) {
            break;
        }
    }
    let mut said = String::new();
    let _ = BufReader::new(&mut client).read_line(&mut said);
    assert!(!said.starts_with("HTTP/1.1 200"), "{said}");
    assert!(
        began.elapsed() < Duration::from_secs(3),
        "{:?}",
        began.elapsed()
    );
}

/// What is left on a port a proxy listened on.
#[derive(Debug, PartialEq, Eq)]
enum Left {
    /// Nothing listens there: this test could take the port itself.
    Free,
    /// The proxy: it answered a request with a refusal it keeps.
    TheProxy,
    /// Another listener, which took the port once the proxy let it go (#2609).
    Another,
}

/// Whether `port` is free, by binding it: never while any socket still listens there. A moment
/// is allowed for the proxy's accept to see the stop and close its listener.
fn free(port: u16) -> bool {
    let until = std::time::Instant::now() + Duration::from_secs(1);
    loop {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return true;
        }
        if std::time::Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// What is left on `port`, the HTTP port of a proxy keeping `refusals`. Where something still
/// listens, it is asked to tunnel to a host nothing lists: the proxy would keep that refusal in
/// `refusals` before it answered, and any other listener never could.
fn left_on(port: u16, refusals: &Refusals) -> Left {
    if free(port) {
        return Left::Free;
    }
    if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
        let _ = stream.write_all(b"CONNECT left-behind.example:443 HTTP/1.1\r\n\r\n");
        let _ = stream.read(&mut [0u8; 64]);
    }
    if refusals.refused().is_empty() {
        Left::Another
    } else {
        Left::TheProxy
    }
}

/// A proxy is dropped, `after_drop` is given its HTTP port, and both its ports must be left
/// free. A port another listener took first proves nothing either way, so a new proxy is tried,
/// a few times at most; a port the dropped proxy still answers on fails at once.
fn a_dropped_proxy_leaves_its_ports_free(mut after_drop: impl FnMut(u16)) {
    for _ in 0..5 {
        let refusals = Refusals::default();
        let proxy = Proxy::start_keeping(vec![], refusals.clone()).expect("a proxy");
        let (http, socks) = (proxy.port(), proxy.socks_port());
        drop(proxy);
        after_drop(http);
        match left_on(http, &refusals) {
            Left::TheProxy => panic!("a dropped proxy still answers on {http}"),
            Left::Another => continue,
            Left::Free if free(socks) => return,
            Left::Free => continue,
        }
    }
    panic!("no dropped proxy left both its ports free, in five tries");
}

#[test]
fn a_proxy_stops_listening_when_it_is_dropped() {
    a_dropped_proxy_leaves_its_ports_free(|_| {});
}

/// Another listener taking a dropped proxy's port the moment it is free is never the proxy
/// still listening (#2609): the check is not fooled by it, and still passes.
#[test]
fn a_port_another_listener_takes_after_the_drop_is_not_taken_for_the_proxy() {
    let mut taken: Option<TcpListener> = None;
    let mut tries = 0;
    a_dropped_proxy_leaves_its_ports_free(|port| {
        tries += 1;
        if taken.is_some() {
            return;
        }
        let until = std::time::Instant::now() + Duration::from_secs(1);
        while taken.is_none() && std::time::Instant::now() < until {
            taken = TcpListener::bind(("127.0.0.1", port)).ok();
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    assert!(taken.is_some(), "the freed port was never taken");
    assert!(tries >= 2, "the taken port passed as free");
}

/// The check tells a proxy still listening from another listener: what it proves is not empty.
#[test]
fn a_proxy_still_listening_is_told_from_another_listener() {
    let refusals = Refusals::default();
    let proxy = Proxy::start_keeping(vec![], refusals.clone()).expect("a proxy");
    assert_eq!(left_on(proxy.port(), &refusals), Left::TheProxy);
}

#[test]
fn a_host_is_allowed_only_as_listed_or_under_a_listed_wildcard() {
    let listed = [
        "api.openai.com".to_owned(),
        "*.githubusercontent.com".to_owned(),
    ];
    for (host, want) in [
        ("api.openai.com", true),
        ("API.OpenAI.com", true),
        ("api.openai.com.", true),
        ("openai.com", false),
        ("evil-api.openai.com", false),
        ("api.openai.com.evil.test", false),
        ("raw.githubusercontent.com", true),
        ("a.b.githubusercontent.com", true),
        ("githubusercontent.com", false),
        ("evilgithubusercontent.com", false),
        ("raw.githubusercontent.com.evil.test", false),
        ("", false),
        ("127.0.0.1", false),
    ] {
        assert_eq!(allows(&listed, host), want, "{host}");
    }
}

#[test]
fn a_plain_request_whose_host_header_names_another_host_is_refused() {
    // The address is a listed host's, and the `Host` another's: a shared front end would
    // answer for the other one.
    let (listener, port) = an_echo_server();
    listener.set_nonblocking(true).expect("nonblocking");
    let proxy = Proxy::allowing(vec![LOOPBACK.to_owned()], vec![port]).expect("a proxy");
    let mut client = to(&proxy);
    let said = status(
        &mut client,
        &format!("GET http://127.0.0.1:{port}/ HTTP/1.1\r\nHost: unlisted.example\r\n\r\n"),
    );
    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    std::thread::sleep(Duration::from_millis(200));
    assert!(listener.accept().is_err());
}

#[test]
fn a_plain_request_carries_its_body_and_nothing_after_it() {
    let (listener, port) = an_echo_server();
    let upstream = std::thread::spawn(move || {
        let mut stream = proxys(&listener, b"POST /one");
        let mut got = Vec::new();
        let _ = stream.read_to_end(&mut got);
        let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\n\r\n");
        String::from_utf8(got).expect("text")
    });
    let proxy = Proxy::allowing(vec![exactly(port)], vec![port]).expect("a proxy");
    let mut client = to(&proxy);
    client
        .write_all(
            format!(
                "POST http://127.0.0.1:{port}/one HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                 Content-Length: 4\r\n\r\nbodyGET http://127.0.0.1:{port}/two HTTP/1.1\r\n\r\n"
            )
            .as_bytes(),
        )
        .expect("sent");
    let got = upstream.join().expect("the server");
    assert!(got.starts_with("POST /one HTTP/1.1\r\n"), "{got}");
    assert!(got.ends_with("\r\n\r\nbody"), "{got}");
    assert!(!got.contains("/two"), "a second request was carried: {got}");
}

#[test]
fn a_plain_request_with_a_body_of_no_stated_length_is_refused() {
    let proxy = Proxy::allowing(vec![LOOPBACK.to_owned()], vec![80]).expect("a proxy");
    let mut client = to(&proxy);
    let said = status(
        &mut client,
        "POST http://127.0.0.1/ HTTP/1.1\r\nHost: 127.0.0.1\r\nTransfer-Encoding: chunked\r\n\r\n",
    );
    assert_eq!(said, "HTTP/1.1 400 Bad Request");
}

#[test]
fn connections_past_the_limit_are_refused_rather_than_given_a_thread() {
    let proxy = Proxy::limited(
        vec![LOOPBACK.to_owned()],
        vec![443],
        Limits {
            connections: 2,
            idle: Duration::from_secs(60),
            head: Duration::from_secs(30),
        },
    )
    .expect("a proxy");
    // Two that hold their connection open, saying nothing.
    let _held = [to(&proxy), to(&proxy)];
    std::thread::sleep(Duration::from_millis(100));
    let mut third = to(&proxy);
    let mut said = String::new();
    BufReader::new(&mut third)
        .read_line(&mut said)
        .expect("an answer");
    assert_eq!(said.trim_end(), "HTTP/1.1 503 Service Unavailable");
}

#[test]
fn a_tunnel_idle_both_ways_is_closed() {
    let (listener, port) = an_echo_server();
    let _server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("a connection");
        std::thread::sleep(Duration::from_secs(5));
        drop(stream);
    });
    let proxy = Proxy::limited(
        vec![exactly(port)],
        vec![port],
        Limits {
            connections: 8,
            idle: Duration::from_millis(300),
            head: Duration::from_secs(30),
        },
    )
    .expect("a proxy");
    let mut client = to(&proxy);
    let said = status(
        &mut client,
        &format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\n\r\n"),
    );
    assert_eq!(said, "HTTP/1.1 200 Connection established");
    let began = std::time::Instant::now();
    let mut buf = [0u8; 1];
    let n = client.read(&mut buf).unwrap_or(0);
    assert_eq!(n, 0, "the tunnel stayed open");
    assert!(
        began.elapsed() < Duration::from_secs(4),
        "{:?}",
        began.elapsed()
    );
}

/// #1341: a host listed with a port is reached on that port, and only there, through the proxy
/// every wrapped chat reaches the network through: a private cluster's API on 6443, say.
#[test]
fn a_host_listed_with_its_port_is_tunnelled_to_on_that_port_alone() {
    let (listener, port) = an_echo_server();
    let echoed = echo_once(listener);
    // The proxy a chat is given, which carries only HTTPS's and HTTP's ports for a host
    // without one.
    let proxy = Proxy::start(vec![format!("{LOOPBACK}:{port}")]).expect("a proxy");
    let mut client = to(&proxy);
    let said = status(
        &mut client,
        &format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
    );
    assert_eq!(said, "HTTP/1.1 200 Connection established");
    client.write_all(b"ping").expect("through the tunnel");
    let mut back = [0u8; 4];
    client.read_exact(&mut back).expect("echoed back");
    assert_eq!(&back, b"ping");
    assert_eq!(echoed.join().expect("the server"), b"ping");
}

#[test]
fn a_host_listed_without_a_port_is_not_reached_on_another_one() {
    let (listener, port) = an_echo_server();
    listener.set_nonblocking(true).expect("nonblocking");
    let proxy = Proxy::start(vec![LOOPBACK.to_owned()]).expect("a proxy");
    let mut client = to(&proxy);
    let said = status(
        &mut client,
        &format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\n\r\n"),
    );
    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    std::thread::sleep(Duration::from_millis(200));
    assert!(listener.accept().is_err());
}

#[test]
fn a_listed_port_holds_its_host_to_it_and_an_address_matches_as_written() {
    let listed = [
        "10.100.39.145:6443".to_owned(),
        "*.internal.example:8443".to_owned(),
        "[fd00::7]:443".to_owned(),
        "api.example.com".to_owned(),
    ];
    let defaults = [443];
    for (host, port, want) in [
        ("10.100.39.145", 6443, true),
        ("10.100.39.145", 443, false),
        ("10.100.39.14", 6443, false),
        ("a.internal.example", 8443, true),
        ("a.internal.example", 443, false),
        ("internal.example", 8443, false),
        ("fd00::7", 443, true),
        ("fd00:0::7", 443, true),
        ("fd00::7", 8443, false),
        ("api.example.com", 443, true),
        ("api.example.com", 6443, false),
    ] {
        assert_eq!(
            allows_on(&listed, host, port, &defaults),
            want,
            "{host}:{port}"
        );
    }
}

/// Review of #1341, 1: a wildcard never matches an address literal, through the proxy.
#[test]
fn a_wildcard_never_matches_an_address() {
    let listed = ["*.0.0.1".to_owned(), "*.254.169.254".to_owned()];
    assert!(!allows_on(&listed, "127.0.0.1", 443, &[443]));
    assert!(!allows_on(&listed, "169.254.169.254", 80, &[80]));
    assert!(!allows(&["*.0.0.1:8080".to_owned()], "127.0.0.1"));
}

/// Review of #1341, 3: a name is reached only at the addresses a chat may reach, as the
/// resolver answers at this connect, so a name rebound to this machine or a metadata service
/// reaches nothing. A listed address literal is reached as itself.
#[test]
fn a_name_is_reached_only_at_addresses_a_chat_may_reach() {
    let at = |ip: &str| std::net::SocketAddr::new(ip.parse().unwrap(), 443);
    let own: Vec<std::net::IpAddr> = vec!["192.168.1.7".parse().unwrap()];
    let resolver = |answer: Vec<std::net::SocketAddr>| move |_: &str, _: u16| answer.clone();
    let rebound = resolver(vec![
        at("127.0.0.1"),
        at("169.254.169.254"),
        at("192.168.1.7"),
        at("100.100.100.200"),
        at("10.100.39.145"),
    ]);
    assert_eq!(
        reachable("api.internal.example", 443, &rebound, &none(), &own),
        [at("10.100.39.145")]
    );
    let only_loopback = resolver(vec![at("127.0.0.1")]);
    assert_eq!(
        reachable("127.0.0.1.nip.io", 443, &only_loopback, &none(), &own),
        Vec::<std::net::SocketAddr>::new()
    );
    // An address literal the project listed is reached as itself.
    let literal = resolver(vec![at("10.100.39.145")]);
    assert_eq!(
        reachable("10.100.39.145", 443, &literal, &none(), &own),
        [at("10.100.39.145")]
    );
}

/// Review of #1341, round 3: an AAAA answer that carries a refused IPv4 address is dropped.
#[test]
fn an_aaaa_answer_carrying_this_machine_or_metadata_is_dropped() {
    let at = |ip: &str| std::net::SocketAddr::new(ip.parse().unwrap(), 443);
    let answer = vec![
        at("64:ff9b::a9fe:a9fe"),
        at("::7f00:1"),
        at("2002:7f00:1::1"),
        at("fd00::7"),
    ];
    let resolve = move |_: &str, _: u16| answer.clone();
    assert_eq!(
        reachable("api.internal.example", 443, &resolve, &none(), &[]),
        [at("fd00::7")]
    );
}

/// #1683: a refusal heard before anyone can be told (the chat has no number yet, or the
/// project's hooks do not listen) is kept untold, not dropped, and told once someone can be;
/// a refusal told already is never told again.
#[test]
fn a_refusal_nobody_could_be_told_yet_is_told_once_someone_can_be() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let listening = std::sync::Arc::new(AtomicBool::new(false));
    let told: std::sync::Arc<std::sync::Mutex<Vec<String>>> = std::sync::Arc::default();
    let refusals = Refusals::telling_once_heard({
        let (told, listening) = (
            std::sync::Arc::clone(&told),
            std::sync::Arc::clone(&listening),
        );
        std::sync::Arc::new(move |host: &str, port: u16| {
            if !listening.load(Ordering::SeqCst) {
                return false;
            }
            told.lock().unwrap().push(format!("{host}:{port}"));
            true
        })
    });
    refusals.heard("early.example.com", 443);
    refusals.heard("early.example.com", 443);
    assert!(told.lock().unwrap().is_empty());
    refusals.tell_untold();
    assert!(told.lock().unwrap().is_empty(), "still nobody to tell");
    listening.store(true, Ordering::SeqCst);
    refusals.clone().tell_untold();
    refusals.heard("later.example.com", 443);
    refusals.tell_untold();
    assert_eq!(
        *told.lock().unwrap(),
        ["early.example.com:443", "later.example.com:443"],
        "each once, the early one as soon as someone could be told"
    );
    assert_eq!(
        refusals.refused(),
        ["early.example.com:443", "later.example.com:443"]
    );
}

/// What it refused because nothing lists it is its own record, each host and port once, told
/// as it happens; a request it could not read, or whose `Host` names another host, is not.
#[test]
fn a_host_nothing_lists_is_kept_once_and_told_as_the_proxy_refused_it() {
    let told: std::sync::Arc<std::sync::Mutex<Vec<String>>> = std::sync::Arc::default();
    let refusals = Refusals::telling({
        let told = std::sync::Arc::clone(&told);
        std::sync::Arc::new(move |host: &str, port: u16| {
            told.lock().unwrap().push(format!("{host}:{port}"));
        })
    });
    let proxy = Proxy::start_keeping(vec!["example.com".to_owned()], refusals).expect("a proxy");
    for _ in 0..3 {
        let said = status(
            &mut to(&proxy),
            "CONNECT api.cluster.example-k8s.com:6443 HTTP/1.1\r\n\r\n",
        );
        assert_eq!(said, "HTTP/1.1 403 Forbidden");
    }
    // A listed host on a port it is not listed on is refused, and kept, on that port.
    let said = status(&mut to(&proxy), "CONNECT example.com:6443 HTTP/1.1\r\n\r\n");
    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    // Neither of these is a host a grant would let through.
    let said = status(
        &mut to(&proxy),
        "GET http://example.com/ HTTP/1.1\r\nHost: other.example\r\n\r\n",
    );
    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    let said = status(&mut to(&proxy), "NONSENSE\r\n\r\n");
    assert_eq!(said, "HTTP/1.1 400 Bad Request");
    assert_eq!(
        proxy.refusals().refused(),
        vec![
            "api.cluster.example-k8s.com:6443".to_owned(),
            "example.com:6443".to_owned()
        ]
    );
    proxy.settled();
    assert_eq!(*told.lock().unwrap(), proxy.refusals().refused());
}

/// #1353: a refusal is kept and answered at once, whoever is told it: a listener still busy
/// with it (the app's file write and window emit) holds up neither the client's 403 nor the
/// record, and is told it once it is free.
#[test]
fn a_refusal_is_answered_while_whoever_is_told_it_is_still_busy() {
    let (free, busy) = std::sync::mpsc::channel::<()>();
    let busy = std::sync::Mutex::new(busy);
    let told: std::sync::Arc<std::sync::Mutex<Vec<String>>> = std::sync::Arc::default();
    let refusals = Refusals::telling({
        let told = std::sync::Arc::clone(&told);
        std::sync::Arc::new(move |host: &str, port: u16| {
            let _ = busy.lock().unwrap().recv_timeout(Duration::from_secs(15));
            told.lock().unwrap().push(format!("{host}:{port}"));
        })
    });
    let proxy = Proxy::start_keeping(vec!["example.com".to_owned()], refusals).expect("a proxy");
    let started = std::time::Instant::now();
    let said = status(
        &mut to(&proxy),
        "CONNECT slow.example.org:443 HTTP/1.1\r\n\r\n",
    );
    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the answer waited on the listener: {:?}",
        started.elapsed()
    );
    assert_eq!(proxy.refusals().refused(), ["slow.example.org:443"]);
    assert!(told.lock().unwrap().is_empty(), "told before it was free");
    free.send(()).expect("the listener");
    proxy.settled();
    assert_eq!(*told.lock().unwrap(), ["slow.example.org:443"]);
}

#[test]
fn a_proxys_record_keeps_a_few_hosts_and_names_an_ipv6_one_in_brackets() {
    let refusals = Refusals::default();
    refusals.heard("::1", 6443);
    refusals.heard("API.example.com", 443);
    refusals.heard("api.example.com", 443);
    for n in 0..100 {
        refusals.heard(&format!("h{n}.example"), 443);
    }
    let refused = refusals.refused();
    assert_eq!(refused.len(), REFUSALS_KEPT);
    assert_eq!(refused[..2], ["[::1]:6443", "API.example.com:443"]);
}

/// #1353: a host refused past the few kept is not kept, but it is counted, once each, so what
/// lists the kept ones can say how many more there were. Its clones count with it.
#[test]
fn a_proxys_record_counts_each_host_past_the_few_it_keeps() {
    let refusals = Refusals::default();
    let clone = refusals.clone();
    for n in 0..REFUSALS_KEPT {
        refusals.heard(&format!("kept{n}.example"), 443);
    }
    assert_eq!(refusals.more(), 0, "none past the kept ones yet");
    for n in 0..5 {
        clone.heard(&format!("more{n}.example"), 443);
    }
    // The same host again, in another case, and a kept one again: neither is one more.
    refusals.heard("MORE0.example", 443);
    refusals.heard("kept0.example", 443);
    // The same name on another port is another host and port.
    refusals.heard("more0.example", 80);
    assert_eq!(refusals.refused().len(), REFUSALS_KEPT);
    assert_eq!(refusals.more(), 6);
    assert_eq!(clone.more(), 6);
}

/// #1353: each host counted past the kept ones is told as the count it brings the record to,
/// so a Notice already up can say "and N more": a host counted already, or a kept one again,
/// tells nothing, and nothing is told before the record is full.
#[test]
fn a_proxys_record_tells_each_new_count_past_the_few_it_keeps() {
    use std::sync::{Arc, Mutex};
    let heard: Arc<Mutex<Vec<usize>>> = Arc::default();
    let refusals = Refusals::default().telling_more({
        let heard = Arc::clone(&heard);
        Arc::new(move |more| heard.lock().unwrap().push(more))
    });
    for n in 0..REFUSALS_KEPT {
        refusals.heard(&format!("kept{n}.example"), 443);
    }
    assert!(
        heard.lock().unwrap().is_empty(),
        "none past the kept ones yet"
    );
    refusals.heard("more0.example", 443);
    refusals.heard("MORE0.example", 443);
    refusals.heard("kept0.example", 443);
    refusals.clone().heard("more1.example", 443);
    assert_eq!(*heard.lock().unwrap(), [1, 2]);
}

/// #1353: what is counted past the kept ones is bounded too: a command that tries a million
/// hosts costs the record no more than a few thousand did.
#[test]
fn a_proxys_count_past_the_kept_hosts_stops_at_its_bound() {
    let refusals = Refusals::default();
    for n in 0..REFUSALS_KEPT + MORE_COUNTED_AT_MOST + 50 {
        refusals.heard(&format!("h{n}.example"), 443);
    }
    assert_eq!(refusals.more(), MORE_COUNTED_AT_MOST);
}

// ---- #1664: a pair of ports per chat, SOCKS5, the local-address check, every connection told -

/// What a test hears from a proxy: each refusal, each local-address refusal, each carried line.
#[derive(Default, Clone)]
struct Heard {
    refused: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    local: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    reached: std::sync::Arc<std::sync::Mutex<Vec<super::Line>>>,
}

impl Heard {
    fn serving(&self, reach: Reach) -> super::Serving {
        let push = |into: &std::sync::Arc<std::sync::Mutex<Vec<String>>>| {
            let into = std::sync::Arc::clone(into);
            std::sync::Arc::new(move |host: &str, port: u16| {
                into.lock().unwrap().push(format!("{host}:{port}"));
            }) as super::Told
        };
        let reached = std::sync::Arc::clone(&self.reached);
        super::Serving {
            refusals: Refusals::telling(push(&self.refused)).telling_local(push(&self.local)),
            reached: Some(std::sync::Arc::new(move |target, by, times| {
                reached
                    .lock()
                    .unwrap()
                    .push((target.map(str::to_owned), by, times));
            })),
            ..super::Serving::of(reach)
        }
    }

    fn refused(&self) -> Vec<String> {
        self.refused.lock().unwrap().clone()
    }

    fn local(&self) -> Vec<String> {
        self.local.lock().unwrap().clone()
    }

    fn reached(&self) -> Vec<super::Line> {
        self.reached.lock().unwrap().clone()
    }
}

fn to_socks(proxy: &Proxy) -> TcpStream {
    let stream = TcpStream::connect(("127.0.0.1", proxy.socks_port())).expect("the SOCKS port");
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .expect("a timeout");
    stream
}

/// A SOCKS5 `CONNECT` to `host` on `port` with no authentication: the reply code.
fn socks_connect(stream: &mut TcpStream, host: &str, port: u16) -> u8 {
    stream.write_all(&[5, 1, 0]).expect("a greeting");
    let mut chosen = [0u8; 2];
    stream.read_exact(&mut chosen).expect("a method");
    assert_eq!(chosen, [5, 0], "no authentication is the method served");
    let mut request = vec![5, 1, 0];
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => {
            request.push(1);
            request.extend(ip.octets());
        }
        Ok(std::net::IpAddr::V6(ip)) => {
            request.push(4);
            request.extend(ip.octets());
        }
        Err(_) => {
            request.push(3);
            request.push(u8::try_from(host.len()).expect("a short name"));
            request.extend(host.as_bytes());
        }
    }
    request.extend(port.to_be_bytes());
    stream.write_all(&request).expect("a request");
    let mut reply = [0u8; 10];
    stream.read_exact(&mut reply).expect("a reply");
    assert_eq!(reply[0], 5);
    reply[1]
}

#[test]
fn a_chats_proxy_listens_on_a_pair_of_ports_of_its_own() {
    let proxy = Proxy::start(vec!["example.com".to_owned()]).expect("a proxy");
    assert_ne!(proxy.port(), proxy.socks_port());
    let other = Proxy::start(vec!["example.com".to_owned()]).expect("a proxy");
    let mut ports = [
        proxy.port(),
        proxy.socks_port(),
        other.port(),
        other.socks_port(),
    ];
    ports.sort_unstable();
    ports
        .windows(2)
        .for_each(|pair| assert_ne!(pair[0], pair[1]));
}

#[test]
fn a_socks_connect_to_a_listed_host_is_tunnelled_to() {
    let (listener, port) = an_echo_server();
    let echoed = echo_once(listener);
    let heard = Heard::default();
    let proxy = Proxy::serving(super::Serving {
        tunnel_ports: vec![port],
        ..heard.serving(Reach::of(vec![(exactly(port), By::You)]))
    })
    .expect("a proxy");
    let mut client = to_socks(&proxy);
    assert_eq!(socks_connect(&mut client, LOOPBACK, port), 0);
    client.write_all(b"ping").expect("through the tunnel");
    let mut back = [0u8; 4];
    client.read_exact(&mut back).expect("echoed back");
    assert_eq!(&back, b"ping");
    assert_eq!(echoed.join().expect("the server"), b"ping");
    assert_eq!(heard.reached(), [(Some(exactly(port)), "you", 1)]);
}

#[test]
fn a_socks_connect_to_a_host_nothing_lists_is_refused_with_socks_own_reply() {
    let (listener, port) = an_echo_server();
    listener.set_nonblocking(true).expect("nonblocking");
    let heard = Heard::default();
    let proxy = Proxy::serving(heard.serving(Reach::open(vec!["example.com".to_owned()])))
        .expect("a proxy");
    // Not allowed by the ruleset: SOCKS's own word for it.
    assert_eq!(
        socks_connect(&mut to_socks(&proxy), "unlisted.example.org", 443),
        2
    );
    proxy.settled();
    assert_eq!(heard.refused(), ["unlisted.example.org:443"]);
    assert_eq!(socks_connect(&mut to_socks(&proxy), LOOPBACK, port), 2);
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        listener.accept().is_err(),
        "the proxy connected to a host it refused"
    );
    proxy.settled();
    // Each refused connection is counted for the record too.
    assert_eq!(
        heard.reached(),
        [
            (Some("unlisted.example.org:443".to_owned()), "ask", 1),
            (Some(exactly(port)), "refused", 1),
        ]
    );
}

#[test]
fn a_socks_client_that_asks_anything_but_an_unauthenticated_connect_is_refused() {
    let proxy = Proxy::start(vec!["example.com".to_owned()]).expect("a proxy");
    // Only a username and password offered: no acceptable method.
    let mut client = to_socks(&proxy);
    client.write_all(&[5, 1, 2]).expect("a greeting");
    let mut chosen = [0u8; 2];
    client.read_exact(&mut chosen).expect("a method");
    assert_eq!(chosen, [5, 0xff]);
    // BIND: command not supported.
    let mut client = to_socks(&proxy);
    client.write_all(&[5, 1, 0]).expect("a greeting");
    client.read_exact(&mut chosen).expect("a method");
    client
        .write_all(&[
            5, 2, 0, 3, 11, b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'.', b'c', b'o', b'm', 1,
            187,
        ])
        .expect("a request");
    let mut reply = [0u8; 10];
    client.read_exact(&mut reply).expect("a reply");
    assert_eq!(reply[1], 7);
    // SOCKS4 is not spoken: closed with no reply.
    let mut client = to_socks(&proxy);
    client
        .write_all(&[4, 1, 0, 80, 1, 2, 3, 4, 0])
        .expect("a request");
    let mut buf = [0u8; 1];
    assert_eq!(client.read(&mut buf).unwrap_or(0), 0);
}

/// **Which chat a connection is from is the port it came in on** (#1664): a chat that reaches
/// another chat's proxy is that chat's, decided by that chat's layers, never by anything the
/// connection says, and the first chat hears nothing of it. The sandbox lets no chat reach
/// another chat's ports (`a_chats_profile_reaches_only_its_own_two_ports`); this is what the
/// proxy holds besides.
#[test]
fn a_connection_is_the_chat_whose_port_it_came_in_on_whatever_it_says() {
    let (listener, port) = an_echo_server();
    let echoed = echo_once(listener);
    let (mine, theirs) = (Heard::default(), Heard::default());
    let my_proxy = Proxy::serving(heard_on(
        &mine,
        Reach::open(vec!["example.com".to_owned()]),
        port,
    ))
    .expect("my proxy");
    let their_proxy = Proxy::serving(heard_on(
        &theirs,
        Reach::of(vec![(exactly(port), By::Chat)]),
        port,
    ))
    .expect("their proxy");
    // On my port, naming the other chat's credentials as best a client can: still mine.
    let said = status(
        &mut to(&my_proxy),
        &format!(
            "CONNECT 127.0.0.1:{port} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
             Proxy-Authorization: Basic dGhlaXJzOg==\r\nX-Purlis-Chat: 2\r\n\r\n"
        ),
    );
    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    my_proxy.settled();
    their_proxy.settled();
    assert_eq!(mine.local(), [exactly(port)]);
    assert!(theirs.local().is_empty() && theirs.refused().is_empty());
    // On theirs, it is theirs, by their layers.
    let mut client = to(&their_proxy);
    let said = status(
        &mut client,
        &format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
    );
    assert_eq!(said, "HTTP/1.1 200 Connection established");
    client.write_all(b"ping").expect("through the tunnel");
    assert_eq!(echoed.join().expect("the server"), b"ping");
    assert_eq!(theirs.reached(), [(Some(exactly(port)), "chat", 1)]);
    assert_eq!(mine.reached(), [(Some(exactly(port)), "refused", 1)]);
}

fn heard_on(heard: &Heard, reach: Reach, port: u16) -> super::Serving {
    super::Serving {
        tunnel_ports: vec![port],
        plain_ports: vec![port],
        ..heard.serving(reach)
    }
}

/// The local-address check (#1664): this machine, link-local and cloud metadata addresses are
/// refused as a Block, listed or not, unless that exact address and port is listed, and never
/// connected to.
#[test]
fn this_machine_and_metadata_are_refused_unless_listed_exactly() {
    let (listener, port) = an_echo_server();
    listener.set_nonblocking(true).expect("nonblocking");
    let heard = Heard::default();
    let proxy = Proxy::serving(heard_on(
        &heard,
        Reach::open(vec![LOOPBACK.to_owned(), "169.254.169.254".to_owned()]),
        port,
    ))
    .expect("a proxy");
    for target in [
        format!("127.0.0.1:{port}"),
        format!("169.254.169.254:{port}"),
    ] {
        let said = status(
            &mut to(&proxy),
            &format!("CONNECT {target} HTTP/1.1\r\n\r\n"),
        );
        assert_eq!(said, "HTTP/1.1 403 Forbidden", "{target}");
    }
    assert_eq!(
        socks_connect(&mut to_socks(&proxy), "169.254.169.254", 80),
        2
    );
    std::thread::sleep(Duration::from_millis(200));
    assert!(listener.accept().is_err(), "the proxy reached this machine");
    proxy.settled();
    assert_eq!(
        heard.local(),
        [
            format!("127.0.0.1:{port}"),
            format!("169.254.169.254:{port}"),
            "169.254.169.254:80".to_owned()
        ]
    );
    assert!(
        heard.refused().is_empty(),
        "never offered as a host to allow"
    );
}

/// A name that resolves only to this machine is refused as a local address, listed or not.
#[test]
fn a_name_resolving_to_this_machine_is_refused() {
    let (listener, port) = an_echo_server();
    listener.set_nonblocking(true).expect("nonblocking");
    let heard = Heard::default();
    let proxy = Proxy::serving(heard_on(
        &heard,
        Reach::open(vec!["localhost".to_owned()]),
        port,
    ))
    .expect("a proxy");
    let said = status(
        &mut to(&proxy),
        &format!("CONNECT localhost:{port} HTTP/1.1\r\n\r\n"),
    );
    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    assert_eq!(socks_connect(&mut to_socks(&proxy), "localhost", port), 2);
    std::thread::sleep(Duration::from_millis(200));
    assert!(listener.accept().is_err(), "the proxy reached this machine");
    proxy.settled();
    assert_eq!(heard.local(), [format!("localhost:{port}")]);
}

/// The local-address check on what a name resolves to, without a resolver: the exact address
/// and port listed is let through, nothing else of this machine is.
#[test]
fn a_resolved_local_address_is_reached_only_where_listed_exactly() {
    let at = |ip: &str, port: u16| std::net::SocketAddr::new(ip.parse().unwrap(), port);
    let own: Vec<std::net::IpAddr> = vec!["192.168.1.7".parse().unwrap()];
    let answer = vec![at("127.0.0.1", 5432), at("192.168.1.7", 5432)];
    let resolve = move |_: &str, _: u16| answer.clone();
    let listed = Reach::of(vec![("127.0.0.1:5432".to_owned(), By::You)]);
    assert_eq!(
        reachable("db.internal.example", 5432, &resolve, &listed, &own),
        [at("127.0.0.1", 5432)]
    );
    assert!(reachable("db.internal.example", 5432, &resolve, &none(), &own).is_empty());
}

/// A listed name pointed at this machine, a link-local address or a metadata service, in any
/// spelling an IPv6 answer can carry one, reaches nothing; a public address beside it is kept.
#[test]
fn a_listed_name_resolving_to_any_spelling_of_a_local_address_reaches_nothing() {
    let at = |ip: &str, port: u16| std::net::SocketAddr::new(ip.parse().unwrap(), port);
    let own: Vec<std::net::IpAddr> = vec!["2001:db8::7".parse().unwrap()];
    let listed = Reach::open(vec!["rebound.example.com".to_owned()]);
    for local in [
        "127.0.0.1",
        "0.0.0.0",
        "169.254.169.254",
        "::1",
        "::",
        "fe80::1",
        "fd00:ec2::254",
        "::ffff:127.0.0.1",
        "::ffff:169.254.169.254",
        "64:ff9b::a9fe:a9fe",
        "2002:7f00:1::1",
        "2001:db8::7",
    ] {
        let answer = vec![at(local, 443)];
        let resolve = move |_: &str, _: u16| answer.clone();
        assert!(
            reachable("rebound.example.com", 443, &resolve, &listed, &own).is_empty(),
            "{local}"
        );
    }
    let answer = vec![at("::1", 443), at("2001:db8::8", 443)];
    let resolve = move |_: &str, _: u16| answer.clone();
    assert_eq!(
        reachable("rebound.example.com", 443, &resolve, &listed, &own),
        [at("2001:db8::8", 443)]
    );
}

#[test]
fn both_ports_stop_listening_when_the_chat_ends() {
    let proxy = Proxy::start(vec!["example.com".to_owned()]).expect("a proxy");
    let (http, socks) = (proxy.port(), proxy.socks_port());
    drop(proxy);
    std::thread::sleep(Duration::from_millis(200));
    assert!(TcpStream::connect(("127.0.0.1", http)).is_err());
    assert!(TcpStream::connect(("127.0.0.1", socks)).is_err());
}

/// The connection limit is the chat's, over both its ports together.
#[test]
fn the_connection_limit_counts_both_ports_together() {
    let proxy = Proxy::limited(
        vec!["example.com".to_owned()],
        vec![443],
        Limits {
            connections: 2,
            idle: Duration::from_secs(60),
            head: Duration::from_secs(30),
        },
    )
    .expect("a proxy");
    let _held = [to(&proxy), to_socks(&proxy)];
    std::thread::sleep(Duration::from_millis(100));
    let mut third = to_socks(&proxy);
    third.write_all(&[5, 1, 0]).ok();
    let mut chosen = [0u8; 2];
    third.read_exact(&mut chosen).expect("an answer");
    assert_eq!(chosen, [5, 0xff]);
}

#[test]
fn every_connection_carried_is_told_once_a_window_with_how_many() {
    use super::{TALLY_HOSTS, TALLY_WINDOW, Tally};
    let start = std::time::Instant::now();
    let mut tally = Tally::default();
    let a = "registry.npmjs.org:443";
    assert_eq!(
        tally.heard(a, "open", start),
        [(Some(a.to_owned()), "open", 1)]
    );
    for n in 1..=4 {
        assert!(
            tally
                .heard(a, "open", start + Duration::from_secs(n))
                .is_empty()
        );
    }
    // A window on: the four held back are told, with the one now.
    let later = start + TALLY_WINDOW + Duration::from_secs(1);
    assert_eq!(
        tally.heard(a, "open", later),
        [
            (Some(a.to_owned()), "open", 4),
            (Some(a.to_owned()), "open", 1)
        ]
    );
    assert!(tally.heard(a, "open", later).is_empty());
    // Past what one tally tells apart, connections are counted together, per layer.
    let mut many = Tally::default();
    for n in 0..TALLY_HOSTS {
        assert_eq!(
            many.heard(&format!("h{n}.example:443"), "you", start).len(),
            1
        );
    }
    assert!(many.heard("past.example:443", "you", start).is_empty());
    assert!(many.heard("past2.example:443", "you", start).is_empty());
    let ended = many.ended();
    assert_eq!(ended, [(None, "you", 2)]);
    assert_eq!(tally.ended(), [(Some(a.to_owned()), "open", 1)]);
}

/// #1699: what a burst left untold is told once its window has passed, with nothing heard
/// since, so the record's line is timed at the window's end, not at the next connection.
#[test]
fn a_bursts_count_is_told_once_its_window_passes_with_no_connection_since() {
    use super::{TALLY_WINDOW, Tally};
    let start = std::time::Instant::now();
    let mut tally = Tally::default();
    let a = "registry.npmjs.org:443";
    tally.heard(a, "open", start);
    tally.heard(a, "open", start);
    tally.heard(a, "open", start);
    assert!(
        tally.due(start + TALLY_WINDOW / 2).is_empty(),
        "its window runs"
    );
    assert_eq!(
        tally.due(start + TALLY_WINDOW),
        [(Some(a.to_owned()), "open", 2)]
    );
    assert!(tally.due(start + TALLY_WINDOW).is_empty(), "told once");
    assert!(tally.ended().is_empty());
    // Told again at once, as a host it let go of.
    assert_eq!(
        tally.heard(a, "open", start + TALLY_WINDOW),
        [(Some(a.to_owned()), "open", 1)]
    );
}

/// A chat's Seatbelt profile lets it connect to its own proxy's two ports and nowhere else on
/// the network, so no chat reaches another chat's ports (#1664).
#[test]
fn a_chats_profile_reaches_only_its_own_two_ports() {
    let dir = tempfile::tempdir().expect("a dir");
    let (cwd, tmp) = (dir.path().join("chat"), dir.path().join("tmp"));
    std::fs::create_dir_all(&cwd).expect("cwd");
    std::fs::create_dir_all(&tmp).expect("tmp");
    let profile = crate::sandbox::seatbelt::profile(
        &[],
        &crate::sandbox::seatbelt::Own::default(),
        &cwd,
        &tmp,
        &[4040, 4041],
        None,
    )
    .expect("a profile");
    let outbound: Vec<&str> = profile
        .lines()
        .filter(|line| line.contains("network-outbound") && line.contains("remote ip"))
        .collect();
    assert_eq!(
        outbound,
        [
            "(allow network-outbound (remote ip \"localhost:4040\"))",
            "(allow network-outbound (remote ip \"localhost:4041\"))",
        ]
    );
    assert!(!profile.contains("localhost:*"));
}

// ---- the live ask (#1666): held while the person is asked --------------------------------------

/// A proxy whose asks hold for `hold`, and the board the window answers on. `.invalid` never
/// resolves (RFC 2606), so a held connection that is let on is answered 502 by the proxy trying
/// it, and one that is refused 403 without a try: which of the two says what the hold decided.
fn asking(hold: Duration) -> (Proxy, std::sync::Arc<crate::sandbox::asks::Asks>, Heard) {
    let heard = Heard::default();
    let asks = std::sync::Arc::new(crate::sandbox::asks::Asks::timed(
        std::sync::Arc::new(|_| {}),
        hold,
        Duration::from_millis(20),
    ));
    let proxy = Proxy::serving(super::Serving {
        asks: Some(std::sync::Arc::clone(&asks)),
        ..heard.serving(none())
    })
    .expect("a proxy");
    (proxy, asks, heard)
}

fn until_held(asks: &crate::sandbox::asks::Asks) {
    for _ in 0..1000 {
        if asks.holding() == 1 {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the connection was never held");
}

#[test]
fn a_held_tunnel_goes_on_the_moment_its_host_is_allowed() {
    let (proxy, asks, heard) = asking(Duration::from_secs(30));
    let mut client = to(&proxy);
    let answered = std::thread::spawn(move || {
        status(
            &mut client,
            "CONNECT held.invalid:443 HTTP/1.1\r\nHost: held.invalid:443\r\n\r\n",
        )
    });
    until_held(&asks);
    asks.allow(
        &crate::sandbox::hosts::Host::parse("held.invalid").unwrap(),
        By::You,
    );
    // Carried: the proxy tried the host, which never resolves.
    assert_eq!(answered.join().unwrap(), "HTTP/1.1 502 Bad Gateway");
    assert!(heard.refused().is_empty(), "no Block for a host let on");
}

#[test]
fn a_held_socks_connect_goes_on_the_moment_its_host_is_allowed() {
    let (proxy, asks, _) = asking(Duration::from_secs(30));
    let mut client = to_socks(&proxy);
    let answered = std::thread::spawn(move || socks_connect(&mut client, "held.invalid", 443));
    until_held(&asks);
    asks.allow(
        &crate::sandbox::hosts::Host::parse("held.invalid").unwrap(),
        By::Chat,
    );
    // Host unreachable: carried, and tried.
    assert_eq!(answered.join().unwrap(), 4);
}

#[test]
fn a_held_connection_nobody_answers_is_refused_recorded_and_told_to_retry_later() {
    let (proxy, asks, heard) = asking(Duration::from_millis(300));
    let mut client = to(&proxy);
    let said = status(
        &mut client,
        "CONNECT held.invalid:443 HTTP/1.1\r\nHost: held.invalid:443\r\n\r\n",
    );
    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    proxy.settled();
    // In the record's tally, as an ask refused; no second Block: the ask's Notice is up.
    assert!(
        heard
            .reached
            .lock()
            .unwrap()
            .iter()
            .any(|(target, by, _)| target.as_deref() == Some("held.invalid:443") && *by == "ask"),
    );
    assert!(heard.refused().is_empty());
    let retry = asks.allow(
        &crate::sandbox::hosts::Host::parse("held.invalid").unwrap(),
        By::You,
    );
    assert_eq!(retry, vec!["held.invalid:443".to_owned()]);
}

/// Keep blocked after an Allow of the same host refuses it again, through both the HTTP and the
/// SOCKS proxy: the later answer wins.
#[test]
fn keep_blocked_after_an_allow_refuses_the_host_through_both_proxies() {
    let (proxy, asks, _) = asking(Duration::from_millis(300));
    let held = crate::sandbox::hosts::Host::parse("held.invalid").unwrap();
    asks.allow(&held, By::Chat);
    // Carried: the proxy tried the host, which never resolves.
    assert_eq!(socks_connect(&mut to_socks(&proxy), "held.invalid", 443), 4);

    asks.keep_blocked(&held);

    let said = status(
        &mut to(&proxy),
        "CONNECT held.invalid:443 HTTP/1.1\r\nHost: held.invalid:443\r\n\r\n",
    );
    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    assert_eq!(socks_connect(&mut to_socks(&proxy), "held.invalid", 443), 2);
}

#[test]
fn without_a_board_an_ask_is_refused_at_once_as_before() {
    let heard = Heard::default();
    let proxy = Proxy::serving(heard.serving(none())).expect("a proxy");
    let mut client = to(&proxy);
    let said = status(
        &mut client,
        "CONNECT held.invalid:443 HTTP/1.1\r\nHost: held.invalid:443\r\n\r\n",
    );
    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    proxy.settled();
    assert_eq!(heard.refused(), vec!["held.invalid:443".to_owned()]);
}

/// A chat's proxy as the app arms it (#1666, #1709): nothing listed when the chat started, and
/// a board that holds an ask for `hold` and lets on at once a host the project's files allow
/// already ([`crate::sandbox::grant::allowed_already`], read from disk at each connection). The board
/// tells what it asks into the list answered.
fn armed_in(
    root: &std::path::Path,
    hold: Duration,
) -> (
    Proxy,
    std::sync::Arc<crate::sandbox::asks::Asks>,
    Heard,
    std::sync::Arc<std::sync::Mutex<Vec<crate::sandbox::asks::Heard>>>,
) {
    let heard = Heard::default();
    let told = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let tell = {
        let told = std::sync::Arc::clone(&told);
        std::sync::Arc::new(move |said| told.lock().unwrap().push(said))
    };
    let root = root.to_path_buf();
    let asks = std::sync::Arc::new(
        crate::sandbox::asks::Asks::timed(tell, hold, Duration::from_millis(20)).knowing(
            std::sync::Arc::new(move |host| {
                crate::sandbox::grant::allowed_already(&root, host).map(By::from)
            }),
        ),
    );
    let proxy = Proxy::serving(super::Serving {
        asks: Some(std::sync::Arc::clone(&asks)),
        ..heard.serving(none())
    })
    .expect("a proxy");
    (proxy, asks, heard, told)
}

/// #1709: a host the project's hosts files allow already, written there after the chat
/// started, offers no Allow and its connection is let on at once: the project's own host
/// (its committed file) and one of yours (this machine's file, confirmed in Settings). A host
/// neither lists is still asked about, so the board was live. Writes a manifest and this
/// machine's file, so it first runs on CI.
#[test]
fn a_host_the_projects_hosts_files_allow_already_is_let_on_at_once_and_never_asked_about() {
    let root = tempfile::tempdir().expect("a project");
    let (proxy, asks, heard, told) = armed_in(root.path(), Duration::from_millis(600));
    // Allowed since the chat started, in the project's two hosts files.
    std::fs::write(
        root.path().join(crate::plane::MANIFEST),
        "[sandbox]\nmode = \"on\"\negress = []\nhosts = [\"team.invalid\"]\n",
    )
    .expect("the project's file");
    std::fs::write(
        crate::names::local_settings(root.path()),
        "[sandbox]\nhosts = [\"mine.invalid\"]\n",
    )
    .expect("this machine's file");
    crate::sandbox::local::confirm_host(root.path(), "mine.invalid").expect("confirmed");

    for host in ["team.invalid", "mine.invalid"] {
        let started = std::time::Instant::now();
        let said = status(
            &mut to(&proxy),
            &format!("CONNECT {host}:443 HTTP/1.1\r\nHost: {host}:443\r\n\r\n"),
        );
        // Carried: the proxy tried the host, which never resolves. Never held its ask's time.
        assert_eq!(said, "HTTP/1.1 502 Bad Gateway", "{host}");
        assert!(
            started.elapsed() < Duration::from_millis(600),
            "{host} was held: {:?}",
            started.elapsed()
        );
    }
    assert_eq!(asks.holding(), 0);
    assert!(told.lock().unwrap().is_empty(), "an Allow was offered");
    assert!(heard.refused().is_empty(), "no Block for a host let on");
    // Carried from now on at the scope that holds each: yours, and everyone's in the project.
    assert_eq!(asks.allows("mine.invalid", 443, &[443]), Some(By::You));
    assert_eq!(asks.allows("team.invalid", 443, &[443]), Some(By::Project));

    // A host neither file lists is held and asked about, as before.
    let said = status(
        &mut to(&proxy),
        "CONNECT other.invalid:443 HTTP/1.1\r\nHost: other.invalid:443\r\n\r\n",
    );
    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    assert!(
        told.lock()
            .unwrap()
            .contains(&crate::sandbox::asks::Heard::Asked(vec![
                "other.invalid:443".to_owned()
            ])),
        "{:?}",
        told.lock().unwrap()
    );
}

/// Keep blocked holds for this chat even for a host the project's hosts list names, through
/// both the HTTP and the SOCKS proxy: the later answer wins. Writes a manifest, so it first runs
/// on CI.
#[test]
fn keep_blocked_holds_for_the_chat_even_for_a_host_the_projects_hosts_list_names() {
    let root = tempfile::tempdir().expect("a project");
    let (proxy, asks, _heard, _told) = armed_in(root.path(), Duration::from_millis(300));
    std::fs::write(
        root.path().join(crate::plane::MANIFEST),
        "[sandbox]\nmode = \"on\"\negress = []\nhosts = [\"team.invalid\"]\n",
    )
    .expect("the project's file");

    asks.keep_blocked(&crate::sandbox::hosts::Host::parse("team.invalid").unwrap());

    let said = status(
        &mut to(&proxy),
        "CONNECT team.invalid:443 HTTP/1.1\r\nHost: team.invalid:443\r\n\r\n",
    );
    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    assert_eq!(socks_connect(&mut to_socks(&proxy), "team.invalid", 443), 2);
    assert_eq!(asks.allows("team.invalid", 443, &[443]), None);
}

// ---- qw384b: a service on this Mac by name, for a client that never proxies this machine -----

/// A Claude Code chat's proxy holding `port` for the chat, as its start compiles it.
fn holding(heard: &Heard, port: u16) -> Proxy {
    Proxy::serving(super::Serving {
        any_port: true,
        ..heard.serving(Reach::of(
            crate::sandbox::localport::reach_entries(port)
                .into_iter()
                .map(|entry| (entry, By::Chat))
                .collect(),
        ))
    })
    .expect("a proxy")
}

#[test]
fn this_mac_by_name_is_tunnelled_to_a_granted_port_and_each_connection_recorded() {
    let (listener, port) = an_echo_server();
    let echoed = echo_once(listener);
    let heard = Heard::default();
    let proxy = holding(&heard, port);
    let mut client = to(&proxy);
    let name = format!("{}:{port}", crate::sandbox::localport::THIS_MAC);
    let said = status(
        &mut client,
        &format!("CONNECT {name} HTTP/1.1\r\nHost: {name}\r\n\r\n"),
    );
    assert_eq!(said, "HTTP/1.1 200 Connection established");
    client.write_all(b"ping").expect("through the tunnel");
    let mut back = [0u8; 4];
    client.read_exact(&mut back).expect("echoed back");
    assert_eq!(&back, b"ping");
    assert_eq!(echoed.join().expect("the server"), b"ping");
    drop(client);
    drop(proxy);
    // Recorded by the name it was reached by, and the layer that granted it.
    assert_eq!(heard.reached(), [(Some(name), "chat", 1)]);
}

#[test]
fn this_mac_by_name_is_refused_on_a_port_not_granted_and_once_withdrawn() {
    let (listener, port) = an_echo_server();
    listener.set_nonblocking(true).expect("nonblocking");
    let heard = Heard::default();
    let proxy = holding(&heard, port);
    let other = format!(
        "{}:{}",
        crate::sandbox::localport::THIS_MAC,
        port.wrapping_add(1).max(1)
    );
    assert_eq!(
        status(
            &mut to(&proxy),
            &format!("CONNECT {other} HTTP/1.1\r\n\r\n")
        ),
        "HTTP/1.1 403 Forbidden"
    );
    proxy.withdraw_port(port);
    let name = format!("{}:{port}", crate::sandbox::localport::THIS_MAC);
    assert_eq!(
        status(&mut to(&proxy), &format!("CONNECT {name} HTTP/1.1\r\n\r\n")),
        "HTTP/1.1 403 Forbidden"
    );
    assert_eq!(
        status(
            &mut to(&proxy),
            &format!("CONNECT {LOOPBACK}:{port} HTTP/1.1\r\n\r\n")
        ),
        "HTTP/1.1 403 Forbidden"
    );
    std::thread::sleep(Duration::from_millis(200));
    assert!(listener.accept().is_err(), "a withdrawn port was reached");
    proxy.settled();
    // Refused as this machine: a Block of a service on this Mac, never a host's ask.
    assert!(heard.local().contains(&name), "{:?}", heard.local());
    assert!(heard.refused().is_empty(), "{:?}", heard.refused());
}

/// #2097 review: a name of this Mac is pinned at connect to this machine's loopback on the port
/// the chat holds, and never looked up, so no second lookup can send it anywhere else; any
/// other name is looked up at connect, as before, and held to the local-address check.
#[test]
fn a_name_of_this_mac_is_pinned_to_loopback_at_connect_and_never_looked_up() {
    let reach = Reach::of(
        crate::sandbox::localport::reach_entries(6443)
            .into_iter()
            .map(|entry| (entry, By::Chat))
            .collect(),
    );
    let asked = std::cell::Cell::new(0);
    // Loopback first, a public address after: what a name rebound between two lookups answers.
    let rebinding = |_: &str, port: u16| {
        asked.set(asked.get() + 1);
        if asked.get() == 1 {
            vec![SocketAddr::from(([127, 0, 0, 1], port))]
        } else {
            vec![SocketAddr::from(([203, 0, 113, 5], port))]
        }
    };
    for name in crate::sandbox::localport::NAMES_OF_THIS_MAC {
        assert_eq!(
            super::addresses(name, 6443, &reach, &[], &rebinding),
            [SocketAddr::from(([127, 0, 0, 1], 6443))],
            "{name}"
        );
    }
    assert_eq!(asked.get(), 0, "a name of this Mac was looked up");
    // Any other name is looked up at connect, and only its addresses a chat may reach are kept.
    asked.set(1);
    assert_eq!(
        super::addresses("evil.example", 6443, &reach, &[], &rebinding),
        [SocketAddr::from(([203, 0, 113, 5], 6443))]
    );
    asked.set(0);
    assert_eq!(
        super::addresses("evil.example", 6443, &Reach::default(), &[], &rebinding),
        Vec::<SocketAddr>::new()
    );
}

/// A chat planned on another machine (#2445): its runner's policy decides every internal
/// address it reaches, at every connect, whatever its reach lists.
#[test]
fn an_internal_address_the_runners_policy_does_not_list_is_never_connected_to() {
    let held = super::connect_held("10.255.255.1", 443, &Reach::anything(), &[], &|at| {
        crate::sandbox::internal::Internal::default().carries("10.255.255.1", at, Some(&[]))
    });

    assert!(matches!(held, super::Connected::OnlyLocal));
}

#[test]
fn a_proxy_for_a_chat_planned_on_another_machine_refuses_an_internal_address() {
    let heard = Heard::default();
    let proxy = Proxy::serving(super::Serving {
        internal: Some(crate::sandbox::internal::Internal::default()),
        ..heard.serving(Reach::anything())
    })
    .expect("a proxy");

    let said = status(&mut to(&proxy), "CONNECT 10.255.255.1:443 HTTP/1.1\r\n\r\n");

    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    assert_eq!(socks_connect(&mut to_socks(&proxy), "10.255.255.1", 443), 2);
    proxy.settled();
    assert_eq!(heard.local(), ["10.255.255.1:443"], "told once");
}

/// A proxy for a chat planned on another machine, whose plan left out the services on
/// `left_out` of this machine (#2510), on an echo server's port, and its board.
fn leaving_out(
    left_out: Vec<u16>,
    port: u16,
) -> (Proxy, std::sync::Arc<crate::sandbox::asks::Asks>, Heard) {
    let heard = Heard::default();
    let asks = std::sync::Arc::new(crate::sandbox::asks::Asks::timed(
        std::sync::Arc::new(|_| {}),
        Duration::from_secs(30),
        Duration::from_millis(20),
    ));
    let proxy = Proxy::serving(super::Serving {
        asks: Some(std::sync::Arc::clone(&asks)),
        left_out,
        ..heard_on(&heard, none(), port)
    })
    .expect("a proxy");
    (proxy, asks, heard)
}

/// A tunnel to `localhost` on `port` through `proxy`, answered on a thread of its own, and the
/// client it was asked on.
fn tunnel_to_localhost(proxy: &Proxy, port: u16) -> (TcpStream, std::thread::JoinHandle<String>) {
    let client = to(proxy);
    let mut asking = client.try_clone().expect("a clone");
    let answered = std::thread::spawn(move || {
        status(
            &mut asking,
            &format!("CONNECT localhost:{port} HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"),
        )
    });
    (client, answered)
}

/// #2510: a service of this machine left out of a chat's plan is held and asked about, by its
/// loopback address whichever name the chat used, and reached the moment it is allowed.
#[test]
fn a_service_left_out_is_held_and_reached_once_it_is_allowed() {
    let (listener, port) = an_echo_server();
    let echoed = echo_once(listener);
    let (proxy, asks, heard) = leaving_out(vec![port], port);
    let (mut client, answered) = tunnel_to_localhost(&proxy, port);
    until_held(&asks);
    assert_eq!(
        asks.asked(),
        [exactly(port)],
        "asked by its loopback address"
    );

    asks.allow_left_out(port);

    assert_eq!(
        answered.join().unwrap(),
        "HTTP/1.1 200 Connection established"
    );
    client.write_all(b"ping").expect("through the tunnel");
    assert_eq!(echoed.join().expect("the server"), b"ping");
    assert!(heard.local().is_empty(), "no Block for a service let on");
}

/// #2510: the person's no to a service left out ends its hold at once, and it is not reached.
#[test]
fn a_service_left_out_kept_blocked_is_refused_at_once() {
    let (listener, port) = an_echo_server();
    listener.set_nonblocking(true).expect("nonblocking");
    let (proxy, asks, _) = leaving_out(vec![port], port);
    let (_client, answered) = tunnel_to_localhost(&proxy, port);
    until_held(&asks);

    asks.keep_left_out_blocked(port);

    assert_eq!(answered.join().unwrap(), "HTTP/1.1 403 Forbidden");
    std::thread::sleep(Duration::from_millis(200));
    assert!(listener.accept().is_err(), "the proxy reached this machine");
}

/// #2510: a port of this machine the chat's plan did not leave out is refused at once as this
/// machine, never asked about.
#[test]
fn a_port_of_this_machine_not_left_out_is_refused_at_once_and_never_asked_about() {
    let (listener, port) = an_echo_server();
    listener.set_nonblocking(true).expect("nonblocking");
    let (proxy, asks, heard) = leaving_out(vec![port.wrapping_add(1).max(1)], port);

    let said = status(
        &mut to(&proxy),
        &format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\n\r\n"),
    );

    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    assert!(asks.asked().is_empty(), "never asked about");
    std::thread::sleep(Duration::from_millis(200));
    assert!(listener.accept().is_err(), "the proxy reached this machine");
    proxy.settled();
    assert_eq!(heard.local(), [exactly(port)]);
}

/// #2510: a service left out is refused at once, not asked about, where the proxy asks nobody.
#[test]
fn a_service_left_out_is_refused_where_the_proxy_asks_nobody() {
    let (listener, port) = an_echo_server();
    listener.set_nonblocking(true).expect("nonblocking");
    let heard = Heard::default();
    let proxy = Proxy::serving(super::Serving {
        left_out: vec![port],
        ..heard_on(&heard, none(), port)
    })
    .expect("a proxy");

    let said = status(
        &mut to(&proxy),
        &format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\n\r\n"),
    );

    assert_eq!(said, "HTTP/1.1 403 Forbidden");
    std::thread::sleep(Duration::from_millis(200));
    assert!(listener.accept().is_err(), "the proxy reached this machine");
}
