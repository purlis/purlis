// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a failed MCP server start comes to, as the app watches its run (qw381).

use super::*;

fn watched(frames: &[Frame]) -> Seen {
    let (mut watch, seen) = Watch::new(Box::new(std::io::sink()));
    for frame in frames {
        let mut line = serde_json::to_vec(frame).unwrap();
        line.push(b'\n');
        // Written in two halves, as a pipe may hand it on.
        let half = line.len() / 2;
        watch.write_all(&line[..half]).unwrap();
        watch.write_all(&line[half..]).unwrap();
    }
    drop(watch);
    Arc::try_unwrap(seen).unwrap().into_inner().unwrap()
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// T-3: a server that answered is no failure, whatever its status later.
#[test]
fn a_server_that_spoke_did_not_fail_to_start() {
    let seen = watched(&[
        Frame::Started,
        Frame::Stdout(b64(b"{}\n")),
        Frame::Stderr(b64(b"bye\n")),
        Frame::Exit(1),
    ]);
    assert_eq!(seen.failure(), None);
}

#[test]
fn a_refused_start_fails_with_the_apps_sentence() {
    let seen = watched(&[Frame::Refused {
        why: "chat 7 is not one this app has open".into(),
        code: 1,
    }]);
    assert_eq!(
        seen.failure().as_deref(),
        Some("chat 7 is not one this app has open")
    );
}

/// The server's own last words, on one line.
#[test]
fn a_server_that_ended_before_it_spoke_fails_with_what_it_said() {
    let seen = watched(&[
        Frame::Started,
        Frame::Note("purlis's own note".into()),
        Frame::Stderr(b64(
            b"starting\nError: connect ECONNREFUSED 10.0.0.1:9200\n",
        )),
        Frame::Exit(1),
    ]);
    assert_eq!(
        seen.failure().as_deref(),
        Some("starting | Error: connect ECONNREFUSED 10.0.0.1:9200")
    );
    let silent = watched(&[Frame::Started, Frame::Exit(3)]);
    assert_eq!(
        silent.failure().as_deref(),
        Some("it ended with status 3 before it answered")
    );
}

/// A run still going has not failed.
#[test]
fn a_run_that_has_not_ended_has_not_failed() {
    assert_eq!(watched(&[Frame::Started]).failure(), None);
}

/// Bounded to its last characters, and nothing in it can draw a line of its own.
#[test]
fn a_long_error_keeps_its_end_and_escapes_what_has_no_glyph() {
    let long = format!("{}the end\u{1b}[31m", "x".repeat(2 * ERROR_SHOWN_AT_MOST));
    let shown = error_shown(&long);
    assert!(shown.starts_with('…'), "{shown}");
    assert!(shown.contains("the end"));
    assert!(!shown.contains('\u{1b}'));
    assert!(shown.chars().count() <= ERROR_SHOWN_AT_MOST + 8, "{shown}");
    assert_eq!(
        error_shown(&shown),
        shown,
        "shown once is shown the same again"
    );
}

/// A value this process read from a vault is masked wherever it stands in the error.
#[test]
fn a_value_this_process_read_is_masked_in_the_error() {
    let value = "qw381 plain vault words 5be0";
    crate::secrets::seen::remember(value);
    assert_eq!(
        error_shown(&format!("login as {value} failed")),
        "login as *** failed"
    );
}

/// The second run is handed what the first was sent, and once the server has spoken nothing is
/// kept for another.
#[test]
fn the_stdin_a_first_run_took_is_kept_until_the_server_speaks() {
    #[derive(Clone, Default)]
    struct Sent(Arc<Mutex<Vec<u8>>>);
    impl Write for Sent {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let inputs = |sent: &Sent| -> Vec<u8> {
        let text = String::from_utf8(sent.0.lock().unwrap().clone()).unwrap();
        let mut got = Vec::new();
        for line in text.lines() {
            if let super::super::brokered::Input::Stdin(bytes) = serde_json::from_str(line).unwrap()
            {
                use base64::Engine as _;
                got.extend(
                    base64::engine::general_purpose::STANDARD
                        .decode(bytes)
                        .unwrap(),
                );
            }
        }
        got
    };
    let feed = Feed::new(
        Some(Box::new(std::io::Cursor::new(b"hello\n".to_vec()))),
        true,
    );
    let first = Sent::default();
    feed.attach(Box::new(first.clone()));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !feed.lock().closed && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(inputs(&first), b"hello\n");
    assert!(feed.replayable());
    let second = Sent::default();
    feed.attach(Box::new(second.clone()));
    assert_eq!(inputs(&second), b"hello\n");
    assert!(
        String::from_utf8(second.0.lock().unwrap().clone())
            .unwrap()
            .contains("stdin_closed"),
        "the end of stdin is handed on too"
    );
    feed.spoke();
    assert!(!feed.replayable());
}
