// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Brokered writes from inside a chat the app started (ADR 0067 §2, #1333): `purlis persona
//! remember`, `purlis workspace remember`/`note`, `purlis workspace todo` and the MCP server's
//! `persona_remember` hand their write to the app over the chat's hook socket, and print what
//! the app wrote as they print their own writes.
//!
//! The app here is a listener that answers each write with the core's own
//! `purlis_core::brokered::answer`, for chat 3 in `alpha` running as `steward`: what the real
//! app does once it has looked the chat up. That the command did not write it itself is read
//! off the trace, which credits a brokered write to the chat it was made for.
#![cfg(unix)]

mod support;

use std::io::{BufRead, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use purlis_core::active::Place;
use purlis_core::brokered::{self, Asker, Write};
use purlis_core::hookwire::{self, Answer, Ask, CHAT_ENV, Listener, SOCKET_ENV, TOKEN_ENV};

/// A project with workspaces `alpha` and `beta` and persona `steward`, and no manifest: none of
/// these commands reads one.
fn a_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    for ws in ["alpha", "beta"] {
        std::fs::create_dir_all(root.join("workspaces").join(ws)).unwrap();
    }
    std::fs::create_dir_all(root.join("personas/steward")).unwrap();
    std::fs::write(
        root.join("personas/steward/persona.md"),
        "---\nname: steward\n---\n# Steward\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("home")).unwrap();
    dir
}

fn root(tmp: &tempfile::TempDir) -> PathBuf {
    tmp.path().join("project")
}

/// The app: every write it is asked for is made by the core for chat 3 in `alpha`, as
/// `steward`, unless `answer` overrides it; every ask is handed to the receiver.
struct App {
    _reading: hookwire::Reading,
    token: hookwire::ChatToken,
    socket: PathBuf,
    asked: mpsc::Receiver<Ask>,
}

fn an_app(tmp: &tempfile::TempDir, answer: Option<Answer>) -> App {
    let socket = tmp.path().join("app").join("hooks.sock");
    let (tx, asked) = mpsc::channel();
    let tx = Mutex::new(tx);
    let listener = Listener::bind(tmp.path(), &socket).unwrap_or_else(|e| panic!("a socket: {e}"));
    let token = listener.tokens().issue_to_this_process(3).expect("a token");
    let project = root(tmp);
    let reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, ask| {
            tx.lock().unwrap().send(ask.clone()).unwrap();
            if let Some(answer) = &answer {
                return answer.clone();
            }
            let Ask::Write(write) = ask else {
                return Answer::No {
                    why: "not a write".to_owned(),
                };
            };
            let asker = Asker {
                chat: write.chat,
                place: Place::Workspace("alpha".to_owned()),
                persona: Some("steward".to_owned()),
            };
            // As the app answers (app brokered.rs): a file written, or a verb's lines (#1408).
            match brokered::answer(
                &project,
                &asker,
                &write.write,
                chrono::Local::now().naive_local(),
            ) {
                Ok(brokered::Done::Written(written)) => Answer::Written {
                    to: written.to,
                    path: written.path,
                },
                Ok(brokered::Done::Said { lines, code }) => Answer::Said { lines, code },
                Err(why) => Answer::No { why },
            }
        }),
    );
    App {
        _reading: reading,
        token,
        socket,
        asked,
    }
}

impl App {
    /// The environment of chat 3, which the app started.
    fn chat_env(&self) -> Vec<(String, String)> {
        vec![
            (SOCKET_ENV.to_owned(), self.socket.display().to_string()),
            (CHAT_ENV.to_owned(), "3".to_owned()),
            (TOKEN_ENV.to_owned(), self.token.expose().to_owned()),
            ("PURLIS_PERSONA".to_owned(), "steward".to_owned()),
            // The workspace the app starts it in, pinned as the app pins it.
            (
                purlis_core::active::WORKSPACE_ENV.to_owned(),
                "alpha".to_owned(),
            ),
        ]
    }

    fn write_asked(&self) -> Write {
        match self.asked.recv_timeout(Duration::from_secs(5)) {
            Ok(Ask::Write(ask)) => {
                assert_eq!(ask.chat, 3);
                ask.write
            }
            other => panic!("the app was not asked for a write: {other:?}"),
        }
    }
}

/// `purlis <args>` in `alpha`, with nothing of the caller's environment but what is named.
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

/// What the app wrote for chat 3: the trace's brokered events.
fn brokered_for_chat_3(tmp: &tempfile::TempDir) -> Vec<String> {
    std::fs::read_to_string(purlis_core::trace::file(&root(tmp), "3"))
        .unwrap_or_default()
        .lines()
        .filter(|line| line.contains("\"event\": \"brokered\""))
        .map(str::to_owned)
        .collect()
}

fn files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|name| name != "MEMORY.md")
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn persona_remember_in_a_chat_is_written_by_the_app_in_the_chat_s_own_persona() {
    let tmp = a_project();
    let app = an_app(&tmp, None);

    let ran = purlis(
        &tmp,
        &["persona", "remember", "Deploys go through the pipeline"],
        &app.chat_env(),
    );

    assert!(ran.status.success(), "{}", said(&ran));
    assert_eq!(
        app.write_asked(),
        Write::PersonaRemember {
            text: "Deploys go through the pipeline".to_owned(),
            title: None,
            shared: false,
        }
    );
    assert!(
        said(&ran).contains(
            "Remembered (persistent) → personas/steward/memory/deploys-go-through-the-pipeline.md"
        ),
        "{}",
        said(&ran)
    );
    assert_eq!(
        files(&root(&tmp).join("personas/steward/memory")),
        ["deploys-go-through-the-pipeline.md"]
    );
    assert_eq!(brokered_for_chat_3(&tmp).len(), 1);
}

#[test]
fn persona_remember_shared_in_a_chat_is_written_by_the_app_in_shared_memory() {
    let tmp = a_project();
    let app = an_app(&tmp, None);

    let ran = purlis(
        &tmp,
        &[
            "persona",
            "remember",
            "--shared",
            "--title",
            "Forge",
            "The forge is self-hosted",
        ],
        &app.chat_env(),
    );

    assert!(ran.status.success(), "{}", said(&ran));
    assert_eq!(
        app.write_asked(),
        Write::PersonaRemember {
            text: "The forge is self-hosted".to_owned(),
            title: Some("Forge".to_owned()),
            shared: true,
        }
    );
    assert!(
        said(&ran).contains("Remembered (shared persistent) → personas/_shared/memory/"),
        "{}",
        said(&ran)
    );
    assert_eq!(files(&root(&tmp).join("personas/_shared/memory")).len(), 1);
    assert_eq!(brokered_for_chat_3(&tmp).len(), 1);
}

#[test]
fn workspace_remember_and_note_in_a_chat_are_written_by_the_app_in_the_chat_s_workspace() {
    let tmp = a_project();
    let app = an_app(&tmp, None);

    let remembered = purlis(
        &tmp,
        &["workspace", "remember", "The build needs two jobs"],
        &app.chat_env(),
    );
    let noted = purlis(&tmp, &["ws", "note", "CI runs on Linux"], &app.chat_env());

    for ran in [&remembered, &noted] {
        assert!(ran.status.success(), "{}", said(ran));
        assert!(
            said(ran).contains("Remembered in 'alpha' → workspaces/alpha/memory/"),
            "{}",
            said(ran)
        );
    }
    assert_eq!(
        app.write_asked(),
        Write::WorkspaceRemember {
            text: "The build needs two jobs".to_owned(),
            title: None,
        }
    );
    assert_eq!(
        app.write_asked(),
        Write::WorkspaceRemember {
            text: "CI runs on Linux".to_owned(),
            title: None,
        }
    );
    assert_eq!(files(&root(&tmp).join("workspaces/alpha/memory")).len(), 2);
    assert_eq!(brokered_for_chat_3(&tmp).len(), 2);
}

#[test]
fn workspace_todo_in_a_chat_is_written_by_the_app_in_the_chat_s_workspace() {
    let tmp = a_project();
    let app = an_app(&tmp, None);

    let ran = purlis(
        &tmp,
        &["ws", "todo", "Port the docs command"],
        &app.chat_env(),
    );

    assert!(ran.status.success(), "{}", said(&ran));
    assert_eq!(
        app.write_asked(),
        Write::Todo {
            text: "Port the docs command".to_owned(),
        }
    );
    assert!(
        said(&ran).contains("Todo recorded in 'alpha' → workspaces/alpha/todos/"),
        "{}",
        said(&ran)
    );
    assert_eq!(files(&root(&tmp).join("workspaces/alpha/todos")).len(), 1);
    assert_eq!(brokered_for_chat_3(&tmp).len(), 1);
}

#[test]
fn a_write_the_app_refuses_is_refused_here_and_never_written_here() {
    let tmp = a_project();
    let app = an_app(
        &tmp,
        Some(Answer::No {
            why: "chat 3 has asked purlis for 60 writes in the last 60 seconds".to_owned(),
        }),
    );

    let ran = purlis(&tmp, &["ws", "todo", "One too many"], &app.chat_env());

    assert_eq!(ran.status.code(), Some(1), "{}", said(&ran));
    assert!(said(&ran).contains("60 writes"), "{}", said(&ran));
    assert!(files(&root(&tmp).join("workspaces/alpha/todos")).is_empty());
}

#[test]
fn an_app_that_does_not_answer_writes_has_them_written_here_as_before() {
    let tmp = a_project();
    let app = an_app(
        &tmp,
        Some(Answer::No {
            why: hookwire::NOTHING_ANSWERS.to_owned(),
        }),
    );

    let ran = purlis(
        &tmp,
        &["workspace", "remember", "Written here"],
        &app.chat_env(),
    );

    assert!(ran.status.success(), "{}", said(&ran));
    assert_eq!(files(&root(&tmp).join("workspaces/alpha/memory")).len(), 1);
    assert!(brokered_for_chat_3(&tmp).is_empty());
}

#[test]
fn a_write_naming_another_workspace_is_not_the_app_s_to_make() {
    // The app writes only the chat's own workspace; `-w` is written here, where the chat's
    // sandbox decides, as it always was.
    let tmp = a_project();
    let app = an_app(&tmp, None);

    let ran = purlis(
        &tmp,
        &["ws", "todo", "-w", "beta", "Over there"],
        &app.chat_env(),
    );

    assert!(ran.status.success(), "{}", said(&ran));
    assert!(app.asked.try_recv().is_err(), "the app was asked");
    assert_eq!(files(&root(&tmp).join("workspaces/beta/todos")).len(), 1);
}

// ---- `-w <its own workspace>`, and `-w` naming another in a sandboxed chat (#1408) ----------

/// One memory of workspace `ws`, written as the terminal writes it: its slug.
fn a_memory_in(tmp: &tempfile::TempDir, ws: &str, text: &str) -> String {
    let path = purlis_core::workspaces::Plane::open(root(tmp))
        .workspace(ws)
        .unwrap()
        .remember(text, chrono::Local::now().naive_local())
        .expect("remembered");
    path.file_stem().unwrap().to_string_lossy().into_owned()
}

#[test]
fn naming_its_own_workspace_with_w_is_the_app_s_to_make_as_with_no_w() {
    let tmp = a_project();
    let app = an_app(&tmp, None);
    let kept = a_memory_in(&tmp, "alpha", "Kept, then edited");
    let gone = a_memory_in(&tmp, "alpha", "Archived, back, then forgotten");
    use purlis_core::brokered::Store;

    let asked: Vec<(Vec<&str>, Write)> = vec![
        (
            vec!["ws", "remember", "-w", "alpha", "Spelled out"],
            Write::WorkspaceRemember {
                text: "Spelled out".to_owned(),
                title: None,
            },
        ),
        (
            vec!["ws", "note", "--workspace", "alpha", "Spelled out too"],
            Write::WorkspaceRemember {
                text: "Spelled out too".to_owned(),
                title: None,
            },
        ),
        (
            vec!["ws", "todo", "-w", "alpha", "Its own todo"],
            Write::Todo {
                text: "Its own todo".to_owned(),
            },
        ),
        (
            vec!["ws", "vision", "-w", "alpha", "Its own vision"],
            Write::WorkspaceVision {
                text: "Its own vision".to_owned(),
            },
        ),
        (
            vec!["ws", "edit", "-w", "alpha", &kept, "--title", "Retitled"],
            Write::EditMemory {
                of: Store::Workspace,
                slug: kept.clone(),
                title: Some("Retitled".to_owned()),
                text: None,
            },
        ),
        (
            vec!["ws", "archive", "-w", "alpha", &gone],
            Write::ArchiveMemory {
                of: Store::Workspace,
                slug: gone.clone(),
            },
        ),
        (
            vec!["ws", "unarchive", "-w", "alpha", &gone],
            Write::UnarchiveMemory {
                of: Store::Workspace,
                slug: gone.clone(),
                restore_as: None,
            },
        ),
        (
            vec!["ws", "forget", "-w", "alpha", &gone],
            Write::Forget {
                of: Store::Workspace,
                slug: gone.clone(),
            },
        ),
    ];
    for (args, write) in asked {
        let ran = purlis(&tmp, &args, &app.chat_env());
        assert!(ran.status.success(), "{args:?}: {}", said(&ran));
        assert_eq!(app.write_asked(), write, "{args:?}");
    }

    // The todo it recorded, closed with its own workspace spelled out.
    let todo = files(&root(&tmp).join("workspaces/alpha/todos"))
        .into_iter()
        .next()
        .expect("a todo");
    let slug = todo.trim_end_matches(".md").to_owned();
    let ran = purlis(
        &tmp,
        &["ws", "todo", "-w", "alpha", "done", &slug],
        &app.chat_env(),
    );
    assert!(ran.status.success(), "{}", said(&ran));
    assert!(matches!(app.write_asked(), Write::TodoClose { .. }));
    assert_eq!(brokered_for_chat_3(&tmp).len(), 9);
}

#[test]
fn a_sandboxed_chat_naming_another_workspace_with_w_is_refused_and_nothing_is_written() {
    let tmp = a_project();
    let theirs = a_memory_in(&tmp, "beta", "Not this chat's");
    let before = std::fs::read_dir(root(&tmp).join("workspaces/beta/memory"))
        .unwrap()
        .count();
    // A chat the app started under the sandbox. No app is asked: the refusal comes first.
    let env = vec![
        (CHAT_ENV.to_owned(), "3".to_owned()),
        (hookwire::SANDBOXED_ENV.to_owned(), "1".to_owned()),
        (
            purlis_core::active::WORKSPACE_ENV.to_owned(),
            "alpha".to_owned(),
        ),
    ];

    for args in [
        vec!["ws", "remember", "-w", "beta", "Over there"],
        vec!["ws", "note", "-w", "beta", "Over there"],
        vec!["ws", "todo", "-w", "beta", "Over there"],
        vec!["ws", "vision", "-w", "beta", "Over there"],
        vec!["ws", "edit", "-w", "beta", &theirs, "--title", "Mine now"],
        vec!["ws", "archive", "-w", "beta", &theirs],
        vec!["ws", "unarchive", "-w", "beta", &theirs],
        vec!["ws", "forget", "-w", "beta", &theirs],
        vec!["ws", "todo", "-w", "beta", "done", "anything"],
    ] {
        let ran = purlis(&tmp, &args, &env);
        assert_eq!(ran.status.code(), Some(1), "{args:?}: {}", said(&ran));
        assert!(
            said(&ran).contains("a sandboxed chat changes only its own workspace"),
            "{args:?}: {}",
            said(&ran)
        );
        assert!(said(&ran).contains("'beta'"), "{args:?}: {}", said(&ran));
    }
    let beta = root(&tmp).join("workspaces/beta");
    assert_eq!(
        std::fs::read_dir(beta.join("memory")).unwrap().count(),
        before
    );
    assert!(!beta.join("todos").exists());
    assert!(!beta.join("workspace.md").exists());
    // Reading another workspace is not refused.
    let read = purlis(&tmp, &["ws", "recall", "-w", "beta"], &env);
    assert!(read.status.success(), "{}", said(&read));
}

// ---- `workspace create` in a chat (#1408) -----------------------------------------------------

#[test]
fn workspace_create_in_a_chat_is_made_by_the_app_and_says_what_a_terminal_says() {
    let tmp = a_project();
    let app = an_app(&tmp, None);

    let ran = purlis(
        &tmp,
        &["ws", "create", "gamma", "--vision", "Port the docs command"],
        &app.chat_env(),
    );

    assert!(ran.status.success(), "{}", said(&ran));
    assert_eq!(
        app.write_asked(),
        Write::WorkspaceCreate {
            name: "gamma".to_owned(),
            vision: Some("Port the docs command".to_owned()),
        }
    );
    assert!(
        said(&ran).contains("Workspace 'gamma' ready"),
        "{}",
        said(&ran)
    );
    assert!(root(&tmp).join("workspaces/gamma/workspace.md").is_file());
    assert_eq!(brokered_for_chat_3(&tmp).len(), 1);

    // One that is there is refused by the app, and nothing here repairs it either.
    let again = purlis(&tmp, &["ws", "create", "beta"], &app.chat_env());
    assert_eq!(again.status.code(), Some(1), "{}", said(&again));
    assert!(said(&again).contains("already there"), "{}", said(&again));
    assert!(!root(&tmp).join("workspaces/beta/workspace.md").exists());
}

#[test]
fn workspace_create_with_what_the_app_does_not_make_runs_here_as_before() {
    // `--use` selects it for this session, which a chat started without the sandbox does
    // itself; the app is not asked. A sandboxed chat is refused it before anything is made
    // (qw387, `brokered_git.rs`).
    let tmp = a_project();
    let app = an_app(&tmp, None);

    let ran = purlis(&tmp, &["ws", "create", "delta", "--use"], &app.chat_env());

    assert!(app.asked.try_recv().is_err(), "the app was asked");
    assert!(brokered_for_chat_3(&tmp).is_empty(), "{}", said(&ran));
}

// ---- `workspace remove` in a chat (#1408) -----------------------------------------------------

#[test]
fn a_sandboxed_chat_removing_a_workspace_is_refused_before_anything_is_deleted() {
    // Removing a workspace deletes what other chats and the person rely on, which no write
    // purlis makes for a chat reaches yet. A sandboxed chat is refused before the delete
    // starts, naming the way round.
    let tmp = a_project();
    let theirs = a_memory_in(&tmp, "beta", "Not this chat's");
    let mine = a_memory_in(&tmp, "alpha", "This chat's own");
    let sandboxed = |workspace: Option<&str>| {
        let mut env = vec![
            (CHAT_ENV.to_owned(), "3".to_owned()),
            (hookwire::SANDBOXED_ENV.to_owned(), "1".to_owned()),
        ];
        env.extend(
            workspace.map(|ws| (purlis_core::active::WORKSPACE_ENV.to_owned(), ws.to_owned())),
        );
        env
    };

    // Another workspace, its own, and from a chat at the project root, forced or not.
    for (args, env) in [
        (vec!["ws", "remove", "beta"], sandboxed(Some("alpha"))),
        (
            vec!["ws", "remove", "beta", "--force"],
            sandboxed(Some("alpha")),
        ),
        (vec!["ws", "remove", "alpha"], sandboxed(Some("alpha"))),
        (vec!["ws", "remove", "beta"], sandboxed(None)),
    ] {
        let ran = purlis(&tmp, &args, &env);
        assert_eq!(ran.status.code(), Some(1), "{args:?}: {}", said(&ran));
        assert!(
            said(&ran).contains("nothing was removed"),
            "{args:?}: {}",
            said(&ran)
        );
        assert!(
            said(&ran).contains("from the window"),
            "{args:?}: {}",
            said(&ran)
        );
    }
    for (ws, slug) in [("beta", &theirs), ("alpha", &mine)] {
        let memory = root(&tmp).join("workspaces").join(ws).join("memory");
        assert!(
            files(&memory)
                .iter()
                .any(|name| name.contains(slug.as_str())),
            "{ws}: {:?}",
            files(&memory)
        );
    }

    // A chat the app started without the sandbox removes it here, as a terminal does.
    let unsandboxed = vec![(CHAT_ENV.to_owned(), "3".to_owned())];
    let ran = purlis(&tmp, &["ws", "remove", "beta"], &unsandboxed);
    assert!(ran.status.success(), "{}", said(&ran));
    assert!(!root(&tmp).join("workspaces/beta").exists());
}

/// The environment of chat 3 as the app starts it under the sandbox at the project root, where
/// Safe remove runs: its connection to the app, and the sandbox's mark.
fn sandboxed_chat_of(app: &App) -> Vec<(String, String)> {
    vec![
        (SOCKET_ENV.to_owned(), app.socket.display().to_string()),
        (CHAT_ENV.to_owned(), "3".to_owned()),
        (TOKEN_ENV.to_owned(), app.token.expose().to_owned()),
        (hookwire::SANDBOXED_ENV.to_owned(), "1".to_owned()),
    ]
}

#[test]
fn a_sandboxed_chat_removing_a_workspace_asks_the_person_in_the_window_and_removes_nothing() {
    // qw356: the refusal was a dead end, in the chat purlis's own Safe remove opens. The
    // command now hands the name to the app, which asks the person, and says so.
    let tmp = a_project();
    let theirs = a_memory_in(&tmp, "beta", "Not this chat's");
    let asked_said = purlis_core::removalask::asked_said("beta");
    let app = an_app(
        &tmp,
        Some(Answer::No {
            why: asked_said.clone(),
        }),
    );

    for args in [
        vec!["ws", "remove", "beta"],
        vec!["ws", "remove", "beta", "--force"],
    ] {
        let ran = purlis(&tmp, &args, &sandboxed_chat_of(&app));

        // Nothing was removed, so it is no success; and the sentence is the app's.
        assert_eq!(ran.status.code(), Some(1), "{args:?}: {}", said(&ran));
        assert!(said(&ran).contains(&asked_said), "{args:?}: {}", said(&ran));
        assert!(
            !said(&ran).contains("Ask the person to remove it"),
            "{args:?}: {}",
            said(&ran)
        );
        // The app was handed the name and the chat, and nothing else: no field carries a force.
        match app.asked.recv_timeout(Duration::from_secs(5)) {
            Ok(Ask::Removal(removal)) => assert_eq!(
                removal,
                hookwire::RemovalAsk {
                    chat: 3,
                    removing: hookwire::Removing::Workspace {
                        name: "beta".to_owned(),
                    },
                }
            ),
            other => panic!("{args:?}: the app was not asked about a removal: {other:?}"),
        }
    }
    let memory = root(&tmp).join("workspaces/beta/memory");
    assert!(
        files(&memory)
            .iter()
            .any(|name| name.contains(theirs.as_str())),
        "{:?}",
        files(&memory)
    );
}

#[test]
fn a_removal_no_app_asks_the_person_about_is_refused_naming_the_window_as_before() {
    // An app older than the ask answers that nothing here takes it: the chat is still refused
    // before anything is deleted, and told the way round it had before.
    let tmp = a_project();
    let app = an_app(
        &tmp,
        Some(Answer::No {
            why: hookwire::NOTHING_ANSWERS.to_owned(),
        }),
    );

    let ran = purlis(&tmp, &["ws", "remove", "beta"], &sandboxed_chat_of(&app));

    assert_eq!(ran.status.code(), Some(1), "{}", said(&ran));
    assert!(said(&ran).contains("nothing was removed"), "{}", said(&ran));
    assert!(said(&ran).contains("from the window"), "{}", said(&ran));
    assert!(root(&tmp).join("workspaces/beta").exists());
}

/// The environment of chat 3, which the app started under the sandbox in `workspace` (`None`:
/// at the project root).
fn sandboxed_chat(workspace: Option<&str>) -> Vec<(String, String)> {
    let mut env = vec![
        (CHAT_ENV.to_owned(), "3".to_owned()),
        (hookwire::SANDBOXED_ENV.to_owned(), "1".to_owned()),
    ];
    env.extend(workspace.map(|ws| (purlis_core::active::WORKSPACE_ENV.to_owned(), ws.to_owned())));
    env
}

#[test]
fn a_sandboxed_chat_removing_a_persona_is_refused_before_anything_is_deleted() {
    // Removing a persona deletes its definition, its memory and its curation actions, which the
    // sandbox can stop partway. Refused before the delete starts, forced or not, naming the way
    // round.
    let tmp = a_project();
    let persona = root(&tmp).join("personas/steward");
    std::fs::create_dir_all(persona.join("memory")).unwrap();
    std::fs::write(persona.join("memory/kept.md"), "kept\n").unwrap();

    for (args, env) in [
        (
            vec!["persona", "remove", "steward"],
            sandboxed_chat(Some("alpha")),
        ),
        (
            vec!["persona", "remove", "steward", "--force"],
            sandboxed_chat(Some("alpha")),
        ),
        (vec!["persona", "remove", "steward"], sandboxed_chat(None)),
    ] {
        let ran = purlis(&tmp, &args, &env);
        assert_eq!(ran.status.code(), Some(1), "{args:?}: {}", said(&ran));
        for words in ["nothing was removed", "from the window"] {
            assert!(said(&ran).contains(words), "{args:?}: {}", said(&ran));
        }
    }
    assert!(persona.join("persona.md").is_file());
    assert!(persona.join("memory/kept.md").is_file());

    // A chat the app started without the sandbox removes it here, as a terminal does.
    let unsandboxed = vec![(CHAT_ENV.to_owned(), "3".to_owned())];
    let ran = purlis(&tmp, &["persona", "remove", "steward"], &unsandboxed);
    assert!(ran.status.success(), "{}", said(&ran));
    assert!(!persona.exists());
}

#[test]
fn a_sandboxed_chat_removing_a_branch_folder_is_refused_before_git_is_asked() {
    // `git worktree remove` deletes the folder's files before git's record of it, which the
    // sandbox can stop partway. Refused before git runs: so before it is even asked whether the
    // clone is there.
    let tmp = a_project();
    for (args, env) in [
        (
            vec!["worktree", "remove", "thing", "piece"],
            sandboxed_chat(Some("alpha")),
        ),
        (
            vec![
                "wt",
                "remove",
                "thing",
                "piece",
                "--force",
                "--delete-branch",
            ],
            sandboxed_chat(None),
        ),
    ] {
        let ran = purlis(&tmp, &args, &env);
        assert_eq!(ran.status.code(), Some(1), "{args:?}: {}", said(&ran));
        for words in ["nothing was removed", "from the window"] {
            assert!(said(&ran).contains(words), "{args:?}: {}", said(&ran));
        }
    }

    // Outside the sandbox it goes on to git, which finds no clone here.
    let unsandboxed = vec![(CHAT_ENV.to_owned(), "3".to_owned())];
    let ran = purlis(
        &tmp,
        &["worktree", "remove", "thing", "piece"],
        &unsandboxed,
    );
    assert!(!ran.status.success(), "{}", said(&ran));
    assert!(
        !said(&ran).contains("nothing was removed"),
        "{}",
        said(&ran)
    );
}

/// The project's manifest, written with `text`. A machine that keeps a chat from writing a
/// manifest anywhere cannot run a test that calls this; CI runs it.
fn a_manifest(tmp: &tempfile::TempDir, text: &str) -> PathBuf {
    let manifest = root(tmp).join(purlis_core::names::PLANE_MANIFEST.write);
    std::fs::write(&manifest, text).unwrap();
    manifest
}

#[test]
fn a_terminal_removes_a_workspace_and_leaves_the_project_s_manifest_as_it_was() {
    // The hold asks the sandbox itself by opening the manifest to write, which a terminal may.
    let tmp = a_project();
    let manifest = a_manifest(&tmp, "# a project\n");
    let before = std::fs::metadata(&manifest).unwrap().modified().unwrap();

    let ran = purlis(&tmp, &["ws", "remove", "beta"], &[]);

    assert!(ran.status.success(), "{}", said(&ran));
    assert!(!root(&tmp).join("workspaces/beta").exists());
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), "# a project\n");
    assert_eq!(
        std::fs::metadata(&manifest).unwrap().modified().unwrap(),
        before
    );
}

/// A removal is held where the sandbox itself holds the process: the kernel refuses writing the
/// manifest with `EPERM`, as Seatbelt refuses a chat, here through macOS's user-immutable flag.
#[cfg(target_os = "macos")]
#[test]
fn a_removal_the_sandbox_itself_holds_is_refused_before_anything_is_deleted() {
    struct Immutable(PathBuf);
    impl Immutable {
        fn flag(path: &Path, flag: &str) {
            let done = std::process::Command::new("/usr/bin/chflags")
                .arg(flag)
                .arg(path)
                .status()
                .unwrap();
            assert!(done.success(), "chflags {flag}");
        }
    }
    impl Drop for Immutable {
        fn drop(&mut self) {
            Self::flag(&self.0, "nouchg");
        }
    }

    let tmp = a_project();
    let theirs = a_memory_in(&tmp, "beta", "Not this chat's");
    let manifest = a_manifest(&tmp, "# a project\n");
    Immutable::flag(&manifest, "uchg");
    let _held = Immutable(manifest);

    for args in [
        vec!["ws", "remove", "beta"],
        vec!["ws", "remove", "beta", "--force"],
    ] {
        let ran = purlis(&tmp, &args, &[(CHAT_ENV.to_owned(), "3".to_owned())]);
        assert_eq!(ran.status.code(), Some(1), "{args:?}: {}", said(&ran));
        assert!(
            said(&ran).contains("nothing was removed"),
            "{args:?}: {}",
            said(&ran)
        );
    }
    let memory = root(&tmp).join("workspaces/beta/memory");
    assert!(
        files(&memory)
            .iter()
            .any(|name| name.contains(theirs.as_str())),
        "{:?}",
        files(&memory)
    );
}

// ---- the MCP server's persona_remember -------------------------------------------------------

/// `purlis mcp` in chat 3, initialized, and `persona_remember` called once with `arguments`.
fn persona_remember_over_mcp(
    tmp: &tempfile::TempDir,
    env: &[(String, String)],
    arguments: serde_json::Value,
) -> serde_json::Value {
    tool_over_mcp(tmp, env, "persona_remember", arguments)
}

/// `purlis mcp` in chat 3, initialized, and `tool` called once with `arguments`.
fn tool_over_mcp(
    tmp: &tempfile::TempDir,
    env: &[(String, String)],
    tool: &str,
    arguments: serde_json::Value,
) -> serde_json::Value {
    let root = root(tmp);
    let mut child = support::purlis()
        .arg("mcp")
        .current_dir(root.join("workspaces/alpha"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("CHARTER_ROOT", &root)
        .env("HOME", tmp.path().join("home"))
        .env("TMPDIR", std::env::temp_dir())
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("purlis mcp starts");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut read = || {
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        serde_json::from_str::<serde_json::Value>(&line).expect("JSON-RPC")
    };
    for message in [
        serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": { "name": "test-harness", "version": "1" } } }),
        serde_json::json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
            "name": tool, "arguments": arguments } }),
    ] {
        writeln!(stdin, "{message}").unwrap();
    }
    stdin.flush().unwrap();
    let _initialized = read();
    let called = read();
    let _ = child.kill();
    let _ = child.wait();
    called["result"].clone()
}

#[test]
fn the_persona_remember_tool_in_a_chat_is_written_by_the_app() {
    let tmp = a_project();
    let app = an_app(&tmp, None);

    let result = persona_remember_over_mcp(
        &tmp,
        &app.chat_env(),
        serde_json::json!({ "text": "Releases are tagged by hand", "shared": true }),
    );

    assert_ne!(result["isError"], true, "{result}");
    assert_eq!(
        app.write_asked(),
        Write::PersonaRemember {
            text: "Releases are tagged by hand".to_owned(),
            title: None,
            shared: true,
        }
    );
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    assert!(
        text.contains("Remembered (shared persistent) → personas/_shared/memory/"),
        "{result}"
    );
    assert_eq!(files(&root(&tmp).join("personas/_shared/memory")).len(), 1);
    assert_eq!(brokered_for_chat_3(&tmp).len(), 1);
}

#[test]
fn the_persona_remember_tool_outside_any_chat_the_app_started_writes_the_persona_itself() {
    // No chat number: no app could have written it, so the server does.
    let tmp = a_project();

    let result = persona_remember_over_mcp(
        &tmp,
        &[("PURLIS_PERSONA".to_owned(), "steward".to_owned())],
        serde_json::json!({ "text": "Written by the server" }),
    );

    assert_ne!(result["isError"], true, "{result}");
    assert_eq!(
        files(&root(&tmp).join("personas/steward/memory")),
        ["written-by-the-server.md"]
    );
}

#[test]
fn the_persona_remember_tool_in_a_chat_whose_harness_gave_it_no_connection_writes_nothing() {
    // Codex hands the server the chat's number and never its socket: the server runs outside
    // the sandbox, so it refuses rather than write what no sandbox bounds (#1333).
    let tmp = a_project();

    let result = persona_remember_over_mcp(
        &tmp,
        &[
            ("PURLIS_PERSONA".to_owned(), "steward".to_owned()),
            (
                purlis_core::active::SESSION_ID_ENV.to_owned(),
                "3".to_owned(),
            ),
        ],
        serde_json::json!({ "text": "Never written", "shared": true }),
    );

    assert_eq!(result["isError"], true, "{result}");
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    assert!(text.contains("purlis persona remember"), "{result}");
    assert!(!root(&tmp).join("personas/_shared/memory").exists());
}

#[test]
fn the_persona_remember_tool_whose_app_dropped_the_ask_writes_nothing() {
    let tmp = a_project();
    let app = an_app(
        &tmp,
        Some(Answer::No {
            why: hookwire::NOTHING_ANSWERS.to_owned(),
        }),
    );

    let result = persona_remember_over_mcp(
        &tmp,
        &app.chat_env(),
        serde_json::json!({ "text": "Never written", "shared": true }),
    );

    assert_eq!(result["isError"], true, "{result}");
    assert!(!root(&tmp).join("personas/_shared/memory").exists());
}

#[test]
fn the_persona_remember_tool_whose_app_has_gone_writes_the_persona_itself() {
    // The socket the chat was given is not there any more: there is no app to ask.
    let tmp = a_project();
    let gone = tmp.path().join("app").join("hooks.sock");

    let result = persona_remember_over_mcp(
        &tmp,
        &[
            ("PURLIS_PERSONA".to_owned(), "steward".to_owned()),
            (SOCKET_ENV.to_owned(), gone.display().to_string()),
            (CHAT_ENV.to_owned(), "3".to_owned()),
        ],
        serde_json::json!({ "text": "After the app" }),
    );

    assert_ne!(result["isError"], true, "{result}");
    assert_eq!(
        files(&root(&tmp).join("personas/steward/memory")),
        ["after-the-app.md"]
    );
}

// ---- workspace.md: the vision and the decisions and glossary sections (#1384) --------------

fn alpha_charter(tmp: &tempfile::TempDir) -> String {
    std::fs::read_to_string(root(tmp).join("workspaces/alpha/workspace.md")).unwrap_or_default()
}

#[test]
fn workspace_vision_in_a_chat_is_written_by_the_app_in_the_chat_s_workspace() {
    let tmp = a_project();
    let app = an_app(&tmp, None);

    let ran = purlis(
        &tmp,
        &["ws", "vision", "Ship the docs site"],
        &app.chat_env(),
    );

    assert!(ran.status.success(), "{}", said(&ran));
    assert_eq!(
        app.write_asked(),
        Write::WorkspaceVision {
            text: "Ship the docs site".to_owned(),
        }
    );
    assert_eq!(
        purlis_core::mdsection::section_body(&alpha_charter(&tmp), "Vision"),
        "Ship the docs site"
    );
    assert_eq!(brokered_for_chat_3(&tmp).len(), 1);
}

#[test]
fn the_workspace_section_tool_in_a_chat_is_written_by_the_app() {
    let tmp = a_project();
    let app = an_app(&tmp, None);

    let result = tool_over_mcp(
        &tmp,
        &app.chat_env(),
        "workspace_section",
        serde_json::json!({ "section": "decisions", "text": "Deploys go out on Fridays" }),
    );

    assert_ne!(result["isError"], true, "{result}");
    assert_eq!(
        app.write_asked(),
        Write::WorkspaceSection {
            section: purlis_core::brokered::Section::Decisions,
            text: "Deploys go out on Fridays".to_owned(),
        }
    );
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    assert!(
        text.contains("Added to ## Context & decisions of 'alpha' → workspaces/alpha/workspace.md"),
        "{result}"
    );
    assert!(
        purlis_core::mdsection::section_body(&alpha_charter(&tmp), "Context & decisions")
            .ends_with("- Deploys go out on Fridays"),
        "{}",
        alpha_charter(&tmp)
    );
    assert_eq!(brokered_for_chat_3(&tmp).len(), 1);
}

#[test]
fn the_workspace_vision_tool_outside_any_chat_the_app_started_writes_it_itself() {
    // No chat number: no app could have written it, so the server does, in the workspace a
    // `purlis` command run here would act on.
    let tmp = a_project();

    let result = tool_over_mcp(
        &tmp,
        &[],
        "workspace_vision",
        serde_json::json!({ "text": "Written by the server" }),
    );

    assert_ne!(result["isError"], true, "{result}");
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    assert!(
        text.contains("Vision set for 'alpha' → workspaces/alpha/workspace.md"),
        "{result}"
    );
    assert_eq!(
        purlis_core::mdsection::section_body(&alpha_charter(&tmp), "Vision"),
        "Written by the server"
    );
}

#[test]
fn the_workspace_md_tools_in_a_chat_whose_harness_gave_it_no_connection_write_nothing() {
    let tmp = a_project();
    let codex = [(
        purlis_core::active::SESSION_ID_ENV.to_owned(),
        "3".to_owned(),
    )];

    for (tool, arguments) in [
        (
            "workspace_vision",
            serde_json::json!({ "text": "Never written" }),
        ),
        (
            "workspace_section",
            serde_json::json!({ "section": "glossary", "text": "Never written" }),
        ),
    ] {
        let result = tool_over_mcp(&tmp, &codex, tool, arguments);
        assert_eq!(result["isError"], true, "{result}");
        // The way round is one the chat can take wherever it runs (#1598): a chat in a clone is
        // refused a write to workspace.md by its sandbox, so a direct edit is not the only one.
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        match tool {
            "workspace_vision" => assert!(text.contains("purlis workspace vision"), "{result}"),
            _ => assert!(text.contains("ask the person"), "{result}"),
        }
    }
    assert!(!root(&tmp).join("workspaces/alpha/workspace.md").exists());
}

// ---- the MCP server's session_record (#1332), under the same rule (#1408) ------------------

const RECORD_BODY: &str = "## Goal\n\ng\n\n## Done\n\nd\n\n## Decisions\n\nx\n\n## Open\n\no\n\n\
                           ## How to resume\n\nr\n";

fn records_in_alpha(tmp: &tempfile::TempDir) -> Vec<String> {
    files(&root(tmp).join("workspaces/alpha/sessions"))
        .into_iter()
        .filter(|name| name != "index.md")
        .collect()
}

fn session_record_over_mcp(tmp: &tempfile::TempDir, env: &[(String, String)]) -> serde_json::Value {
    tool_over_mcp(
        tmp,
        env,
        "session_record",
        serde_json::json!({ "title": "Over MCP", "body": RECORD_BODY }),
    )
}

#[test]
fn the_session_record_tool_outside_any_chat_the_app_started_writes_the_record_itself() {
    let tmp = a_project();

    let result = session_record_over_mcp(&tmp, &[]);

    assert_ne!(result["isError"], true, "{result}");
    assert_eq!(records_in_alpha(&tmp).len(), 1);
}

#[test]
fn the_session_record_tool_in_a_chat_whose_harness_gave_it_no_connection_writes_nothing() {
    let tmp = a_project();

    let result = session_record_over_mcp(
        &tmp,
        &[(
            purlis_core::active::SESSION_ID_ENV.to_owned(),
            "3".to_owned(),
        )],
    );

    assert_eq!(result["isError"], true, "{result}");
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    assert!(text.contains("purlis session record"), "{result}");
    assert!(records_in_alpha(&tmp).is_empty());
}

#[test]
fn the_session_record_tool_whose_app_dropped_the_ask_writes_nothing() {
    let tmp = a_project();
    let app = an_app(
        &tmp,
        Some(Answer::No {
            why: hookwire::NOTHING_ANSWERS.to_owned(),
        }),
    );

    let result = session_record_over_mcp(&tmp, &app.chat_env());

    assert_eq!(result["isError"], true, "{result}");
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    assert!(text.contains("purlis session record"), "{result}");
    assert!(records_in_alpha(&tmp).is_empty());
}

#[test]
fn the_session_record_tool_whose_app_has_gone_writes_the_record_itself() {
    let tmp = a_project();
    let gone = tmp.path().join("app").join("hooks.sock");

    let result = session_record_over_mcp(
        &tmp,
        &[
            (SOCKET_ENV.to_owned(), gone.display().to_string()),
            (CHAT_ENV.to_owned(), "3".to_owned()),
        ],
    );

    assert_ne!(result["isError"], true, "{result}");
    assert_eq!(records_in_alpha(&tmp).len(), 1);
}

#[test]
fn the_session_record_tool_refuses_a_title_that_is_not_one_line_before_asking_anyone() {
    let tmp = a_project();
    let app = an_app(&tmp, None);

    let result = tool_over_mcp(
        &tmp,
        &app.chat_env(),
        "session_record",
        serde_json::json!({ "title": "Two\nlines", "body": RECORD_BODY }),
    );

    assert_eq!(result["isError"], true, "{result}");
    assert!(app.asked.try_recv().is_err(), "the app was asked");
    assert!(records_in_alpha(&tmp).is_empty());
}

#[test]
fn a_sandboxed_chat_rewrites_no_workspaces_manifest_and_says_who_does() {
    let tmp = a_project();
    let manifest = root(&tmp).join("workspaces/alpha/workspace.json");
    let held = "{\"name\": \"alpha\", \"repos\": []}\n";
    std::fs::write(&manifest, held).unwrap();
    let mut chat = sandboxed_chat(Some("alpha"));
    // The Skill studio's grant is behind its flag (#2762).
    chat.push((purlis_core::skills::STUDIO_ENV.to_owned(), "1".to_owned()));
    for args in [
        &["workspace", "rename", "alpha", "gamma"][..],
        &["workspace", "fork", "alpha", "delta"][..],
        &["skill", "grant", "lint", "-w", "alpha", "--off"][..],
    ] {
        let out = purlis(&tmp, args, &chat);
        assert!(!out.status.success(), "{args:?}: {}", said(&out));
        assert!(
            said(&out).contains("a sandboxed chat does not write"),
            "{args:?}: {}",
            said(&out)
        );
    }
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), held);
    for made in ["gamma", "delta"] {
        assert!(!root(&tmp).join("workspaces").join(made).exists(), "{made}");
    }
    // Outside a chat, the same command is the person's, and runs.
    let out = purlis(&tmp, &["workspace", "rename", "alpha", "gamma"], &[]);
    assert!(
        !said(&out).contains("a sandboxed chat does not write"),
        "{}",
        said(&out)
    );
}

#[test]
fn workspace_snapshot_in_a_sandboxed_chat_is_recorded_by_the_app_in_its_own_workspace() {
    let tmp = a_project();
    // A clone in `alpha`, on a branch, with a commit: what a snapshot records.
    let web = root(&tmp).join("workspaces/alpha/web");
    std::fs::create_dir_all(&web).unwrap();
    std::fs::write(web.join("README"), "web\n").unwrap();
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["add", "README"][..],
        &["commit", "-q", "-m", "README"][..],
    ] {
        let done = std::process::Command::new("git")
            .arg("-C")
            .arg(&web)
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.templateDir=",
            ])
            .args(args)
            .output()
            .expect("git runs");
        assert!(done.status.success(), "git {args:?}: {done:?}");
    }
    let app = an_app(&tmp, None);
    let mut chat = app.chat_env();
    chat.push((hookwire::SANDBOXED_ENV.to_owned(), "1".to_owned()));

    let ran = purlis(
        &tmp,
        &[
            "workspace",
            "snapshot",
            "--description",
            "the login work",
            "--force",
        ],
        &chat,
    );

    assert!(ran.status.success(), "{}", said(&ran));
    assert_eq!(
        app.write_asked(),
        Write::WorkspaceSnapshot {
            description: Some("the login work".to_owned()),
            force: true,
        }
    );
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root(&tmp).join("workspaces/alpha/workspace.json"))
            .expect("the app recorded it"),
    )
    .expect("JSON");
    assert_eq!(manifest["description"], "the login work", "{manifest}");

    // Another workspace's is not the chat's to record.
    let other = purlis(&tmp, &["workspace", "snapshot", "beta"], &chat);
    assert!(!other.status.success(), "{}", said(&other));
    assert!(!root(&tmp).join("workspaces/beta/workspace.json").exists());
}
