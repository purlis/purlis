// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's front door (#2019): what it carries, what it refuses, and what it keeps.

use super::*;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

fn none(_: Option<&str>) -> Vec<String> {
    Vec::new()
}

fn head(lines: &[&str]) -> String {
    format!("{}\r\n\r\n", lines.join("\r\n"))
}

#[test]
fn a_request_to_this_door_by_its_own_names_is_carried_asking_the_server_to_close() {
    for host in ["localhost:5173", "127.0.0.1:5173", "[::1]:5173"] {
        let carried = judge(
            &head(&[
                "GET /src/main.ts HTTP/1.1",
                &format!("Host: {host}"),
                "Accept: */*",
            ]),
            5173,
            &none,
        )
        .expect("carried");
        assert_eq!(
            carried.head,
            format!(
                "GET /src/main.ts HTTP/1.1\r\nHost: {host}\r\nAccept: */*\r\nConnection: close\r\n\r\n"
            )
        );
        assert_eq!(carried.body, 0);
        assert!(!carried.upgrade);
    }
}

#[test]
fn a_host_that_is_not_this_doors_is_misdirected() {
    for host in [
        "evil.example:5173",
        "localhost:5174",
        "127.0.0.1",
        "localhost",
        "127.0.0.2:5173",
        "LOCALHOST.:5173",
        "",
    ] {
        assert_eq!(
            judge(
                &head(&["GET / HTTP/1.1", &format!("Host: {host}")]),
                5173,
                &none
            ),
            Err(Refused::Host),
            "{host:?}"
        );
    }
    assert_eq!(
        judge(&head(&["GET / HTTP/1.1"]), 5173, &none),
        Err(Refused::Host)
    );
    assert_eq!(Refused::Host.answer().0, "421 Misdirected Request");
}

#[test]
fn a_page_of_another_site_is_forbidden_and_this_doors_own_page_is_not() {
    for origin in [
        "https://evil.example",
        "http://localhost:3000",
        "null",
        "http://127.0.0.1:5173.evil.example",
        "https://localhost:5173",
    ] {
        assert_eq!(
            judge(
                &head(&[
                    "POST /api HTTP/1.1",
                    "Host: localhost:5173",
                    &format!("Origin: {origin}")
                ]),
                5173,
                &none
            ),
            Err(Refused::Origin),
            "{origin}"
        );
    }
    for origin in [
        "http://localhost:5173",
        "http://127.0.0.1:5173",
        "http://[::1]:5173",
    ] {
        assert!(
            judge(
                &head(&[
                    "POST /api HTTP/1.1",
                    "Host: localhost:5173",
                    &format!("Origin: {origin}")
                ]),
                5173,
                &none
            )
            .is_ok(),
            "{origin}"
        );
    }
    assert_eq!(Refused::Origin.answer().0, "403 Forbidden");
}

#[test]
fn a_browsers_cookies_never_reach_the_server_and_its_jar_does() {
    let jar = |token: Option<&str>| match token {
        Some(TOKEN) => vec!["session=abc".to_owned()],
        _ => Vec::new(),
    };
    let carried = judge(
        &head(&[
            "GET / HTTP/1.1",
            "Host: localhost:5173",
            &format!("Cookie: grafana_session=secret; purlis-door-5173={TOKEN}"),
            "Cookie: other=1",
        ]),
        5173,
        &jar,
    )
    .expect("carried");
    assert_eq!(carried.token.as_deref(), Some(TOKEN));
    assert!(
        !carried.head.contains("grafana_session"),
        "{}",
        carried.head
    );
    assert!(!carried.head.contains("other=1"), "{}", carried.head);
    assert!(!carried.head.contains("purlis-door"), "{}", carried.head);
    assert!(
        carried.head.contains("\r\nCookie: session=abc\r\n"),
        "{}",
        carried.head
    );
    // A token the door did not make names no jar.
    let forged = judge(
        &head(&[
            "GET / HTTP/1.1",
            "Host: localhost:5173",
            "Cookie: purlis-door-5173=../x",
        ]),
        5173,
        &jar,
    )
    .expect("carried");
    assert_eq!(forged.token, None);
}

#[test]
fn a_server_sets_cookies_in_the_door_s_jar_never_in_the_browser() {
    let mut kept = Vec::new();
    let head = answered(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nSet-Cookie: session=abc; Path=/; HttpOnly\r\nConnection: keep-alive\r\nKeep-Alive: timeout=5\r\n\r\n",
        5173,
        Some(TOKEN),
        &mut |set| kept.push(set.to_owned()),
    )
    .expect("read");
    assert_eq!(kept, ["session=abc; Path=/; HttpOnly"]);
    assert_eq!(
        head,
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nSet-Cookie: purlis-door-5173={TOKEN}; Path=/; HttpOnly; SameSite=Strict\r\nConnection: close\r\n\r\n"
        )
    );
    // A browser that has its token is set nothing.
    let again = answered("HTTP/1.1 200 OK\r\n\r\n", 5173, None, &mut |_| {}).expect("read");
    assert_eq!(again, "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n");
}

#[test]
fn a_jar_keeps_one_browsers_cookies_by_name_and_lets_a_spent_one_go() {
    let mut jar = Jar::default();
    jar.keep(TOKEN, "session=abc; Path=/");
    jar.keep(TOKEN, "theme=dark");
    jar.keep(TOKEN, "session=def");
    assert_eq!(jar.of(Some(TOKEN)), ["theme=dark", "session=def"]);
    jar.keep(TOKEN, "theme=dark; Max-Age=0");
    assert_eq!(jar.of(Some(TOKEN)), ["session=def"]);
    // Another browser's jar is its own, and no token names none.
    assert!(jar.of(Some("ffffffffffffffffffffffffffffffff")).is_empty());
    assert!(jar.of(None).is_empty());
    for _ in 0..(JARS_MOST + 4) {
        jar.keep(&new_token().expect("a token"), "a=1");
    }
    assert!(jar.of(Some(TOKEN)).is_empty(), "the oldest jar was let go");
}

#[test]
fn an_upgrade_keeps_its_headers_and_the_switch_keeps_the_connection() {
    let carried = judge(
        &head(&[
            "GET /?token=x HTTP/1.1",
            "Host: localhost:5173",
            "Origin: http://localhost:5173",
            "Connection: Upgrade",
            "Upgrade: websocket",
            "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==",
            "Sec-WebSocket-Version: 13",
        ]),
        5173,
        &none,
    )
    .expect("carried");
    assert!(carried.upgrade);
    assert!(
        carried.head.contains("\r\nUpgrade: websocket\r\n"),
        "{}",
        carried.head
    );
    assert!(
        carried.head.ends_with("Connection: Upgrade\r\n\r\n"),
        "{}",
        carried.head
    );
    let switched = answered(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: x\r\n\r\n",
        5173,
        None,
        &mut |_| {},
    )
    .expect("read");
    assert!(
        switched.contains("\r\nConnection: Upgrade\r\n"),
        "{switched}"
    );
    assert!(!switched.contains("Connection: close"), "{switched}");
    // An upgrade from another site is refused like any request from one.
    assert_eq!(
        judge(
            &head(&[
                "GET / HTTP/1.1",
                "Host: localhost:5173",
                "Origin: https://evil.example",
                "Connection: Upgrade",
                "Upgrade: websocket",
            ]),
            5173,
            &none
        ),
        Err(Refused::Origin)
    );
}

#[test]
fn a_request_two_parsers_could_read_two_ways_is_refused() {
    for lines in [
        &[
            "GET http://localhost:5173/ HTTP/1.1",
            "Host: localhost:5173",
        ][..],
        &["CONNECT localhost:5173 HTTP/1.1", "Host: localhost:5173"],
        &["GET / HTTP/1.0", "Host: localhost:5173"],
        &[
            "GET / HTTP/1.1",
            "Host: localhost:5173",
            "Host: localhost:5173",
        ],
        &["GET / HTTP/1.1", "Host : localhost:5173"],
        &[
            "GET / HTTP/1.1",
            "Host: localhost:5173",
            "Content-Length: 1",
            "Content-Length: 2",
        ],
        &[
            "GET / HTTP/1.1",
            "Host: localhost:5173",
            "Content-Length: -1",
        ],
        &["get / HTTP/1.1", "Host: localhost:5173"],
    ] {
        assert_eq!(
            judge(&head(lines), 5173, &none),
            Err(Refused::Malformed),
            "{lines:?}"
        );
    }
    assert_eq!(
        judge(
            &head(&[
                "POST / HTTP/1.1",
                "Host: localhost:5173",
                "Transfer-Encoding: chunked"
            ]),
            5173,
            &none
        ),
        Err(Refused::Chunked)
    );
}

#[test]
fn a_body_is_carried_by_its_length() {
    let carried = judge(
        &head(&[
            "POST /api HTTP/1.1",
            "Host: localhost:5173",
            "Content-Length: 11",
        ]),
        5173,
        &none,
    )
    .expect("carried");
    assert_eq!(carried.body, 11);
}

/// Over sockets: a door carries a request to a server on its socket and back, refuses another
/// site's, and lets its port go once dropped. First runs on CI: this sandbox binds no socket.
mod sockets {
    use super::*;
    use std::os::unix::net::UnixListener;

    fn free_port() -> u16 {
        TcpListener::bind(("127.0.0.1", 0))
            .and_then(|listener| listener.local_addr())
            .expect("a free port")
            .port()
    }

    /// A server on `socket` that answers each request with what it was sent, and a cookie.
    fn echo_on(socket: &Path) {
        let listener = UnixListener::bind(socket).expect("the server's socket");
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let mut buf = vec![0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let said = String::from_utf8_lossy(&buf[..n]).into_owned();
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nSet-Cookie: session=abc\r\nContent-Length: {}\r\n\r\n{said}",
                    said.len()
                );
            }
        });
    }

    fn ask(port: u16, lines: &[&str]) -> String {
        let mut client = TcpStream::connect(("127.0.0.1", port)).expect("the door answers");
        client.write_all(head(lines).as_bytes()).expect("sent");
        let mut answer = String::new();
        let _ = client.read_to_string(&mut answer);
        answer
    }

    #[test]
    fn a_door_carries_its_own_request_to_the_socket_and_refuses_another_sites() {
        let dir = tempfile::tempdir().expect("a folder");
        let socket = dir.path().join("5173.sock");
        echo_on(&socket);
        let port = free_port();
        let door = Door::open(port, &socket).expect("the door opens");
        let host = format!("Host: 127.0.0.1:{port}");
        let carried = ask(
            port,
            &["GET /hello HTTP/1.1", &host, "Cookie: grafana=secret"],
        );
        assert!(carried.starts_with("HTTP/1.1 200 OK\r\n"), "{carried}");
        assert!(carried.contains("GET /hello HTTP/1.1"), "{carried}");
        assert!(!carried.contains("grafana"), "{carried}");
        assert!(!carried.contains("Set-Cookie: session"), "{carried}");
        assert!(
            carried.contains(&format!("Set-Cookie: purlis-door-{port}=")),
            "{carried}"
        );
        let refused = ask(port, &["GET / HTTP/1.1", "Host: evil.example"]);
        assert!(refused.starts_with("HTTP/1.1 421 "), "{refused}");
        let other = ask(
            port,
            &["GET / HTTP/1.1", &host, "Origin: https://evil.example"],
        );
        assert!(other.starts_with("HTTP/1.1 403 "), "{other}");
        drop(door);
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            TcpListener::bind(("127.0.0.1", port)).is_ok(),
            "its port was let go"
        );
    }

    #[test]
    fn a_door_never_opens_on_a_port_something_listens_on() {
        let taken = TcpListener::bind(("127.0.0.1", 0)).expect("a port");
        let port = taken.local_addr().expect("bound").port();
        assert!(in_use(port));
        let dir = tempfile::tempdir().expect("a folder");
        assert_eq!(
            Door::open(port, &dir.path().join("x.sock"))
                .map(|_| ())
                .map_err(|why| why.kind()),
            Err(io::ErrorKind::AddrInUse)
        );
    }

    #[test]
    fn a_door_says_so_where_the_chats_server_is_not_running() {
        let dir = tempfile::tempdir().expect("a folder");
        let port = free_port();
        let _door = Door::open(port, &dir.path().join("none.sock")).expect("the door opens");
        let answer = ask(
            port,
            &["GET / HTTP/1.1", &format!("Host: localhost:{port}")],
        );
        assert!(answer.starts_with("HTTP/1.1 502 "), "{answer}");
    }
}
