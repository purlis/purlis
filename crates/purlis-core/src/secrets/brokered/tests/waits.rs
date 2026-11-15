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
    asked_with(vault, PATIENCE, app)
}

/// [`asked_of`], with the asker waiting on the app with `patience`.
fn asked_with(
    vault: &str,
    patience: Patience,
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
        patience,
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

/// The patience of a `secret exec` in a chat whose start named `harness` (`PURLIS_HARNESS`).
fn patience_on(harness: Option<&str>) -> Patience {
    let project = keyring_project();
    let env = harness.map_or_else(
        || Env::of(&[]),
        |name| Env::of(&[(crate::hookwire::HARNESS_ENV, name)]),
    );
    patience_of(&Ctx::new(project.path(), env))
}

#[test]
fn secret_exec_waits_on_the_app_a_second_past_its_harness_hold_or_three_minutes() {
    // Q244, as amended: the hold is three quarters of the harness's command timeout, so the
    // command ends saying why before the harness kills it, and the asker waits a second past it,
    // so the app's still-open answer comes first.
    assert_eq!(
        patience_on(Some("claude-code")).deadline,
        Duration::from_secs(91)
    );
    assert_eq!(
        patience_on(Some("opencode")).deadline,
        Duration::from_secs(91)
    );
    assert_eq!(
        patience_on(Some("codex")).deadline,
        Duration::from_millis(8_500)
    );
    // Where the harness is not known, three minutes.
    assert_eq!(patience_on(None).deadline, Duration::from_secs(180));
    assert_eq!(
        patience_on(Some("not-a-harness")).deadline,
        Duration::from_secs(180)
    );
    // It says what it waits on after three seconds, on every harness.
    for harness in [Some("claude-code"), Some("opencode"), Some("codex"), None] {
        assert_eq!(
            patience_on(harness).slow_after,
            Duration::from_secs(3),
            "{harness:?}"
        );
    }
}

#[test]
fn the_wait_and_its_deadline_are_said_as_a_waiting_command_says_them() {
    // Lowercase clauses in the present tense, as `askwait` says a wait on the person; the wait
    // names the deadline it uses, and the error says what the person answers, never where to look.
    let claude_code = of_the_app(Some(crate::harness::Harness::ClaudeCode));
    let kc = Step::Keyring("kc".into());
    let gh = ["gh".to_owned(), "pr".to_owned(), "list".to_owned()];
    assert_eq!(
        waiting_on(&kc, &gh, claude_code),
        "waiting on the app that started this chat while it reads vault 'kc' from the system \
         keyring, before it runs `gh pr list`, for up to 91 seconds"
    );
    assert_eq!(
        stopped_waiting(None, &kc, claude_code).to_string(),
        "stopped waiting on the app that started this chat: no answer for 91 seconds while it \
         reads vault 'kc' from the system keyring, so nothing ran; the person may be answering \
         the system keyring's prompt or an ask for vault 'kc'; retry once answered"
    );
    assert_eq!(
        stopped_waiting(
            Some("waiting on you in Asks"),
            &Step::Network("op".into()),
            of_the_app(None),
        )
        .to_string(),
        "stopped waiting on the app that started this chat: nothing more for 3 minutes since it \
         said: waiting on you in Asks, so nothing ran; the person may be answering 1Password's \
         unlock or an ask for vault 'op'; retry once answered"
    );
    assert_eq!(
        stopped_waiting(None, &Step::TokenMint("gh".into()), claude_code).to_string(),
        "stopped waiting on the app that started this chat: no answer for 91 seconds while it \
         mints a GitHub App's token for vault 'gh', over the network, so nothing ran; the person \
         may be answering an ask for vault 'gh'; retry once answered"
    );
}

#[test]
fn once_the_command_has_started_it_runs_past_the_deadline_and_ends_with_its_own_status() {
    // The deadline is a wait on the app before the child starts; a command that runs long, in
    // silence, is not cut off and nothing is said about waiting.
    let (code, said, took) = asked_of("kc", |mut app| {
        let _ = send(&mut app, &Frame::Started);
        std::thread::sleep(PATIENCE.deadline * 2);
        let _ = send(&mut app, &Frame::Exit(4));
    });
    assert_eq!(code, Some(4), "{said:?}");
    assert!(said.is_empty(), "no wait line: {said:?}");
    assert!(
        took >= PATIENCE.deadline * 2,
        "it waited for the exit: {took:?}"
    );
}

#[test]
fn an_ask_the_app_holds_ends_with_its_still_open_answer_not_the_askers_deadline() {
    // Q244, as amended: the app sends its note as it raises the ask and holds the ask from a
    // moment later, so its still-open answer (75) comes a hold after the note and a little more.
    // The asker, waiting as it would past that hold, hears it rather than stopping with 1.
    let hold = PATIENCE.deadline;
    let raising = Duration::from_millis(50);
    let patience = Patience {
        deadline: deadline_past(hold),
        ..PATIENCE
    };
    let (code, said, _) = asked_with("kc", patience, move |mut app| {
        let _ = send(&mut app, &Frame::Note("waiting on you in Asks".into()));
        std::thread::sleep(raising + hold);
        let _ = send(
            &mut app,
            &Frame::Refused {
                why: "the ask is still open".into(),
                code: 75,
            },
        );
    });
    assert_eq!(code, Some(75), "{said:?}");
    assert!(
        said.last()
            .is_some_and(|s| line(s) == "the ask is still open"),
        "{said:?}"
    );
}

#[test]
fn a_wait_of_a_fraction_of_a_second_names_it_to_the_tenth() {
    let codex = of_the_app(Some(crate::harness::Harness::Codex));
    let wait = waiting_on(&Step::Keyring("kc".into()), &["ls".to_owned()], codex);
    assert!(wait.ends_with("for up to 8.5 seconds"), "{wait}");
}
