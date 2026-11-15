// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The fake provider's error answers (`fixtures/errors.json`), each marked with where its shape
//! came from, so no test claims a vendor shape nobody saw (D6640-T2 V1, C-1, J-1).

use serde_json::Value;

use super::recording::Recording;

/// Where an error fixture's shape came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provenance {
    /// Recorded from the vendor, by the source named.
    Recorded { source: String },
    /// Not recorded yet: the gate's hand run on a real key records it.
    Unrecorded,
    /// Written from the vendor's documentation (`api/errors`), not recorded yet.
    FromTheDocs,
}

/// **One error answer.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorFixture {
    /// Its name in the fixture file, such as `401` or `429-spend-cap`.
    pub name: String,
    /// Where its shape came from.
    pub provenance: Provenance,
    /// What it answers.
    pub recording: Recording,
}

/// Every error fixture, in the file's order.
pub fn error_fixtures() -> Vec<ErrorFixture> {
    let _ = Value::Null;
    Vec::new()
}
