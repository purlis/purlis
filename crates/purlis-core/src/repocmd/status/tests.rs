// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_stack_the_inventory_does_not_carry_is_a_question_mark() {
    let mut by_name = BTreeMap::new();
    by_name.insert("a".to_string(), serde_json::json!({"stack": "rust"}));
    by_name.insert("b".to_string(), serde_json::json!({"name": "b"}));

    assert_eq!(stack_of(&by_name, "a"), "rust");
    assert_eq!(stack_of(&by_name, "b"), "?", "a record with no stack");
    assert_eq!(stack_of(&by_name, "c"), "?", "no record at all");
}

#[test]
fn a_path_is_shortened_only_where_a_tilde_cannot_be_read_as_a_home() {
    // #969: nothing expands a `~` inside a value, so a path holding one is shown whole.
    let odd = std::path::PathBuf::from("/srv/~acct2/plane");
    assert_eq!(short_path(&odd), "/srv/~acct2/plane");
}
