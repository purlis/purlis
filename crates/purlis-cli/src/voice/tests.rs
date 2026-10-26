// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_column_is_its_widest_cell_and_a_gap() {
    assert_eq!(column(["2026-03-02", "—"]), 12);
    assert_eq!(pad("—", 12), "—           ");
    // A cell is text: what would reorder or colour it is shown, and counted as shown.
    assert_eq!(column(["a\u{202e}b"]), 12);
    assert_eq!(pad("a\u{202e}b", 12), "a⟦U+202E⟧b  ");
    assert_eq!(pad("\x1b[31mx", 13), "⟦U+001B⟧[31mx");
}

/// SK-54 (#3225): a message is markup, so a `String` built at run time enters escaped, while
/// purlis's own colour — the mark's, and a span the message styles itself — still shows.
#[test]
fn a_voice_line_escapes_what_it_splices_and_keeps_purlis_s_colour() {
    let name = "x\x1b[8my\u{202e}z\u{200b}";
    let line = said(Sgr::Green, "✓", format!("saved {name}").into(), true);
    assert_eq!(
        line.as_str(),
        "\x1b[32m✓\x1b[0m saved x⟦U+001B⟧[8my⟦U+202E⟧z⟦U+200B⟧"
    );

    let styled = Markup::new()
        .own("kept ")
        .styled(Sgr::Bold, Markup::new().text(name));
    assert_eq!(
        said(Sgr::Red, "✗", styled, true).as_str(),
        "\x1b[31m✗\x1b[0m kept \x1b[1mx⟦U+001B⟧[8my⟦U+202E⟧z⟦U+200B⟧\x1b[0m"
    );
    // In a pipe the mark is plain, and the message is what it was.
    assert_eq!(
        said(Sgr::Cyan, "•", "plain words".into(), false).as_str(),
        "• plain words"
    );
}
