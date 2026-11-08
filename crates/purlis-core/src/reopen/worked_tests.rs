// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::super::{Record, path, write};
use super::*;
use std::fs;
use std::path::PathBuf;

/// A plane with the workspaces named, each a folder.
fn plane_with(workspaces: &[&str]) -> tempfile::TempDir {
    let plane = tempfile::tempdir().unwrap();
    for name in workspaces {
        fs::create_dir_all(plane.path().join("workspaces").join(name)).unwrap();
    }
    plane
}

/// A chat running as `persona` in `cwd`, a path from the plane's root.
fn chat(persona: Option<&str>, cwd: &str) -> Chat {
    Chat {
        program: "claude".to_owned(),
        cwd: Some(PathBuf::from(cwd)),
        persona: persona.map(str::to_owned),
        ..Chat::default()
    }
}

fn record(chats: Vec<Chat>) -> Record {
    Record {
        chats,
        ..Record::default()
    }
}

fn named(pairs: &[(&str, &[&str])]) -> Worked {
    pairs
        .iter()
        .map(|(workspace, personas)| {
            (
                (*workspace).to_owned(),
                personas.iter().map(|one| (*one).to_owned()).collect(),
            )
        })
        .collect()
}

#[test]
fn a_persona_whose_chat_closed_is_still_one_that_worked_in_its_workspace() {
    let plane = plane_with(&["alpha", "beta"]);
    fs::create_dir_all(plane.path().join("workspaces/beta/repos/api")).unwrap();
    write(
        plane.path(),
        &record(vec![
            chat(Some("devops"), "workspaces/alpha"),
            chat(Some("steward"), "workspaces/beta/repos/api"),
        ]),
    )
    .unwrap();

    // devops's chat closed: the next record holds only steward's.
    write(
        plane.path(),
        &record(vec![chat(Some("steward"), "workspaces/beta/repos/api")]),
    )
    .unwrap();

    assert_eq!(
        read(plane.path()),
        named(&[("alpha", &["devops"]), ("beta", &["steward"])])
    );
}

#[test]
fn a_fresh_start_keeps_who_worked_where() {
    let plane = plane_with(&["alpha"]);
    write(
        plane.path(),
        &record(vec![chat(Some("devops"), "workspaces/alpha")]),
    )
    .unwrap();

    write(
        plane.path(),
        &record(vec![]).chosen(super::super::Choice::StartFresh),
    )
    .unwrap();

    assert_eq!(read(plane.path()), named(&[("alpha", &["devops"])]));
}

#[test]
fn a_chat_on_no_persona_or_outside_every_workspace_is_not_counted() {
    let plane = plane_with(&["alpha"]);
    write(
        plane.path(),
        &record(vec![
            chat(None, "workspaces/alpha"),
            chat(Some("steward"), "."),
            chat(Some("../escape"), "workspaces/alpha"),
        ]),
    )
    .unwrap();

    assert_eq!(read(plane.path()), Worked::new());
    let text = fs::read_to_string(path(plane.path())).unwrap();
    assert!(
        !text.contains("\"worked\""),
        "a record with nobody in it writes no key: {text}"
    );
}

#[test]
fn a_workspace_that_is_gone_is_let_go_of() {
    let plane = plane_with(&["alpha", "beta"]);
    write(
        plane.path(),
        &record(vec![
            chat(Some("devops"), "workspaces/alpha"),
            chat(Some("steward"), "workspaces/beta"),
        ]),
    )
    .unwrap();

    fs::remove_dir_all(plane.path().join("workspaces/beta")).unwrap();
    assert_eq!(read(plane.path()), named(&[("alpha", &["devops"])]));

    write(plane.path(), &record(vec![])).unwrap();
    fs::create_dir_all(plane.path().join("workspaces/beta")).unwrap();
    assert_eq!(
        read(plane.path()),
        named(&[("alpha", &["devops"])]),
        "made again under its name, it starts with nobody"
    );
}

#[test]
fn a_record_that_does_not_read_or_a_name_out_of_rule_is_none_of_it() {
    let plane = plane_with(&["alpha"]);
    let file = path(plane.path());
    fs::create_dir_all(file.parent().unwrap()).unwrap();

    fs::write(&file, "not json").unwrap();
    assert_eq!(read(plane.path()), Worked::new());

    fs::write(
        &file,
        r#"{"version": 1, "at": 0, "chats": [], "worked": {"alpha": ["devops", "../x"], "../y": ["ops"]}}"#,
    )
    .unwrap();
    assert_eq!(read(plane.path()), named(&[("alpha", &["devops"])]));

    fs::write(
        &file,
        r#"{"version": 1, "at": 0, "chats": [], "worked": "everyone"}"#,
    )
    .unwrap();
    assert_eq!(read(plane.path()), Worked::new());

    fs::write(
        &file,
        r#"{"version": 2, "at": 0, "chats": [], "worked": {"alpha": ["devops"]}}"#,
    )
    .unwrap();
    assert_eq!(
        read(plane.path()),
        Worked::new(),
        "a record of another version is ignored whole"
    );
}
