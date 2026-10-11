// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis workspace create` run in a chat (#1408, ADR 0067 §2's "creating a workspace"): the
//! app makes a new workspace, with its vision, and nothing that is already there.

use super::*;
use crate::repocmd::Say;

fn now() -> chrono::NaiveDateTime {
    "2026-10-12T12:00:00".parse().expect("a stamp")
}

/// A project with workspace `alpha`, which holds one file.
fn a_project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("project");
    std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
    std::fs::write(root.join("workspaces/alpha/notes.md"), "mine\n").unwrap();
    (dir, root)
}

fn chat_in_alpha() -> Asker {
    Asker {
        chat: 7,
        place: Place::Workspace("alpha".to_owned()),
        persona: Some("steward".to_owned()),
    }
}

fn create(name: &str, vision: Option<&str>) -> Write {
    Write::WorkspaceCreate {
        name: name.to_owned(),
        vision: vision.map(str::to_owned),
    }
}

/// Every file under `dir`, with its bytes.
fn tree(dir: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut all = std::collections::BTreeMap::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(at) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                all.insert(path.clone(), Vec::new());
                todo.push(path);
            } else if kind.is_symlink() {
                all.insert(path.clone(), b"link".to_vec());
            } else {
                all.insert(path.clone(), std::fs::read(&path).unwrap());
            }
        }
    }
    all
}

#[test]
fn a_chat_creates_a_new_workspace_with_its_vision_and_it_is_credited_to_the_chat() {
    let (_dir, root) = a_project();

    let done = answer(
        &root,
        &chat_in_alpha(),
        &create("gamma", Some("Port the docs command")),
        now(),
    )
    .expect("made");

    let Done::Said { lines, code } = done else {
        panic!("a create answers in lines: {done:?}");
    };
    assert_eq!(code, 0, "{lines:?}");
    assert!(
        lines.iter().any(
            |line| matches!(line, Say::Done(text) if text.contains("Workspace 'gamma' ready"))
        ),
        "{lines:?}"
    );
    let ws = root.join("workspaces/gamma");
    assert!(ws.join("workspace.md").is_file());
    let charter = std::fs::read_to_string(ws.join("workspace.md")).unwrap();
    assert!(charter.contains("Port the docs command"), "{charter}");
    let trace = std::fs::read_to_string(crate::trace::file(&root, "7")).expect("chat 7's trace");
    assert!(trace.contains("\"workspace_create\""), "{trace}");
    assert!(trace.contains("workspaces/gamma"), "{trace}");
}

#[test]
fn a_workspace_that_is_there_is_refused_and_left_as_it_was() {
    // A terminal's `create` on a workspace that is there repairs it; a chat's makes only a
    // new one, so it never rewrites one a chat already has, its own or another.
    let (_dir, root) = a_project();
    std::fs::create_dir_all(root.join("workspaces/beta")).unwrap();
    let before = tree(&root);

    for name in ["alpha", "beta"] {
        let why = answer(
            &root,
            &chat_in_alpha(),
            &create(name, Some("Over it")),
            now(),
        )
        .expect_err(name);
        assert!(why.contains("already"), "{why}");
        assert!(why.contains("nothing was created"), "{why}");
    }
    assert_eq!(tree(&root), before);
}

#[cfg(unix)]
#[test]
fn anything_at_the_new_workspace_s_path_is_refused_and_never_followed() {
    let (dir, root) = a_project();
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, root.join("workspaces/gamma")).unwrap();
    std::os::unix::fs::symlink(dir.path().join("nowhere"), root.join("workspaces/delta")).unwrap();
    std::fs::write(root.join("workspaces/epsilon"), "a file\n").unwrap();
    let before = tree(dir.path());

    for name in ["gamma", "delta", "epsilon"] {
        let why = answer(
            &root,
            &chat_in_alpha(),
            &create(name, Some("Over it")),
            now(),
        )
        .expect_err(name);
        assert!(why.contains("nothing was created"), "{name}: {why}");
    }
    assert_eq!(tree(dir.path()), before);
    assert!(!dir.path().join("nowhere").exists());
}

#[cfg(unix)]
#[test]
fn a_workspaces_folder_that_is_a_link_is_never_written_through() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, root.join("workspaces")).unwrap();

    let why = answer(&root, &chat_in_alpha(), &create("gamma", None), now()).expect_err("link");

    assert!(why.contains("nothing was created"), "{why}");
    assert!(tree(&elsewhere).is_empty());
}

#[test]
fn a_name_that_is_not_a_workspace_s_creates_nothing() {
    let (dir, root) = a_project();
    let before = tree(dir.path());

    for name in ["", "../escape", "a/b", ".hidden", "/abs", "nul", "gamma\n"] {
        // Refused before anything is looked at, or by `create`'s own refusal, said.
        let refused = answer(&root, &chat_in_alpha(), &create(name, None), now());
        assert!(
            matches!(refused, Err(_) | Ok(Done::Said { code: 1.., .. })),
            "{name:?} was made: {refused:?}"
        );
    }
    assert_eq!(tree(dir.path()), before);
}

#[test]
fn a_create_reads_off_the_wire_with_its_name_and_vision_and_nothing_else() {
    let write: Write =
        serde_json::from_str(r#"{"op":"workspace_create","name":"gamma","vision":"Docs"}"#)
            .expect("it reads");
    assert_eq!(write, create("gamma", Some("Docs")));
    let bare: Write =
        serde_json::from_str(r#"{"op":"workspace_create","name":"gamma"}"#).expect("it reads");
    assert_eq!(bare, create("gamma", None));
    // What a terminal's create takes beside them is not the chat's to ask for: the write has
    // nowhere to carry it, so a line that names it asks for the plain create.
    for more in [
        r#"{"op":"workspace_create","name":"gamma","live":true}"#,
        r#"{"op":"workspace_create","name":"gamma","use_it":true}"#,
        r#"{"op":"workspace_create","name":"gamma","repos":["widget"]}"#,
    ] {
        let read: Write = serde_json::from_str(more).expect("it reads");
        assert_eq!(read, create("gamma", None), "{more}");
    }
}

#[test]
fn a_vision_too_long_to_carry_is_refused_and_nothing_is_created() {
    let (_dir, root) = a_project();
    let long = "x".repeat(MOST_TEXT_BYTES + 1);

    let why = answer(
        &root,
        &chat_in_alpha(),
        &create("gamma", Some(&long)),
        now(),
    )
    .expect_err("too long");

    assert!(why.contains("bytes"), "{why}");
    assert!(!root.join("workspaces/gamma").exists());
}

#[test]
fn the_largest_create_a_chat_may_ask_for_is_no_longer_on_the_socket_than_the_largest_memory() {
    // `hookwire`'s own test shows the largest memory fits one line of the socket; a create's
    // name and vision share the same bound, so it fits as well.
    let most = MOST_TEXT_BYTES;
    let line_of = |write: Write| {
        assert_eq!(write.check(), Ok(()));
        let ask = crate::hookwire::Ask::Write(Box::new(crate::hookwire::WriteAsk {
            chat: u32::MAX,
            write,
        }));
        crate::hookwire::line_with(None, &ask)
            .expect("a line")
            .len()
    };
    let memory = line_of(Write::PersonaRemember {
        text: "\u{1}".repeat(most),
        title: None,
        shared: true,
    });
    let created = line_of(create(
        &"\u{1}".repeat(most / 2),
        Some(&"\u{1}".repeat(most / 2)),
    ));
    assert!(created <= memory, "{created} > {memory}");
    assert!(create("x", Some(&"x".repeat(most))).check().is_err());
}
