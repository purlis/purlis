// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a hook marks of the prompt the person sent (SK-12, SK-38 #3029): a filter of its lines
//! keyed by the chat's prompt key (#3369), only on a harness that confirms a typed skill by the
//! prompt, its lead alone on one that names the skill, and never the prompt itself.

use super::*;
use crate::skillspan::{PromptMarks, PromptSalt};

const ID: &str = "01JB00000000000000000000C7";
const KEY: &str = "5f1c0e3a9b7d2468ace013579bdf2468ace013579bdf2468ace013579bdf2468";
const PROMPT: &str = "Use the purlis-compact skill on the persona ops\n$deploy env: prod";

fn env_of(harness: &str) -> impl Fn(&str) -> Option<String> + use<> {
    let pairs = [
        (SOCKET_ENV, "/tmp/s.sock".to_owned()),
        (CHAT_ENV, "7".to_owned()),
        (CHAT_ID_ENV, ID.to_owned()),
        (PROMPT_KEY_ENV, KEY.to_owned()),
        (HARNESS_ENV, harness.to_owned()),
    ];
    move |want| {
        pairs
            .iter()
            .find(|(k, _)| *k == want)
            .map(|(_, v)| v.clone())
    }
}

fn sent(env: &dyn Fn(&str) -> Option<String>) -> Report {
    let payload = serde_json::json!({
        "session_id": "11111111-2222-4333-8444-555555555555",
        "hook_event_name": "UserPromptSubmit",
        "prompt": PROMPT,
    })
    .to_string();
    Report::read(Event::UserPromptSubmit, &payload, env).expect("a report")
}

fn salt() -> PromptSalt {
    PromptSalt::of_key(KEY).expect("a chat's prompt key")
}

#[test]
fn codex_and_opencode_mark_every_line_of_the_prompt_under_the_chats_salt() {
    for harness in ["codex", "opencode"] {
        let marks = sent(&env_of(harness)).detail.prompt_marks;
        assert!(
            marks.holds(PromptMarks::digest("$deploy env: prod", &salt())),
            "{harness}"
        );
        let unsalted = PromptSalt::of_key("someone else").expect("a salt");
        assert!(
            !marks.holds(PromptMarks::digest("$deploy env: prod", &unsalted)),
            "{harness}: marked under another salt"
        );
        // The chat's id is in its environment too, and names its records: it keys nothing.
        let by_id = PromptSalt::of_key(ID).expect("a salt");
        assert!(
            !marks.holds(PromptMarks::digest("$deploy env: prod", &by_id)),
            "{harness}: marked under the chat's id"
        );
    }
}

#[test]
fn a_claude_code_chat_marks_no_filter_of_its_prompt_only_the_lead_a_curation_prompt_is_known_by() {
    let marks = sent(&env_of("claude-code")).detail.prompt_marks;
    assert!(
        !marks.holds(PromptMarks::digest("$deploy env: prod", &salt())),
        "Claude Code names a typed skill; no line of its prompt is marked"
    );
    assert_eq!(marks, PromptMarks::lead_of(PROMPT, &salt()));
    assert!(!marks.is_empty(), "a curation prompt's lead is marked");
}

#[test]
fn a_hook_in_a_chat_with_no_prompt_key_marks_nothing_even_with_an_id() {
    let env = env_of("codex");
    let no_key = |want: &str| (want != PROMPT_KEY_ENV).then(|| env(want)).flatten();
    assert!(sent(&no_key).detail.prompt_marks.is_empty());
}

#[test]
fn a_chats_prompt_key_is_never_handed_on_to_a_chat_it_starts() {
    assert!(NOT_INHERITED.contains(&PROMPT_KEY_ENV));
}

#[test]
fn the_prompt_is_in_no_line_and_no_debug_dump_of_the_report() {
    for harness in ["claude-code", "codex", "opencode"] {
        let report = sent(&env_of(harness));
        let line = serde_json::to_string(&report).expect("a line");
        let dumped = format!("{report:?}");
        for word in ["compact", "deploy", "prod"] {
            assert!(!line.contains(word), "{harness}: {line}");
            assert!(!dumped.contains(word), "{harness}: {dumped}");
        }
        assert!(!dumped.contains(KEY), "{harness}: the key in {dumped}");
        assert!(!line.contains(KEY), "{harness}: the key in {line}");
    }
}
