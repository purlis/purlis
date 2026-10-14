// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn the_clipboards_text_is_answered_as_it_is() {
    let held = "one\ntwo\r\n  three\t".to_owned();
    assert_eq!(pasted(Some(held.clone())), Ok(Some(held)));
}

#[test]
fn a_clipboard_with_no_text_or_an_empty_one_answers_nothing() {
    // A picture or files on the clipboard read as no text at all.
    assert_eq!(pasted(None), Ok(None));
    assert_eq!(pasted(Some(String::new())), Ok(None));
}

#[test]
fn text_at_the_bound_is_answered_and_a_byte_more_is_refused_in_a_sentence() {
    let most = "a".repeat(PASTE_LIMIT);
    assert_eq!(pasted(Some(most.clone())), Ok(Some(most)));
    let refused = pasted(Some("a".repeat(PASTE_LIMIT + 1))).unwrap_err();
    assert_eq!(refused, TOO_MUCH);
    // The refusal says the bound and nothing of what was on the clipboard.
    assert!(refused.contains("1 MiB"));
    assert!(!refused.contains("aaaa"));
}

#[test]
fn the_read_is_the_windows_alone_and_no_link_is_served_it() {
    use purlis_session_protocol::ui;
    assert!(ui::window_only("clipboard_text"));
    assert!(!ui::served_on_a_link("clipboard_text"));
    assert!(!ui::SERVED_ON_A_LINK.contains(&"clipboard_text"));
}

#[test]
fn a_clipboard_holding_no_text_is_none_and_a_read_that_failed_says_it_failed() {
    assert_eq!(
        held(Ok("their text".to_owned())),
        Ok(Some("their text".to_owned()))
    );
    // A picture or files, or nothing at all: no text, and no failure.
    assert_eq!(held(Err(arboard::Error::ContentNotAvailable)), Ok(None));
    // Any other answer is a read that failed, and is said as one: never "holds no text".
    for failed in [
        arboard::Error::ClipboardNotSupported,
        arboard::Error::ClipboardOccupied,
        arboard::Error::ConversionFailure,
    ] {
        let said = held(Err(failed)).unwrap_err();
        assert!(said.starts_with(NOT_READ), "{said}");
    }
}
