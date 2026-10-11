// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #1083: the briefing's memory digest, asked while a persona's `memory/` is swapped for a link
//! to another persona's, never counts or names that persona's memories.

use crate::held::racer::{project, while_swapped};

#[test]
fn a_persona_memory_swapped_for_a_link_never_has_another_personas_index_read() {
    let (_tmp, root) = project(&[
        ("personas/devops/memory", 1, "Ours"),
        ("personas/ops/memory", 3, "Theirs"),
    ]);
    while_swapped(
        &root,
        "personas/devops/memory",
        "../ops/memory",
        || super::memory_digest(&root, "devops"),
        |digest| {
            assert!(
                !digest.contains("Theirs"),
                "read another persona's:\n{digest}"
            );
            assert!(
                !digest.contains("(3)"),
                "counted another persona's:\n{digest}"
            );
        },
    );
}
