// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #1083: the footer's todo count, asked while the workspace's `todos/` is swapped for a link to
//! another workspace's, never counts that workspace's todos.

use crate::held::racer::{project, while_swapped};

#[test]
fn a_todo_store_swapped_for_a_link_never_counts_another_workspaces_todos() {
    let (_tmp, root) = project(&[
        ("workspaces/alpha/todos", 1, "Ours"),
        ("workspaces/beta/todos", 3, "Theirs"),
    ]);
    while_swapped(
        &root,
        "workspaces/alpha/todos",
        "../beta/todos",
        || super::todo_count(&root, "alpha"),
        |n| assert!(n <= 1, "counted {n}: another workspace's todos"),
    );
}
