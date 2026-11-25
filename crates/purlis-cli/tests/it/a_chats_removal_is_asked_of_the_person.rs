// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **B2 and B3, one removal ask** (T1.1-67, R-328), through the production wiring: a sandboxed
//! chat runs `purlis persona remove` and `purlis worktree remove --force`, the command hands the
//! removal to the app over the chat's hook socket (`Ask::Removal`), and the chat is told what the
//! app answered: that the person was asked, that nobody was (no window listens), that the app and
//! this purlis are different builds, or that no app was there.
//!
//! The app here is a listener that answers as the real one does once it has looked the chat up
//! (`app/src-tauri/src/removalasks.rs`): by `purlis_core::windowonly::ask_to_remove`, for chat 3
//! in `alpha`. Every project runs under `[policy] template = "strict"`, set explicitly (Q308).
#![cfg(unix)]

use crate::support;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use purlis_core::hookwire::{
    self, Answer, Ask, CHAT_ENV, Listener, Removing, SANDBOXED_ENV, SOCKET_ENV, TOKEN_ENV,
};
use purlis_core::windowonly::{self, Control};

fn git(dir: &Path, args: &[&str]) {
    let mut command = std::process::Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(args);
    assert!(command.output().unwrap().status.success(), "git {args:?}");
}

/// A project, strict, with the persona `steward` holding a memory and workspace `alpha` holding
/// the clone `thing` and its branch folder `piece`, which holds an uncommitted change.
fn a_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    let clone = root.join("workspaces/alpha/repos/thing");
    std::fs::create_dir_all(&clone).unwrap();
    std::fs::write(
        root.join("charter.toml"),
        "[policy]\ntemplate = \"strict\"\n",
    )
    .unwrap();
    git(&clone, &["init", "-q", "-b", "main"]);
    std::fs::write(clone.join("README.md"), "one\n").unwrap();
    git(&clone, &["add", "README.md"]);
    git(&clone, &["commit", "-q", "-m", "one"]);
    let steward = root.join("personas/steward");
    std::fs::create_dir_all(steward.join("memory")).unwrap();
    std::fs::write(
        steward.join("persona.md"),
        "---\nrole: steward\n---\n# steward\n",
    )
    .unwrap();
    std::fs::write(steward.join("memory/one.md"), "one\n").unwrap();
    std::fs::create_dir_all(dir.path().join("home")).unwrap();
    // The branch folder, cut with git as a terminal would, with a change the removal discards.
    let folder = root.join("workspaces/alpha/.worktrees/thing/piece");
    std::fs::create_dir_all(folder.parent().unwrap()).unwrap();
    git(
        &clone,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "piece",
            folder.to_str().unwrap(),
        ],
    );
    std::fs::write(
        root.join("workspaces/alpha/.worktrees/thing/piece/README.md"),
        "changed\n",
    )
    .unwrap();
    dir
}

fn root(tmp: &tempfile::TempDir) -> PathBuf {
    tmp.path().join("project")
}

fn purlis(
    tmp: &tempfile::TempDir,
    args: &[&str],
    env: &[(String, String)],
) -> std::process::Output {
    support::purlis()
        .args(args)
        .current_dir(root(tmp).join("workspaces/alpha"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("CHARTER_ROOT", root(tmp))
        .env("HOME", tmp.path().join("home"))
        .env("TMPDIR", std::env::temp_dir())
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .output()
        .expect("purlis runs")
}

fn said(ran: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    )
}

/// The app, keeping what it was asked.
struct App {
    _reading: hookwire::Reading,
    env: Vec<(String, String)>,
    asked: Arc<Mutex<Vec<Ask>>>,
}

fn an_app(
    tmp: &tempfile::TempDir,
    answer: impl Fn(&Path, &Ask) -> Answer + Send + Sync + 'static,
) -> App {
    let socket = tmp.path().join("app").join("hooks.sock");
    let listener = Listener::bind(tmp.path(), &socket).unwrap_or_else(|e| panic!("a socket: {e}"));
    let token = listener.tokens().issue_to_this_process(3).expect("a token");
    let project = root(tmp);
    let asked = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&asked);
    let reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, ask| {
            kept.lock().unwrap().push(ask.clone());
            answer(&project, &ask)
        }),
    );
    App {
        _reading: reading,
        env: vec![
            (SOCKET_ENV.to_owned(), socket.display().to_string()),
            (CHAT_ENV.to_owned(), "3".to_owned()),
            (TOKEN_ENV.to_owned(), token.expose().to_owned()),
            (SANDBOXED_ENV.to_owned(), "1".to_owned()),
            (
                purlis_core::active::WORKSPACE_ENV.to_owned(),
                "alpha".to_owned(),
            ),
        ],
        asked,
    }
}

/// What the real app answers a removal ask: `ask_to_remove` for chat 3's workspace.
fn as_the_app(root: &Path, ask: &Ask) -> Answer {
    let Ask::Removal(asked) = ask else {
        return Answer::No {
            why: "not a removal".to_owned(),
        };
    };
    let who = purlis_core::pieces::Who::here(None, Some(asked.chat.to_string()), None);
    let asking = windowonly::Asking {
        session: asked.chat,
        who: &who,
        live: Some(&[]),
        claimed: false,
    };
    Answer::No {
        why: windowonly::ask_to_remove(root, &asking, Some("alpha"), &asked.removing).words(),
    }
}

/// The person's press, as the window makes it.
fn pressed(root: &Path, control: &Control) -> Result<String, String> {
    windowonly::remove_listed(
        root,
        control,
        &purlis_core::pieces::Who::here(None, None, None),
    )
}

#[test]
fn a_chat_removing_a_persona_raises_the_one_removal_ask_and_is_told_the_person_was_asked() {
    let tmp = a_project();
    let root = root(&tmp);
    // A window listens for the project.
    windowonly::telling(&root, Arc::new(|| {}));
    let app = an_app(&tmp, as_the_app);

    let ran = purlis(&tmp, &["persona", "remove", "steward"], &app.env);

    assert_eq!(ran.status.code(), Some(1), "{}", said(&ran));
    assert!(
        said(&ran).contains("purlis asked the person in Asks"),
        "{}",
        said(&ran)
    );
    // The app was handed the name and the chat, and nothing else.
    assert!(matches!(
        app.asked.lock().unwrap().as_slice(),
        [Ask::Removal(asked)]
            if asked.chat == 3
                && asked.removing == Removing::Persona { name: "steward".to_owned() }
    ));
    // One ask for the chat, listing exactly what goes, and nothing was removed by asking.
    let raised = windowonly::every(&root);
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].session, 3);
    let Control::Remove { listed, exact, .. } = &raised[0].control else {
        panic!("{:?}", raised[0].control);
    };
    assert!(*exact, "{listed:?}");
    assert!(
        listed
            .iter()
            .any(|l| l == "personas/steward/memory: 1 memories"),
        "{listed:?}"
    );
    assert!(root.join("personas/steward/persona.md").is_file());

    // The person's press removes exactly that, and clears the ask.
    pressed(&root, &raised[0].control).unwrap();
    assert!(!root.join("personas/steward").exists());
    assert_eq!(windowonly::every(&root), []);
    windowonly::project_ended(&root);
}

#[test]
fn a_chat_forcing_a_branch_folder_removal_raises_the_one_removal_ask_listing_what_is_lost() {
    let tmp = a_project();
    let root = root(&tmp);
    windowonly::telling(&root, Arc::new(|| {}));
    let app = an_app(&tmp, as_the_app);

    let ran = purlis(
        &tmp,
        &["worktree", "remove", "thing", "piece", "--force"],
        &app.env,
    );

    assert_eq!(ran.status.code(), Some(1), "{}", said(&ran));
    assert!(
        said(&ran).contains("purlis asked the person in Asks"),
        "{}",
        said(&ran)
    );
    // Only the repo and the piece travelled, and the flag that says a plain removal is not enough.
    assert!(matches!(
        app.asked.lock().unwrap().as_slice(),
        [Ask::Removal(asked)]
            if asked.chat == 3
                && asked.removing == Removing::BranchFolder {
                    repo: "thing".to_owned(),
                    piece: "piece".to_owned(),
                    force: true,
                    ignored: false,
                }
    ));
    let raised = windowonly::every(&root);
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].control.label(), "Remove and discard");
    let Control::Remove { listed, .. } = &raised[0].control else {
        panic!("{:?}", raised[0].control);
    };
    assert!(listed.iter().any(|l| l.contains("README.md")), "{listed:?}");
    // Nothing was discarded by asking.
    let piece = root.join("workspaces/alpha/.worktrees/thing/piece");
    assert_eq!(
        std::fs::read_to_string(piece.join("README.md")).unwrap(),
        "changed\n"
    );

    // The press discards exactly what was listed, and only while it still holds exactly that.
    pressed(&root, &raised[0].control).unwrap();
    assert!(!piece.exists());
    windowonly::project_ended(&root);
}

#[test]
fn where_no_window_listens_the_chat_is_told_nobody_was_asked_and_a_block_is_written() {
    let tmp = a_project();
    let root = root(&tmp);
    let blocks: Arc<Mutex<Vec<u32>>> = Arc::default();
    windowonly::blocking(&root, {
        let blocks = Arc::clone(&blocks);
        Arc::new(move |session, _stop| blocks.lock().unwrap().push(session))
    });
    let app = an_app(&tmp, as_the_app);

    let ran = purlis(&tmp, &["persona", "remove", "steward"], &app.env);

    assert!(said(&ran).contains("Nobody was asked"), "{}", said(&ran));
    assert!(
        !said(&ran).contains("asked the person in Asks"),
        "{}",
        said(&ran)
    );
    assert_eq!(
        *blocks.lock().unwrap(),
        [3],
        "one block, for the chat that was refused"
    );
    windowonly::project_ended(&root);
}

#[test]
fn an_app_older_than_the_new_removing_variants_is_told_apart_from_a_refusal() {
    let tmp = a_project();
    let app = an_app(&tmp, |_, _| Answer::No {
        why: hookwire::UNKNOWN_LINE.to_owned(),
    });

    for args in [
        &["persona", "remove", "steward"][..],
        &["worktree", "remove", "thing", "piece", "--force"][..],
    ] {
        let ran = purlis(&tmp, args, &app.env);
        let said = said(&ran);
        for words in [
            "different builds",
            "restart purlis",
            "update the purlis plugin",
        ] {
            assert!(said.contains(words), "{args:?}: {said}");
        }
        assert!(
            !said.contains("could not read this line"),
            "{args:?}: {said}"
        );
    }
    assert!(root(&tmp).join("personas/steward/persona.md").is_file());
}

#[test]
fn with_no_app_at_all_the_chat_is_told_nobody_was_asked_and_nothing_is_removed() {
    let tmp = a_project();
    let env = vec![
        (
            SOCKET_ENV.to_owned(),
            tmp.path().join("gone.sock").display().to_string(),
        ),
        (CHAT_ENV.to_owned(), "3".to_owned()),
        (SANDBOXED_ENV.to_owned(), "1".to_owned()),
    ];

    let ran = purlis(&tmp, &["persona", "remove", "steward"], &env);

    assert!(
        said(&ran).contains("No purlis window answered, so nobody was asked"),
        "{}",
        said(&ran)
    );
    assert!(root(&tmp).join("personas/steward/persona.md").is_file());
}

#[test]
fn a_workspace_removal_asks_as_today_while_an_app_answers() {
    // The workspace's removal is not widened by the removal ask: the app answers it as it did
    // (`removalask::asked_said`), the chat is told the person was asked, and nothing is removed.
    let tmp = a_project();
    let root = root(&tmp);
    let app = an_app(&tmp, |_, ask| {
        let Ask::Removal(asked) = ask else {
            return Answer::No {
                why: "not a removal".to_owned(),
            };
        };
        let Removing::Workspace { name } = &asked.removing else {
            return Answer::No {
                why: "not a workspace".to_owned(),
            };
        };
        Answer::No {
            why: purlis_core::removalask::asked_said(name),
        }
    });

    let ran = purlis(&tmp, &["ws", "remove", "alpha"], &app.env);

    assert_eq!(ran.status.code(), Some(1), "{}", said(&ran));
    assert!(
        said(&ran).contains("asked the person, in its window, whether to remove 'alpha'"),
        "{}",
        said(&ran)
    );
    assert!(
        said(&ran).contains("Wait for their answer"),
        "{}",
        said(&ran)
    );
    assert!(!said(&ran).contains("nobody was asked"), "{}", said(&ran));
    // Only the name travelled, and nothing was removed.
    assert!(matches!(
        app.asked.lock().unwrap().as_slice(),
        [Ask::Removal(asked)]
            if asked.removing == Removing::Workspace { name: "alpha".to_owned() }
    ));
    assert!(root.join("workspaces/alpha").exists());
}
