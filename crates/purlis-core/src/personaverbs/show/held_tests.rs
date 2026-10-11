// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #1083: `persona show`'s memory line, asked while the persona's `memory/` is swapped for a
//! link to another persona's, never counts that persona's memories.

use crate::held::racer::{project, while_swapped};
use crate::repocmd::Say;

#[test]
fn a_persona_memory_swapped_for_a_link_never_counts_another_personas_memories() {
    let (_tmp, root) = project(&[
        ("personas/devops/memory", 1, "Ours"),
        ("personas/ops/memory", 3, "Theirs"),
    ]);
    while_swapped(
        &root,
        "personas/devops/memory",
        "../ops/memory",
        || {
            let mut said = Vec::new();
            super::memory_summary(&root, "devops", "s1", &mut |say| {
                if let Say::Out(text) = say {
                    said.push(text);
                }
            });
            said.join("\n")
        },
        |said| {
            let line = said
                .lines()
                .find(|l| l.starts_with("memory:"))
                .unwrap_or_default()
                .to_owned();
            assert!(
                !line.contains(" 3 own"),
                "counted another persona's: {line}"
            );
        },
    );
}
