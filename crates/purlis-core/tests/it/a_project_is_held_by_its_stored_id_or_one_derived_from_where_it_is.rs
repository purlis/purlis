// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A project is named by a stable id: the ULID its `charter.toml` keeps as `[project] id`, so
//! every clone and every device names it the same (V76, ADR 0068 as amended by FD-26). The
//! session host holds a project by that id, and one that keeps none by an id derived from where
//! it is; holding a project writes nothing into it (#2004, D-2004-7, ruled 2026-10-16).

use purlis_core::scaffold::planefile;

fn project(text: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("charter.toml"), text).unwrap();
    root
}

const AN_ID: &str = "01J9ZZPRJCT0000000000000AB";

#[test]
fn an_id_written_by_hand_in_lowercase_reads_in_its_canonical_spelling() {
    purlis_core::unsteered!();
    let root = project(&format!(
        "[project]\nid = \"{}\"\n",
        AN_ID.to_ascii_lowercase()
    ));
    assert_eq!(planefile::project_id(root.path()).as_deref(), Some(AN_ID));
}

#[test]
fn an_id_that_is_not_a_ulid_reads_as_none() {
    purlis_core::unsteered!();
    let root = project("[project]\nid = \"not-a-ulid\"\n");
    assert_eq!(planefile::project_id(root.path()), None);
}

/// Holding a project writes nothing into it. One that keeps no id is held by one derived from
/// where it is, the same at every ask, whether or not this purlis may write it (a newer schema,
/// #826).
#[test]
fn a_project_with_no_id_is_held_without_a_write_and_by_the_same_id_each_time() {
    purlis_core::unsteered!();
    for text in ["schema = 1\n", "schema = 99\n"] {
        let root = project(text);

        let first = planefile::id_to_hold_by(root.path()).unwrap();
        let again = planefile::id_to_hold_by(root.path()).unwrap();

        assert!(ulid::Ulid::from_string(&first).is_ok(), "{first} is a ULID");
        assert_eq!(first, again);
        assert_eq!(
            std::fs::read_to_string(root.path().join("charter.toml")).unwrap(),
            text,
            "nothing was written"
        );
        let other = project(text);
        assert_ne!(planefile::id_to_hold_by(other.path()).unwrap(), first);
    }
}

#[test]
fn a_project_that_keeps_an_id_is_held_by_it() {
    purlis_core::unsteered!();
    let root = project(&format!("schema = 1\n\n[project]\nid = \"{AN_ID}\"\n"));

    assert_eq!(planefile::id_to_hold_by(root.path()).unwrap(), AN_ID);
}
