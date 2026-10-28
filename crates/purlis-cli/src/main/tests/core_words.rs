// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use clap::CommandFactory;
use std::collections::BTreeSet;

/// Every word this binary's parser answers as its first argument: each command's name,
/// every other name clap takes for it, and clap's own `help` — read off the parser, so a
/// command added to it is in this set the day it is added.
fn words_the_parser_answers() -> BTreeSet<String> {
    let mut root = Cli::command();
    root.build();
    root.get_subcommands()
        .flat_map(|sub| {
            std::iter::once(sub.get_name().to_owned())
                .chain(sub.get_all_aliases().map(str::to_owned))
        })
        .collect()
}

#[test]
fn every_word_charter_answers_is_one_no_extension_may_take() {
    // **charter-app#342: an extension can never take over a core word.** The refusal lives
    // in the core, which the app calls to approve an extension and which cannot read this
    // parser — so this is where the list it refuses by is held to the parser, both ways.
    // A new core command that is not in `CORE_WORDS` fails here, in the change adding it.
    let parser = words_the_parser_answers();
    let refused: BTreeSet<String> = purlis_core::extension::cli::CORE_WORDS
        .iter()
        .map(|&word| word.to_owned())
        .collect();
    let unprotected: Vec<&String> = parser.difference(&refused).collect();
    assert!(
        unprotected.is_empty(),
        "`charter {unprotected:?}` is a core command an extension could still take as its \
         id — add it to purlis_core::extension::cli::CORE_WORDS"
    );
    let stale: Vec<&String> = refused.difference(&parser).collect();
    assert!(
        stale.is_empty(),
        "CORE_WORDS refuses {stale:?}, which is no longer a word charter answers"
    );
}

#[test]
fn every_word_purlis_answers_is_a_name_no_profile_may_take() {
    // #2422: a profile named like a command would shadow it, so the core refuses the name.
    // The core cannot read this parser either, so here every word it answers is declared
    // as a profile and each one must come back refused for its name.
    let parser = words_the_parser_answers();
    let local: String = parser
        .iter()
        .map(|word| format!("[harness.{word}]\nkind = \"claude\"\ncommand = [\"claude\"]\n"))
        .collect();
    let set = purlis_core::profiles::current_of(purlis_core::profiles::derive_from(
        None,
        Ok(Some(local)),
    ));
    let refused_for_the_name: BTreeSet<&str> = set
        .refused
        .iter()
        .filter(|r| r.reason.contains("is named like the command"))
        .map(|r| r.name.as_str())
        .collect();
    let unprotected: Vec<&String> = parser
        .iter()
        .filter(|word| !refused_for_the_name.contains(word.as_str()))
        .collect();
    assert!(
        unprotected.is_empty(),
        "`purlis {unprotected:?}` is a command a profile may still be named after — a word \
         the parser answers must be in purlis_core::extension::cli::CORE_WORDS"
    );
}
