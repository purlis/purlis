// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn now(state: State) -> Now {
    Now {
        state,
        asking: false,
        edits_lines: Some(false),
    }
}

#[test]
fn a_paste_holds_one_line_and_nothing_after_it_can_submit() {
    let typed = pasted("@a\x1b[201~\r\nb\u{202E}c\u{200B}d\u{2028}");
    assert_eq!(typed, "\x1b[200~@a[201~bcd \x1b[201~");
    assert!(!typed.contains(['\r', '\n']));
    assert!(typed.ends_with(crate::curation::PASTE_ENDS));
}

#[test]
fn invisible_and_bidirectional_characters_are_not_typeable() {
    for c in [
        '\u{202A}', '\u{202E}', '\u{2066}', '\u{2069}', '\u{200B}', '\u{200F}',
    ] {
        assert!(not_typeable(c), "U+{:04X}", u32::from(c));
    }
    for c in ['\u{2060}', '\u{FEFF}', '\u{2028}', '\u{2029}', '\n', '\x1b'] {
        assert!(not_typeable(c), "U+{:04X}", u32::from(c));
    }
    for c in ['a', 'é', '/', ' ', '#', '!'] {
        assert!(!not_typeable(c), "{c:?}");
    }
}

#[test]
fn a_path_a_harness_would_read_as_a_mode_or_a_command_is_written_from_here() {
    assert_eq!(neutral("!rm${IFS}-rf".to_owned()), "./!rm${IFS}-rf");
    assert_eq!(neutral("#note".to_owned()), "./#note");
    assert_eq!(neutral("@x".to_owned()), "./@x");
    assert_eq!(neutral("-rf".to_owned()), "./-rf");
    assert_eq!(neutral("src/main.rs".to_owned()), "src/main.rs");
    assert_eq!(neutral(".github".to_owned()), ".github");
    assert_eq!(neutral("/abs/x".to_owned()), "/abs/x");
}

#[test]
fn a_chat_mid_turn_is_never_typed_into() {
    for harness in [Harness::ClaudeCode, Harness::Codex] {
        assert_eq!(
            may_type_into(harness, now(State::Running)),
            Err(NotNow::MidTurn)
        );
    }
}

#[test]
fn a_chat_asking_the_operator_is_never_typed_into() {
    let asking = Now {
        asking: true,
        ..now(State::Waiting)
    };
    assert_eq!(
        may_type_into(Harness::ClaudeCode, asking),
        Err(NotNow::Asking)
    );
}

#[test]
fn a_chat_handed_back_is_typed_into_once_its_terminal_is_raw() {
    assert_eq!(
        may_type_into(Harness::ClaudeCode, now(State::Waiting)),
        Ok(())
    );
    let canonical = Now {
        edits_lines: Some(true),
        ..now(State::Waiting)
    };
    assert_eq!(
        may_type_into(Harness::ClaudeCode, canonical),
        Err(NotNow::NotYetAtItsPrompt)
    );
}

#[test]
fn a_chat_no_hook_has_spoken_for_is_never_typed_into_on_any_harness() {
    for harness in [Harness::ClaudeCode, Harness::Codex] {
        assert_eq!(
            may_type_into(harness, now(State::Unknown)),
            Err(NotNow::NotHeardFrom),
            "{harness:?}"
        );
    }
    assert_eq!(may_type_into(Harness::Codex, now(State::Waiting)), Ok(()));
}

#[test]
fn opencode_and_an_ended_chat_are_never_typed_into() {
    assert_eq!(
        may_type_into(Harness::Opencode, now(State::Waiting)),
        Err(NotNow::CannotBeTyped("opencode"))
    );
    assert_eq!(
        may_type_into(Harness::ClaudeCode, now(State::Done)),
        Err(NotNow::Ended)
    );
}

#[test]
fn a_chat_at_its_prompt_is_typed_into_on_every_harness_and_one_not_heard_from_never_is() {
    // Use skill… (#2775): opencode too, once its hooks said it waits at its prompt.
    assert_eq!(at_its_prompt(now(State::Waiting)), Ok(()));
    assert_eq!(
        at_its_prompt(now(State::Unknown)),
        Err(NotNow::NotHeardFrom)
    );
    assert_eq!(at_its_prompt(now(State::Running)), Err(NotNow::MidTurn));
    let canonical = Now {
        edits_lines: Some(true),
        ..now(State::Waiting)
    };
    assert_eq!(at_its_prompt(canonical), Err(NotNow::NotYetAtItsPrompt));
}

#[test]
fn plain_words_name_a_file_a_folder_and_lines() {
    let r = |path, lines, folder| Reference::for_tests(path, lines, folder).plain();
    assert_eq!(r("src/main.rs", None, false), "src/main.rs");
    assert_eq!(r("src", None, true), "src/");
    assert_eq!(
        r(
            "src/main.rs",
            Some(Lines {
                first: 10,
                last: 20
            }),
            false
        ),
        "src/main.rs:10-20"
    );
    assert_eq!(
        r("my dir/a b.txt", Some(Lines::one(3)), false),
        "\"my dir/a b.txt:3\""
    );
}
