// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::unknown_word_note;

#[test]
fn an_unknown_word_says_where_else_purlis_looked() {
    assert_eq!(
        unknown_word_note("stauts", true),
        "No extension on this machine is called 'stauts' either, installed or built into the \
         app."
    );
    let outside = unknown_word_note("stauts", false);
    assert!(
        outside.starts_with("No extension installed on this machine is called 'stauts'")
            && outside.contains("not inside the app's bundle"),
        "{outside}"
    );
}

/// SK-50 (#3173): an extension's command name and title are its manifest's, so a conceal, a
/// bidi override or a zero-width character in them is shown as its mark, never obeyed.
#[test]
fn an_extension_command_s_name_and_title_are_shown_escaped() {
    let command = purlis_core::extension::cli::CliCommand {
        name: "sy\u{200b}nc".into(),
        title: "Syncs \x1b[8mhidden\u{202e}".into(),
        writes: true,
    };
    assert_eq!(
        super::command_line(&command).as_str(),
        "  sy⟦U+200B⟧nc — Syncs ⟦U+001B⟧[8mhidden⟦U+202E⟧ (writes)"
    );
}
