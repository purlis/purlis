// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #1083: the roster's memory column, asked while a persona's `memory/` is swapped for a link to
//! another persona's, never counts that persona's memories.

use std::collections::BTreeMap;

use crate::held::racer::{project, while_swapped};

#[test]
fn a_persona_memory_swapped_for_a_link_never_counts_another_personas_memories() {
    let (_tmp, root) = project(&[
        ("personas/devops/memory", 1, "Ours"),
        ("personas/ops/memory", 3, "Theirs"),
    ]);
    let names = vec!["devops".to_owned()];
    while_swapped(
        &root,
        "personas/devops/memory",
        "../ops/memory",
        || super::rows(&root, &names, &BTreeMap::new()).0,
        |rows| assert!(rows[0].memories <= 1, "counted {}", rows[0].memories),
    );
}
