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
fn a_request_another_page_of_this_machine_sent_never_uses_the_jar() {
    let jar = |token: Option<&str>| match token {
        Some(TOKEN) => vec!["session=abc".to_owned()],
        _ => Vec::new(),
    };
    let sent = |site: &str| {
        judge(
            &head(&[
                "GET /api/me HTTP/1.1",
                "Host: 127.0.0.1:5173",
                &format!("Sec-Fetch-Site: {site}"),
                &format!("Cookie: purlis-door-5173={TOKEN}"),
            ]),
            5173,
            &jar,
        )
        .expect("carried")
    };
    for site in ["same-site", "cross-site", "SAME-SITE"] {
        let carried = sent(site);
        assert!(!carried.jarred, "{site}");
        assert_eq!(carried.token, None, "{site}");
        assert!(
            !carried.head.contains("session=abc"),
            "{site}: {}",
            carried.head
        );
    }
    // Its own page, or the person typing the address, uses it.
    for site in ["same-origin", "none"] {
        let carried = sent(site);
        assert!(carried.jarred, "{site}");
        assert!(
            carried.head.contains("\r\nCookie: session=abc\r\n"),
            "{site}"
        );
    }
}

#[test]
fn a_jar_keeps_no_cookie_longer_than_a_browser_does_and_holds_its_own_bytes() {
    let mut jar = Jar::default();
    jar.keep(TOKEN, &format!("huge={}", "x".repeat(COOKIE_MOST_BYTES)));
    assert!(
        jar.of(Some(TOKEN)).is_empty(),
        "a cookie past the longest kept"
    );
    for n in 0..100 {
        jar.keep(TOKEN, &format!("c{n}={}", "y".repeat(2000)));
    }
    let held = jar.of(Some(TOKEN));
    let bytes: usize = held.iter().map(|cookie| cookie.len() - 1).sum();
    assert!(bytes <= JAR_MOST_BYTES, "{bytes}");
    // The newest are kept, the oldest let go.
    assert!(held.last().is_some_and(|cookie| cookie.starts_with("c99=")));
    assert!(!held.iter().any(|cookie| cookie.starts_with("c0=")));
}

#[test]
fn a_chat_not_served_is_told_in_words_that_agree_with_its_ports() {
    let one = unserved_said(&[5173]).expect("said");
    assert!(one.contains("serve on port 5173,"), "{one}");
    assert!(one.contains("does not serve on it this run"), "{one}");
    assert!(one.contains("already holds it,"), "{one}");
    let two = unserved_said(&[3000, 5173]).expect("said");
    assert!(two.contains("serve on ports 3000 and 5173,"), "{two}");
    assert!(
        two.contains("does not serve on any of them this run"),
        "{two}"
    );
    assert!(two.contains("already holds each of them,"), "{two}");
    assert_eq!(unserved_said(&[]), None);
    let failed = unservable_said(&[5173], "a folder is where its socket goes").expect("said");
    assert!(
        failed.contains("could not open the way to its server: a folder is where its socket goes."),
        "{failed}"
    );
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

#[test]
fn a_servers_bound_name_is_read_up_to_its_first_nul() {
    // macOS keeps the whole address a server bound, padding included.
    assert_eq!(
        up_to_nul(Path::new("/run/serve/5173.sock\0\0\0")),
        Path::new("/run/serve/5173.sock")
    );
    assert_eq!(
        up_to_nul(Path::new("/run/serve/5173.sock")),
        Path::new("/run/serve/5173.sock")
    );
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
    fn a_door_carries_only_to_the_socket_bound_at_its_own_path() {
        let dir = tempfile::tempdir().expect("a folder");
        let elsewhere = tempfile::tempdir().expect("another folder");
        let other = elsewhere.path().join("other.sock");
        echo_on(&other);
        let socket = dir.path().join("5173.sock");
        std::os::unix::fs::symlink(&other, &socket).expect("a link in the socket's place");
        let port = free_port();
        let _door = Door::open(port, &socket).expect("the door opens");
        let answer = ask(
            port,
            &["GET / HTTP/1.1", &format!("Host: localhost:{port}")],
        );
        assert!(answer.starts_with("HTTP/1.1 502 "), "{answer}");
        std::fs::remove_file(&socket).expect("the link taken away");
        std::fs::hard_link(&other, &socket).expect("a second name for the other socket");
        let answer = ask(
            port,
            &["GET / HTTP/1.1", &format!("Host: localhost:{port}")],
        );
        assert!(answer.starts_with("HTTP/1.1 502 "), "{answer}");
    }

    #[test]
    fn a_door_carries_only_to_the_folder_it_opened_on() {
        let base = tempfile::tempdir().expect("a folder");
        let folder = base.path().join("serve");
        std::fs::create_dir(&folder).expect("the chat's folder");
        let socket = folder.join("5173.sock");
        let port = free_port();
        let _door = Door::open(port, &socket).expect("the door opens");
        // A folder put in its place since, with a server at the same name, is not the chat's.
        std::fs::rename(&folder, base.path().join("moved")).expect("moved away");
        std::fs::create_dir(&folder).expect("another folder");
        echo_on(&socket);
        let answer = ask(
            port,
            &["GET / HTTP/1.1", &format!("Host: localhost:{port}")],
        );
        assert!(answer.starts_with("HTTP/1.1 502 "), "{answer}");
    }

    /// **The two backends a door carries to** (#3316): the chat's own socket, or slots its
    /// server dialled in. Each test below is run against both.
    enum Served {
        Socket(tempfile::TempDir),
        Dialled(Arc<Slots>),
    }

    /// A door on a free port to a server that answers with `answer`, through `backend`
    /// (`"socket"` or `"dialled"`): the door, its port, and what it is served by.
    fn door_to(
        backend: &str,
        answer: impl Fn(UnixStream) + Send + Sync + 'static,
    ) -> (Door, u16, Served) {
        let port = free_port();
        let answer = Arc::new(answer);
        if backend == "socket" {
            let dir = tempfile::tempdir().expect("a folder");
            let socket = dir.path().join(format!("{port}.sock"));
            let listener = UnixListener::bind(&socket).expect("the server's socket");
            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    let answer = Arc::clone(&answer);
                    std::thread::spawn(move || answer(stream));
                }
            });
            let door = Door::open(port, &socket).expect("the door opens");
            (door, port, Served::Socket(dir))
        } else {
            let (door, slots) = Door::dialled(port).expect("the door opens");
            for _ in 0..4 {
                dial(&slots, Arc::clone(&answer));
            }
            (door, port, Served::Dialled(slots))
        }
    }

    /// As the shim does: one slot parked, which on its go is handed to `answer`, and another
    /// dialled in its place.
    fn dial(slots: &Arc<Slots>, answer: Arc<dyn Fn(UnixStream) + Send + Sync>) {
        let (parked, mut server) = UnixStream::pair().expect("a pair");
        if slots.park(parked).is_err() {
            return;
        }
        let slots = Arc::clone(slots);
        std::thread::spawn(move || {
            let mut said = [0u8; PARKED.len() + GO.len()];
            if server.read_exact(&mut said).is_err() || said != format!("{PARKED}{GO}").as_bytes() {
                return;
            }
            dial(&slots, Arc::clone(&answer));
            answer(server);
        });
    }

    /// Answers a request with what it was sent, and a cookie.
    fn echo(mut stream: UnixStream) {
        let mut buf = vec![0u8; 8192];
        let n = stream.read(&mut buf).unwrap_or(0);
        let said = String::from_utf8_lossy(&buf[..n]).into_owned();
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nSet-Cookie: session=abc\r\nContent-Length: {}\r\n\r\n{said}",
            said.len()
        );
    }

    #[test]
    fn both_backends_carry_their_own_request_and_refuse_another_sites() {
        for backend in ["socket", "dialled"] {
            let (door, port, _served) = door_to(backend, echo);
            let host = format!("Host: 127.0.0.1:{port}");
            let carried = ask(
                port,
                &["GET /hello HTTP/1.1", &host, "Cookie: grafana=secret"],
            );
            assert!(
                carried.starts_with("HTTP/1.1 200 OK\r\n"),
                "{backend}: {carried}"
            );
            assert!(
                carried.contains("GET /hello HTTP/1.1"),
                "{backend}: {carried}"
            );
            assert!(!carried.contains("grafana"), "{backend}: {carried}");
            let refused = ask(port, &["GET / HTTP/1.1", "Host: evil.example"]);
            assert!(refused.starts_with("HTTP/1.1 421 "), "{backend}: {refused}");
            let other = ask(
                port,
                &["GET / HTTP/1.1", &host, "Origin: https://evil.example"],
            );
            assert!(other.starts_with("HTTP/1.1 403 "), "{backend}: {other}");
            drop(door);
        }
    }

    #[test]
    fn both_backends_keep_the_servers_cookies_in_the_jar_and_hand_them_back() {
        for backend in ["socket", "dialled"] {
            let (_door, port, _served) = door_to(backend, echo);
            let host = format!("Host: localhost:{port}");
            let first = ask(port, &["GET / HTTP/1.1", &host]);
            let token = first
                .split(&format!("purlis-door-{port}="))
                .nth(1)
                .and_then(|rest| rest.get(..32))
                .expect("the door's own cookie")
                .to_owned();
            let cookie = format!("Cookie: purlis-door-{port}={token}");
            let second = ask(port, &["GET /again HTTP/1.1", &host, &cookie]);
            assert!(
                second.contains("Cookie: session=abc"),
                "{backend}: {second}"
            );
            assert!(!second.contains("purlis-door-"), "{backend}: {second}");
        }
    }

    #[test]
    fn both_backends_carry_an_upgrade_both_ways() {
        for backend in ["socket", "dialled"] {
            let (_door, port, _served) = door_to(backend, |mut stream: UnixStream| {
                let mut buf = vec![0u8; 8192];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
                );
                let mut word = [0u8; 4];
                if stream.read_exact(&mut word).is_ok() {
                    let _ = stream.write_all(&word);
                }
            });
            let mut client = TcpStream::connect(("127.0.0.1", port)).expect("the door answers");
            let lines = [
                "GET /hmr HTTP/1.1".to_owned(),
                format!("Host: localhost:{port}"),
                "Upgrade: websocket".to_owned(),
                "Connection: Upgrade".to_owned(),
            ];
            client
                .write_all(head(&lines.iter().map(String::as_str).collect::<Vec<_>>()).as_bytes())
                .expect("sent");
            let mut switched = vec![0u8; 4096];
            let n = client.read(&mut switched).expect("switched");
            let switched = String::from_utf8_lossy(&switched[..n]).into_owned();
            assert!(
                switched.starts_with("HTTP/1.1 101"),
                "{backend}: {switched}"
            );
            client.write_all(b"ping").expect("sent up");
            let mut back = [0u8; 4];
            client.read_exact(&mut back).expect("sent back");
            assert_eq!(&back, b"ping", "{backend}");
        }
    }

    #[test]
    fn both_backends_carry_one_request_a_connection() {
        for backend in ["socket", "dialled"] {
            let (_door, port, _served) = door_to(backend, echo);
            let host = format!("Host: localhost:{port}");
            let one = head(&["GET /one HTTP/1.1", &host]);
            let two = head(&["GET /two HTTP/1.1", &host]);
            let mut client = TcpStream::connect(("127.0.0.1", port)).expect("the door answers");
            client
                .write_all(format!("{one}{two}").as_bytes())
                .expect("sent");
            let mut answer = String::new();
            let _ = client.read_to_string(&mut answer);
            assert!(answer.contains("GET /one"), "{backend}: {answer}");
            assert!(!answer.contains("GET /two"), "{backend}: {answer}");
        }
    }

    #[test]
    fn a_dialled_door_with_no_slot_parked_answers_502_within_its_wait() {
        // A socket at the path a door to this port would carry to, which a dialled door never
        // reaches: it has slots alone.
        let dir = tempfile::tempdir().expect("a folder");
        let port = free_port();
        echo_on(&dir.path().join(format!("{port}.sock")));
        let (_door, _slots) = Door::dialled(port).expect("the door opens");
        let began = std::time::Instant::now();
        let answer = ask(
            port,
            &["GET / HTTP/1.1", &format!("Host: localhost:{port}")],
        );
        assert!(answer.starts_with("HTTP/1.1 502 "), "{answer}");
        assert!(began.elapsed() < SLOT_WITHIN + Duration::from_secs(2));
    }

    #[test]
    fn a_dialled_door_passes_over_a_slot_whose_server_has_gone() {
        let port = free_port();
        let (_door, slots) = Door::dialled(port).expect("the door opens");
        let (gone, server) = UnixStream::pair().expect("a pair");
        slots.park(gone).expect("parked");
        drop(server);
        dial(&slots, Arc::new(echo));
        let answer = ask(
            port,
            &["GET /live HTTP/1.1", &format!("Host: localhost:{port}")],
        );
        assert!(answer.starts_with("HTTP/1.1 200 OK"), "{answer}");
    }

    #[test]
    fn a_dialled_door_dropped_closes_every_parked_slot() {
        let port = free_port();
        let (door, slots) = Door::dialled(port).expect("the door opens");
        let (parked, mut server) = UnixStream::pair().expect("a pair");
        slots.park(parked).expect("parked");
        drop(door);
        let mut said = String::new();
        server.read_to_string(&mut said).expect("read to its end");
        assert_eq!(said, PARKED, "the slot was closed");
        let (more, _server) = UnixStream::pair().expect("a pair");
        assert!(
            slots.park(more).is_err(),
            "a closed door parks nothing more"
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

/// **Slots are capped** (#3316): one past [`SLOTS_MOST`] is handed back unparked, and a slot
/// whose server has gone makes room for another.
#[test]
fn slots_are_capped_and_a_gone_one_makes_room() {
    let slots = Slots::new(5173);
    let mut servers = Vec::new();
    for _ in 0..SLOTS_MOST {
        let (parked, server) = UnixStream::pair().expect("a pair");
        slots.park(parked).expect("parked under the cap");
        servers.push(server);
    }
    let (over, _server) = UnixStream::pair().expect("a pair");
    assert!(slots.park(over).is_err(), "a slot over the cap is refused");
    assert_eq!(slots.parked(), SLOTS_MOST);
    drop(servers.pop());
    let (room, _server) = UnixStream::pair().expect("a pair");
    assert!(slots.park(room).is_ok(), "a gone slot made room");
    slots.close();
    for mut server in servers {
        let mut said = String::new();
        server.read_to_string(&mut said).expect("read to its end");
        assert_eq!(said, PARKED, "closed");
    }
}
