// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use serde_json::json;

use super::key_events;

#[test]
fn enter_is_one_key_down_with_its_text_and_one_key_up() {
    assert_eq!(
        key_events("Enter").unwrap(),
        vec![
            json!({"type":"keyDown","key":"Enter","code":"Enter","windowsVirtualKeyCode":13,"modifiers":0,"text":"\r","unmodifiedText":"\r"}),
            json!({"type":"keyUp","key":"Enter","code":"Enter","windowsVirtualKeyCode":13,"modifiers":0}),
        ]
    );
}

#[test]
fn a_chord_holds_its_modifiers_around_the_key() {
    assert_eq!(
        key_events("Shift+Tab").unwrap(),
        vec![
            json!({"type":"rawKeyDown","key":"Shift","code":"ShiftLeft","windowsVirtualKeyCode":16,"modifiers":8}),
            json!({"type":"rawKeyDown","key":"Tab","code":"Tab","windowsVirtualKeyCode":9,"modifiers":8}),
            json!({"type":"keyUp","key":"Tab","code":"Tab","windowsVirtualKeyCode":9,"modifiers":8}),
            json!({"type":"keyUp","key":"Shift","code":"ShiftLeft","windowsVirtualKeyCode":16,"modifiers":0}),
        ]
    );
}

#[test]
fn select_all_works_with_control_or_meta_on_every_platform() {
    for chord in ["Control+a", "Meta+a"] {
        let events = key_events(chord).unwrap();
        assert_eq!(events.len(), 4, "{chord}");
        assert_eq!(events[1]["key"], "a");
        assert_eq!(events[1]["code"], "KeyA");
        assert_eq!(events[1]["commands"], json!(["selectAll"]), "{chord}");
        assert!(events[1].get("text").is_none(), "a chord types nothing");
    }
}

#[test]
fn a_letter_with_shift_types_its_capital() {
    let events = key_events("Shift+k").unwrap();
    assert_eq!(events[1]["type"], "keyDown");
    assert_eq!(events[1]["key"], "K");
    assert_eq!(events[1]["text"], "K");
    assert_eq!(events[1]["windowsVirtualKeyCode"], 75);
}

#[test]
fn a_key_that_is_not_one_is_refused_in_words() {
    let refused = key_events("Hyper+Enter").unwrap_err();
    assert_eq!(
        refused.to_string(),
        "`Hyper+Enter` is not a key purlis knows: name one such as Enter, Tab, Escape, ArrowDown, Shift+Tab or Control+a"
    );
    assert!(key_events("").is_err());
    assert!(key_events("Enter+").is_err());
}
