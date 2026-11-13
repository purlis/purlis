// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`secret exec` never waits on the app in silence** (R-356, Q232): where the app's first
//! frame is slow, the asker says what the app is waiting on (the keychain, a GitHub App's token
//! mint, the network), and where it never comes, it stops at its deadline naming that step.
//!
//! The app is a thread on the other end of an in-memory connection, so nothing binds a socket.

use super::*;

/// A project whose vault `kc` is read from the system keyring, and `op` from 1Password.
fn keyring_project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("vaults.json"),
        serde_json::json!({ "vaults": {
            "kc": {"provider": "keyring", "config": {}, "persona": "devops"},
            "op": {"provider": "1password", "config": {"vault": "Team"}, "persona": "devops"},
        }})
        .to_string(),
    )
    .unwrap();
    tmp
}

/// Short enough for a test, in the same proportions as the app's.
const PATIENCE: Patience = Patience {
    slow_after: Duration::from_millis(100),
    deadline: Duration::from_millis(900),
};

/// What `secret exec` on `vault` printed, and its status, against an app that does `app` with
/// its end of the connection once it has read the ask.
fn asked_of(
    vault: &str,
    app: impl FnOnce(std::os::unix::net::UnixStream) + Send + 'static,
) -> (Option<i32>, Vec<Say>, Duration) {
    let project = keyring_project();
    let ctx = Ctx::new(project.path(), Env::of(&[]));
    let req = Request {
        vault: vault.into(),
        env: vec!["X=TOKEN".into()],
        ..Request::default()
    };
    let (ours, theirs) = std::os::unix::net::UnixStream::pair().unwrap();
    let app = std::thread::spawn(move || {
        let mut ask = String::new();
        let _ = std::io::BufReader::new(&theirs).read_line(&mut ask);
        app(theirs);
    });
    let mut rec = Rec::default();
    let began = std::time::Instant::now();
    let code = converse_within(
        &ctx,
        &req,
        &["gh".to_owned(), "pr".to_owned(), "list".to_owned()],
        7,
        None,
        ours,
        &mut rec,
        PATIENCE,
    );
    let took = began.elapsed();
    drop(app);
    (code, rec.said, took)
}

fn line(say: &Say) -> String {
    match say {
        Say::Info(m) | Say::Ok(m) | Say::Warn(m) | Say::Err(m) => m.to_string(),
    }
}

#[test]
fn purlis_secret_exec_says_what_it_waits_on_and_stops_at_its_deadline() {
    // A first frame that comes late: the step is said before it, once.
    let (code, said, _) = asked_of("kc", |mut app| {
        std::thread::sleep(Duration::from_millis(400));
        let _ = send(&mut app, &Frame::Note("the frame".into()));
        let _ = send(&mut app, &Frame::Exit(0));
    });
    assert_eq!(code, Some(0), "{said:?}");
    let lines: Vec<String> = said.iter().map(line).collect();
    assert_eq!(lines.len(), 2, "the step, then the frame: {lines:?}");
    assert!(
        matches!(said[0], Say::Info(_)) && lines[0].contains("the system keyring"),
        "the step it waits on comes first: {lines:?}"
    );
    assert!(
        lines[0].contains("vault 'kc'") && lines[0].contains("`gh pr list`"),
        "{lines:?}"
    );
    assert_eq!(lines[1], "the frame");

    // A 1Password vault waits on the network.
    let (_, said, _) = asked_of("op", |mut app| {
        std::thread::sleep(Duration::from_millis(400));
        let _ = send(&mut app, &Frame::Exit(0));
    });
    assert!(
        said.first()
            .is_some_and(|s| line(s).contains("the network")),
        "{said:?}"
    );

    // A first frame that comes at once: nothing is said about waiting.
    let (code, said, _) = asked_of("kc", |mut app| {
        let _ = send(&mut app, &Frame::Exit(3));
    });
    assert_eq!((code, said.len()), (Some(3), 0), "{said:?}");

    // An app that never answers: it stops at its deadline, naming the step, and runs nothing.
    let (code, said, took) = asked_of("kc", |app| {
        std::thread::sleep(Duration::from_secs(5));
        drop(app);
    });
    assert!(
        took < Duration::from_secs(3),
        "it stopped at its deadline: {took:?}"
    );
    assert!(took >= PATIENCE.deadline, "not before it: {took:?}");
    assert_eq!(code, Some(1), "{said:?}");
    let last = said.last().expect("it said why it stopped");
    assert!(
        matches!(last, Say::Err(_)) && line(last).contains("the system keyring"),
        "the error names the step: {said:?}"
    );

    // A note, then nothing: the deadline counts again from the note, and the error says what the
    // app last said it waits on.
    let (code, said, took) = asked_of("kc", |mut app| {
        let _ = send(&mut app, &Frame::Note("waiting on you in Asks".into()));
        std::thread::sleep(Duration::from_secs(5));
        drop(app);
    });
    assert!(took < Duration::from_secs(3), "{took:?}");
    assert_eq!(code, Some(1), "{said:?}");
    assert!(
        said.last()
            .is_some_and(|s| line(s).contains("waiting on you in Asks")),
        "{said:?}"
    );
}
