// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The manifest's order** (operator ruling 2026-10-26: two family pull requests never touch the
//! same lines). `build.rs` (`families`) assembles [`parts::MANIFEST`] from each family's own file,
//! so the order is a rule rather than a list anyone edits: what the first twelve versions shipped,
//! in the order they shipped it (which version 12's digest holds), then each later family by the
//! version it came in and its name. And every published listing of the families that a pull
//! request writes into the repository (the schema's keys, the family table in `themes.md`) is in
//! the families' names' order, so two pull requests that add two families add lines in two places.

use super::parts;

/// The families after the twelve versions' own, as `build.rs` places them: by `since`, then name.
#[test]
fn a_family_added_later_is_placed_by_its_version_then_its_name() {
    let names: Vec<&str> = parts::MANIFEST.iter().map(|one| one.name).collect();
    let mut unique = names.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), names.len(), "a family name twice: {names:?}");

    // What version 12 listed is a prefix: the digest holds its order, this that nothing later
    // comes before it.
    let shipped = parts::MANIFEST
        .iter()
        .take_while(|one| one.since <= 12)
        .count();
    assert!(
        parts::MANIFEST[shipped..].iter().all(|one| one.since > 12),
        "a family of the first twelve versions listed after a later one: {names:?}"
    );
    let later: Vec<(u32, &str)> = parts::MANIFEST[shipped..]
        .iter()
        .map(|one| (one.since, one.name))
        .collect();
    let mut sorted = later.clone();
    sorted.sort_unstable();
    assert_eq!(later, sorted);
}

/// The schema's part keys are in the families' names' order, whatever the manifest's.
#[test]
fn the_schema_lists_the_families_by_name() {
    let schema = super::schema();
    let keys = schema["properties"]["parts"]["properties"]
        .as_object()
        .expect("the parts keys");
    let mut families: Vec<&str> = Vec::new();
    for key in keys.keys() {
        let family = key.split('.').next().unwrap_or_default();
        if families.last() != Some(&family) {
            families.push(family);
        }
    }
    let mut sorted = families.clone();
    sorted.sort_unstable();
    assert_eq!(families, sorted, "each family's keys once, by name");
    assert_eq!(families.len(), parts::MANIFEST.len());
}
