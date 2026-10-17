// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_gitattributes_charter_cannot_read_is_refused_rather_than_written_over_as_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitattributes");
    // Not UTF-8: there, and unreadable as text.
    std::fs::write(&path, b"*.bin binary \xff\xfe\n").unwrap();

    assert!(ensure_gitattributes(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"*.bin binary \xff\xfe\n");
}

#[test]
fn the_merge_rules_are_written_once_kept_beside_anyone_elses_and_brought_up_to_date() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitattributes");

    assert!(ensure_gitattributes(&path).unwrap(), "a fresh file");
    let first = std::fs::read_to_string(&path).unwrap();
    assert!(first.starts_with(MERGE_RULES_BEGIN), "{first}");
    assert!(
        first.contains("personas/_dispatch/*.jsonl merge=union\n"),
        "{first}"
    );
    assert!(first.trim_end().ends_with(MERGE_RULES_END), "{first}");
    assert!(
        !ensure_gitattributes(&path).unwrap(),
        "twice changes nothing"
    );

    // Somebody's own line, and a block an older charter wrote: kept, and replaced.
    std::fs::write(
        &path,
        format!("*.png binary\n{MERGE_RULES_BEGIN}\nold.jsonl merge=union\n{MERGE_RULES_END}\n"),
    )
    .unwrap();
    assert!(ensure_gitattributes(&path).unwrap());
    let now = std::fs::read_to_string(&path).unwrap();
    assert!(now.starts_with("*.png binary\n"), "{now}");
    assert!(!now.contains("old.jsonl"), "{now}");
    assert_eq!(now.matches(MERGE_RULES_BEGIN).count(), 1, "{now}");
}

/// What a plane that is not migrated writes: charter's markers (V93g).
const MERGE_RULES_BEGIN: &str = names::MERGE_RULES_BEGIN.reads[0];
const MERGE_RULES_END: &str = names::MERGE_RULES_END.reads[0];

/// Exactly what charter wrote before the rename, between somebody's own lines.
const CHARTERS_ATTRIBUTES: &str = "*.png binary\n\
               # >>> charter merge rules (managed by charter) >>>\n\
               old.jsonl merge=union\n\
               # <<< charter merge rules <<<\n\
               *.pdf binary\n";

#[test]
fn a_plane_not_yet_migrated_keeps_charters_merge_rules_block_byte_for_byte() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    let path = dir.path().join(".gitattributes");
    std::fs::write(&path, CHARTERS_ATTRIBUTES).unwrap();

    assert!(ensure_gitattributes(&path).unwrap(), "the stale rules");
    let now = std::fs::read_to_string(&path).unwrap();
    let block = merge_rules_block(dir.path());
    assert!(block.starts_with(MERGE_RULES_BEGIN), "{block}");
    assert_eq!(now, format!("*.png binary\n{block}\n*.pdf binary\n"));
    assert!(!now.contains("purlis"), "{now}");
    assert!(!ensure_gitattributes(&path).unwrap(), "byte for byte after");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), now);

    // A purlis block in such a plane is recognised too: one block, charter's markers.
    std::fs::write(
        &path,
        now.replace(MERGE_RULES_BEGIN, names::MERGE_RULES_BEGIN.write)
            .replace(MERGE_RULES_END, names::MERGE_RULES_END.write),
    )
    .unwrap();
    assert!(ensure_gitattributes(&path).unwrap());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), now);
}

#[test]
fn a_migrated_plane_rewrites_charters_merge_rules_block_in_place_under_the_purlis_markers() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("purlis.toml"), "").unwrap();
    let path = dir.path().join(".gitattributes");
    std::fs::write(&path, CHARTERS_ATTRIBUTES).unwrap();

    assert!(ensure_gitattributes(&path).unwrap());
    let now = std::fs::read_to_string(&path).unwrap();
    assert!(!now.contains("charter merge rules"), "{now}");
    assert!(!now.contains("old.jsonl"), "{now}");
    assert_eq!(
        now,
        format!(
            "*.png binary\n{}\n*.pdf binary\n",
            merge_rules_block(dir.path())
        ),
        "rewritten where it stood"
    );
    assert_eq!(now.matches("merge rules (managed by").count(), 1, "{now}");
    assert!(
        now.starts_with("*.png binary\n# >>> purlis merge rules (managed by purlis) >>>\n"),
        "{now}"
    );
    assert!(
        !ensure_gitattributes(&path).unwrap(),
        "the purlis block is recognised"
    );
}

#[test]
fn a_half_block_somebody_edited_is_left_as_it_is_and_the_rules_are_written_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitattributes");
    for start in [
        format!("{MERGE_RULES_BEGIN}\nmine\n"),
        format!("{MERGE_RULES_END}\nx\n"),
    ] {
        std::fs::write(&path, &start).unwrap();
        assert!(ensure_gitattributes(&path).unwrap());
        let once = std::fs::read_to_string(&path).unwrap();
        assert!(
            once.starts_with(&start),
            "the half block was touched: {once}"
        );
        assert!(
            !ensure_gitattributes(&path).unwrap(),
            "a second run changed it"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), once);
    }
}
