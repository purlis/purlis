// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn pairs(text: &str) -> Vec<(String, String)> {
    Fence::find(text).map(|f| f.pairs()).unwrap_or_default()
}

fn pair(key: &str, value: &str) -> (String, String) {
    (key.to_owned(), value.to_owned())
}

#[test]
fn a_value_holding_three_dashes_is_text_and_the_frontmatter_ends_at_the_fence_line() {
    let text = "---\ndescription: before --- after\nrole: a---b\nskills: a, b\n---\nThe charter.\n";
    let fence = Fence::find(text).unwrap();
    assert_eq!(
        fence.pairs(),
        vec![
            pair("description", "before --- after"),
            pair("role", "a---b"),
            pair("skills", "a, b")
        ]
    );
    assert_eq!(fence.body(), "The charter.\n");
}

#[test]
fn a_value_holding_three_dashes_is_rewritten_correctly() {
    let text = "---\ndescription: before --- after\nskills: a, b\n---\nThe charter.\n";
    assert_eq!(
        with_keys(text, &[("skills", Some("a, b, c"))]),
        Ok("---\ndescription: before --- after\nskills: a, b, c\n---\nThe charter.\n".to_owned())
    );
}

#[test]
fn only_a_line_of_three_dashes_and_blanks_is_a_fence() {
    // Trailing blanks are allowed; an indented `---`, `----` or `--- x` is text.
    let text = "---  \nname: n\n  ---\n----\n--- x\nrole: r\n---\t\nbody\n";
    let fence = Fence::find(text).unwrap();
    assert_eq!(fence.pairs(), vec![pair("name", "n"), pair("role", "r")]);
    assert_eq!(fence.body(), "body\n");
    // A first line that is not a fence opens nothing.
    for none in ["---name: n\n---\n", " ---\nname: n\n---\n", "name: n\n---\nx\n---\n"] {
        assert_eq!(Fence::find(none), None, "{none:?}");
    }
}

#[test]
fn an_unclosed_frontmatter_is_none_and_all_body() {
    for text in ["---\nname: n\n", "---\nname: n\nrole: r---\n", "---"] {
        assert_eq!(Fence::find(text), None, "{text:?}");
        assert!(unclosed(text), "{text:?}");
    }
    assert!(!unclosed("# just a body\n"));
    assert!(!unclosed("---\n---\n"));
}

#[test]
fn a_closing_fence_with_no_break_and_crlf_breaks_are_read() {
    assert_eq!(pairs("---\nname: n\n---"), vec![pair("name", "n")]);
    let crlf = "---\r\nname: n\r\nrole: r\r\n---\r\nbody\r\n";
    let fence = Fence::find(crlf).unwrap();
    assert_eq!(fence.pairs(), vec![pair("name", "n"), pair("role", "r")]);
    assert_eq!(fence.body(), "body\r\n");
    assert_eq!(
        with_keys(crlf, &[("role", Some("x")), ("icon", Some("star"))]),
        Ok("---\r\nname: n\r\nrole: x\r\nicon: star\r\n---\r\nbody\r\n".to_owned())
    );
}

#[test]
fn values_purlis_define_writes_read_as_written() {
    let text = "---\nname: ops\nrole: Ops: on-call\nvault: none\ndelegate-when: Use when: paged\ncolor: #ff0000\n---\n";
    assert_eq!(
        pairs(text),
        vec![
            pair("name", "ops"),
            pair("role", "Ops: on-call"),
            pair("vault", "none"),
            pair("delegate-when", "Use when: paged"),
            pair("color", "#ff0000"),
        ]
    );
}

#[test]
fn unknown_keys_notes_order_and_the_body_are_kept_byte_for_byte() {
    let text = "---\n# who it is\nname:   steward   \nweird_key: {a: 1, b: [2, 3]}\n\nskills: a\nrole: r\n---\n\nBody\n---\nwith dashes.\n  indented\n";
    assert_eq!(
        with_keys(text, &[("skills", Some("a, b"))]).unwrap(),
        text.replace("skills: a\n", "skills: a, b\n")
    );
    assert_eq!(
        with_keys(text, &[("pinned-skills", Some("x"))]).unwrap(),
        text.replace("role: r\n---", "role: r\npinned-skills: x\n---")
    );
    assert_eq!(
        with_keys(text, &[("skills", None)]).unwrap(),
        text.replace("skills: a\n", "")
    );
}

#[test]
fn a_key_written_twice_is_one_line_where_the_first_stood() {
    let text = "---\nprofile: a\nname: ops\nprofile: b\n---\n";
    assert_eq!(
        with_keys(text, &[("profile", Some("codex"))]),
        Ok("---\nprofile: codex\nname: ops\n---\n".to_owned())
    );
    assert_eq!(
        with_keys(text, &[("profile", None)]),
        Ok("---\nname: ops\n---\n".to_owned())
    );
}

#[test]
fn an_empty_frontmatter_gains_the_key_and_none_has_no_frontmatter_to_hold_it() {
    assert_eq!(
        with_keys("---\n---\nbody\n", &[("skills", Some("a"))]),
        Ok("---\nskills: a\n---\nbody\n".to_owned())
    );
    let refused = with_keys("# no frontmatter\n", &[("profile", Some("codex"))]).unwrap_err();
    assert_eq!(refused, Unwritten::NoFrontmatter("profile".to_owned()));
    assert_eq!(
        refused.to_string(),
        "has no frontmatter to hold a `profile:` line"
    );
}

#[test]
fn a_value_that_would_not_read_back_as_itself_is_refused_and_nothing_is_written() {
    let text = "---\nrole: r\n---\n";
    for value in ["two\nlines", "a\r---", "x\u{2028}y", " padded", "padded\t"] {
        let refused = with_keys(text, &[("role", Some(value))]).unwrap_err();
        assert!(matches!(refused, Unwritten::Value { .. }), "{value:?}");
        assert!(
            refused.to_string().contains("as one line that reads back the same"),
            "{refused}"
        );
    }
    // A `---` inside a value is not a line of its own, so it is written and read back.
    let written = with_keys(text, &[("role", Some("---"))]).unwrap();
    assert_eq!(written, "---\nrole: ---\n---\n");
    assert_eq!(pairs(&written), vec![pair("role", "---")]);
}

#[test]
fn what_the_writer_writes_the_reader_reads_back() {
    let text = "---\nname: n\nicon: old\n---\nbody --- here\n";
    let keys = [("icon", Some("star")), ("color", Some("#00ff00")), ("name", None)];
    let written = with_keys(text, &keys).unwrap();
    assert_eq!(
        pairs(&written),
        vec![pair("icon", "star"), pair("color", "#00ff00")]
    );
    assert_eq!(Fence::find(&written).unwrap().body(), "body --- here\n");
}

#[test]
fn lines_split_as_python_splits_them_with_their_spans() {
    let text = "a\r\nb\rc\u{2028}d";
    let spans: Vec<&str> = lines(text).iter().map(|l| &text[l.start..l.end]).collect();
    assert_eq!(spans, ["a", "b", "c", "d"]);
    assert_eq!(lines("a\n").len(), 1);
    assert_eq!(lines("").len(), 0);
}

#[test]
fn an_unclosed_frontmatter_is_refused_not_written_over() {
    let refused = with_keys("---\nname: n\n", &[("icon", Some("star"))]).unwrap_err();
    assert_eq!(refused, Unwritten::Unclosed("icon".to_owned()));
    assert!(refused.to_string().contains("no `---` line closes"), "{refused}");
}
