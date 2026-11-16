// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The shipped table** of corrections and loss lists (design §3.11, D6632 §3, T1a-33, #6782):
//! `harness/tested.toml`, shipped in the binary beside the harness declarations and read once.
//!
//! An entry is keyed by harness, model provider declaration and, where a model behaves apart
//! from its provider's others, a model pattern. It carries a grade only where it overrides the
//! rule, the losses, and a source: `"measured on <date>, <harness version>"` or
//! `"reported: <link>"`. **An entry without a source is refused.** An entry names no wire
//! format: the pair's is the first of the harness's `[route] formats` the provider serves
//! ([`crate::keyroute::pair_format`]), and a pair sharing none is refused. A Chat Completions
//! pair (one whose worked-out format is `openai-chat`) says whether the harness asks for usage
//! (`asks_for_usage`), and an entry for one without it is refused; no other pair carries the
//! field. A loss is a word
//! (`fast-disabled`), never a sentence: what the window says for one is window copy, drawn from
//! the copy module by the ticket that shows it (1c).
//!
//! **A harness's last tested version** is its built-in declaration's `tested` field
//! ([`last_tested`]), never a second copy here. For Claude Code it is the version CI installs,
//! `tests/fixtures/harness-versions/claude.txt`, and a test keeps the two equal.
//!
//! Every refusal is one sentence naming the file, the entry and the field.

use std::collections::BTreeSet;

use crate::harness::Harness;
use crate::model_provider;

/// A grade an entry gives where it overrides the rule (design §3.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grade {
    Blocked,
    Native,
    Supported,
    Degraded,
}

impl Grade {
    fn of_word(word: &str) -> Option<Self> {
        match word {
            "blocked" => Some(Self::Blocked),
            "native" => Some(Self::Native),
            "supported" => Some(Self::Supported),
            "degraded" => Some(Self::Degraded),
            _ => None,
        }
    }
}

/// Where an entry's facts come from. File data, not window copy: nothing in 1a shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// `"measured on <date>, <harness version>"`, the date as `YYYY-MM-DD`.
    Measured { on: String, version: String },
    /// `"reported: <link>"`, an `https` link.
    Reported { link: String },
}

impl Source {
    fn of_words(words: &str) -> Option<Self> {
        if let Some(rest) = words.strip_prefix("measured on ") {
            let (on, version) = rest.split_once(", ")?;
            let plain = !version.is_empty() && !version.contains(char::is_whitespace);
            return (a_date(on) && plain).then(|| Self::Measured {
                on: on.to_owned(),
                version: version.to_owned(),
            });
        }
        let link = words.strip_prefix("reported: ")?;
        let host = link.strip_prefix("https://")?;
        let plain = !host.is_empty() && !link.contains(char::is_whitespace);
        plain.then(|| Self::Reported {
            link: link.to_owned(),
        })
    }
}

/// Whether `words` is a `YYYY-MM-DD` date with a month and a day in range.
fn a_date(words: &str) -> bool {
    let parts: Vec<&str> = words.split('-').collect();
    let [year, month, day] = parts[..] else {
        return false;
    };
    let number = |part: &str, len: usize| {
        (part.len() == len && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse::<u32>().ok())
            .flatten()
    };
    number(year, 4).is_some()
        && number(month, 2).is_some_and(|m| (1..=12).contains(&m))
        && number(day, 2).is_some_and(|d| (1..=31).contains(&d))
}

/// One entry of the table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The harness declaration's `name`: `claude`, `codex` or `opencode`.
    pub harness: String,
    /// The model provider declaration's `id`.
    pub provider: String,
    /// The model pattern, where the entry is for some of the provider's models only.
    pub model: Option<String>,
    /// The wire format the pair speaks, worked out, never written: the first of the harness's
    /// `[route] formats` the provider serves ([`crate::keyroute::pair_format`]).
    pub format: model_provider::Format,
    /// The grade, where it overrides the rule.
    pub grade: Option<Grade>,
    /// What the pair loses, each a word.
    pub losses: Vec<String>,
    pub source: Source,
    /// Whether the harness asks for usage: set for a Chat Completions pair alone.
    pub asks_for_usage: Option<bool>,
}

/// The table, read and checked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    entries: Vec<Entry>,
}

impl Table {
    /// Every entry, in the file's order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The entry keyed exactly by `harness`, `provider` and `model` (`None` for the entry
    /// covering the provider's every model). Matching a model against patterns is 1b's.
    pub fn entry(&self, harness: &str, provider: &str, model: Option<&str>) -> Option<&Entry> {
        self.entries.iter().find(|entry| {
            entry.harness == harness
                && entry.provider == provider
                && entry.model.as_deref() == model
        })
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTable {
    #[serde(default)]
    entry: Vec<RawEntry>,
}

/// Every field optional here, so a missing one is refused by name below.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    harness: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    grade: Option<String>,
    losses: Option<Vec<String>>,
    source: Option<String>,
    asks_for_usage: Option<bool>,
}

/// **Reads the table in `text`**, named `file` in every refusal, against the harness
/// declarations `harnesses` and the model provider declarations `providers` (an entry naming
/// another is refused), or the one sentence saying why it is not one. The first failure wins.
pub fn parse(
    text: &str,
    harnesses: &[crate::harness_declaration::Declaration],
    providers: &[model_provider::Declaration],
    file: &str,
) -> Result<Table, String> {
    let raw: RawTable = toml::from_str(text).map_err(|e| format!("{file}: {}", e.message()))?;
    let mut keys = BTreeSet::new();
    let mut entries = Vec::with_capacity(raw.entry.len());
    for (at, raw) in raw.entry.into_iter().enumerate() {
        let entry = read_entry(raw, harnesses, providers)
            .map_err(|why| format!("{file}: entry {}: {why}", at + 1))?;
        let key = (
            entry.harness.clone(),
            entry.provider.clone(),
            entry.model.clone(),
        );
        if !keys.insert(key) {
            return Err(format!(
                "{file}: entry {}: another entry has the same `harness`, `provider` and `model`",
                at + 1
            ));
        }
        entries.push(entry);
    }
    Ok(Table { entries })
}

fn read_entry(
    raw: RawEntry,
    harnesses: &[crate::harness_declaration::Declaration],
    providers: &[model_provider::Declaration],
) -> Result<Entry, String> {
    let missing = |field: &str| format!("has no `{field}`");
    let harness = raw.harness.ok_or_else(|| missing("harness"))?;
    let Some(declaration) = harnesses.iter().find(|declared| declared.name == harness) else {
        return Err(format!(
            "`harness` names {harness:?}, which is no harness purlis ships"
        ));
    };
    let provider_id = raw.provider.ok_or_else(|| missing("provider"))?;
    let Some(provider) = providers.iter().find(|p| p.id == provider_id) else {
        return Err(format!(
            "`provider` names {provider_id:?}, which is no model provider declaration"
        ));
    };
    let Some(format) = crate::keyroute::pair_format(declaration, provider) else {
        return Err(format!(
            "{harness:?} and {provider_id:?} share no wire format: the harness speaks none the \
             provider serves"
        ));
    };
    if raw.model.as_deref().is_some_and(str::is_empty) {
        return Err("`model` is empty".to_owned());
    }
    let grade = raw
        .grade
        .map(|word| {
            Grade::of_word(&word).ok_or_else(|| {
                format!("`grade` is {word:?}, not one of blocked, native, supported or degraded")
            })
        })
        .transpose()?;
    let losses = raw.losses.ok_or_else(|| missing("losses"))?;
    let mut seen = BTreeSet::new();
    for loss in &losses {
        let a_word = !loss.is_empty()
            && loss
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !a_word {
            return Err(format!(
                "the loss {loss:?} is not a word of lower-case letters, digits and `-`"
            ));
        }
        if !seen.insert(loss) {
            return Err(format!("the loss {loss:?} is listed twice"));
        }
    }
    let source = raw.source.ok_or_else(|| {
        "has no `source`: say \"measured on <date>, <harness version>\" or \"reported: <link>\""
            .to_owned()
    })?;
    let source = Source::of_words(&source).ok_or_else(|| {
        format!(
            "`source` is {source:?}, not \"measured on <YYYY-MM-DD>, <harness version>\" or \
             \"reported: <https link>\""
        )
    })?;
    let chat_completions = format == model_provider::Format::OpenaiChat;
    match (chat_completions, raw.asks_for_usage) {
        (true, None) => {
            return Err(
                "has no `asks_for_usage`: a Chat Completions pair says whether the harness asks \
                 for usage"
                    .to_owned(),
            );
        }
        (false, Some(_)) => {
            return Err(
                "has `asks_for_usage`, which only a Chat Completions pair carries".to_owned(),
            );
        }
        _ => {}
    }
    Ok(Entry {
        harness,
        provider: provider_id,
        model: raw.model,
        format,
        grade,
        losses,
        source,
        asks_for_usage: raw.asks_for_usage,
    })
}

/// **The table this build ships**, read once against the built-in model providers.
///
/// # Panics
///
/// When the shipped file is not a table this reader reads, which a test catches first.
pub fn shipped() -> &'static Table {
    static SHIPPED: std::sync::LazyLock<Table> = std::sync::LazyLock::new(|| {
        parse(
            include_str!("tested.toml"),
            crate::harness_declaration::builtins(),
            model_provider::built_ins(),
            "tested.toml",
        )
        .unwrap_or_else(|why| panic!("the shipped table this build ships is refused: {why}"))
    });
    &SHIPPED
}

/// **The version of `harness` purlis last tested**: its built-in declaration's `tested` field,
/// the one place it is kept (R-315). A machine's own replacement of the built-in's facts never
/// changes it, since it is what purlis tested, not what this machine says. What the
/// ignored-route Notice names (R-246).
pub fn last_tested(harness: Harness) -> Option<&'static str> {
    crate::harness_declaration::of(harness).tested.as_deref()
}

#[cfg(test)]
mod tests;
