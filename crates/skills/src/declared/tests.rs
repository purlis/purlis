// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn argument(name: &str, value: &str) -> Argument {
    Argument {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

fn skill_md(frontmatter: &str) -> String {
    format!("---\nname: x\ndescription: Does x.\n{frontmatter}---\n\nbody $ARGUMENTS\n")
}

#[test]
fn a_skill_that_says_nothing_is_picked_by_the_harness_and_takes_no_arguments() {
    let declared = declared_in(&skill_md(""));
    assert_eq!(declared, Declared::default());
}

#[test]
fn disable_model_invocation_makes_a_skill_manual_only_in_every_spelling_of_yes() {
    for yes in ["true", "True", "yes", "on", "1", "\"true\""] {
        let declared = declared_in(&skill_md(&format!("disable-model-invocation: {yes}\n")));
        assert!(declared.manual, "{yes}");
    }
    for no in ["false", "no", "0", ""] {
        let declared = declared_in(&skill_md(&format!("disable-model-invocation: {no}\n")));
        assert!(!declared.manual, "{no}");
    }
}

#[test]
fn arguments_are_read_as_a_string_a_flow_list_or_a_block_list_in_order() {
    let spaced = declared_in(&skill_md("arguments: pr focus\n"));
    assert_eq!(spaced.arguments, ["pr", "focus"]);
    let flow = declared_in(&skill_md("arguments: [pr, \"focus\"]\n"));
    assert_eq!(flow.arguments, ["pr", "focus"]);
    let block = declared_in(&skill_md(
        "arguments:\n  - pr\n  - 'focus'\nargument-hint: <pr> [focus]\n",
    ));
    assert_eq!(block.arguments, ["pr", "focus"]);
    assert_eq!(block.hint.as_deref(), Some("<pr> [focus]"));
}

#[test]
fn a_hint_alone_asks_for_arguments_without_names() {
    let declared = declared_in(&skill_md("argument-hint: \"[issue number]\"\n"));
    assert!(declared.arguments.is_empty());
    assert_eq!(declared.hint.as_deref(), Some("[issue number]"));
}

#[test]
fn values_are_given_in_the_declared_order_on_one_line_and_trailing_empties_dropped() {
    let declared = declared_in(&skill_md("arguments: pr focus scope\n"));
    let values = |all: &[&str]| all.iter().map(|v| (*v).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        Given::of(&declared, &values(&["42", "the\n tests", ""])),
        Given::Named(vec![argument("pr", "42"), argument("focus", "the tests")])
    );
    // A middle one left empty keeps the later one's place.
    assert_eq!(
        Given::of(&declared, &values(&["", "x"])),
        Given::Named(vec![argument("pr", ""), argument("focus", "x")])
    );
    assert_eq!(Given::of(&declared, &values(&["", " "])), Given::Nothing);
}

#[test]
fn a_hinted_skill_takes_one_line_and_a_skill_with_nothing_declared_takes_nothing() {
    let hinted = declared_in(&skill_md("argument-hint: [issue]\n"));
    assert_eq!(
        Given::of(&hinted, &["  12\r\nand 13 ".to_owned()]),
        Given::Raw("12 and 13".to_owned())
    );
    let plain = declared_in(&skill_md(""));
    assert_eq!(
        Given::of(&plain, &["typed anyway".to_owned()]),
        Given::Nothing
    );
}

#[test]
fn a_skill_md_that_cannot_be_read_declares_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(
        declared_at(&tmp.path().join("gone/SKILL.md")),
        Declared::default()
    );
}

#[test]
fn what_was_given_reads_in_plain_words() {
    assert_eq!(Given::Nothing.in_words(), None);
    assert_eq!(
        Given::Raw("v1.2".to_owned()).in_words().as_deref(),
        Some("v1.2")
    );
    let named = Given::Named(vec![
        Argument {
            name: "pr".to_owned(),
            value: "42".to_owned(),
        },
        Argument {
            name: "focus".to_owned(),
            value: String::new(),
        },
        Argument {
            name: "tone".to_owned(),
            value: "short".to_owned(),
        },
    ]);
    assert_eq!(
        named.in_words().as_deref(),
        Some("pr: 42; focus: none; tone: short")
    );
}

#[test]
fn a_when_to_use_is_read_on_its_line_or_from_a_block_under_it() {
    let one = declared_in("---\nname: a\nwhen_to_use: On a release\n---\n");
    assert_eq!(one.when_to_use.as_deref(), Some("On a release"));
    let block =
        declared_in("---\nname: a\nwhen_to_use: >-\n  On a release\n  or a tag\nb: c\n---\n");
    assert_eq!(block.when_to_use.as_deref(), Some("On a release or a tag"));
}
