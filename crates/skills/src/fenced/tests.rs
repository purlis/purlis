// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn keys_and_lines(yaml: &str) -> Vec<(String, usize)> {
    entries(yaml)
        .unwrap()
        .into_iter()
        .map(|entry| (entry.key, entry.line))
        .collect()
}

#[test]
fn each_top_level_key_is_read_with_the_line_it_starts_on() {
    let yaml = "# a comment\nname: one\nskills:\n  - a\n  - b\n\nrole: |\n  many\n  lines\n";
    assert_eq!(
        keys_and_lines(yaml),
        [
            ("name".to_owned(), 1),
            ("skills".to_owned(), 2),
            ("role".to_owned(), 6)
        ]
    );
}

#[test]
fn a_value_holding_three_dashes_ends_no_frontmatter() {
    let text = "---\ndescription: a --- b\nskills: x\n---\nbody --- here\n";
    let fence = fence(text).unwrap();
    let found = entries(&text[fence.yaml.clone()]).unwrap();
    assert_eq!(found[0].value.text(), Some("a --- b"));
    assert_eq!(found[1].value.text(), Some("x"));
    assert_eq!(&text[fence.body..], "body --- here\n");
}

#[test]
fn a_list_and_a_scalar_say_their_words() {
    let found = entries("a: [x, \"y z\"]\nb: 'w, v'\nc:\nd: ~\n").unwrap();
    assert_eq!(found[0].value.items(), Some(vec!["x", "y z"]));
    assert_eq!(found[0].value.text(), None);
    assert_eq!(found[1].value.text(), Some("w, v"));
    assert_eq!(found[2].value.text(), Some(""));
    assert_eq!(found[3].value.text(), Some(""));
}

#[test]
fn only_blanks_and_comments_have_no_entries() {
    assert_eq!(entries("\n# nothing\n\n"), Ok(Vec::new()));
}

#[test]
fn a_frontmatter_purlis_cannot_change_a_line_of_is_refused() {
    for (yaml, why) in [
        ("a: b: c\n", "not yaml"),
        ("a: 1\na: 2\n", "twice"),
        ("{a: 1}\n", "flow"),
        ("- a\n- b\n", "a list"),
        ("a: &x 1\nb: *x\n", "reused"),
        ("a: !tag 1\n", "tagged"),
        ("<<: {a: 1}\n", "merge"),
        ("? [a]\n: 1\n", "odd key"),
    ] {
        assert!(entries(yaml).is_err(), "{why}: {yaml:?}");
    }
}
