// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #1083: the persona counter, asked while a persona's `memory/` is swapped for a link to
//! another persona's or a workspace's, never counts what is there.

use crate::held::racer::{project, while_swapped};

#[test]
fn a_persona_memory_swapped_for_a_link_never_counts_anothers_memories() {
    for target in ["../ops/memory", "../../workspaces/beta/memory"] {
        let (_tmp, root) = project(&[
            ("personas/devops/memory", 1, "Ours"),
            ("personas/ops/memory", 3, "Theirs"),
            ("workspaces/beta/memory", 3, "Theirs"),
        ]);
        while_swapped(
            &root,
            "personas/devops/memory",
            target,
            || super::memory_count(&root, "devops"),
            |n| assert!(n <= 1, "{target}: counted {n}"),
        );
    }
}
