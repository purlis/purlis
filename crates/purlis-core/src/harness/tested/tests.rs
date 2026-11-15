// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The shipped table: its file, its source rule, the Chat Completions pairs' usage field, the
//! Anthropic built-in's losses, and Claude Code's last tested version (T1a-33, R-315, R-294).

use super::*;

/// The table as this build ships it.
const SHIPPED: &str = include_str!("../tested.toml");

/// What Claude Code answered `--version` with, as CI installs it: the pin.
const CLAUDE_CODE_PIN: &str =
    include_str!("../../../../../tests/fixtures/harness-versions/claude.txt");

/// A provider declaration serving Chat Completions alone, so a pair with it is a Chat
/// Completions pair. No built-in serves one in 1a.
fn a_chat_completions_provider() -> model_provider::Declaration {
    model_provider::parse(
        "id = \"chatty\"\nname = \"Chatty\"\nkind = \"vendor\"\n\n\
         [[endpoint]]\nformat = \"openai-chat\"\nbase = \"https://api.chatty.example\"\n",
        &model_provider::From::Yours,
        model_provider::built_ins(),
        "chatty.toml",
    )
    .expect("a Chat Completions declaration")
}

#[test]
fn the_shipped_table_parses() {
    let read = parse(SHIPPED, model_provider::built_ins(), "tested.toml");
    assert!(
        read.as_ref().is_ok_and(|table| !table.entries().is_empty()),
        "{read:?}"
    );
    assert_eq!(shipped(), &read.expect("read"));
}

#[test]
fn an_entry_without_a_source_is_refused() {
    let entry = "[[entry]]\nharness = \"claude\"\nprovider = \"anthropic\"\n\
                 format = \"anthropic-messages\"\nlosses = []\n";
    let sourced = format!("{entry}source = \"reported: https://example.com/losses\"\n");
    let refused = parse(entry, model_provider::built_ins(), "tested.toml");
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.contains("tested.toml") && why.contains("source")),
        "{refused:?}"
    );
    assert!(
        parse(&sourced, model_provider::built_ins(), "tested.toml").is_ok(),
        "the same entry with a source is read"
    );
    for not_a_source in [
        "",
        "measured",
        "measured on 2026-11-16",
        "measured on yesterday, 2.1.300",
        "reported: ",
        "reported: not a link",
    ] {
        let malformed = format!("{entry}source = \"{not_a_source}\"\n");
        let read = parse(&malformed, model_provider::built_ins(), "tested.toml");
        assert!(
            read.as_ref().is_err_and(|why| why.contains("source")),
            "{not_a_source:?} is no source: {read:?}"
        );
    }
    let measured = format!("{entry}source = \"measured on 2026-11-16, 2.1.300\"\n");
    assert_eq!(
        parse(&measured, model_provider::built_ins(), "tested.toml")
            .map(|table| table.entries()[0].source.clone()),
        Ok(Source::Measured {
            on: "2026-11-16".into(),
            version: "2.1.300".into()
        })
    );
}

#[test]
fn a_chat_completions_entry_without_asks_for_usage_is_refused() {
    let mut providers = model_provider::built_ins().to_vec();
    providers.push(a_chat_completions_provider());
    let entry = "[[entry]]\nharness = \"codex\"\nprovider = \"chatty\"\n\
                 format = \"openai-chat\"\nlosses = []\n\
                 source = \"reported: https://example.com/chatty\"\n";
    let refused = parse(entry, &providers, "tested.toml");
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.contains("tested.toml") && why.contains("asks_for_usage")),
        "{refused:?}"
    );
    let said = format!("{entry}asks_for_usage = true\n");
    assert_eq!(
        parse(&said, &providers, "tested.toml").map(|table| table.entries()[0].asks_for_usage),
        Ok(Some(true))
    );
}

#[test]
fn each_harness_has_a_last_tested_version() {
    for harness in Harness::ALL {
        let declared = crate::harness_declaration::of(harness).tested.as_deref();
        assert!(declared.is_some(), "{harness:?} declares no tested version");
        assert_eq!(
            last_tested(harness),
            declared,
            "{harness:?}'s last tested version is its declaration's `tested`"
        );
    }
}

#[test]
fn the_declarations_tested_version_and_the_ci_pin_move_together() {
    let pin = crate::profilestore::pin::version_in("claude", CLAUDE_CODE_PIN)
        .map(|version| version.to_string());
    assert!(
        pin.is_some(),
        "the pin names a version: {CLAUDE_CODE_PIN:?}"
    );
    assert_eq!(
        crate::harness_declaration::of(Harness::ClaudeCode)
            .tested
            .clone(),
        pin,
        "harness_declaration/claude.toml's `tested` is the CI pin, \
         tests/fixtures/harness-versions/claude.txt: move both together"
    );
}

#[test]
fn the_anthropic_entry_lists_its_six_losses() {
    assert_eq!(
        shipped()
            .entry("claude", "anthropic", None)
            .map(|entry| entry.losses.clone()),
        Some(
            [
                "background-on-main-model-unless-haiku",
                "remote-control-and-voice-off",
                "fast-disabled",
                "tool-streaming-off",
                "feedback-local",
                "hipaa-not-covered",
            ]
            .map(str::to_owned)
            .to_vec()
        )
    );
}
