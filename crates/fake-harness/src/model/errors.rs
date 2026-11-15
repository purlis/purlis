// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The fake provider's error answers (`fixtures/errors.json`), each marked with where its shape
//! came from, so no test claims a vendor shape nobody saw (D6640-T2 V1, C-1, J-1).

use serde_json::Value;

use super::recording::Recording;

/// The fixture file, built in so a test reads it wherever it runs.
const FIXTURES: &str = include_str!("../../fixtures/errors.json");

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
///
/// # Panics
/// When the fixture file is malformed, which its own tests catch.
pub fn error_fixtures() -> Vec<ErrorFixture> {
    let file: Value = serde_json::from_str(FIXTURES).expect("fixtures/errors.json is JSON");
    file["fixtures"]
        .as_array()
        .expect("fixtures/errors.json lists its fixtures")
        .iter()
        .map(fixture)
        .collect()
}

fn fixture(entry: &Value) -> ErrorFixture {
    let text = |key: &str| entry[key].as_str().unwrap_or_default().to_owned();
    let name = text("name");
    let provenance = match entry["provenance"].as_str() {
        Some("recorded") => Provenance::Recorded {
            source: text("recorded"),
        },
        Some("unrecorded") => Provenance::Unrecorded,
        Some("from the docs, unrecorded") => Provenance::FromTheDocs,
        other => panic!("error fixture {name} has no known provenance: {other:?}"),
    };
    let mut recording = Recording::whole(&text("content_type"), text("body").as_bytes());
    recording.status = entry["status"]
        .as_u64()
        .and_then(|status| u16::try_from(status).ok())
        .unwrap_or_else(|| panic!("error fixture {name} has no status"));
    recording.headers.extend(
        entry["headers"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|pair| Some((pair[0].as_str()?.to_owned(), pair[1].as_str()?.to_owned()))),
    );
    ErrorFixture {
        name,
        provenance,
        recording,
    }
}
