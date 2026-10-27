// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_known_model_is_priced_at_its_own_dearest_token() {
    let priced = of_model(Some("claude-sonnet-4-6"));
    assert_eq!(
        priced,
        Priced {
            usd_per_million: 15.0,
            from: PricedFrom::Model,
            model: Some("claude-sonnet-4-6".into()),
        }
    );
    assert!((limit_usd(200_000, &priced) - 3.0).abs() < 1e-9);
}

#[test]
fn a_snapshot_a_context_window_and_the_case_are_read_past() {
    for id in [
        "claude-opus-4-1-20250805",
        "Claude-Opus-4-1",
        "claude-opus-4-1[1m]",
        " claude-opus-4-1 ",
    ] {
        let priced = of_model(Some(id));
        assert_eq!(
            (priced.usd_per_million, priced.from),
            (75.0, PricedFrom::Model),
            "{id}"
        );
    }
    assert_eq!(of_model(Some("claude-opus-4-20250514")).usd_per_million, 75.0);
    assert_eq!(of_model(Some("sonnet[1m]")).usd_per_million, 22.5);
}

#[test]
fn no_model_or_one_not_priced_is_limited_at_the_dearest() {
    for model in [
        None,
        Some(""),
        Some("claude-opus-9"),
        Some("anthropic.claude-sonnet-4-6"),
        Some("gpt-5"),
    ] {
        let priced = of_model(model);
        assert_eq!(priced.usd_per_million, DEAREST_USD_PER_MILLION, "{model:?}");
        assert_eq!(priced.from, PricedFrom::Dearest, "{model:?}");
    }
    assert_eq!(
        of_model(Some("gpt-5")).model.as_deref(),
        Some("gpt-5"),
        "the model it runs on is still said"
    );
}

#[test]
fn the_dearest_is_the_most_of_the_table_so_no_model_is_priced_above_it() {
    let most = PRICES.iter().map(|(_, price)| *price).fold(0.0, f64::max);
    assert!((most - DEAREST_USD_PER_MILLION).abs() < 1e-9);
    assert!(PRICES.iter().all(|(_, price)| *price > 0.0));
    assert!(
        PRICES
            .iter()
            .all(|(id, _)| base_id(id) == *id && !id.is_empty()),
        "every key is as base_id reads one"
    );
}

#[test]
fn a_model_no_reader_could_read_is_priced_at_the_dearest_and_not_said() {
    let priced = of_model(Some("claude-opus-4-1\u{202e}"));
    assert_eq!(priced.from, PricedFrom::Dearest);
    assert_eq!(priced.model, None);
}
