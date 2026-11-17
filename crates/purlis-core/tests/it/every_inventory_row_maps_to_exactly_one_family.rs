// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every row of the #6970 inventory maps to exactly one family** (ruling Q308b: "every
//! inventory row maps to a family"; the follow-up's item 7).
//!
//! `docs/policy-inventory-families.md` holds the inventory's 1,199 rows, each with one family of
//! [`purlis_core::enforcement::Family`], or `fixed` for what Q308b keeps outside the policy (the
//! kill switch, the grader bundles, a chat changing its own policy, and the person's own rules).
//! This test reads every table row after the document's `<!-- rows -->` marker and fails where a
//! row names no family, more than one, or one purlis does not know, or where a section's count
//! moved from the inventory's.

use std::collections::BTreeMap;
use std::path::Path;

use purlis_core::enforcement::Family;

/// What a row outside the policy names instead of a family.
const FIXED: &str = "fixed";

/// The inventory's own count of rows per section (#6970, "Count per family").
const SECTIONS: &[(&str, usize)] = &[
    ("Sandbox files and paths", 81),
    ("Network and egress", 61),
    ("Ports", 19),
    ("Processes", 48),
    ("Guards", 316),
    ("Asks and consents", 67),
    ("Dispatch", 121),
    ("Vaults and secrets", 84),
    ("The browser", 79),
    ("Skills", 84),
    ("The harness permission mode", 30),
    ("Admin locks", 32),
    ("Other", 177),
];

/// One row: its section, subsection, id, and the family cell as written.
struct Row {
    section: String,
    subsection: String,
    id: String,
    family: String,
}

/// The cells of a table line, split on `|` that no backslash escapes.
fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut escaped = false;
    for c in inner.chars() {
        if c == '|' && !escaped {
            out.push(cell.trim().to_owned());
            cell.clear();
        } else {
            cell.push(c);
        }
        escaped = c == '\\';
    }
    out.push(cell.trim().to_owned());
    out
}

fn rows(text: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    let (mut section, mut subsection) = (String::new(), String::new());
    let after = text
        .split_once("<!-- rows -->")
        .expect("the document marks where its rows start")
        .1;
    for line in after.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            section = heading.trim().to_owned();
            subsection.clear();
        } else if let Some(heading) = line.strip_prefix("### ") {
            subsection = heading.trim().to_owned();
        } else if line.starts_with('|') {
            let cells = cells(line);
            let id = cells.first().cloned().unwrap_or_default();
            if id == "#" || id.chars().all(|c| matches!(c, '-' | ':')) {
                continue;
            }
            rows.push(Row {
                section: section.clone(),
                subsection: subsection.clone(),
                id,
                family: cells.last().cloned().unwrap_or_default(),
            });
        }
    }
    rows
}

fn document() -> String {
    let doc = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/policy-inventory-families.md");
    std::fs::read_to_string(doc).expect("docs/policy-inventory-families.md is readable")
}

/// The family a row's last cell names: one word in backticks, a family's key or `fixed`.
fn named(cell: &str) -> Result<&str, String> {
    let word = cell
        .strip_prefix('`')
        .and_then(|rest| rest.strip_suffix('`'))
        .ok_or_else(|| format!("{cell:?} is not one word in backticks"))?;
    if word.contains(['`', ',', ' ', '/']) {
        return Err(format!("{cell:?} names more than one family"));
    }
    if word != FIXED && Family::of_key(word).is_none() {
        return Err(format!("{cell:?} is no family purlis knows"));
    }
    Ok(word)
}

#[test]
fn every_inventory_row_maps_to_exactly_one_family() {
    let rows = rows(&document());
    let mut wrong = Vec::new();
    let mut per_section: BTreeMap<&str, usize> = BTreeMap::new();
    let mut per_family: BTreeMap<&str, usize> = BTreeMap::new();
    let mut seen = std::collections::BTreeSet::new();
    for row in &rows {
        *per_section.entry(row.section.as_str()).or_default() += 1;
        match named(&row.family) {
            Ok(family) => *per_family.entry(family).or_default() += 1,
            Err(why) => wrong.push(format!("{} › {} #{}: {why}", row.section, row.subsection, row.id)),
        }
        if !seen.insert((row.section.as_str(), row.subsection.as_str(), row.id.as_str())) {
            wrong.push(format!(
                "{} › {} #{} is listed twice",
                row.section, row.subsection, row.id
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert_eq!(rows.len(), 1199, "the inventory has 1,199 rows");
    for (section, count) in SECTIONS {
        assert_eq!(
            per_section.get(section).copied().unwrap_or(0),
            *count,
            "{section}"
        );
    }
    assert_eq!(per_section.len(), SECTIONS.len(), "{per_section:?}");
    // Every family Q308b added owns at least one row.
    for family in Family::ALL.into_iter().filter(|f| f.added_by_q308b()) {
        assert!(per_family.contains_key(family.key()), "{}", family.key());
    }
}

#[test]
fn a_row_with_no_family_two_families_or_an_unknown_one_is_refused() {
    assert_eq!(named("`limits`"), Ok("limits"));
    assert_eq!(named("`fixed`"), Ok("fixed"));
    assert!(named("limits").is_err());
    assert!(named("").is_err());
    assert!(named("`limits`, `trust`").is_err());
    assert!(named("`limits/trust`").is_err());
    assert!(named("`walls`").is_err());
    let text = "<!-- rows -->\n## S\n| # | Decision | Where | Family |\n|---|---|---|---|\n| 1 | a \\| b | x | `trust` |\n";
    let rows = rows(text);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].family, "`trust`");
}
