// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A task starts in its asking chat's permission mode, capped at auto (#3787). Every expected
//! answer is written out from the operator's table.

use super::*;

#[test]
fn claude_code_s_modes_read_as_the_operator_s_table_says() {
    // Claude Code 2.1.312's own names for its modes, as its hook payloads carry them.
    for (mode, reported) in [
        ("default", Reported::Asks),
        ("manual", Reported::Asks),
        ("acceptEdits", Reported::Asks),
        ("plan", Reported::Asks),
        // Not classifier-backed: it refuses what is not allowed, unasked. Never travels.
        ("dontAsk", Reported::Asks),
        ("auto", Reported::Classifier),
        ("bypassPermissions", Reported::PromptsOff),
        // A word this purlis does not know asks.
        ("autonomous", Reported::Asks),
        ("Auto", Reported::Asks),
        ("", Reported::Asks),
    ] {
        assert_eq!(
            Reported::of_claude_code(Some(mode)),
            Some(reported),
            "{mode:?}"
        );
    }
    // A payload with no mode says nothing, and takes nothing back.
    assert_eq!(Reported::of_claude_code(None), None);
}

#[test]
fn a_task_starts_at_its_asker_s_mode_capped_at_auto() {
    assert_eq!(for_a_task(Some(Reported::Asks)), Level::Asks);
    assert_eq!(for_a_task(Some(Reported::Classifier)), Level::Classifier);
    // Bypass never travels: the task gets auto.
    assert_eq!(for_a_task(Some(Reported::PromptsOff)), Level::Classifier);
    // An asker whose harness never said asks.
    assert_eq!(for_a_task(None), Level::Asks);
}

#[test]
fn the_last_report_that_says_a_mode_is_the_one_kept() {
    let mut heard = Heard::default();
    assert_eq!(heard.for_a_task(), Level::Asks);
    heard.heard(Some(Reported::Classifier));
    assert_eq!(heard.for_a_task(), Level::Classifier);
    // A report that names no mode leaves it as it was.
    heard.heard(None);
    assert_eq!(heard.for_a_task(), Level::Classifier);
    // The person switched the chat back to asking: its next task asks too.
    heard.heard(Some(Reported::Asks));
    assert_eq!(heard.for_a_task(), Level::Asks);
    heard.heard(Some(Reported::PromptsOff));
    assert_eq!(heard.for_a_task(), Level::Classifier);
}

#[test]
fn a_reported_mode_is_one_word_on_the_hook_s_line() {
    for (reported, word) in [
        (Reported::Asks, "\"asks\""),
        (Reported::Classifier, "\"classifier\""),
        (Reported::PromptsOff, "\"prompts_off\""),
    ] {
        assert_eq!(serde_json::to_string(&reported).expect("json"), word);
        assert_eq!(
            serde_json::from_str::<Reported>(word).expect("reads"),
            reported
        );
    }
}

#[test]
fn bypass_never_travels_to_a_task_on_any_harness() {
    use crate::dispatchunattended::{Inherited, bypass_in, start_of_a_persona_chat};
    use crate::harness::Harness;
    // Whatever the asker reported, on every harness, the words a task starts with switch no
    // prompt off: the command a task runs is one `bypass_in` finds nothing in.
    for harness in [Harness::ClaudeCode, Harness::Codex, Harness::Opencode] {
        for asker in [
            None,
            Some(Reported::Asks),
            Some(Reported::Classifier),
            Some(Reported::PromptsOff),
        ] {
            let command = vec!["harness".to_owned()];
            let mut line = command.clone();
            line.extend(
                harness
                    .adapter()
                    .task_mode_words(for_a_task(asker), &command),
            );
            assert_eq!(bypass_in(&line), None, "{harness:?} {asker:?}: {line:?}");
        }
    }
    // And a profile that switches them off is still refused for a task, at any level.
    let profile = [
        "claude".to_owned(),
        "--dangerously-skip-permissions".to_owned(),
    ];
    assert!(
        start_of_a_persona_chat(
            crate::start::Start::default(),
            "devops",
            Some(Inherited {
                profile: "yolo",
                command: &profile,
            }),
        )
        .is_err()
    );
}

fn report(
    conversation: Option<&str>,
    agent: Option<&str>,
    mode: Reported,
) -> crate::hookwire::Report {
    use crate::hookwire::Conversation;
    crate::hookwire::Report {
        chat: 7,
        event: crate::state::Event::UserPromptSubmit,
        conversation: conversation.map_or(Conversation::Unknown, |id| {
            Conversation::Named(id.to_owned())
        }),
        pid: Some(4007),
        agent: agent.map(str::to_owned),
        detail: crate::state::Detail {
            mode: Some(mode),
            ..crate::state::Detail::default()
        },
    }
}

#[test]
fn only_the_chat_s_own_harness_names_its_mode() {
    let now = Some("11111111-2222-4333-8444-555555555555");
    let own = report(now, None, Reported::Classifier);
    assert_eq!(of_its_own_harness(&own, now), Some(Reported::Classifier));
    // A harness nested in the chat names another conversation.
    let nested = report(
        Some("99999999-2222-4333-8444-555555555555"),
        None,
        Reported::Classifier,
    );
    assert_eq!(of_its_own_harness(&nested, now), None);
    // One that names none, or a chat the board holds no conversation for.
    assert_eq!(
        of_its_own_harness(&report(None, None, Reported::Classifier), now),
        None
    );
    assert_eq!(of_its_own_harness(&own, None), None);
    // A sub-agent's.
    assert_eq!(
        of_its_own_harness(&report(now, Some("a1"), Reported::Classifier), now),
        None
    );
}

#[test]
fn a_grade_s_chat_and_a_profile_s_own_mode_get_no_added_words() {
    use crate::harness::Harness;
    let words = |command: &[&str], graded: bool| {
        let command: Vec<String> = command.iter().map(|w| (*w).to_owned()).collect();
        task_words(Harness::ClaudeCode, Level::Classifier, &command, graded)
    };
    assert_eq!(words(&["claude"], false), ["--permission-mode", "auto"]);
    // A grade's chat keeps the mode its grade pins, whatever level its record holds.
    assert!(words(&["claude"], true).is_empty());
    // `dontAsk` is stricter than the default, and the profile's word stands: no auto added,
    // and the task still starts on it.
    let dont_ask = ["claude", "--permission-mode", "dontAsk"];
    assert!(words(&dont_ask, false).is_empty());
    let command: Vec<String> = dont_ask.iter().map(|w| (*w).to_owned()).collect();
    assert!(
        crate::dispatchunattended::start_of_a_persona_chat(
            crate::start::Start::default(),
            "devops",
            Some(crate::dispatchunattended::Inherited {
                profile: "strict",
                command: &command,
            }),
        )
        .is_ok()
    );
}
