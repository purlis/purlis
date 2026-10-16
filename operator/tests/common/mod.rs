// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the operator's tests share: reading a YAML file of several documents.

use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

/// Every document of the YAML file at `path`, as JSON, empty documents left out.
pub fn yaml_documents(path: &Path) -> Vec<Value> {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_yaml::Deserializer::from_str(&text)
        .map(|document| {
            let yaml = serde_yaml::Value::deserialize(document).expect("YAML");
            serde_json::to_value(yaml).expect("as JSON")
        })
        .filter(|document| !document.is_null())
        .collect()
}
