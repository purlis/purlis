// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::time::{Duration, Instant};

use super::*;

#[test]
fn a_chat_is_drawn_again_only_a_gap_after_its_last_turn_ended() {
    let spacing = Spacing::new();
    let start = Instant::now();
    assert_eq!(spacing.may("a", start), Ok(()));
    spacing.ended("a", start);
    let soon = start + Duration::from_secs(1);
    assert_eq!(spacing.may("a", soon), Err(GAP - Duration::from_secs(1)));
    assert_eq!(spacing.may("a", start + GAP), Ok(()));
}

#[test]
fn one_chat_s_gap_never_holds_back_another() {
    let spacing = Spacing::new();
    let start = Instant::now();
    spacing.ended("a", start);
    assert_eq!(spacing.may("b", start), Ok(()));
}

#[test]
fn the_gap_is_long_enough_for_a_gallery_picture_to_be_drawn_whole_between() {
    assert!(GAP >= crate::themepreview::BUDGET);
    let said = Spacing::said(Duration::from_millis(300));
    assert!(said.contains("ask again in 1 s"), "{said}");
}

#[test]
fn what_is_kept_is_only_the_chats_still_within_their_gap() {
    let spacing = Spacing::new();
    let start = Instant::now();
    for n in 0..100 {
        spacing.ended(&format!("chat-{n}"), start);
    }
    assert_eq!(spacing.may("other", start + GAP), Ok(()));
    assert!(spacing.ended.lock().unwrap().is_empty());
}
