// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::*;

const STATE: &str = "Zm9yLXRoaXMtc2lnbi1pbi1hbG9uZQ";

fn redirect_target() -> String {
    format!("/auth/callback?code=the-code&state={STATE}")
}

/// A folder of records where a login waits with [`STATE`].
fn waiting_here() -> (tempfile::TempDir, waiting::Waiting) {
    let dir = tempfile::tempdir().unwrap();
    let held = waiting::Waiting::record(dir.path(), STATE).unwrap();
    (dir, held)
}

/// The redirects a forward carried, and what it was told.
type Carried = Arc<Mutex<Vec<String>>>;
type Told = Receiver<Result<(), String>>;

/// A forward whose carrying is recorded and answered with `carried`.
fn forward(carried: Result<(), String>) -> (Forward, Carried, Told) {
    let targets = Arc::new(Mutex::new(Vec::new()));
    let (told, heard) = channel();
    let carry: Carry = {
        let targets = Arc::clone(&targets);
        Arc::new(move |target: &str| {
            targets.lock().unwrap().push(target.to_owned());
            carried.clone()
        })
    };
    let forward = Forward::open(0, STATE, carry, move |how| {
        let _ = told.send(how);
    })
    .expect("a free port is listened on");
    (forward, targets, heard)
}

/// `request`, sent to this machine's `at` on `port`, and all it was answered.
fn sent_to(at: &str, port: u16, request: &str) -> String {
    let mut stream = TcpStream::connect((at, port)).expect("it listens");
    stream.write_all(request.as_bytes()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut said = String::new();
    let _ = stream.read_to_string(&mut said);
    said
}

fn get(target: &str, host: &str) -> String {
    format!("GET {target} HTTP/1.1\r\nHost: {host}\r\nAccept: text/html\r\n\r\n")
}

/// **This sign-in's redirect is carried once, and the port closes with it**: the browser is told
/// it worked on a page of purlis's own, and nothing can connect after.
#[test]
fn the_redirect_is_carried_once_and_the_port_closes() {
    let (forward, targets, heard) = forward(Ok(()));
    let port = forward.port();

    let said = sent_to(
        "127.0.0.1",
        port,
        &get(&redirect_target(), &format!("localhost:{port}")),
    );

    assert!(said.starts_with("HTTP/1.1 200"), "{said}");
    assert!(said.contains("Go back to purlis"), "{said}");
    assert_eq!(*targets.lock().unwrap(), [redirect_target()]);
    assert_eq!(heard.recv_timeout(Duration::from_secs(10)), Ok(Ok(())));
    assert!(
        TcpStream::connect(("127.0.0.1", port)).is_err(),
        "the port stays closed after the redirect"
    );
}

/// **Only this sign-in's redirect reaches the runner**: another path, another method, a state
/// that is not this one's, two states, a request addressed to any other name, and a request with
/// no name at all are all answered here. The right one still goes through after them.
#[test]
fn only_this_sign_ins_redirect_reaches_the_runner() {
    let (forward, targets, heard) = forward(Ok(()));
    let port = forward.port();
    let host = format!("localhost:{port}");
    let wrong = [
        get("/cancel", &host),
        get("/success", &host),
        get("/auth/callback", &host),
        get("/auth/callback/../cancel?code=c", &host),
        get("/auth/callback?code=c&state=another", &host),
        get("/auth/callback?code=c", &host),
        get(&format!("/auth/callback?state=a&state={STATE}"), &host),
        get(&redirect_target(), "evil.example:1455"),
        get(
            &redirect_target(),
            &format!("localhost:{}", port.wrapping_add(1)),
        ),
        format!(
            "POST {} HTTP/1.1\r\nHost: {host}\r\nContent-Length: 0\r\n\r\n",
            redirect_target()
        ),
        format!("GET {} HTTP/1.1\r\n\r\n", redirect_target()),
        format!(
            "GET {} HTTP/1.1\r\nHost: {host}\r\nHost: evil.example\r\n\r\n",
            redirect_target()
        ),
    ];
    for request in &wrong {
        let said = sent_to("127.0.0.1", port, request);
        assert!(said.starts_with("HTTP/1.1 404"), "{request}: {said}");
    }
    assert!(targets.lock().unwrap().is_empty());

    sent_to("127.0.0.1", port, &get(&redirect_target(), &host));

    assert_eq!(*targets.lock().unwrap(), [redirect_target()]);
    assert_eq!(heard.recv_timeout(Duration::from_secs(10)), Ok(Ok(())));
}

/// A connection that says nothing, as a browser's speculative one, does not stop the redirect.
#[test]
fn a_connection_that_says_nothing_does_not_stop_the_redirect() {
    let (forward, targets, _heard) = forward(Ok(()));
    let port = forward.port();
    let silent = TcpStream::connect(("127.0.0.1", port)).unwrap();

    sent_to(
        "127.0.0.1",
        port,
        &get(&redirect_target(), &format!("127.0.0.1:{port}")),
    );

    drop(silent);
    assert_eq!(targets.lock().unwrap().len(), 1);
}

/// **Connections that say nothing never hold up the redirect**, however many there are.
#[test]
fn connections_that_say_nothing_do_not_hold_up_the_redirect() {
    let (forward, targets, heard) = forward(Ok(()));
    let port = forward.port();
    let silent: Vec<_> = (0..8)
        .map(|_| TcpStream::connect(("127.0.0.1", port)).unwrap())
        .collect();
    let started = std::time::Instant::now();

    sent_to(
        "127.0.0.1",
        port,
        &get(&redirect_target(), &format!("localhost:{port}")),
    );

    assert_eq!(heard.recv_timeout(Duration::from_secs(10)), Ok(Ok(())));
    assert!(started.elapsed() < ASKED_WITHIN, "{:?}", started.elapsed());
    assert_eq!(targets.lock().unwrap().len(), 1);
    drop(silent);
}

/// **A redirect that could not be carried says so**, to the browser and to the sign-in.
#[test]
fn a_redirect_not_carried_says_so() {
    let (forward, _, heard) = forward(Err("the link is down".to_owned()));
    let port = forward.port();

    let said = sent_to(
        "127.0.0.1",
        port,
        &get(&redirect_target(), &format!("localhost:{port}")),
    );

    assert!(said.starts_with("HTTP/1.1 502"), "{said}");
    assert!(!said.contains("the link is down"), "{said}");
    assert_eq!(
        heard.recv_timeout(Duration::from_secs(10)),
        Ok(Err("the link is down".to_owned()))
    );
}

/// **Closing it ends it at once**: no connection reaches it after, and nothing was carried.
#[test]
fn closing_it_closes_the_port() {
    let (forward, targets, _heard) = forward(Ok(()));
    let port = forward.port();

    drop(forward);

    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
    assert!(targets.lock().unwrap().is_empty());
}

/// **A taken port is said, and nothing listens**: on either of the loopback's addresses.
#[test]
fn a_taken_port_is_said() {
    for at in ["127.0.0.1:0", "[::1]:0"] {
        let Ok(held) = TcpListener::bind(at) else {
            continue;
        };
        let port = held.local_addr().unwrap().port();

        let taken = Forward::open(port, STATE, Arc::new(|_: &str| Ok(())), |_| ());

        assert_eq!(
            taken.map(|_| ()).unwrap_err().kind(),
            std::io::ErrorKind::AddrInUse,
            "{at}"
        );
    }
}

/// **A login found on one address is answered there alone**: a listener on the other hears
/// nothing, and the answer is said not to have reached a login.
#[test]
fn a_login_found_on_one_address_is_answered_there_alone() {
    let Ok(other) = TcpListener::bind("[::1]:0") else {
        return;
    };
    other.set_nonblocking(true).unwrap();
    let port = other.local_addr().unwrap().port();
    assert_eq!(listening_on(port), Some(IpAddr::from(Ipv6Addr::LOCALHOST)));
    // The look's own connection.
    while other.accept().is_err() {
        std::thread::sleep(Duration::from_millis(5));
    }

    assert!(
        asked(&addresses(Some(Ipv4Addr::LOCALHOST.into())), port, PATH).is_err(),
        "nothing listens on the address the login was found on"
    );
    assert!(other.accept().is_err(), "the other address heard nothing");
}

/// **A login's answer follows the socket's owner, not who answers** (Linux, where owners can be
/// read): a listener of another process group gets nothing, wherever the login was found.
#[cfg(target_os = "linux")]
#[test]
fn a_listener_of_another_group_is_never_answered() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    let ours: u32 = stat
        .rsplit_once(')')
        .unwrap()
        .1
        .split_whitespace()
        .nth(2)
        .unwrap()
        .parse()
        .unwrap();
    let here = IpAddr::from(Ipv4Addr::LOCALHOST);

    let of_group = |group| {
        answered_at(
            waiting::Taken {
                at: Some(here),
                group: Some(group),
            },
            port,
        )
    };
    assert_eq!(of_group(ours), vec![here]);
    // A group with no process in it holds no socket.
    assert_eq!(of_group(u32::MAX - 1), Vec::<IpAddr>::new());
    // Nothing recorded of a group: where it was found.
    assert_eq!(
        answered_at(
            waiting::Taken {
                at: Some(here),
                group: None
            },
            port
        ),
        vec![here]
    );
    drop(listener);
}

/// **A login on both addresses is answered on either.**
#[cfg(target_os = "linux")]
#[test]
fn a_login_on_both_addresses_owns_both() {
    let v4 = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = v4.local_addr().unwrap().port();
    let Ok(_v6) = TcpListener::bind(("::1", port)) else {
        return;
    };
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    let ours: u32 = stat
        .rsplit_once(')')
        .unwrap()
        .1
        .split_whitespace()
        .nth(2)
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        answered_at(
            waiting::Taken {
                at: None,
                group: Some(ours)
            },
            port
        ),
        LOOPBACK.to_vec()
    );
}

/// **It listens on the loopback alone**.
#[test]
fn it_listens_on_the_loopback_alone() {
    for listener in listening(0).unwrap() {
        assert!(listener.local_addr().unwrap().ip().is_loopback());
    }
}

/// A stand-in for a login's own listener: answers each connection in turn with the next of the
/// answers `answers` writes for its port, and hands back the requests it heard.
fn login_answering(
    answers: impl FnOnce(u16) -> Vec<String>,
) -> (u16, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let answers = answers(port);
    let heard = std::thread::spawn(move || {
        let mut heard = Vec::new();
        for answer in answers {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut piece = [0u8; 1024];
            while !request.windows(4).any(|end| end == b"\r\n\r\n") {
                let read = stream.read(&mut piece).unwrap();
                request.extend_from_slice(&piece[..read]);
            }
            heard.push(String::from_utf8(request).unwrap());
            stream.write_all(answer.as_bytes()).unwrap();
        }
        heard
    });
    (port, heard)
}

/// **Codex's login is answered as it asks**: it redirects to its own page, carrying a token, and
/// signs in only once that page is asked for. It is asked on the runner, and nothing of it
/// comes back.
#[test]
fn codexs_next_page_is_asked_on_the_runner_and_nothing_of_it_comes_back() {
    let (port, heard) = login_answering(|port| {
        vec![
            format!(
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port}/success?id_token=t\r\n\r\n"
            ),
            "HTTP/1.1 200 OK\r\n\r\ndone".to_owned(),
        ]
    });
    let (dir, _waiting) = waiting_here();

    assert_eq!(deliver_on(dir.path(), port, &redirect_target()), Ok(()));

    let heard = heard.join().unwrap();
    assert!(heard[0].starts_with(&format!("GET {} HTTP/1.1\r\n", redirect_target())));
    assert!(heard[1].starts_with("GET /success HTTP/1.1\r\n"));
    assert!(!heard[1].contains("id_token"));
}

/// A login that writes its next page by this machine's name is followed the same.
#[test]
fn a_next_page_written_by_the_machines_name_is_followed() {
    let (port, heard) = login_answering(|port| {
        vec![
            format!("HTTP/1.1 302 Found\r\nLocation: http://localhost:{port}/success\r\n\r\n"),
            "HTTP/1.1 200 OK\r\n\r\ndone".to_owned(),
        ]
    });
    let (dir, _waiting) = waiting_here();

    assert_eq!(deliver_on(dir.path(), port, &redirect_target()), Ok(()));

    assert_eq!(heard.join().unwrap().len(), 2);
}

/// opencode's login answers the redirect itself.
#[test]
fn opencodes_login_answers_the_redirect_itself() {
    let (port, heard) = login_answering(|_| vec!["HTTP/1.1 200 OK\r\n\r\nsigned in".to_owned()]);
    let (dir, _waiting) = waiting_here();

    assert_eq!(deliver_on(dir.path(), port, &redirect_target()), Ok(()));

    assert_eq!(heard.join().unwrap().len(), 1);
}

/// **What the login refuses comes back as a status alone**, and a redirect anywhere but its own
/// next page is not followed.
#[test]
fn a_refusal_comes_back_as_a_status_alone() {
    for answer in [
        "HTTP/1.1 400 Bad Request\r\n\r\nInvalid OAuth state the-secret",
        "HTTP/1.1 302 Found\r\nLocation: https://example.com/the-secret\r\n\r\n",
        "HTTP/1.1 302 Found\r\nLocation: http://localhost:1/success\r\n\r\n",
        "HTTP/1.1 302 Found\r\nLocation: /elsewhere\r\n\r\n",
    ] {
        let (port, heard) = login_answering(|_| vec![answer.to_owned()]);
        let (dir, _waiting) = waiting_here();

        let why = deliver_on(dir.path(), port, &redirect_target()).unwrap_err();

        assert!(!why.contains("the-secret"), "{why}");
        assert!(!why.contains("example.com"), "{why}");
        heard.join().unwrap();
    }
}

/// **Nothing but a redirect is carried out on a runner**: whatever a link asks, the login's
/// listener hears only [`PATH`] and a query, in one line.
#[test]
fn nothing_but_a_redirect_is_delivered() {
    let dir = tempfile::tempdir().unwrap();
    for target in [
        "/",
        "/cancel",
        "/success?id_token=t",
        "/auth/callback",
        "/auth/callback?code=c HTTP/1.1\r\nHost: evil",
        "/auth/callback?code=c\r\nX: y",
        "/auth/callback?code=c#fragment",
        "//evil.example/auth/callback?code=c",
        "http://evil.example/auth/callback?code=c",
    ] {
        assert!(!is_a_redirect(target), "{target}");
        assert!(deliver_on(dir.path(), 1, target).is_err(), "{target}");
    }
    assert!(!is_a_redirect(&format!(
        "/auth/callback?code={}",
        "x".repeat(MOST_TARGET)
    )));
    assert!(is_a_redirect(&redirect_target()));
}

/// With no login listening, it says so.
#[test]
fn with_no_login_listening_it_says_so() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let (dir, _waiting) = waiting_here();
    assert_eq!(
        deliver_on(dir.path(), port, &redirect_target()),
        Err("no sign-in is waiting on the runner".to_owned())
    );
}

/// **A redirect goes only to a login held here that waits with its own `state`**, and the port is
/// never reached otherwise: what listens there, another login or a chat, hears nothing of it.
#[test]
fn a_redirect_no_login_waits_for_reaches_nothing_on_the_runner() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let dir = tempfile::tempdir().unwrap();

    let refused = Err("no sign-in is waiting on the runner".to_owned());
    // Nothing waits at all: the login that was cancelled.
    assert_eq!(deliver_on(dir.path(), port, &redirect_target()), refused);
    // Another sign-in's login waits.
    let _other = waiting::Waiting::record(dir.path(), "another-sign-ins-state").unwrap();
    assert_eq!(deliver_on(dir.path(), port, &redirect_target()), refused);

    assert_eq!(
        listener.accept().map(|_| ()).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "nothing connected"
    );
}

/// **One answer goes to one login**: the same redirect again reaches nothing.
#[test]
fn a_login_waiting_takes_one_redirect() {
    let (port, heard) = login_answering(|_| vec!["HTTP/1.1 200 OK\r\n\r\n".to_owned()]);
    let (dir, _waiting) = waiting_here();

    assert_eq!(deliver_on(dir.path(), port, &redirect_target()), Ok(()));
    assert_eq!(heard.join().unwrap().len(), 1);
    assert_eq!(
        deliver_on(dir.path(), port, &redirect_target()),
        Err("no sign-in is waiting on the runner".to_owned())
    );
}

/// **A sign-in that ended carries nothing**, though its redirect was read just before.
#[test]
fn a_sign_in_that_ended_carries_nothing() {
    let asked: Carried = Arc::default();
    let carry: Carry = {
        let asked = Arc::clone(&asked);
        Arc::new(move |target: &str| {
            asked.lock().unwrap().push(target.to_owned());
            Ok(())
        })
    };

    let ended = AtomicBool::new(true);
    assert_eq!(carried_while_open(&ended, &carry, "/t"), None);
    assert!(asked.lock().unwrap().is_empty());

    let open = AtomicBool::new(false);
    assert_eq!(carried_while_open(&open, &carry, "/t"), Some(Ok(())));
    assert_eq!(asked.lock().unwrap().len(), 1);
}
