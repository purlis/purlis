// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The shipped table: its file, its source rule, the Chat Completions pairs' usage field, the
//! Anthropic built-in's losses, and Claude Code's last tested version (T1a-33, R-315, R-294).

use super::*;
use crate::harness_declaration::builtins as harnesses;

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

/// A provider declaration serving both formats, Messages first.
fn a_provider_serving_both() -> model_provider::Declaration {
    model_provider::parse(
        "id = \"duo\"\nname = \"Duo\"\nkind = \"vendor\"\n\n\
         [[endpoint]]\nformat = \"anthropic-messages\"\nbase = \"https://messages.duo.example\"\n\n\
         [[endpoint]]\nformat = \"openai-chat\"\nbase = \"https://chat.duo.example\"\n",
        &model_provider::From::Yours,
        model_provider::built_ins(),
        "duo.toml",
    )
    .expect("a declaration serving both formats")
}

/// The built-in harnesses, with Codex speaking Chat Completions, which no built-in does in 1a.
fn harnesses_with_a_chat_completions_codex() -> Vec<crate::harness_declaration::Declaration> {
    let mut harnesses = harnesses().to_vec();
    for harness in &mut harnesses {
        if harness.name == "codex" {
            harness.route.formats = vec![model_provider::Format::OpenaiChat];
        }
    }
    harnesses
}

/// The built-in model providers and `more`.
fn providers_and(more: &[model_provider::Declaration]) -> Vec<model_provider::Declaration> {
    let mut providers = model_provider::built_ins().to_vec();
    providers.extend_from_slice(more);
    providers
}

#[test]
fn the_shipped_table_parses() {
    let read = parse(
        SHIPPED,
        harnesses(),
        model_provider::built_ins(),
        "tested.toml",
    );
    assert!(
        read.as_ref().is_ok_and(|table| !table.entries().is_empty()),
        "{read:?}"
    );
    assert_eq!(shipped(), &read.expect("read"));
}

#[test]
fn an_entry_without_a_source_is_refused() {
    let entry = "[[entry]]\nharness = \"claude\"\nprovider = \"anthropic\"\nlosses = []\n";
    let sourced = format!("{entry}source = \"reported: https://example.com/losses\"\n");
    let refused = parse(
        entry,
        harnesses(),
        model_provider::built_ins(),
        "tested.toml",
    );
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.contains("tested.toml") && why.contains("source")),
        "{refused:?}"
    );
    assert!(
        parse(
            &sourced,
            harnesses(),
            model_provider::built_ins(),
            "tested.toml"
        )
        .is_ok(),
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
        let read = parse(
            &malformed,
            harnesses(),
            model_provider::built_ins(),
            "tested.toml",
        );
        assert!(
            read.as_ref().is_err_and(|why| why.contains("source")),
            "{not_a_source:?} is no source: {read:?}"
        );
    }
    let measured = format!("{entry}source = \"measured on 2026-11-16, 2.1.300\"\n");
    assert_eq!(
        parse(
            &measured,
            harnesses(),
            model_provider::built_ins(),
            "tested.toml"
        )
        .map(|table| table.entries()[0].source.clone()),
        Ok(Source::Measured {
            on: "2026-11-16".into(),
            version: "2.1.300".into()
        })
    );
}

#[test]
fn a_chat_completions_entry_without_asks_for_usage_is_refused() {
    let harnesses = harnesses_with_a_chat_completions_codex();
    let providers = providers_and(&[a_chat_completions_provider()]);
    let entry = "[[entry]]\nharness = \"codex\"\nprovider = \"chatty\"\nlosses = []\n\
                 source = \"reported: https://example.com/chatty\"\n";
    let refused = parse(entry, &harnesses, &providers, "tested.toml");
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.contains("tested.toml") && why.contains("asks_for_usage")),
        "{refused:?}"
    );
    let said = format!("{entry}asks_for_usage = true\n");
    assert_eq!(
        parse(&said, &harnesses, &providers, "tested.toml")
            .map(|table| (table.entries()[0].format, table.entries()[0].asks_for_usage)),
        Ok((model_provider::Format::OpenaiChat, Some(true)))
    );
}

/// Whether a pair asks for usage follows the format worked out from the harness's `[route]
/// formats` and what the provider serves, never a format the entry names.
#[test]
fn asks_for_usage_follows_the_pairs_worked_out_format() {
    let providers = providers_and(&[a_provider_serving_both()]);
    // Claude Code speaks Messages alone, so with a provider serving both its pair is Messages.
    let entry = "[[entry]]\nharness = \"claude\"\nprovider = \"duo\"\nlosses = []\n\
                 source = \"reported: https://example.com/duo\"\n";
    assert_eq!(
        parse(entry, harnesses(), &providers, "tested.toml")
            .map(|table| (table.entries()[0].format, table.entries()[0].asks_for_usage)),
        Ok((model_provider::Format::AnthropicMessages, None))
    );
    let said = format!("{entry}asks_for_usage = true\n");
    let refused = parse(&said, harnesses(), &providers, "tested.toml");
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.contains("asks_for_usage")),
        "a Messages pair carries no asks_for_usage: {refused:?}"
    );
}

#[test]
fn an_entry_whose_pair_shares_no_format_is_refused() {
    let providers = providers_and(&[a_chat_completions_provider()]);
    for (harness, provider) in [("claude", "chatty"), ("codex", "anthropic")] {
        let entry = format!(
            "[[entry]]\nharness = \"{harness}\"\nprovider = \"{provider}\"\nlosses = []\n\
             source = \"reported: https://example.com/{harness}\"\n"
        );
        let refused = parse(&entry, harnesses(), &providers, "tested.toml");
        assert!(
            refused
                .as_ref()
                .is_err_and(|why| why.contains("tested.toml")
                    && why.contains(harness)
                    && why.contains(provider)
                    && why.contains("no wire format")),
            "{harness} × {provider}: {refused:?}"
        );
    }
}

#[test]
fn an_entry_naming_its_format_is_refused() {
    let entry = "[[entry]]\nharness = \"claude\"\nprovider = \"anthropic\"\n\
                 format = \"anthropic-messages\"\nlosses = []\n\
                 source = \"reported: https://example.com/losses\"\n";
    let refused = parse(
        entry,
        harnesses(),
        model_provider::built_ins(),
        "tested.toml",
    );
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.contains("tested.toml") && why.contains("format")),
        "the pair's format is worked out, never written: {refused:?}"
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
