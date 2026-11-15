// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The shipped table (T1a-33, #6782): stub, its tests first.

use crate::harness::Harness;
use crate::model_provider;

/// A grade an entry gives where it overrides the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grade {
    Blocked,
    Native,
    Supported,
    Degraded,
}

/// Where an entry's facts come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Measured { on: String, version: String },
    Reported { link: String },
}

/// One entry of the table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub harness: String,
    pub provider: String,
    pub model: Option<String>,
    pub format: model_provider::Format,
    pub grade: Option<Grade>,
    pub losses: Vec<String>,
    pub source: Source,
    pub asks_for_usage: Option<bool>,
}

/// The table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    entries: Vec<Entry>,
}

impl Table {
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn entry(&self, harness: &str, provider: &str, model: Option<&str>) -> Option<&Entry> {
        let _ = (harness, provider, model);
        None
    }
}

pub fn parse(
    text: &str,
    providers: &[model_provider::Declaration],
    file: &str,
) -> Result<Table, String> {
    let _ = (text, providers, file);
    Err("not built yet".into())
}

pub fn shipped() -> &'static Table {
    static EMPTY: Table = Table {
        entries: Vec::new(),
    };
    &EMPTY
}

pub fn last_tested(harness: Harness) -> Option<&'static str> {
    let _ = harness;
    None
}

#[cfg(test)]
mod tests;
