// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Counts agree with their noun** (`docs/ui-copy.md`: *1 file*, *2 files*). Every sentence
//! purlis writes with a count says it through [`counted`], and a word that follows the count
//! (*is*/*are*, *it*/*them*) through [`plural`], so nothing shipped says "N file(s)".
//! `crates/purlis-core/tests/it/no_count_says_thing_s.rs` fails on a new one.

use std::fmt::Display;

/// `1 file`, `2 files`, `0 files`: the count, then the singular for exactly one and the plural
/// for every other count. Both words are given, so an irregular plural is written out
/// (`copy`/`copies`, `harness`/`harnesses`).
pub fn counted<N>(n: N, one: &str, many: &str) -> String
where
    N: Copy + Display + PartialEq + From<u8>,
{
    format!("{n} {}", plural(n, one, many))
}

/// `one` for a count of exactly one, `many` otherwise: for the word a count governs without
/// being written beside it (`is`/`are`, `it`/`them`).
pub fn plural<'a, N>(n: N, one: &'a str, many: &'a str) -> &'a str
where
    N: PartialEq + From<u8>,
{
    if n == N::from(1) { one } else { many }
}

#[cfg(test)]
mod tests;
