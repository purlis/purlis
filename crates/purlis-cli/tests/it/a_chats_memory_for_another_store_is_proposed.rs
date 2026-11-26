// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A sandboxed chat's `remember` or `todo` for a store that is not its own is proposed to the
//! person** (M74.1 T1.1-68, C1 and C2). It used to be refused with no one asked. Now the app is
//! handed it as `Ask::ProposeMemory`, and nothing is written here. Only the additive verbs go
//! this way: forget, edit, archive, move and a todo's close stay refused, in their old words.
//! That the app holds it and only Apply writes it is the app's test (`proposals/tests.rs`).
#![cfg(unix)]

use crate::support;

use std::path::PathBuf;
use std::process::{Output, Stdio};
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use purlis_core::brokered::Write;
use purlis_core::hookwire::{self, Answer, Ask, CHAT_ENV, Listener, SOCKET_ENV, TOKEN_ENV};
use purlis_core::memoryproposal::Target;

/// A project with workspaces `alpha` and `beta` and personas `steward` and `devops`.
fn a_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    for ws in ["alpha", "beta"] {
        std::fs::create_dir_all(root.join("workspaces").join(ws)).unwrap();
    }
    for persona in ["steward", "devops"] {
        std::fs::create_dir_all(root.join("personas").join(persona)).unwrap();
        std::fs::write(
            root.join("personas").join(persona).join("persona.md"),
            format!("---\nname: {persona}\n---\n# {persona}\n"),
        )
        .unwrap();
    }
    std::fs::create_dir_all(dir.path().join("home")).unwrap();
    dir
}

fn root(tmp: &tempfile::TempDir) -> PathBuf {
    tmp.path().join("project")
}

/// The app: every ask is handed to the receiver and answered with `answer`.
struct App {
    _reading: hookwire::Reading,
    token: hookwire::ChatToken,
    socket: PathBuf,
    asked: mpsc::Receiver<Ask>,
}

fn an_app(tmp: &tempfile::TempDir, answer: Answer) -> App {
    let socket = tmp.path().join("app").join("hooks.sock");
    let (tx, asked) = mpsc::channel();
    let tx = Mutex::new(tx);
    let listener = Listener::bind(tmp.path(), &socket).unwrap_or_else(|e| panic!("a socket: {e}"));
    let token = listener.tokens().issue_to_this_process(3).expect("a token");
    let reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, ask| {
            tx.lock().unwrap().send(ask.clone()).unwrap();
            answer.clone()
        }),
    );
    App {
        _reading: reading,
        token,
        socket,
        asked,
    }
}

fn held() -> Answer {
    Answer::Said {
        lines: vec![purlis_core::repocmd::Say::Done(
            "held for the person".into(),
        )],
        code: 0,
    }
}

impl App {
    /// The environment of chat 3 as the app starts it under the sandbox, in workspace `alpha`,
    /// as `steward`.
    fn chat_env(&self) -> Vec<(String, String)> {
        vec![
            (SOCKET_ENV.to_owned(), self.socket.display().to_string()),
            (CHAT_ENV.to_owned(), "3".to_owned()),
            (TOKEN_ENV.to_owned(), self.token.expose().to_owned()),
            (hookwire::SANDBOXED_ENV.to_owned(), "1".to_owned()),
            ("PURLIS_PERSONA".to_owned(), "steward".to_owned()),
            (
                purlis_core::active::WORKSPACE_ENV.to_owned(),
                "alpha".to_owned(),
            ),
        ]
    }

    fn proposed(&self) -> (Write, Target) {
        match self.asked.recv_timeout(Duration::from_secs(5)) {
            Ok(Ask::ProposeMemory(ask)) => {
                assert_eq!(ask.chat, 3);
                (ask.write, ask.target)
            }
            other => panic!("the app was not asked to hold a memory: {other:?}"),
        }
    }
}

fn purlis(tmp: &tempfile::TempDir, args: &[&str], env: &[(String, String)]) -> Output {
    let root = root(tmp);
    support::purlis()
        .args(args)
        .current_dir(root.join("workspaces/alpha"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("CHARTER_ROOT", &root)
        .env("HOME", tmp.path().join("home"))
        .env("TMPDIR", std::env::temp_dir())
        .env("NO_COLOR", "1")
        .env("CHARTER_NO_BACKGROUND_CHECKS", "1")
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::null())
        .output()
        .expect("purlis runs")
}

fn said(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Nothing was written into any store the chat does not own.
fn nothing_written(tmp: &tempfile::TempDir) {
    for dir in [
        "personas/devops/memory",
        "workspaces/beta/memory",
        "workspaces/beta/todos",
    ] {
        let dir = root(tmp).join(dir);
        let entries = std::fs::read_dir(&dir).map_or(0, Iterator::count);
        assert_eq!(entries, 0, "{} holds something", dir.display());
    }
}

#[test]
fn a_remember_for_another_persona_is_proposed_to_the_person_and_nothing_is_written() {
    let tmp = a_project();
    let app = an_app(&tmp, held());

    let ran = purlis(
        &tmp,
        &["persona", "remember", "devops", "Run the drill monthly"],
        &app.chat_env(),
    );

    assert!(ran.status.success(), "{}", said(&ran));
    assert!(said(&ran).contains("held for the person"), "{}", said(&ran));
    assert_eq!(
        app.proposed(),
        (
            Write::PersonaRemember {
                text: "Run the drill monthly".to_owned(),
                title: None,
                shared: false,
            },
            Target::Persona("devops".to_owned())
        )
    );
    nothing_written(&tmp);
}

#[test]
fn a_remember_a_note_and_a_todo_for_another_workspace_are_proposed() {
    let tmp = a_project();
    let app = an_app(&tmp, held());
    let beta = Target::Workspace("beta".to_owned());

    for (args, write) in [
        (
            ["ws", "remember", "-w", "beta", "Beta uses blue"],
            Write::WorkspaceRemember {
                text: "Beta uses blue".to_owned(),
                title: None,
            },
        ),
        (
            ["ws", "note", "-w", "beta", "Beta uses green"],
            Write::WorkspaceRemember {
                text: "Beta uses green".to_owned(),
                title: None,
            },
        ),
        (
            ["ws", "todo", "-w", "beta", "Rotate the key"],
            Write::Todo {
                text: "Rotate the key".to_owned(),
            },
        ),
    ] {
        let ran = purlis(&tmp, &args, &app.chat_env());
        assert!(ran.status.success(), "{args:?}: {}", said(&ran));
        assert_eq!(app.proposed(), (write, beta.clone()), "{args:?}");
    }
    nothing_written(&tmp);
}

/// Every way into another store that is not proposed, with the words it is refused in. Move and
/// optimize are the person's to rule on; archive, unarchive, a todo's close and a vision were
/// never offered.
const STAY_REFUSED: [(&[&str], &str); 5] = [
    (
        &["ws", "archive", "-w", "beta", "slug"],
        "a sandboxed chat changes only its own workspace",
    ),
    (
        &["ws", "todo", "-w", "beta", "done", "anything"],
        "a sandboxed chat changes only its own workspace",
    ),
    (
        &["ws", "vision", "-w", "beta", "New vision"],
        "a sandboxed chat changes only its own workspace",
    ),
    (
        &["persona", "archive-memory", "devops", "slug"],
        "a sandboxed chat changes only the memory of the persona it runs as",
    ),
    (
        &["persona", "optimize", "devops", "--apply"],
        "a sandboxed chat changes only the memory of the persona it runs as",
    ),
];

/// A forget and an edit, which are proposed where an app takes them, and refused in these
/// words where none does.
const FORGET_AND_EDIT: [(&[&str], &str); 4] = [
    (
        &["ws", "forget", "-w", "beta", "slug"],
        "a sandboxed chat changes only its own workspace",
    ),
    (
        &["ws", "edit", "-w", "beta", "slug", "--title", "Mine"],
        "a sandboxed chat changes only its own workspace",
    ),
    (
        &["persona", "forget", "devops", "slug"],
        "a sandboxed chat changes only the memory of the persona it runs as",
    ),
    (
        &[
            "persona",
            "edit-memory",
            "devops",
            "slug",
            "--title",
            "Mine",
        ],
        "a sandboxed chat changes only the memory of the persona it runs as",
    ),
];

/// **Only the ruled verbs are proposed.** Every other way into another store is refused, in the
/// words it always had, and the app is not asked.
#[test]
fn archive_optimize_a_todos_close_and_a_vision_stay_refused_and_ask_nobody() {
    let tmp = a_project();
    let app = an_app(&tmp, held());
    for (args, words) in STAY_REFUSED {
        let ran = purlis(&tmp, args, &app.chat_env());
        assert_eq!(ran.status.code(), Some(1), "{args:?}: {}", said(&ran));
        assert!(said(&ran).contains(words), "{args:?}: {}", said(&ran));
    }
    assert!(app.asked.try_recv().is_err(), "the app was asked");
    nothing_written(&tmp);
}

/// The environment of a sandboxed chat whose harness handed it no connection to the app.
fn without_a_connection() -> Vec<(String, String)> {
    vec![
        (CHAT_ENV.to_owned(), "3".to_owned()),
        (hookwire::SANDBOXED_ENV.to_owned(), "1".to_owned()),
        ("PURLIS_PERSONA".to_owned(), "steward".to_owned()),
        (
            purlis_core::active::WORKSPACE_ENV.to_owned(),
            "alpha".to_owned(),
        ),
    ]
}

/// The same refusals with no app to ask at all: they read the same, so the words a chat learns
/// do not depend on whether a window listens. A forget and an edit read the same too, where no
/// app takes them.
#[test]
fn the_refusals_read_the_same_where_no_app_is_reached() {
    let tmp = a_project();
    for (args, words) in STAY_REFUSED.into_iter().chain(FORGET_AND_EDIT) {
        let ran = purlis(&tmp, args, &without_a_connection());
        assert_eq!(ran.status.code(), Some(1), "{args:?}: {}", said(&ran));
        assert!(said(&ran).contains(words), "{args:?}: {}", said(&ran));
    }
    nothing_written(&tmp);
}

/// An add for another store, with no app to hand it to, is refused in the words it always had,
/// and nothing is written.
#[test]
fn an_add_for_another_store_with_no_app_reached_is_refused_in_its_old_words() {
    let tmp = a_project();
    for (args, words) in [
        (
            vec!["persona", "remember", "devops", "Run the drill monthly"],
            "a sandboxed chat changes only the memory of the persona it runs as",
        ),
        (
            vec!["ws", "remember", "-w", "beta", "Beta uses blue"],
            "a sandboxed chat changes only its own workspace",
        ),
        (
            vec!["ws", "todo", "-w", "beta", "Rotate the key"],
            "a sandboxed chat changes only its own workspace",
        ),
    ] {
        let ran = purlis(&tmp, &args, &without_a_connection());
        assert_eq!(ran.status.code(), Some(1), "{args:?}: {}", said(&ran));
        assert!(said(&ran).contains(words), "{args:?}: {}", said(&ran));
    }
    nothing_written(&tmp);
}

/// **A forget and an edit of another store's memory are proposed** (the person's ruling of
/// 2026-11-25): the app is handed each, and nothing is written here.
#[test]
fn a_forget_and_an_edit_of_another_stores_memory_are_proposed() {
    use purlis_core::brokered::Store;
    let tmp = a_project();
    let app = an_app(&tmp, held());

    for (args, write, target) in [
        (
            &["ws", "forget", "-w", "beta", "slug"][..],
            Write::Forget {
                of: Store::Workspace,
                slug: "slug".to_owned(),
            },
            Target::Workspace("beta".to_owned()),
        ),
        (
            &["ws", "edit", "-w", "beta", "slug", "--title", "Mine"][..],
            Write::EditMemory {
                of: Store::Workspace,
                slug: "slug".to_owned(),
                title: Some("Mine".to_owned()),
                text: None,
            },
            Target::Workspace("beta".to_owned()),
        ),
        (
            &["persona", "forget", "devops", "slug"][..],
            Write::Forget {
                of: Store::Persona,
                slug: "slug".to_owned(),
            },
            Target::Persona("devops".to_owned()),
        ),
        (
            &[
                "persona",
                "edit-memory",
                "devops",
                "slug",
                "--title",
                "Mine",
            ][..],
            Write::EditMemory {
                of: Store::Persona,
                slug: "slug".to_owned(),
                title: Some("Mine".to_owned()),
                text: None,
            },
            Target::Persona("devops".to_owned()),
        ),
    ] {
        let ran = purlis(&tmp, args, &app.chat_env());
        assert!(ran.status.success(), "{args:?}: {}", said(&ran));
        assert_eq!(app.proposed(), (write, target), "{args:?}");
    }
    nothing_written(&tmp);
}

/// Shared memory is reached with `--shared`, and a forget of it by another persona's name is not
/// proposed: it stays refused, and the app is not asked.
#[test]
fn a_forget_of_shared_memory_through_another_persona_is_not_proposed() {
    let tmp = a_project();
    let app = an_app(&tmp, held());

    let ran = purlis(
        &tmp,
        &["persona", "forget", "devops", "slug", "--shared"],
        &app.chat_env(),
    );

    assert_eq!(ran.status.code(), Some(1), "{}", said(&ran));
    assert!(app.asked.try_recv().is_err(), "the app was asked");
}

#[test]
fn a_remember_of_scratch_for_another_persona_is_not_proposed() {
    let tmp = a_project();
    let app = an_app(&tmp, held());

    let scratch = purlis(
        &tmp,
        &["persona", "remember", "devops", "--ephemeral", "Scratch"],
        &app.chat_env(),
    );
    assert_eq!(scratch.status.code(), Some(1), "{}", said(&scratch));
    assert!(app.asked.try_recv().is_err(), "the app was asked");
}

#[test]
fn where_no_app_takes_it_the_refusal_stands_in_its_own_words() {
    let tmp = a_project();
    // An app older than the ask answers a line it cannot read with this.
    let app = an_app(
        &tmp,
        Answer::No {
            why: hookwire::NOTHING_ANSWERS.to_owned(),
        },
    );

    let ran = purlis(
        &tmp,
        &["persona", "remember", "devops", "Run the drill monthly"],
        &app.chat_env(),
    );

    assert_eq!(ran.status.code(), Some(1), "{}", said(&ran));
    assert!(
        said(&ran).contains("a sandboxed chat changes only the memory of the persona it runs as"),
        "{}",
        said(&ran)
    );
    assert!(said(&ran).contains("'devops'"), "{}", said(&ran));
    nothing_written(&tmp);
}

#[test]
fn an_app_that_cannot_read_the_ask_leaves_the_refusal_in_its_old_words() {
    let tmp = a_project();
    let app = an_app(
        &tmp,
        Answer::No {
            why: hookwire::UNKNOWN_LINE.to_owned(),
        },
    );

    let ran = purlis(
        &tmp,
        &["persona", "remember", "devops", "Run the drill monthly"],
        &app.chat_env(),
    );

    assert_eq!(ran.status.code(), Some(1), "{}", said(&ran));
    assert!(
        said(&ran).contains("a sandboxed chat changes only the memory of the persona it runs as"),
        "{}",
        said(&ran)
    );
    assert!(!said(&ran).contains("could not read"), "{}", said(&ran));
    nothing_written(&tmp);
}

#[test]
fn a_text_too_long_to_hand_over_leaves_the_refusal_in_its_old_words() {
    let tmp = a_project();
    let app = an_app(&tmp, held());
    let long = "x".repeat(20_000);

    let ran = purlis(
        &tmp,
        &["persona", "remember", "devops", &long],
        &app.chat_env(),
    );

    assert_eq!(ran.status.code(), Some(1), "{}", said(&ran));
    assert!(
        said(&ran).contains("a sandboxed chat changes only the memory of the persona it runs as"),
        "{}",
        said(&ran)
    );
    assert!(app.asked.try_recv().is_err(), "the app was asked");
    nothing_written(&tmp);
}

#[test]
fn where_the_app_holds_nothing_the_chat_hears_why() {
    let tmp = a_project();
    let app = an_app(
        &tmp,
        Answer::No {
            why: "this chat has 8 proposed changes waiting already".to_owned(),
        },
    );

    let ran = purlis(
        &tmp,
        &["ws", "todo", "-w", "beta", "Rotate the key"],
        &app.chat_env(),
    );

    assert_eq!(ran.status.code(), Some(1), "{}", said(&ran));
    assert!(
        said(&ran).contains("8 proposed changes waiting already"),
        "{}",
        said(&ran)
    );
    nothing_written(&tmp);
}
