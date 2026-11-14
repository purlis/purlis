// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::cell::RefCell;

use super::*;
use crate::pausing::Telling;

/// What `tell` typed, each send's bytes in order, as `look` said the chat stood.
fn typed_as(look: Telling) -> (Result<(), String>, Vec<Vec<u8>>) {
    let sent = RefCell::new(Vec::new());
    let told = tell(
        "steward 3",
        || look,
        |bytes| {
            sent.borrow_mut().push(bytes.to_vec());
            Ok(())
        },
    );
    (told, sent.into_inner())
}

/// **Q267 (T1-33): Tell the chat… types purlis's own line, at once, only where the chat waits**,
/// by T1-36's one way in (`pausing::telling`, then `type_in_turn`): one bracketed paste, then
/// Enter.
#[test]
fn tell_the_chat_types_its_one_line_only_where_the_chat_takes_a_line() {
    let (told, sent) = typed_as(Telling::Now);
    assert_eq!(told, Ok(()));
    assert_eq!(sent, vec![typed().into_bytes()]);
    assert_eq!(
        typed(),
        format!("{}\r", crate::curation::bracketed(TELL_LINE))
    );
}

/// **Q267 amended: never typed mid-turn, nor at a prompt.** Nothing is kept for later either:
/// the person is told why, and presses again once it waits.
#[test]
fn tell_the_chat_types_nothing_into_a_chat_that_is_not_waiting_and_says_so() {
    let (told, sent) = typed_as(Telling::AtTurnEnd);
    assert!(sent.is_empty(), "nothing typed mid-turn or at a prompt");
    let said = told.expect_err("the person is told why");
    assert!(said.contains("steward 3"), "{said}");
    assert!(said.contains("nothing was typed"), "{said}");
}

/// **Takes keys** (Q267 amended): a chat that has ended, is in a stop's last turn, or runs a
/// harness purlis has not measured is typed nothing.
#[test]
fn tell_the_chat_types_nothing_into_a_chat_that_takes_no_keys_and_says_so() {
    let (told, sent) = typed_as(Telling::Never);
    assert!(sent.is_empty());
    let said = told.expect_err("the person is told why");
    assert!(said.contains("steward 3"), "{said}");
}

/// **The line is purlis's own and widens nothing** (Q259): it names nothing the chat or its
/// harness said, no command and no target, and grants nothing.
#[test]
fn the_line_tell_the_chat_types_is_purliss_own_and_allows_nothing() {
    assert!(TELL_LINE.starts_with("purlis: "));
    assert!(TELL_LINE.contains("stays blocked"));
    assert!(!TELL_LINE.to_lowercase().contains("allowed"));
    assert!(!TELL_LINE.contains('\n') && !TELL_LINE.contains('\r'));
    assert!(!TELL_LINE.contains('\u{1b}'));
}

/// A send that fails is said, never swallowed.
#[test]
fn a_send_that_fails_is_said() {
    let told = tell(
        "steward 3",
        || Telling::Now,
        |_| Err("the lane is full".to_owned()),
    );
    let said = told.expect_err("said");
    assert!(said.contains("the lane is full"), "{said}");
}
