// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|one| (*one).to_owned()).collect()
}

#[test]
fn a_value_holding_three_dashes_is_read_and_rewritten_correctly() {
    let text = "---\ndescription: before --- after\nskills: a, b\n---\nThe charter.\n";

    assert_eq!(list(text, "skills"), Ok(names(&["a", "b"])));
    assert_eq!(
        with_list(text, "skills", &names(&["a", "b", "c"])),
        Ok("---\ndescription: before --- after\nskills: a, b, c\n---\nThe charter.\n".to_owned())
    );
}

#[test]
fn unknown_keys_comments_order_and_the_body_are_kept_byte_for_byte() {
    let text = "---\n# who it is\nname:   steward   \nweird_key: {a: 1, b: [2, 3]}\n\nskills: a  # mine\nrole: |\n  keeps\n  the plane\n---\n\nBody --- with dashes.\n  indented\n";

    let added = with_list(text, "skills", &names(&["a", "b"])).unwrap();
    assert_eq!(
        added,
        text.replace("skills: a  # mine", "skills: a, b  # mine")
    );
    let pinned = with_list(text, "pinned-skills", &names(&["x"])).unwrap();
    assert_eq!(
        pinned,
        text.replace("  the plane\n---", "  the plane\npinned-skills: x\n---")
    );
    let gone = with_list(text, "skills", &[]).unwrap();
    assert_eq!(gone, text.replace("skills: a  # mine\n", ""));
}

#[test]
fn a_flow_list_and_a_block_list_keep_their_style() {
    let flow = "---\nskills: [a, b]\nname: n\n---\n";
    assert_eq!(list(flow, "skills"), Ok(names(&["a", "b"])));
    assert_eq!(
        with_list(flow, "skills", &names(&["a"])),
        Ok("---\nskills: [a]\nname: n\n---\n".to_owned())
    );

    let block = "---\nskills:\n    - a\n    - b\n# between\nname: n\n---\n";
    assert_eq!(list(block, "skills"), Ok(names(&["a", "b"])));
    assert_eq!(
        with_list(block, "skills", &names(&["a", "b", "c"])),
        Ok("---\nskills:\n    - a\n    - b\n    - c\n# between\nname: n\n---\n".to_owned())
    );
}

#[test]
fn a_quoted_list_reads_as_yaml_reads_it() {
    let text = "---\nskills: \"a, b\"\n---\n";
    assert_eq!(list(text, "skills"), Ok(names(&["a", "b"])));
}

#[test]
fn a_malformed_frontmatter_is_refused_with_a_sentence_and_never_half_rewritten() {
    for text in [
        "---\ndescription: Use when: anything\nskills: a\n---\n",
        "---\nskills: a\nskills: b\n---\n",
        "---\nskills: &x a\npinned-skills: *x\n---\n",
    ] {
        let refused = with_list(text, "skills", &names(&["a", "b"])).unwrap_err();
        assert!(matches!(refused, NotWritten::Unread(_)), "{text:?}");
        assert!(
            refused.to_string().starts_with("cannot be changed: its frontmatter"),
            "{refused}"
        );
        assert!(list(text, "skills").is_err(), "a read fails closed too");
    }
}

#[test]
fn a_value_that_is_no_list_is_not_written_over() {
    let text = "---\nskills: {a: 1}\n---\n";
    assert_eq!(
        with_list(text, "skills", &names(&["a"])),
        Err(NotWritten::NotAList("skills"))
    );
}

#[test]
fn a_name_a_comma_list_cannot_hold_is_refused_and_a_yaml_list_quotes_it() {
    let comma = "---\nskills: a\n---\n";
    assert_eq!(
        with_list(comma, "skills", &names(&["a", "b, c"])),
        Err(NotWritten::WouldChange("skills"))
    );
    let flow = "---\nskills: [a]\n---\n";
    assert_eq!(
        with_list(flow, "skills", &names(&["a", "b, c"])),
        Ok("---\nskills: [\"a\", \"b, c\"]\n---\n".to_owned())
    );
}

#[test]
fn a_word_is_set_kept_in_place_and_taken_out() {
    let text = "---\nname: n\nprofile: codex # the fast one\nrole: r\n---\nbody\n";
    assert_eq!(
        with_word(text, "profile", Some("work")),
        Ok("---\nname: n\nprofile: work # the fast one\nrole: r\n---\nbody\n".to_owned())
    );
    assert_eq!(
        with_word(text, "profile", None),
        Ok("---\nname: n\nrole: r\n---\nbody\n".to_owned())
    );
    assert_eq!(
        with_word("# no frontmatter\n", "profile", Some("codex")),
        Err(NotWritten::NoFrontmatter("profile"))
    );
}

#[test]
fn an_empty_frontmatter_gains_the_key() {
    assert_eq!(
        with_list("---\n---\nbody\n", "skills", &names(&["a"])),
        Ok("---\nskills: a\n---\nbody\n".to_owned())
    );
}
