// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The built-in declarations this build ships (T1a-07, #6750).

use super::*;

/// The phase's list of built-ins (spec section 5; 1a: `anthropic` only). A built-in added or
/// removed changes this list in the same change, or this file fails.
const PHASE_LIST: &[&str] = &["anthropic"];

/// The Anthropic built-in's digest. It moves only when a covered field of the shipped
/// declaration does, which the runner's check (set iii) reads as another build's built-in.
const ANTHROPIC_DIGEST: &str = "PINNED";

fn anthropic() -> &'static Declaration {
    built_ins()
        .iter()
        .find(|declaration| declaration.id == "anthropic")
        .expect("the Anthropic built-in ships")
}

fn project() -> From {
    From::Project {
        project: "github.com/acme/app".to_owned(),
    }
}

#[test]
fn the_built_in_ids_equal_the_phase_list() {
    let ids: Vec<&str> = built_ins().iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, PHASE_LIST);
    assert!(built_ins().iter().all(|d| d.from == From::BuiltIn));
}

#[test]
fn the_anthropic_built_in_resolves_to_its_messages_endpoint() {
    let anthropic = anthropic();
    assert_eq!(anthropic.name, "Anthropic");
    assert_eq!(anthropic.kind, Kind::Vendor);
    let endpoints: Vec<(Format, &str, &str, Auth)> = anthropic
        .endpoints
        .iter()
        .map(|e| (e.format, e.base.origin(), e.base.path(), e.auth))
        .collect();
    assert_eq!(
        endpoints,
        [(
            Format::AnthropicMessages,
            "https://api.anthropic.com",
            "",
            Auth::XApiKey
        )]
    );
}

#[test]
fn the_anthropic_built_in_names_a_background_model() {
    let anthropic = anthropic();
    let background = anthropic
        .models
        .background
        .as_deref()
        .filter(|model| !model.trim().is_empty());
    assert!(background.is_some(), "no background model is named");
    let row = anthropic
        .models
        .table
        .iter()
        .find(|row| Some(row.id.as_str()) == background);
    let beside = row.map(|row| (row.source.is_some(), row.as_of.is_some()));
    assert_eq!(
        beside,
        Some((true, true)),
        "the background model {background:?} has no row with its source and date"
    );
}

#[test]
fn the_anthropic_built_in_exposes_its_background_model_for_the_haiku_slot() {
    let anthropic = anthropic();
    assert!(anthropic.models.background.is_some());
    assert_eq!(
        anthropic.background_model(),
        anthropic.models.background.as_deref()
    );
}

#[test]
fn a_built_in_id_in_another_tier_is_refused() {
    let text = "id = \"anthropic\"\nname = \"Acme gateway\"\nkind = \"router\"\n\n\
                [[endpoint]]\nformat = \"anthropic-messages\"\nbase = \"https://example.com\"\n";
    // Refused even when the caller hands the reader nothing it already knows: the built-ins
    // are the binary's, so every other tier is read beside them.
    for from in [project(), From::Yours] {
        let why = parse(text, &from, &[], "provider.toml")
            .err()
            .unwrap_or_else(|| panic!("a {} declaration with id anthropic was read", from.word()));
        assert!(why.contains("`id`") && why.contains("anthropic"), "{why}");
    }
}

#[test]
fn the_built_in_has_a_stable_digest() {
    assert_eq!(anthropic().digest(), ANTHROPIC_DIGEST);
}
