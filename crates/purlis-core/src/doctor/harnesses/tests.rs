// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::doctor::Status;
use crate::harness_declaration::{Origin, Refused};

fn builtins() -> Declarations {
    Declarations {
        declared: harness_declaration::builtins().to_vec(),
        refused: Vec::new(),
    }
}

#[test]
fn the_harnesses_purlis_ships_are_named_with_the_most_each_offers() {
    let row = row_of(&builtins());

    assert_eq!(row.name, NAME);
    assert_eq!(row.status, Status::Ok);
    assert_eq!(
        row.detail,
        "3 harnesses — claude: level 2 (hooks); opencode: level 3 (ACP); codex: level 2 \
         (hooks)"
    );
}

#[test]
fn a_projects_terminal_only_harness_is_held_to_level_1() {
    let mut declared = builtins();
    let text = "name = \"gemini\"\nprogram = \"gemini\"\n\n[session]\nchosen_by = \
                \"harness\"\n\n[terminal]\nready_to_type = \"never\"\n\n[levels]\n\
                terminal = true\n";
    declared.declared.push(
        harness_declaration::parse(text, Origin::Project, "harnesses/gemini.toml")
            .expect("a terminal-only declaration reads"),
    );

    let row = row_of(&declared);

    assert_eq!(row.status, Status::Ok);
    assert!(
        row.detail.ends_with("; gemini: level 1 (terminal only)"),
        "{}",
        row.detail
    );
    assert!(row.detail.starts_with("4 harnesses — "), "{}", row.detail);
}

#[test]
fn a_refused_declaration_is_a_warning_that_carries_the_readers_reason() {
    let mut declared = builtins();
    declared.refused.push(Refused {
        file: "harnesses/x.toml".to_owned(),
        reason: "harnesses/x.toml declares 'y', and a declaration's file is named after \
                 the harness it declares.\nRename it."
            .to_owned(),
    });

    let row = row_of(&declared);

    assert_eq!(row.status, Status::Warn);
    assert!(
        row.detail
            .starts_with("1 declaration refused: harnesses/x.toml declares 'y'"),
        "{}",
        row.detail
    );
    // A reason is one line in the row, whatever the file held.
    assert!(!row.detail.contains('\n'), "{}", row.detail);
    assert!(
        row.detail.contains("(3 harnesses read — claude"),
        "{}",
        row.detail
    );
    assert!(row.hint.contains("harnesses/"), "{}", row.hint);
    assert!(!row.deferred());
}

#[test]
fn a_project_declaration_on_disk_is_read_by_the_row() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir_all(root.join("harnesses")).unwrap();
    std::fs::write(
        root.join("harnesses/other.toml"),
        "name = \"else\"\nprogram = \"x\"\n",
    )
    .unwrap();

    let row = row_of(&harness_declaration::read(&root));

    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert!(
        row.detail.contains("harnesses/other.toml"),
        "{}",
        row.detail
    );
}
