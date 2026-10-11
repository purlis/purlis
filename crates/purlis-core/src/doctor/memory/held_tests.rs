// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #1083: `doctor`'s memory rows list a base and compare its index through the store held by
//! descriptor: asked while a workspace's `memory/` is swapped for a link to a persona's, they
//! never list or read what is there.

use crate::held::racer::{project, while_swapped};

#[test]
fn a_memory_swapped_for_a_link_never_has_a_personas_entries_or_index_read() {
    let (_tmp, root) = project(&[
        ("workspaces/alpha/memory", 1, "Ours"),
        ("personas/devops/memory", 3, "Theirs"),
    ]);
    // Theirs drifts, so reading its index would answer a drift ours does not have.
    std::fs::write(
        root.join("personas/devops/memory/MEMORY.md"),
        "# Theirs\n\n- [Gone](gone.md)\n",
    )
    .unwrap();
    let dir = root.join("workspaces/alpha/memory");
    while_swapped(
        &root,
        "workspaces/alpha/memory",
        "../../personas/devops/memory",
        || {
            (
                crate::memstore::read_files(&root, &dir).0.len(),
                crate::memstore::index_drift(&root, &dir),
            )
        },
        |(files, drift)| {
            assert!(files <= 1, "listed {files}");
            if let Ok((dangling, unindexed)) = drift {
                assert!(
                    !dangling.contains(&"gone.md".to_owned()),
                    "read a persona's index"
                );
                assert!(unindexed.len() <= 1, "listed {unindexed:?}");
            }
        },
    );
}
