// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The token cap a grade is held to** (#2779; ADR 0105 §4, confirmed 2026-10-26):
//! `[skill_grades] token_cap` in this machine's local settings file, [`DEFAULT`] where it is not
//! set. The person who pays for a grade's chats sets it, so it lives in the file that is theirs
//! alone, never in the committed one. The plan says it before Grade is pressed, and a grade that
//! reaches it stops its running chat, starts no more, and says what it spent.
//!
//! **A cost bound besides** ([`Cap::cost_usd`]): the cap's tokens at the dearest price any model
//! a grade can run on charges for one ([`DEAREST_USD_PER_MILLION`]). Where the harness says what a
//! chat cost, a grade stops at it too, so what a harness spends past the tokens it reports (a
//! cache, a sub-agent) is still held. It never stops a grade before the token cap whose tokens
//! were all reported.

use std::path::Path;

use crate::settings::{LayerText, Refusal, Which};

/// The table it lives in, at the top of `charter.local.toml`.
pub const TABLE: &str = "skill_grades";

/// Its key in [`TABLE`].
pub const KEY: &str = "token_cap";

/// The cap where none is set: 200,000 tokens a grade.
pub const DEFAULT: u64 = 200_000;

/// The least and the most a cap may be set to.
pub const LEAST: u64 = 1_000;
pub const MOST: u64 = 100_000_000;

/// Where the cap a grade is held to comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum From {
    /// Nothing set it: [`DEFAULT`].
    Default,
    /// This machine's `charter.local.toml`.
    Yours,
}

/// The dearest price, in dollars a million tokens, that any model a grade's chats can run on
/// charges for a token of any kind: Claude Opus 4 and 4.1's output. The cost bound is the cap's
/// tokens at this price, so it is never the tighter of the two for a chat that reported its
/// tokens.
pub const DEAREST_USD_PER_MILLION: f64 = 75.0;

/// The cap a grade is held to, and where it comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Cap {
    pub tokens: u64,
    pub from: From,
}

impl Cap {
    /// **The cost a grade held to this cap may reach**, where its harness says what its chats
    /// cost: the cap's tokens at [`DEAREST_USD_PER_MILLION`].
    pub fn cost_usd(&self) -> f64 {
        self.tokens as f64 * DEAREST_USD_PER_MILLION / 1_000_000.0
    }
}

/// The cap `value` says, or the sentence saying why it is not one.
fn of_value(value: &toml::Value, file: &str) -> Result<u64, String> {
    value
        .as_integer()
        .and_then(|n| u64::try_from(n).ok())
        .filter(|n| (LEAST..=MOST).contains(n))
        .ok_or_else(|| {
            format!(
                "[{TABLE}] {KEY} in {file} is not a whole number of tokens from {LEAST} to {MOST}, \
                 so a grade is held to {DEFAULT}"
            )
        })
}

/// **The cap a grade in the project at `root` is held to**: this machine's, where it is set and
/// reads, else [`DEFAULT`]. A value that does not read is the default, and the Settings tab says
/// why ([`keyed`]).
pub fn token_cap(root: &Path) -> Cap {
    let default = Cap {
        tokens: DEFAULT,
        from: From::Default,
    };
    let LayerText::Text(text) = crate::settings::local_layer_holding(root, TABLE) else {
        return default;
    };
    let Ok(top) = text.parse::<toml::Table>() else {
        return default;
    };
    match top
        .get(TABLE)
        .and_then(toml::Value::as_table)
        .and_then(|table| table.get(KEY))
        .map(|value| of_value(value, Which::Local.file_at(root)))
    {
        Some(Ok(tokens)) => Cap {
            tokens,
            from: From::Yours,
        },
        _ => default,
    }
}

/// **What `text`, the settings file `which` of the project at `root`, says wrong of [`TABLE`]**:
/// a table that is not one, a key it does not know, a cap out of bounds, or the table in the
/// committed file, where a teammate's push would set what this machine's chats may spend.
pub fn keyed_at(root: &Path, text: &str, which: Which) -> Vec<Refusal> {
    let file = which.file_at(root);
    let Ok(top) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let Some(value) = top.get(TABLE) else {
        return Vec::new();
    };
    if which != Which::Local {
        return vec![Refusal::at(
            format!(
                "[{TABLE}] in {file} is not read: a grade's cap is what this machine spends, so \
                 it is set in {} alone",
                Which::Local.file_at(root)
            ),
            &[TABLE],
        )];
    }
    let Some(table) = value.as_table() else {
        return vec![Refusal::at(
            format!("[{TABLE}] in {file} is not a table"),
            &[TABLE],
        )];
    };
    let mut out = Vec::new();
    for (key, value) in table {
        if key != KEY {
            out.push(Refusal::at(
                format!(
                    "[{TABLE}] {} in {file} is not read: the table holds {KEY} alone",
                    crate::shown::short(key)
                ),
                &[TABLE, key],
            ));
        } else if let Err(why) = of_value(value, file) {
            out.push(Refusal::at(why, &[TABLE, KEY]));
        }
    }
    out
}

/// **Sets this machine's cap for the project at `root` to `tokens`**, in `charter.local.toml`,
/// every other line kept; refused out of bounds, and for what the file's own save refuses.
pub fn set_token_cap(root: &Path, tokens: u64) -> Result<Cap, String> {
    if !(LEAST..=MOST).contains(&tokens) {
        return Err(format!(
            "A grade's cap is a whole number of tokens from {LEAST} to {MOST}, so nothing was \
             changed."
        ));
    }
    let file = Which::Local.file_at(root);
    let path = root.join(file);
    let base = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => None,
        Err(why) => {
            return Err(format!("{file} could not be read ({why})."));
        }
    };
    let mut doc: toml_edit::DocumentMut =
        base.as_deref()
            .unwrap_or_default()
            .parse()
            .map_err(|why: toml_edit::TomlError| {
                format!(
                    "{file} is not valid TOML ({}), so nothing was changed.",
                    crate::shown::short(why.message())
                )
            })?;
    let table = doc
        .entry(TABLE)
        .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
    let Some(table) = table.as_table_mut() else {
        return Err(format!(
            "[{TABLE}] in {file} is not a table, so nothing was changed."
        ));
    };
    table[KEY] = toml_edit::value(i64::try_from(tokens).unwrap_or(i64::MAX));
    crate::settings::save(root, Which::Local, base.as_deref(), &doc.to_string())
        .map_err(|why| why.join(" "))?;
    Ok(Cap {
        tokens,
        from: From::Yours,
    })
}

#[cfg(test)]
mod tests;
