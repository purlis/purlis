// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};

use super::*;

fn scrub_of(texts: &[&str]) -> Scrub {
    let mut kept = Kept::default();
    kept.run(texts);
    Scrub::of(&kept)
}

#[test]
fn blanked_text_is_found_in_an_address_however_the_address_encodes_it() {
    let cases = [
        ("hunter2~", "http://fixture.test/login?p=hunter2%7E"),
        ("hunter2~", "http://fixture.test/login?p=hunter2~"),
        ("pass word!", "http://fixture.test/login?p=pass%20word!"),
        ("pass word!", "http://fixture.test/login?p=pass+word%21"),
        ("a*b*c", "http://fixture.test/login?p=a%2Ab%2Ac"),
        ("a*b*c", "http://fixture.test/login?p=a*b*c"),
        ("a/b c", "http://fixture.test/login?p=a%2fb%20c"),
        ("a/b c", "http://fixture.test/login?p=a%252Fb%2520c"),
        ("pässword", "http://fixture.test/login?p=p%C3%A4ssword"),
    ];
    for (value, address) in cases {
        assert_eq!(
            scrub_of(&[value]).text(address),
            "http://fixture.test/login?p=(blanked)",
            "{value} in {address}"
        );
    }
}

#[test]
fn an_encoding_however_many_times_over_and_whichever_marks_it_left_alone_is_read() {
    let value = "fixture key+5310/!x(*)~'";
    let scrub = scrub_of(&[value]);
    // As `encodeURIComponent` writes it, leaving !'()* alone, once, twice and more.
    let mut encoded = "fixture%20key%2B5310%2F!x(*)~'".to_owned();
    for times in 1..=5 {
        assert_eq!(
            scrub.text(&format!("q={encoded}&r=1")),
            "q=(blanked)&r=1",
            "encoded {times} times"
        );
        encoded = encoded.replace('%', "%25");
    }
    // A letter encoded where it has no need to be, and a `+` encoded as a space read by a form.
    assert_eq!(scrub_of(&["abc"]).text("%2561bc"), "(blanked)");
    assert_eq!(scrub_of(&["a b c"]).text("a%2Bb%252Bc"), "(blanked)");
}

#[test]
fn what_a_screenshot_looks_for_is_found_however_the_page_encoded_it() {
    assert!(holds_any(
        "fixture%20key%2B5310%2F!x",
        &["fixture key+5310/!x"]
    ));
    assert!(holds_any("a%2520b%21", &["a b!"]));
    assert!(!holds_any("a%20c", &["a b"]));
}

#[test]
fn a_file_name_loses_the_value_as_the_engine_and_purlis_spell_it() {
    let scrub = scrub_of(&[".secret/123:x?"]);
    for name in ["secret_123_x?.txt", "secret_123_x_.txt"] {
        assert_eq!(scrub.text(name), "(blanked).txt", "{name}");
    }
}

#[test]
fn a_value_in_hex_goes_too() {
    assert_eq!(scrub_of(&["abc"]).text("x 616263 y"), "x (blanked) y");
}

#[test]
fn only_what_matched_changes_so_an_escape_makes_no_line_in_an_answer() {
    assert_eq!(
        scrub_of(&["pass word"]).text("a%0Ab%20c?p=pass%20word&x=%3Cy%3E+z"),
        "a%0Ab%20c?p=(blanked)&x=%3Cy%3E+z"
    );
    assert_eq!(
        scrub_of(&["zzz"]).text("http://fixture.test/a%20b?c=d+e"),
        "http://fixture.test/a%20b?c=d+e",
        "an address that holds none is given back as written"
    );
}

#[test]
fn a_page_text_loses_the_value_in_every_form_as_an_address_does() {
    let value = "sk+a/b c=?>";
    let scrub = scrub_of(&[value]);
    let encoded = "sk%2Ba%2Fb%20c%3D%3F%3E";
    let forms = [
        value.to_owned(),
        encoded.to_owned(),
        encoded.to_lowercase(),
        "sk%2Ba%2Fb+c%3D%3F%3E".to_owned(),
        encoded.replace('%', "%25"),
        STANDARD.encode(value),
        STANDARD.encode(value).trim_end_matches('=').to_owned(),
        URL_SAFE_NO_PAD.encode(value),
        value.replace('/', "_"),
    ];
    for form in forms {
        assert_eq!(
            scrub.text(&format!("Said: {form} here")),
            "Said: (blanked) here",
            "{form}"
        );
    }
}

#[test]
fn a_value_in_a_larger_texts_base64_goes_at_each_alignment() {
    for value in ["long-value-5310", "abcdef", "abcde"] {
        let scrub = scrub_of(&[value]);
        for before in ["", "q", "q=", "qu=", "que="] {
            let whole = STANDARD.encode(format!("{before}{value}&page=1"));
            let said = scrub.text(&whole);
            assert!(
                said.contains("(blanked)"),
                "{value} after {before:?}: {said}"
            );
        }
    }
}

#[test]
fn a_short_value_is_not_looked_for_inside_a_larger_base64() {
    let scrub = scrub_of(&["abcd"]);
    assert!(scrub.holds(&STANDARD.encode("abcd")), "whole, it goes");
    let around = STANDARD.encode("q=abcd&x=y");
    assert_eq!(scrub.text(&around), around);
}

#[test]
fn a_value_too_short_to_scrub_is_not_taken_out_in_any_form() {
    let scrub = scrub_of(&["ab"]);
    for text in ["ab", "YWI=", "YWI", "a%62"] {
        assert_eq!(scrub.text(text), text);
    }
}

#[test]
fn a_part_of_a_value_stays_since_taking_it_out_would_say_which_part_it_is() {
    let scrub = scrub_of(&["squirrel9-long"]);
    for part in ["squirrel9", "squirrel9-lon", "quirrel9-long"] {
        assert_eq!(scrub.text(part), part);
        let encoded = STANDARD.encode(part);
        assert_eq!(scrub.text(&encoded), encoded, "{part} in base64");
    }
}

#[test]
fn a_kept_text_and_its_beginnings_are_taken_out_whole_whatever_order_they_were_kept_in() {
    let kept = ["squ", "squi", "squir", "squirre", "squirrel9"];

    let said = scrub_of(&kept).text("Signed in as SQUIRREL9 today");

    assert_eq!(said, "Signed in as (blanked) today");
}

#[test]
fn a_code_from_boxes_goes_with_any_separators_also_when_they_are_escaped() {
    let mut kept = Kept::default();
    let first = crate::drive::tree::NodeKey {
        frame: "f".to_owned(),
        backend: 1,
    };
    kept.group(&first, &["739214"]);
    let scrub = Scrub::of(&kept);
    for spelled in ["7 3 9-2 1 4", "739+214", "739%20214", "739%E2%80%93214"] {
        assert_eq!(scrub.text(spelled), "(blanked)", "{spelled}");
    }
}

#[test]
fn an_escape_that_stands_for_nothing_is_read_as_written() {
    let scrub = scrub_of(&["100%zz"]);
    assert_eq!(scrub.text("a 100%zz b"), "a (blanked) b");
    assert_eq!(scrub.text("%"), "%");
    assert_eq!(scrub.text("%e2%80"), "%e2%80");
    assert_eq!(scrub.text("%ff%fe"), "%ff%fe");
    let literal = scrub_of(&["a%41b"]);
    assert_eq!(
        literal.text("x a%41b y"),
        "x (blanked) y",
        "as it was written"
    );
}

#[test]
fn a_value_of_marks_only_a_file_name_spells_away_takes_out_no_single_letter() {
    let scrub = scrub_of(&["a\u{200b}\u{200b}\u{200b}"]);
    assert_eq!(scrub.text("a banana"), "a banana");
}

#[test]
fn a_value_that_the_blank_itself_holds_ends_and_leaves_the_blank() {
    for value in ["blank", "blanked", "(blanked)", "lan"] {
        let said = scrub_of(&[value]).text(&format!("a {value} b %20"));
        assert!(
            said.contains("blanked") || said.contains("(b"),
            "{value}: {said}"
        );
        assert!(said.chars().count() < 200, "{value}: {said}");
    }
}

#[test]
fn encoding_past_any_reason_takes_no_longer_than_its_limit() {
    let mut deep = "abc-value".to_owned();
    for _ in 0..40 {
        deep = deep.replace('-', "%2D").replace('%', "%25");
    }
    let _ = scrub_of(&["abc-value"]).text(&deep);
}

#[test]
fn a_form_is_never_listed_twice() {
    let plain = forms("plainword");
    let mut seen: Vec<String> = plain.iter().map(|one| one.to_string()).collect();
    let all = seen.len();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), all);
}

#[test]
fn what_a_screenshot_looks_for_leaves_a_short_text_alone() {
    let mut kept = Kept::default();
    kept.run(&["ab", "abc"]);
    let every = every_form(&kept);
    assert!(every.iter().any(|one| **one == *"abc"));
    assert!(!every.iter().any(|one| **one == *"ab"));
    assert!(!every.iter().any(|one| **one == *"YWI="), "nor its base64");
}

#[test]
fn a_letter_whose_capital_is_spelled_otherwise_is_found_in_either_case() {
    assert_eq!(
        scrub_of(&["stra\u{df}e"]).text("a STRA\u{1e9e}E b"),
        "a (blanked) b"
    );
    assert_eq!(
        scrub_of(&["stra\u{df}e"]).text("a STRASSE b"),
        "a (blanked) b"
    );
    assert_eq!(
        scrub_of(&["\u{3c3}\u{3b1}\u{3c2}"]).text("a \u{3a3}\u{391}\u{3a3} b"),
        "a (blanked) b"
    );
}

#[test]
fn a_value_a_page_wrote_in_latin1_is_found_in_an_address() {
    let scrub = scrub_of(&["p\u{e4}ssword"]);
    assert_eq!(
        scrub.text("http://a.test/?p=p%E4ssword"),
        "http://a.test/?p=(blanked)"
    );
    assert_eq!(
        scrub.text("http://a.test/?p=p%25e4ssword"),
        "http://a.test/?p=(blanked)"
    );
    assert_eq!(
        scrub.text("http://a.test/?p=p%C3%A4ssword"),
        "http://a.test/?p=(blanked)"
    );
}

#[test]
fn a_plus_in_a_value_read_as_a_space_is_found() {
    assert_eq!(scrub_of(&["a+b+c"]).text("x a b c y"), "x (blanked) y");
}

#[test]
fn a_plus_in_a_code_from_boxes_is_no_separator() {
    let mut kept = Kept::default();
    let first = crate::drive::tree::NodeKey {
        frame: "f".to_owned(),
        backend: 1,
    };
    kept.group(&first, &["a+b"]);
    let scrub = Scrub::of(&kept);
    assert_eq!(scrub.text("a%20%2B%20b"), "(blanked)");
}

#[test]
fn what_a_screenshot_looks_for_in_a_code_is_found_when_it_is_escaped() {
    assert!(holds_loosely("1%202%203", &["123"]));
}

#[test]
fn a_value_the_same_as_the_blank_is_still_held() {
    assert!(scrub_of(&["(blanked)"]).holds("a (blanked) b"));
}

#[test]
fn a_string_too_much_to_read_is_said_as_blanked_whole() {
    // Encoded sixteen times over, and long: every reading changes, so each is a whole read.
    let mut deep = "%".to_owned();
    for _ in 0..16 {
        deep = deep.replace('%', "%25");
    }
    let long = format!("{deep}{}", "x".repeat(2_000_000));
    let scrub = scrub_of(&["abc-value"]);
    // Compared by hand: a failure must not print megabytes.
    assert!(scrub.text(&long) == "(blanked)", "said at length");
    assert!(scrub.holds(&long));
    // And a long one that reads as it is stays.
    let plain = "x".repeat(2_000_000);
    assert!(scrub.text(&plain) == plain, "a plain long string changed");
}

#[test]
fn a_value_kept_in_two_cases_is_found_in_the_hex_and_base64_of_each() {
    let scrub = scrub_of(&["hunter2abc", "Hunter2ABC"]);
    let hex: String = "Hunter2ABC"
        .bytes()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(scrub.text(&format!("a {hex} b")), "a (blanked) b");
}
