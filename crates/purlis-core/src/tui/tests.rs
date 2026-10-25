// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What `charter/tui.py` promises, asserted against the same strings its own suite uses.

use super::*;

/// A workspace directory name charter never validated, which is how a wide glyph reaches a
/// column at all. Three CJK characters: six columns, three characters.
const CJK: &str = "日本語";
/// `e` plus U+0301 COMBINING ACUTE ACCENT: one column, two characters.
const COMBINED: &str = "e\u{0301}";

#[test]
fn a_column_is_what_python_says_it_is_and_not_a_character_count() {
    assert_eq!(width("charter"), 7);
    // The whole reason the tables exist: a character count answers 3 here and 2 below, and
    // both are wrong by the amount that shears a row.
    assert_eq!(width(CJK), 6);
    assert_eq!(CJK.chars().count(), 3);
    assert_eq!(width(COMBINED), 1);
    assert_eq!(COMBINED.chars().count(), 2);
    // Fullwidth forms are `F` and count two; halfwidth katakana is `H` and counts one.
    assert_eq!(width("Ａ"), 2);
    assert_eq!(width("ｱ"), 1);
    // East-asian AMBIGUOUS is one, which is what lets the box and the ellipsis be measured
    // at all — `…`, `│`, `⬢` are each one column to charter.
    assert_eq!(width("…"), 1);
    assert_eq!(width("│"), 1);
    assert_eq!(width("⬢"), 1);
    // SGR costs nothing.
    assert_eq!(width("\x1b[32mok\x1b[0m"), 2);
}

#[test]
fn sgr_is_the_only_escape_that_survives_and_the_rest_go_whole() {
    // charter's own markup, untouched.
    assert_eq!(sanitize("\x1b[32mok\x1b[0m"), "\x1b[32mok\x1b[0m");
    // An OSC title string goes WHOLE: dropping the introducer alone would print `0;pwned` as
    // ordinary text, which reads as data.
    assert_eq!(sanitize("a\x1b]0;pwned\x07b"), "ab");
    // Unterminated, to the end of the string, for the same reason.
    assert_eq!(sanitize("a\x1b]0;pwned"), "a");
    // Any other CSI — an erase-in-display — is a command, not content.
    assert_eq!(sanitize("a\x1b[2Jb"), "ab");
    // `ESC c` is a full terminal reset and is two characters.
    assert_eq!(sanitize("a\x1bcb"), "ab");
    // A stray introducer goes and the colour after it stays: the alternation's whole point.
    assert_eq!(sanitize("\x1b\x1b[32mok"), "\x1b[32mok");
    // A tab and a newline shear the columns below, so they keep their separation as a space
    // rather than vanishing: `a\tb` stays two words.
    assert_eq!(sanitize("a\tb\nc"), "a b c");
    // Other C0, DEL and C1 are removed outright. `\u{85}` is NEL, which a terminal reads as
    // a line ending — these two cases came from `purlis-cli`'s own `sanitize`, which this
    // module replaced.
    assert_eq!(sanitize("a\x00b\x07c\x7fd\u{9b}e"), "abcde");
    assert_eq!(sanitize("a\x07b\x7fc\u{85}d"), "abcd");
    assert_eq!(sanitize("a\x1b"), "a");
    // Nothing to do is nothing done.
    assert_eq!(sanitize("plain ünïcode"), "plain ünïcode");
}

#[test]
fn width_measures_what_a_terminal_shows_not_what_it_was_handed() {
    // The measurement goes through `sanitize`, so an escape nobody thought of cannot buy
    // itself columns in the arithmetic.
    assert_eq!(width("a\x1b[2Jb"), 2);
    assert_eq!(width("a\x1b]0;a-very-long-title\x07b"), 2);
    assert_eq!(strip_ansi("\x1b[32mok\x1b[0m"), "ok");
}

#[test]
fn truncate_cuts_at_columns_carries_colour_and_closes_it() {
    // Fits: unchanged.
    assert_eq!(truncate("charter", 7), "charter");
    // Cut: the ellipsis takes one of the columns.
    assert_eq!(truncate("charter", 4), "cha…");
    assert_eq!(width(&truncate("charter", 4)), 4);
    // Zero columns is no line at all, not a bare ellipsis.
    assert_eq!(truncate("charter", 0), "");
    // A wide character is never split in half: at a budget of 4 the ellipsis takes one and
    // one more CJK glyph would take two of the three left — so only one fits.
    assert_eq!(truncate(CJK, 4), "日…");
    assert_eq!(truncate(CJK, 5), "日本…");
    // Escapes are carried verbatim at zero width and a reset is appended, so the style
    // cannot bleed into whatever is printed next.
    assert_eq!(truncate("\x1b[32mcharter\x1b[0m", 4), "\x1b[32mcha…\x1b[0m");
    // Sanitised BEFORE the fits-already path: this is charter #326, where an escape reached
    // the screen because nothing ever looked at a string that already fitted.
    assert_eq!(truncate("a\x1b[2Jb", 80), "ab");
}

#[test]
fn pad_fits_exactly_and_never_leaks_a_style_into_the_padding() {
    assert_eq!(pad("ab", 5, Align::Left), "ab   ");
    assert_eq!(pad("ab", 5, Align::Right), "   ab");
    assert_eq!(pad("ab", 5, Align::Center), " ab  ");
    // Long values are cut, not pushed: that is the difference from `{:<5}`.
    assert_eq!(pad("charter", 5, Align::Left), "char…");
    // Columns, not characters — a CJK name pads by three, not by six.
    assert_eq!(pad(CJK, 9, Align::Left), "日本語   ");
}

#[test]
fn a_fixed_cell_of_purlis_s_own_markup_keeps_its_colour_and_pads_outside_it() {
    // The spaces go OUTSIDE the colour span.
    assert_eq!(
        fit("\x1b[32mok\x1b[0m", 4, Align::Left),
        "\x1b[32mok\x1b[0m  "
    );
    let row = Node::Row {
        cells: vec![
            Cell::fixed("\x1b[32mok\x1b[0m", 4, Align::Left),
            Cell::new("tail"),
        ],
        gap: " ".to_string(),
    };
    assert_eq!(row.render(80), vec!["\x1b[32mok\x1b[0m   tail"]);
}

/// SK-47 (#3150): a table cell is untrusted text. What would reorder it, hide in it or command
/// the terminal from it is shown as `⟦U+XXXX⟧`, and the cell is measured as it is shown.
#[test]
fn a_table_cell_shows_bidi_invisible_and_escape_characters_and_is_measured_as_shown() {
    // Eighteen columns as shown: `a`, eight for each mark, `b`.
    assert_eq!(column("", ["a\u{202e}b\u{200b}"], 0, None), 18);
    assert_eq!(
        pad("a\u{202e}b\u{200b}", 20, Align::Left),
        "a⟦U+202E⟧b⟦U+200B⟧  "
    );
    for c in [
        '\u{202a}', '\u{202e}', '\u{2066}', '\u{2069}', '\u{200d}', '\u{feff}',
    ] {
        assert_eq!(
            pad(&format!("x{c}"), 9, Align::Left),
            format!("x⟦U+{:04X}⟧", u32::from(c))
        );
    }
    // A cell is not markup: a colour, or a conceal, from the text itself is shown, not obeyed.
    assert_eq!(pad("\x1b[8mx", 13, Align::Left), "⟦U+001B⟧[8mx ");
    // Nor does a line end or a tab shear the columns below it.
    assert_eq!(pad("a\nb\tc", 19, Align::Left), "a⟦U+000A⟧b⟦U+0009⟧c");
}

#[test]
fn markup_keeps_purlis_s_colour_and_shows_bidi_and_invisible_characters_escaped() {
    let line = "\x1b[32ma\u{202e}b\u{200b}\x1b[0m";
    assert_eq!(sanitize(line), "\x1b[32ma⟦U+202E⟧b⟦U+200B⟧\x1b[0m");
    assert_eq!(width(line), 18);
    assert_eq!(truncate(line, 80), "\x1b[32ma⟦U+202E⟧b⟦U+200B⟧\x1b[0m");
    // Plain text with nothing to escape is handed back as it is.
    assert!(matches!(sanitize("日本語 ok"), Cow::Borrowed(_)));
}

#[test]
fn a_finished_line_has_no_trailing_space_even_behind_an_escape() {
    assert_eq!(finish("ab  "), "ab");
    assert_eq!(finish("ab \t"), "ab");
    // The one this exists for: a space hiding behind a trailing reset.
    assert_eq!(finish("ab \x1b[0m"), "ab\x1b[0m");
    // And behind several, repeatedly.
    assert_eq!(finish("ab \x1b[0m \x1b[32m"), "ab\x1b[0m\x1b[32m");
    // A space INSIDE the line is content and stays.
    assert_eq!(finish("a b"), "a b");
}

#[test]
fn a_width_is_a_positive_number_whichever_source_produced_it() {
    let env = |name: &str| match name {
        "COLUMNS" => Some("120".to_string()),
        _ => None,
    };
    assert_eq!(env_columns(&env), Some(120));
    assert_eq!(env_columns(&|_: &str| None::<String>), None);
    assert_eq!(env_columns(&|_: &str| Some("wide".to_string())), None);
    // `int("0")` parses happily, which is the whole of charter #594: what the variable SAYS
    // and whether that is a width are two questions, and only `judged` asks the second.
    assert_eq!(env_columns(&|_: &str| Some("0".to_string())), Some(0));

    // `$COLUMNS` wins when it is usable.
    assert_eq!(judged([Some(120), Some(80)], 80, 24), 120);
    // charter #594, both rungs: a source that answers ZERO has not answered. The env one was
    // guarded by a spelling and the tty one walked straight through it — a pty with no
    // window size reports zero, and charter drew every table one column wide.
    assert_eq!(judged([Some(0), Some(80)], 200, 24), 80);
    assert_eq!(judged([Some(0), Some(0)], 200, 24), 200);
    assert_eq!(judged([Some(-1), None], 200, 24), 200);
    assert_eq!(judged([None, None], 200, 24), 200);
    // The floor is the last word on all three paths, including the default's.
    assert_eq!(judged([Some(5), None], 200, 24), 24);
    assert_eq!(judged([None, Some(5)], 200, 24), 24);
    assert_eq!(judged([None, None], 5, 24), 24);
}

#[test]
fn a_column_is_sized_from_the_values_about_to_be_printed() {
    assert_eq!(column("name", ["ab", "abcdef"], 2, None), 8);
    // The header counts too, so a column is never narrower than its own head.
    assert_eq!(column("name-of-it", ["ab"], 2, None), 12);
    // A cap bounds the CONTENT and never the gap.
    assert_eq!(column("h", ["abcdefghij"], 2, Some(4)), 6);
    // Columns, not characters.
    assert_eq!(column("", [CJK], 0, None), 6);
}

#[test]
fn a_row_pads_its_fixed_cells_and_the_line_is_still_clamped() {
    let row = Node::Row {
        cells: vec![
            Cell::fixed("ab", 6, Align::Left),
            Cell::fixed("9", 3, Align::Right),
            Cell::new("tail"),
        ],
        gap: " ".to_string(),
    };
    assert_eq!(row.render(80), vec!["ab       9 tail"]);
    // Over budget, the assembled line is cut rather than wrapped.
    assert_eq!(row.render(10), vec!["ab       …"]);
}

#[test]
fn columns_share_what_is_left_and_blank_fill_the_shorter_side() {
    let node = Node::Columns {
        columns: vec![
            (Block::Lines(vec!["left".into()]), Some(10)),
            (Block::Lines(vec!["a".into(), "b".into(), "c".into()]), None),
        ],
        gap: "  ".to_string(),
    };
    let out = node.render(20);
    assert_eq!(
        out,
        vec![
            "left        a",
            // The left column is blank-filled, so the right column's rows stay in their own
            // column however tall each side is — ten columns of fixed width and the
            // two-column gap, whether or not the left side has a line to put there.
            "            b",
            "            c",
        ]
    );
}

#[test]
fn the_remainder_of_a_flex_share_goes_to_the_leftmost_columns() {
    // 20 columns, no gaps to pay for beyond the two singles, three flex columns: 20 - 2*1 =
    // 18 to share three ways is 6 each with 0 over. Narrow it by one and the leftmost pays.
    let cols = |w: usize| {
        Node::Columns {
            columns: vec![
                (Block::Lines(vec!["aaaaaaaa".into()]), None),
                (Block::Lines(vec!["bbbbbbbb".into()]), None),
                (Block::Lines(vec!["cccccccc".into()]), None),
            ],
            gap: " ".to_string(),
        }
        .render(w)
    };
    // 18 to share: 6, 6, 6.
    assert_eq!(cols(20), vec!["aaaaa… bbbbb… ccccc…"]);
    // 20 to share: 7, 7, 6 — the two leftmost take the remainder, the third does not. The
    // mutation this is here to catch is spending the remainder on every column or on none.
    assert_eq!(cols(22), vec!["aaaaaa… bbbbbb… ccccc…"]);
}

#[test]
fn a_stack_is_its_children_top_to_bottom_each_clamped_on_its_own() {
    let node = Node::stack(["one".to_string(), "two\nthree".to_string()]);
    assert_eq!(node.render(4), vec!["one", "two", "thr…"]);
}

/// SK-47 pinned a joined emoji's marks as an accepted cost; SK-49 (#3172) reverses it for
/// D-1301: a joiner between two emoji, or a presentation selector after one, only changes how
/// its neighbours are drawn, so it is text. A joiner or selector doing no job is still marked.
#[test]
fn a_joined_emoji_reads_as_text_in_a_cell_and_a_stray_joiner_is_marked() {
    // Measured as charter's `tui.py` measures it, a column for each joiner or selector: more
    // than a terminal that draws the ligature uses, so a row is padded short, never cut.
    assert_eq!(pad("👨\u{200d}👩", 12, Align::Left), "👨\u{200d}👩       ");
    assert_eq!(pad("👍\u{fe0f}", 10, Align::Left), "👍\u{fe0f}       ");
    assert_eq!(pad("a\u{200d}b", 12, Align::Left), "a⟦U+200D⟧b  ");
    assert_eq!(
        pad("👍\u{fe0f}\u{fe0f}", 12, Align::Left),
        "👍\u{fe0f}⟦U+FE0F⟧ "
    );
}

/// The column nothing pads is the same text as the rest of the row, so it takes the same rule:
/// a conceal in it would hide every row printed after it.
#[test]
fn a_row_escapes_its_last_unpadded_column_as_it_pads_the_others() {
    assert_eq!(
        row(&[("a", 3), ("b\u{202e}", 11)], "end \x1b[8mhid\u{200b}den"),
        "a  b⟦U+202E⟧  end ⟦U+001B⟧[8mhid⟦U+200B⟧den"
    );
    assert_eq!(row(&[], "plain"), "plain");
}

/// SK-50 (#3173): text spliced into markup is escaped by the splice, so a conceal, a bidi
/// override or a zero-width character in a name is shown as its mark and never obeyed.
#[test]
fn text_spliced_into_markup_is_shown_escaped() {
    let line = Markup::new()
        .text("name \x1b[8mhid\u{200b}den\u{202e}")
        .own("  ")
        .text("tab\there");
    assert_eq!(
        line.as_str(),
        "name ⟦U+001B⟧[8mhid⟦U+200B⟧den⟦U+202E⟧  tab⟦U+0009⟧here"
    );
    assert_eq!(line.to_string(), line.as_str());
}

/// SK-50: purlis's own colour is markup, so a styled span keeps its SGR while the text inside
/// it is escaped, and the span is closed so the style cannot bleed past it.
#[test]
fn purlis_s_own_colour_still_shows_around_escaped_text() {
    let line = Markup::new()
        .styled(Sgr::Green, Markup::new().text("ok\x1b[31m"))
        .own(" done");
    assert_eq!(line.as_str(), "\x1b[32mok⟦U+001B⟧[31m\x1b[0m done");
    assert_eq!(strip_ansi(line.as_str()), "ok⟦U+001B⟧[31m done");
}

/// SK-50: purlis's own words are a literal with nothing the one rule marks; a C1 CSI, a bidi
/// or a zero-width character in one fails a test build, not only an ESC.
#[test]
#[should_panic(expected = "purlis's own words")]
fn own_words_holding_a_c1_csi_fail_a_test_build() {
    let _ = Markup::new().own("a\u{9b}8m");
}

/// SK-54 (#3225): a message line takes `impl Into<Markup>`, so a literal is purlis's own words
/// and a `String` — a `format!` that spliced a name — enters escaped, whole.
#[test]
fn a_string_becomes_markup_escaped_and_a_literal_as_written() {
    let name = "n\x1b[8ma\u{202e}m\u{200b}e";
    let line: Markup = format!("saved {name}\tthere").into();
    assert_eq!(
        line.as_str(),
        "saved n⟦U+001B⟧[8ma⟦U+202E⟧m⟦U+200B⟧e⟦U+0009⟧there"
    );
    let own: Markup = "purlis's own words".into();
    assert_eq!(own, "purlis's own words");
}

/// SK-54: a name holding a line end cannot write a line of its own that reads as purlis's.
#[test]
fn a_name_cannot_forge_a_line_of_purlis_s_through_format() {
    let folder = "x\n✓ all repos refreshed";
    let line: Markup = format!("{folder}: not a clone, skipped").into();
    assert!(!line.contains('\n'), "{line}");
    assert_eq!(
        line.as_str(),
        "x⟦U+000A⟧✓ all repos refreshed: not a clone, skipped"
    );
}

/// SK-54: purlis lays a message out over lines, and a line end in a spliced value is marked.
#[test]
fn markup_lines_keep_purlis_s_line_ends_and_mark_a_value_s() {
    let two = Markup::lines(["first:".into(), Markup::new().text("  got a\nb\u{200b}")]);
    assert_eq!(two.as_str(), "first:\n  got a⟦U+000A⟧b⟦U+200B⟧");
    assert_eq!(
        Markup::lines_of("fatal: x\x1b[2J\nhint: y\u{202e}").as_str(),
        "fatal: x⟦U+001B⟧[2J\nhint: y⟦U+202E⟧"
    );
}

/// SK-54: a line from the hook socket is bytes, so it is read back escaped again; a line
/// purlis escaped reads back the same.
#[test]
fn markup_read_off_the_wire_is_escaped_again() {
    let sent = Markup::new().text("a\x1b[8mb\nc");
    let wire = serde_json::to_string(&sent).unwrap();
    assert_eq!(serde_json::from_str::<Markup>(&wire).unwrap(), sent);
    // A message purlis laid out over lines reads back over the same lines.
    let laid_out = Markup::lines(["first:".into(), Markup::new().text("  b\u{200b}")]);
    let wire = serde_json::to_string(&laid_out).unwrap();
    assert_eq!(serde_json::from_str::<Markup>(&wire).unwrap(), laid_out);
    let forged: Markup = serde_json::from_str("\"x\\u001b[8my\\u202e\"").unwrap();
    assert_eq!(forged.as_str(), "x⟦U+001B⟧[8my⟦U+202E⟧");
}

/// SK-54: a long line is cut by what it shows, never through purlis's own colour.
#[test]
fn a_shortened_line_never_cuts_through_purlis_s_colour() {
    let line = Markup::new()
        .styled(Sgr::Red, Markup::new().own("abcdef"))
        .own("gh");
    assert_eq!(line.clone().shortened(8), line);
    assert_eq!(line.shortened(3).as_str(), "\x1b[31mabc\x1b[0m …");
    assert_eq!(Markup::new().own("abcdef").shortened(2).as_str(), "ab …");
}
