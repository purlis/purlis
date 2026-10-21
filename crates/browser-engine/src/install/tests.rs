// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::io::{Cursor, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use super::{Fetch, Heard, Https};

/// What a listener heard: `(url, status, answered)` for each request.
type Hops = Arc<Mutex<Vec<(String, Option<u16>, bool)>>>;

fn listening() -> (Https, Hops) {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let into = Arc::clone(&heard);
    let https = Https::default().listening(Arc::new(move |hop| {
        into.lock()
            .unwrap()
            .push((hop.url.to_owned(), hop.status, hop.answered));
    }));
    (https, heard)
}

fn body(https: &Https) -> Heard<Cursor<Vec<u8>>> {
    Heard {
        body: Cursor::new(b"the manifest".to_vec()),
        heard: Some((
            https.clone(),
            "https://a.test/m".to_owned(),
            200,
            Instant::now(),
        )),
        whole: false,
    }
}

/// A loopback server answering each connection in turn with the next of `answers` (made
/// knowing its port), each after its wait; every connection stays open, silent, until the
/// test ends.
fn serving(answers: impl FnOnce(u16) -> Vec<(Duration, String)>) -> (u16, mpsc::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let answers = answers(port);
    let (done, ended) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        let mut open = Vec::new();
        for (wait, answer) in answers {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request);
            std::thread::sleep(wait);
            let _ = stream.write_all(answer.as_bytes());
            open.push(stream);
        }
        let _ = ended.recv();
    });
    (port, done)
}

fn plain(total: Duration) -> (Https, Hops) {
    let (https, heard) = listening();
    (
        Https {
            total,
            plain: true,
            ..https
        },
        heard,
    )
}

#[test]
fn a_body_that_stops_mid_way_is_given_up_on_within_the_total_time() {
    let (port, _done) = serving(|_| {
        vec![(
            Duration::ZERO,
            "HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\nthe first".to_owned(),
        )]
    });
    let (https, heard) = plain(Duration::from_secs(2));
    let url = format!("http://127.0.0.1:{port}/engine-manifest.json");
    let started = Instant::now();
    let mut body = https.fetch(&url).expect("the headers came");
    let mut bytes = Vec::new();
    assert!(body.read_to_end(&mut bytes).is_err());
    drop(body);
    let took = started.elapsed();
    assert!(took < Duration::from_secs(10), "given up on after {took:?}");
    assert_eq!(*heard.lock().unwrap(), [(url, Some(200), false)]);
}

#[test]
fn redirects_share_one_total_time() {
    // Each answer a redirect to the same server, a second after it was asked.
    let (port, _done) = serving(|port| {
        (1..=5)
            .map(|n| {
                (
                    Duration::from_secs(1),
                    format!(
                        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port}/{n}\r\n\
                         Content-Length: 0\r\nConnection: close\r\n\r\n"
                    ),
                )
            })
            .collect()
    });
    let (https, _) = plain(Duration::from_secs(2));
    let started = Instant::now();
    let got = https.fetch(&format!("http://127.0.0.1:{port}/0"));
    let took = started.elapsed();
    assert!(got.is_err());
    assert!(
        took < Duration::from_millis(3500),
        "five redirects a second apart took {took:?} under a two-second limit"
    );
}

#[test]
fn a_body_read_to_its_end_is_heard_as_answered_once_it_is_dropped() {
    let (https, heard) = listening();
    let mut whole = body(&https);
    let mut text = String::new();
    whole.read_to_string(&mut text).unwrap();
    assert!(
        heard.lock().unwrap().is_empty(),
        "heard only once it is done with"
    );
    drop(whole);
    assert_eq!(
        *heard.lock().unwrap(),
        [("https://a.test/m".to_owned(), Some(200), true)]
    );
}

#[test]
fn a_body_left_unread_is_heard_as_not_answered() {
    let (https, heard) = listening();
    let mut part = body(&https);
    part.read_exact(&mut [0_u8; 3]).unwrap();
    drop(part);
    assert_eq!(
        *heard.lock().unwrap(),
        [("https://a.test/m".to_owned(), Some(200), false)]
    );
}

/// #2207: a download says how long its body is where its server says, so its progress can be
/// shown as a share of it; a server that does not say gives no length.
#[test]
fn a_download_says_how_long_its_body_is_where_its_server_says() {
    let (port, _done) = serving(|_| {
        vec![
            (
                Duration::ZERO,
                "HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\nthe archive".to_owned(),
            ),
            (
                Duration::ZERO,
                "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nno length".to_owned(),
            ),
        ]
    });
    let (https, _) = plain(Duration::from_secs(10));
    let url = format!("http://127.0.0.1:{port}/archive.zip");

    let (mut body, length) = https.fetch_sized(&url).unwrap();
    let mut read = String::new();
    body.read_to_string(&mut read).unwrap();
    assert_eq!((read.as_str(), length), ("the archive", Some(11)));
    drop(body);

    let (_, length) = https.fetch_sized(&url).unwrap();
    assert_eq!(length, None);
}

/// A download that does not know its length says none.
#[test]
fn a_download_that_knows_no_length_says_none() {
    struct Bytes;
    impl Fetch for Bytes {
        fn fetch(&self, _url: &str) -> std::io::Result<Box<dyn Read + '_>> {
            Ok(Box::new(Cursor::new(b"bytes".to_vec())))
        }
    }
    let (_, length) = Bytes.fetch_sized("https://a.test/x").unwrap();
    assert_eq!(length, None);
}
