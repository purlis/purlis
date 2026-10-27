// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The price a grade's dollar limit is worked out at** (the operator's ruling G12 of
//! 2026-10-27, #3469; ADR 0105 as amended 2026-10-27): the dearest a token of any kind costs on
//! the model a grade's chats run on, or, where that model or its price is not known, the dearest
//! any model in [`PRICES`] costs ([`DEAREST_USD_PER_MILLION`]), so the limit is never set below
//! what its tokens may cost.
//!
//! **The prices are a table in purlis, never read from the network** at grade time: a grade
//! is planned and held without a call out, and the table is dated ([`AS_OF`]) so a stale one is
//! seen. Each price is a model's dearest per-token rate on Anthropic's own API: its output, at
//! the dearer of its context tiers and of fast mode where it has them. A model that is not here
//! (a newer one, a cloud provider's id, whose prices are that provider's) is priced at the
//! dearest, which only ever makes a limit wider, never tighter, than its model's.

use purlis_skills::grade::{Priced, PricedFrom};

/// The day [`PRICES`] was read from Anthropic's published prices.
pub const AS_OF: &str = "2026-10-06";

/// **Each model's dearest price for one token, in dollars a million**, by its id on Anthropic's
/// API, and the aliases Claude Code takes for a family, priced at that family's dearest.
pub const PRICES: &[(&str, f64)] = &[
    ("claude-fable-5-1", 50.0),
    ("claude-mythos-5-1", 50.0),
    ("claude-fable-5", 50.0),
    ("claude-mythos-5", 50.0),
    // Fast mode doubles Opus 5.5's $20 and Opus 5's and 4.8's $25.
    ("claude-opus-5-5", 40.0),
    ("claude-opus-5", 50.0),
    ("claude-opus-4-8", 50.0),
    ("claude-opus-4-7", 25.0),
    ("claude-opus-4-6", 25.0),
    ("claude-opus-4-5", 25.0),
    ("claude-opus-4-1", 75.0),
    ("claude-opus-4-0", 75.0),
    ("claude-opus-4", 75.0),
    ("claude-sonnet-5-5", 10.0),
    ("claude-sonnet-5", 10.0),
    ("claude-sonnet-4-6", 15.0),
    // Past 200,000 tokens of context, Sonnet 4.5 and 4 charge $22.50 for output.
    ("claude-sonnet-4-5", 22.5),
    ("claude-sonnet-4-0", 22.5),
    ("claude-sonnet-4", 22.5),
    // Past 100,000 tokens of prompt, Haiku 5.5 charges $2.50 for output.
    ("claude-haiku-5-5", 2.5),
    ("claude-haiku-4-5", 5.0),
    // Claude Code's aliases name a family, not a model: its dearest.
    ("fable", 50.0),
    ("opus", 75.0),
    ("opusplan", 75.0),
    ("sonnet", 22.5),
    ("haiku", 5.0),
];

/// **The dearest price any model a grade's chats can run on charges for one token**, in dollars
/// a million: Claude Opus 4 and 4.1's output, the most of [`PRICES`].
pub const DEAREST_USD_PER_MILLION: f64 = 75.0;

/// **The most a row read back may say a token cost**, in dollars a million: well past
/// [`DEAREST_USD_PER_MILLION`], so a row a newer purlis priced at a dearer model it knows is
/// still read, and a row that says more is not.
pub const MOST_READ_USD_PER_MILLION: f64 = 1_000.0;

/// **The price a grade on `model` is limited at** ([`PRICES`]), or the dearest where `model` is
/// not said or not priced. An id is read without the case it is written in, a context window
/// Claude Code names after it (`[1m]`) or the date of its snapshot (`-20250805`).
pub fn of_model(model: Option<&str>) -> Priced {
    let model = model.map(str::trim).filter(|model| model_id(model));
    let priced = model.and_then(|model| {
        let key = base_id(model);
        PRICES
            .iter()
            .find(|(id, _)| *id == key)
            .map(|(_, price)| *price)
    });
    Priced {
        usd_per_million: priced.unwrap_or(DEAREST_USD_PER_MILLION),
        from: if priced.is_some() {
            PricedFrom::Model
        } else {
            PricedFrom::Dearest
        },
        model: model.map(str::to_owned),
    }
}

/// **A model's id as a harness names one**: at most 64 of letters, digits and `-_.:@/[]`. One
/// that is not is priced at the dearest and not said, as a person reads what is said.
pub(crate) fn model_id(word: &str) -> bool {
    !word.is_empty()
        && word.len() <= 64
        && word
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:@/[]".contains(&b))
}

/// `model` as [`PRICES`] keys it: in lower case, without a bracketed suffix or a snapshot's date.
fn base_id(model: &str) -> String {
    let lower = model.to_ascii_lowercase();
    let lower = match lower.find('[') {
        Some(at) => &lower[..at],
        None => lower.as_str(),
    };
    match lower.rsplit_once('-') {
        Some((base, date)) if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) => {
            base.to_owned()
        }
        _ => lower.to_owned(),
    }
}

/// **The dollar limit of a cap of `tokens` at `priced`**.
pub fn limit_usd(tokens: u64, priced: &Priced) -> f64 {
    tokens as f64 * priced.usd_per_million / 1_000_000.0
}

#[cfg(test)]
mod tests;
