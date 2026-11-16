// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The first stable release that reads each policy key (T1a-64, H-11): the table beside `KEYS`
//! covers every key the parser takes, and only those.

use super::{DISPATCH_LOCKS, FIRST_READ, FirstRead, KEYS, TOP_KEYS};
use crate::dispatchlimits::Limit;

/// Every key the strict parse takes, as `purlis policy check` names it: the top level's, then
/// `sandbox.<key>` and `dispatch.<key>`, each limit's included.
fn every_key_the_parser_takes() -> Vec<String> {
    let mut keys: Vec<String> = TOP_KEYS.iter().map(|key| (*key).to_owned()).collect();
    keys.extend(KEYS.iter().map(|key| format!("sandbox.{key}")));
    keys.extend(DISPATCH_LOCKS.iter().map(|key| format!("dispatch.{key}")));
    keys.extend(
        Limit::ALL
            .iter()
            .map(|limit| format!("dispatch.{}", limit.word())),
    );
    keys
}

#[test]
fn every_policy_key_has_a_first_version() {
    let taken = every_key_the_parser_takes();
    let listed: Vec<&str> = FIRST_READ.iter().map(|(key, _)| *key).collect();
    for key in &taken {
        assert!(
            listed.contains(&key.as_str()),
            "{key} has no first version in FIRST_READ"
        );
    }
    for key in &listed {
        assert!(
            taken.iter().any(|taken| taken == key),
            "{key} is in FIRST_READ, and the parser doesn't take it"
        );
    }
    // Every release named is one CHANGELOG.md records.
    let changelog = include_str!("../../../../../CHANGELOG.md");
    for (key, first) in FIRST_READ {
        if let FirstRead::Release(version) = first {
            assert!(
                changelog.contains(&format!("\n## [{version}] - ")),
                "{key}'s first version, {version}, is not a release in CHANGELOG.md"
            );
        }
    }
}

#[test]
fn min_version_and_models_are_in_no_stable_release_yet() {
    for key in ["min-version", "models"] {
        let first = FIRST_READ
            .iter()
            .find(|(listed, _)| *listed == key)
            .map(|(_, first)| *first);
        assert_eq!(first, Some(FirstRead::NotYet), "{key}");
    }
    let first = |key: &str| {
        FIRST_READ
            .iter()
            .find(|(listed, _)| *listed == key)
            .map(|(_, first)| *first)
    };
    assert_eq!(first("sandbox.opt-out"), Some(FirstRead::Release("0.5.0")));
    assert_eq!(first("sandbox.run-grants"), Some(FirstRead::Release("0.5.1")));
}
