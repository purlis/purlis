// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Recording a todo and forgetting one, as `charter ws todo` and the window's Todos panel both
//! do it (SI-3): one rule in the core, so a terminal and the window refuse the same things.

use purlis_core::workspaces::{Plane, RecordRefused, Workspace};

fn alpha(tmp: &tempfile::TempDir) -> Workspace {
    std::fs::write(tmp.path().join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(tmp.path().join("workspaces/alpha")).unwrap();
    Plane::open(tmp.path()).workspace("alpha").unwrap()
}

fn at() -> chrono::NaiveDateTime {
    "2026-05-04T11:32:17".parse().unwrap()
}

fn titles(ws: &Workspace) -> Vec<String> {
    ws.todos().unwrap().into_iter().map(|t| t.title).collect()
}

#[test]
fn a_todo_about_work_already_on_the_list_is_refused_with_the_one_it_repeats() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let ws = alpha(&tmp);
    ws.record_todo("Review the rollout plan for staging", at())
        .unwrap();

    let refused = ws
        .record_todo("review the rollout plan for staging today", at())
        .unwrap_err();

    match refused {
        RecordRefused::AlreadyListed(title) => {
            assert_eq!(title, "Review the rollout plan for staging");
        }
        other => panic!("{other}"),
    }
    assert_eq!(titles(&ws), ["Review the rollout plan for staging"]);
}

#[test]
fn a_todo_with_no_words_is_refused_and_nothing_is_written() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let ws = alpha(&tmp);

    let refused = ws.record_todo("   \n ", at()).unwrap_err();

    assert!(matches!(refused, RecordRefused::Empty), "{refused}");
    assert!(titles(&ws).is_empty());
}

#[test]
fn a_forgotten_todo_is_gone_and_the_journal_says_nothing_of_it() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let ws = alpha(&tmp);
    ws.record_todo("Drop the old importer", at()).unwrap();
    let slug = ws.todos().unwrap()[0].slug.clone();

    ws.forget_todo(&slug).unwrap();

    assert!(titles(&ws).is_empty());
    assert!(ws.memories().unwrap().is_empty(), "a forget was journalled");
}

#[test]
fn a_slug_that_reaches_out_of_the_todo_store_is_refused_and_its_target_survives() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let ws = alpha(&tmp);
    let victim = tmp.path().join("workspaces/alpha/workspace.md");
    std::fs::write(&victim, "# alpha\n").unwrap();

    let refused = ws.forget_todo("../workspace").unwrap_err();

    assert_eq!(
        refused.kind(),
        std::io::ErrorKind::InvalidInput,
        "{refused}"
    );
    assert!(victim.exists());
}

fn done_titles(ws: &Workspace) -> Vec<String> {
    ws.done_todos()
        .unwrap()
        .into_iter()
        .map(|t| t.title)
        .collect()
}

#[test]
fn a_done_todo_stays_and_reopens_where_it_was() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let ws = alpha(&tmp);
    ws.record_todo("Ship the importer", at()).unwrap();
    let slug = ws.todos().unwrap()[0].slug.clone();

    ws.close_todo(&slug, at()).unwrap();
    assert!(titles(&ws).is_empty());
    assert_eq!(done_titles(&ws), ["Ship the importer"]);

    let back = ws.reopen_todo(&slug).unwrap();

    assert_eq!(back.file_stem().unwrap().to_string_lossy(), slug);
    assert_eq!(titles(&ws), ["Ship the importer"]);
    assert!(done_titles(&ws).is_empty());
    let index = std::fs::read_to_string(ws.dir().join("todos/MEMORY.md")).unwrap();
    assert!(index.contains(&format!("({slug}.md)")), "{index}");
    // The journal keeps the close it recorded; a reopen adds nothing to it.
    assert_eq!(ws.memories().unwrap().len(), 1);
}

#[test]
fn a_done_todo_reopens_by_its_exact_slug_alone_and_an_open_one_does_not() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let ws = alpha(&tmp);
    ws.record_todo("Ship the importer", at()).unwrap();
    ws.record_todo("Write the notes", at()).unwrap();
    ws.close_todo("ship-the-importer", at()).unwrap();
    let stem = ws.done_todos().unwrap()[0].slug.clone();
    let open = ws.todos().unwrap()[0].slug.clone();

    let refused = ws.reopen_todo(&open).unwrap_err();
    assert_eq!(refused.kind(), std::io::ErrorKind::NotFound, "{refused}");
    assert!(refused.to_string().contains("no done todo"), "{refused}");
    // The exact name, and no other (SI-9d): a bare slug is a command line's to make exact.
    let bare = ws.reopen_todo("ship-the-importer").unwrap_err();
    assert_eq!(bare.kind(), std::io::ErrorKind::NotFound, "{bare}");
    assert!(ws.forget_done_todo("ship-the-importer").is_err());

    ws.reopen_todo(&stem).unwrap();
    assert_eq!(titles(&ws).len(), 2);
}

#[test]
fn a_todo_closed_and_reopened_twice_is_listed_once_in_its_index() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let ws = alpha(&tmp);
    ws.record_todo("Ship the importer", at()).unwrap();
    let slug = ws.todos().unwrap()[0].slug.clone();
    let index = || std::fs::read_to_string(ws.dir().join("todos/MEMORY.md")).unwrap();
    let line = format!("({slug}.md)");

    for _ in 0..2 {
        ws.close_todo(&slug, at()).unwrap();
        assert_eq!(index().matches(&line).count(), 0, "{}", index());
        ws.reopen_todo(&slug).unwrap();
        assert_eq!(index().matches(&line).count(), 1, "{}", index());
    }
    assert_eq!(titles(&ws), ["Ship the importer"]);
    assert!(done_titles(&ws).is_empty());
}

#[test]
fn a_done_todo_is_deleted_only_as_a_done_one_and_cannot_be_reopened() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let ws = alpha(&tmp);
    ws.record_todo("Drop the old importer", at()).unwrap();
    let slug = ws.todos().unwrap()[0].slug.clone();
    ws.close_todo(&slug, at()).unwrap();

    // Forgetting an open todo never reaches a done one (a promote's close, ADR 0106).
    let open_only = ws.forget_todo(&slug).unwrap_err();
    assert_eq!(
        open_only.kind(),
        std::io::ErrorKind::NotFound,
        "{open_only}"
    );
    assert_eq!(done_titles(&ws), ["Drop the old importer"]);

    ws.forget_done_todo(&slug).unwrap();

    assert!(done_titles(&ws).is_empty());
    assert!(titles(&ws).is_empty());
    assert!(ws.reopen_todo(&slug).is_err());
}

#[test]
fn a_done_todo_is_not_work_already_on_the_list() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let ws = alpha(&tmp);
    ws.record_todo("Review the rollout plan for staging", at())
        .unwrap();
    let slug = ws.todos().unwrap()[0].slug.clone();
    ws.close_todo(&slug, at()).unwrap();

    ws.record_todo("Review the rollout plan for staging", at())
        .unwrap();

    assert_eq!(titles(&ws), ["Review the rollout plan for staging"]);
}

#[test]
fn a_slug_that_reaches_out_of_the_done_todos_is_refused() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let ws = alpha(&tmp);
    let victim = tmp.path().join("workspaces/alpha/workspace.md");
    std::fs::write(&victim, "# alpha\n").unwrap();

    let refused = ws.reopen_todo("../../workspace").unwrap_err();

    assert_eq!(
        refused.kind(),
        std::io::ErrorKind::InvalidInput,
        "{refused}"
    );
    assert!(victim.exists());
}
