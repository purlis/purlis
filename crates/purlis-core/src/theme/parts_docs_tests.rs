// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`docs/themes.md`'s family table is the manifest's** (operator ruling 2026-10-26): written from
//! [`parts::manifest_json`], one row a family in the families' names' order, so a pull request
//! that adds a family adds its own row and none beside another's, and no row is written by hand.
//! A family's prose goes in its own paragraph under the table.
//!
//! When it differs, write it again from the repository's root:
//! `PURLIS_THEMES_DOC_WRITE=1 cargo test -p purlis-core --lib the_docs_family_table`. The release
//! step does it too (`node tools/theme-parts-release.mjs`), as each `since: NEXT` gets its number.

use serde_json::Value;

use super::parts;

/// The table's head, which the table is found by.
const HEAD: &str = "| Family | Parts | Variants | States |";

/// Where the page is, from the crate's root.
const PAGE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/docs/themes.md");

/// How the table says the version something came in, after the family's own: nothing when it came
/// with what holds it.
fn since(one: &Value, with: u64) -> String {
    let since = one["since"].as_u64().unwrap_or_default();
    if since <= with {
        String::new()
    } else if since > u64::from(parts::VERSION) {
        " (since the next release)".to_owned()
    } else {
        format!(" (since version {since})")
    }
}

/// `items` as the table lists them: each name in code, and what came later said so.
fn listed(items: &[Value], with: u64, each: impl Fn(&Value) -> String) -> String {
    if items.is_empty() {
        return "none".to_owned();
    }
    items
        .iter()
        .map(|one| {
            format!(
                "`{}`{}{}",
                one["name"].as_str().unwrap_or_default(),
                each(one),
                since(one, with)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// **The family table**, as the manifest has it: its head, its rule and one row a family.
fn table() -> String {
    let manifest = parts::manifest_json();
    let mut families: Vec<&Value> = manifest["families"]
        .as_array()
        .expect("the families")
        .iter()
        .collect();
    families.sort_by_key(|one| one["name"].as_str().unwrap_or_default().to_owned());
    let mut text = format!("{HEAD}\n|---|---|---|---|\n");
    for family in families {
        let own = family["since"].as_u64().unwrap_or_default();
        let empty = Vec::new();
        let array = |key: &str| family[key].as_array().unwrap_or(&empty);
        let parts = listed(array("parts"), own, |part| {
            let properties: Vec<String> = part["properties"]
                .as_array()
                .unwrap_or(&empty)
                .iter()
                .map(|one| format!("`{}`", one.as_str().unwrap_or_default()))
                .collect();
            // A part drawn apart from the root, in its own states only (#3656).
            let apart = part["apart"].as_array().map_or_else(String::new, |states| {
                let states: Vec<String> = states
                    .iter()
                    .map(|one| format!("`{}`", one.as_str().unwrap_or_default()))
                    .collect();
                if states.is_empty() {
                    ", drawn apart, in no variant and no state".to_owned()
                } else {
                    format!(", drawn apart, in no variant: {}", states.join(", "))
                }
            });
            format!(" ({}{apart})", properties.join(", "))
        });
        let variants = listed(array("variants"), own, |_| String::new());
        let states: Vec<String> = array("states")
            .iter()
            .map(|one| format!("`{}`", one.as_str().unwrap_or_default()))
            .collect();
        let states = if states.is_empty() {
            "none".to_owned()
        } else {
            states.join(", ")
        };
        let first = if own <= 1 {
            String::new()
        } else {
            since(family, 1)
        };
        text.push_str(&format!(
            "| `{}`{first} | {parts} | {variants} | {states} |\n",
            family["name"].as_str().unwrap_or_default()
        ));
    }
    text
}

/// The page's table as it is: from its head to the first line that is no row.
fn written(page: &str) -> Option<(usize, usize)> {
    let start = page.find(HEAD)?;
    let rows = page[start..]
        .split_inclusive('\n')
        .take_while(|line| line.starts_with('|'))
        .map(str::len)
        .sum::<usize>();
    Some((start, start + rows))
}

#[test]
fn the_docs_family_table_is_the_manifest_s() {
    let page = std::fs::read_to_string(PAGE).expect("docs/themes.md is read");
    let (start, end) = written(&page).expect("docs/themes.md has the family table");
    let now = table();
    if std::env::var_os("PURLIS_THEMES_DOC_WRITE").is_some_and(|v| v == "1") {
        let again = format!("{}{now}{}", &page[..start], &page[end..]);
        std::fs::write(PAGE, again).expect("docs/themes.md is written");
        return;
    }
    assert!(
        page[start..end] == now,
        "docs/themes.md's family table is not the manifest's: write it again with \
         `PURLIS_THEMES_DOC_WRITE=1 cargo test -p purlis-core --lib the_docs_family_table` from \
         the repository's root, and put what a family needs said beyond its parts in its own \
         paragraph under the table"
    );
}

#[test]
fn the_table_says_what_came_later_and_what_no_release_has_shipped() {
    // VERSION + 1, not the marker: the release step writes a number over every NEXT in code.
    let next = u64::from(parts::VERSION) + 1;
    let one = |since: u64| serde_json::json!({ "name": "x", "since": since });
    assert_eq!(since(&one(3), 3), "");
    assert_eq!(since(&one(4), 3), " (since version 4)");
    assert_eq!(since(&one(next), 3), " (since the next release)");
    let page = "a\n| Family | Parts | Variants | States |\n|---|\n| x |\n\nb";
    let (start, end) = written(page).expect("the table");
    assert_eq!(
        &page[start..end],
        "| Family | Parts | Variants | States |\n|---|\n| x |\n"
    );
}
