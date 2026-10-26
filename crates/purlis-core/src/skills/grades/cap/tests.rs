// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// A project whose local settings git ignores, as `purlis init` leaves one.
fn project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::write(tmp.path().join(".gitignore"), "charter.local.toml\n").unwrap();
    tmp
}

#[test]
fn a_grade_is_held_to_two_hundred_thousand_tokens_where_nothing_set_it() {
    let plane = project();
    assert_eq!(
        token_cap(plane.path()),
        Cap {
            tokens: DEFAULT,
            from: From::Default
        }
    );
    assert_eq!(DEFAULT, 200_000);
}

#[test]
fn a_value_out_of_bounds_or_a_key_it_does_not_know_is_refused_in_its_words() {
    let plane = project();
    let root = plane.path();
    for (text, says) in [
        ("[skill_grades]\ntoken_cap = 5\n", "from 1000 to 100000000"),
        (
            "[skill_grades]\ntoken_cap = \"lots\"\n",
            "not a whole number",
        ),
        ("[skill_grades]\nbudget = 1\n", "holds token_cap alone"),
        ("skill_grades = 3\n", "not a table"),
    ] {
        let said = keyed_at(root, text, Which::Local);
        assert_eq!(said.len(), 1, "{text}: {said:?}");
        assert!(said[0].why.contains(says), "{text}: {}", said[0].why);
    }
    let shared = keyed_at(root, "[skill_grades]\ntoken_cap = 5000\n", Which::Shared);
    assert!(shared[0].why.contains("set in"), "{:?}", shared);
    assert!(keyed_at(root, "[skill_grades]\ntoken_cap = 5000\n", Which::Local).is_empty());
}

#[test]
fn a_cap_out_of_bounds_is_never_written() {
    let plane = project();
    let root = plane.path();
    assert!(set_token_cap(root, 10).is_err());
    assert!(set_token_cap(root, MOST + 1).is_err());
    assert!(!root.join(Which::Local.file_at(root)).exists());
}

#[test]
fn a_cap_set_here_is_read_back_and_kept_in_this_machines_file() {
    let plane = project();
    let root = plane.path();
    crate::testgit::run(root, &["init", "-q", "-b", "main", "."]);
    std::fs::write(root.join(Which::Local.file_at(root)), "# mine\n[harness]\n").unwrap();
    let set = set_token_cap(root, 50_000).unwrap();
    assert_eq!(set.tokens, 50_000);
    assert_eq!(
        token_cap(root),
        Cap {
            tokens: 50_000,
            from: From::Yours
        }
    );
    let text = std::fs::read_to_string(root.join(Which::Local.file_at(root))).unwrap();
    assert!(text.starts_with("# mine\n"), "{text}");
    assert!(text.contains("[skill_grades]\ntoken_cap = 50000"), "{text}");
}

#[test]
fn the_cost_bound_is_the_caps_tokens_at_the_dearest_price() {
    let cap = Cap {
        tokens: DEFAULT,
        from: From::Default,
    };
    assert!((cap.cost_usd() - 15.0).abs() < 1e-9, "{}", cap.cost_usd());
}
