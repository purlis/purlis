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
                "HTTP/1.1 302 Found\r\nLocation: http://localhost:{port}/success?id_token=t\r\n\r\n"
            ),
            "HTTP/1.1 200 OK\r\n\r\ndone".to_owned(),
        ]
    });

    assert_eq!(deliver_on(port, &redirect_target()), Ok(()));

    let heard = heard.join().unwrap();
    assert!(heard[0].starts_with(&format!("GET {} HTTP/1.1\r\n", redirect_target())));
    assert!(heard[1].starts_with("GET /success?id_token=t HTTP/1.1\r\n"));
}

/// opencode's login answers the redirect itself.
#[test]
fn opencodes_login_answers_the_redirect_itself() {
    let (port, heard) = login_answering(|_| vec!["HTTP/1.1 200 OK\r\n\r\nsigned in".to_owned()]);

    assert_eq!(deliver_on(port, &redirect_target()), Ok(()));

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

        let why = deliver_on(port, &redirect_target()).unwrap_err();

        assert!(!why.contains("the-secret"), "{why}");
        assert!(!why.contains("example.com"), "{why}");
        heard.join().unwrap();
    }
}

/// **Nothing but a redirect is carried out on a runner**: whatever a link asks, the login's
/// listener hears only [`PATH`] and a query, in one line.
#[test]
fn nothing_but_a_redirect_is_delivered() {
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
        assert!(deliver_on(1, target).is_err(), "{target}");
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

    assert_eq!(
        deliver_on(port, &redirect_target()),
        Err("no sign-in is waiting on the runner".to_owned())
    );
}
