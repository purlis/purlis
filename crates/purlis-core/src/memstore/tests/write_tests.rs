// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

// The expectations here are what `charter.memstore.write` produced in a temp plane with
// `stamp=datetime(2026, 3, 2, 9, 14)`.

fn stamp() -> chrono::NaiveDateTime {
    "2026-03-02T09:14:00".parse().unwrap()
}

/// A real plane, because the store gate asks whether a path is inside one.
fn store() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "schema = 1\n").unwrap();
    let store = dir.path().join("workspaces/alpha/todos");
    ensure_index(
            dir.path(),
            &store,
            "# Todos — workspace `alpha`\n\nOne line per todo; each links a file holding one thing this task still means to do.\nOpen or done — and done removes it, leaving its trace in the journal instead.\n",
        )
        .unwrap();
    (dir, store)
}

#[test]
fn a_timestamped_memory_is_named_for_its_second_and_its_title() {
    let (tmp, store) = store();

    let path = write(
        tmp.path(),
        &store,
        "Review the rollout plan",
        None,
        true,
        "persistent",
        true,
        stamp(),
    )
    .unwrap();

    assert_eq!(
        path.file_name().unwrap(),
        "20260302-091400-review-the-rollout-plan.md"
    );
}

#[test]
fn a_memory_file_is_a_heading_a_stamp_line_and_the_body() {
    let (tmp, store) = store();

    let path = write(
        tmp.path(),
        &store,
        "Review the rollout plan",
        None,
        true,
        "persistent",
        true,
        stamp(),
    )
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "# Review the rollout plan\n\n_2026-03-02 09:14 · persistent_\n\nReview the rollout plan\n"
    );
}

#[test]
fn the_title_of_a_multi_line_body_is_its_first_line_and_the_body_keeps_the_rest() {
    let (tmp, store) = store();

    let path = write(
        tmp.path(),
        &store,
        "Multi line\nbody here",
        None,
        true,
        "persistent",
        true,
        stamp(),
    )
    .unwrap();

    assert_eq!(path.file_name().unwrap(), "20260302-091400-multi-line.md");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "# Multi line\n\n_2026-03-02 09:14 · persistent_\n\nMulti line\nbody here\n"
    );
}

#[test]
fn two_memories_with_one_title_in_one_second_are_both_kept() {
    let (tmp, store) = store();

    write(
        tmp.path(),
        &store,
        "Review the rollout plan",
        None,
        true,
        "persistent",
        true,
        stamp(),
    )
    .unwrap();
    let second = write(
        tmp.path(),
        &store,
        "Review the rollout plan",
        None,
        true,
        "persistent",
        true,
        stamp(),
    )
    .unwrap();

    assert_eq!(
        second.file_name().unwrap(),
        "20260302-091400-review-the-rollout-plan-2.md"
    );
}

#[test]
fn the_index_gains_one_line_per_memory_in_the_order_they_were_written() {
    let (tmp, store) = store();

    write(
        tmp.path(),
        &store,
        "Review the rollout plan",
        None,
        true,
        "persistent",
        true,
        stamp(),
    )
    .unwrap();
    write(
        tmp.path(),
        &store,
        "Review the rollout plan",
        None,
        true,
        "persistent",
        true,
        stamp(),
    )
    .unwrap();
    write(
        tmp.path(),
        &store,
        "Multi line\nbody here",
        None,
        true,
        "persistent",
        true,
        stamp(),
    )
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(store.join("MEMORY.md")).unwrap(),
        "# Todos — workspace `alpha`\n\nOne line per todo; each links a file holding one thing this task still means to do.\nOpen or done — and done removes it, leaving its trace in the journal instead.\n- [Review the rollout plan](20260302-091400-review-the-rollout-plan.md)\n- [Review the rollout plan](20260302-091400-review-the-rollout-plan-2.md)\n- [Multi line](20260302-091400-multi-line.md)\n"
    );
}

#[test]
fn an_empty_body_is_refused_because_a_secret_must_never_be_written_here() {
    let (tmp, store) = store();

    assert!(
        write(
            tmp.path(),
            &store,
            "   \n\n ",
            None,
            true,
            "persistent",
            true,
            stamp()
        )
        .is_err()
    );
}
