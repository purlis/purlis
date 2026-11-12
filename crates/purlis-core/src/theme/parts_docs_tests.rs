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

/// What the manifest says `name` came in: a family's `since`, or a part's or a variant's of it
/// (`family.part`), or that part's for one of its properties (`family.part.property`). None for a
/// name the manifest does not hold.
fn since_of(manifest: &Value, name: &str) -> Option<u64> {
    let mut path = name.split('.');
    let first = path.next()?;
    let family = manifest["families"]
        .as_array()?
        .iter()
        .find(|one| one["name"] == first)?;
    let Some(inner) = path.next() else {
        return family["since"].as_u64();
    };
    let within = |key: &str| {
        family[key]
            .as_array()
            .and_then(|items| items.iter().find(|one| one["name"] == inner))
    };
    let item = within("parts").or_else(|| within("variants"))?;
    match path.next() {
        None => item["since"].as_u64(),
        Some(property) => item["properties"]
            .as_array()?
            .iter()
            .any(|one| one == property)
            .then(|| item["since"].as_u64())?,
    }
}

/// Every version the page's prose gives a family, a part or a property (`` `tree`, manifest
/// version 13 `` or `` `tooltip.shortcut`, since manifest version 13 ``), against the manifest:
/// what a release shipped says the release's version, and only what none has shipped says
/// "the next release's manifest version" (#6308). The family table is held by
/// [`the_docs_family_table_is_the_manifest_s`].
fn versions_said_wrongly(page: &str, manifest: &Value) -> Vec<String> {
    const NEXT_SAID: &str = "the next release's manifest version";
    let prose = written(page).map_or_else(
        || page.to_owned(),
        |(start, end)| format!("{}{}", &page[..start], &page[end..]),
    );
    let prose = prose.split_whitespace().collect::<Vec<_>>().join(" ");
    let claim = regex::Regex::new(
        r"`([a-z][a-z0-9.-]*)`, (?:since )?(?:manifest version (\d+)|the next release's manifest version)",
    )
    .expect("the claim's pattern");
    let released = u64::from(parts::VERSION);
    let mut wrong = Vec::new();
    for found in claim.captures_iter(&prose) {
        let name = &found[1];
        let said = found.get(2).and_then(|n| n.as_str().parse::<u64>().ok());
        match since_of(manifest, name) {
            None => wrong.push(format!("`{name}` is in no family of the manifest")),
            Some(since) if since > released && said.is_some() => {
                wrong.push(format!("`{name}` is in no release yet: say {NEXT_SAID}"));
            }
            Some(since) if since <= released && said != Some(since) => {
                wrong.push(format!("`{name}` first shipped in manifest version {since}"));
            }
            Some(_) => {}
        }
    }
    let unclaimed = claim.replace_all(&prose, "").matches(NEXT_SAID).count();
    if unclaimed > 0 {
        wrong.push(format!(
            "\"{NEXT_SAID}\" is said {unclaimed} time(s) without naming what it is about, as \
             `name`, {NEXT_SAID}: nothing then tells when a release has shipped it"
        ));
    }
    wrong
}

#[test]
fn every_version_the_docs_prose_gives_a_family_is_the_manifest_s_6308() {
    let page = std::fs::read_to_string(PAGE).expect("docs/themes.md is read");
    let wrong = versions_said_wrongly(&page, &parts::manifest_json());
    assert!(
        wrong.is_empty(),
        "docs/themes.md gives a version the manifest does not:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn a_version_in_the_prose_is_checked_whichever_way_it_is_said() {
    let released = u64::from(parts::VERSION);
    let next = released + 1;
    let manifest = serde_json::json!({ "families": [
        { "name": "old", "since": 2, "parts": [
            { "name": "root", "since": 2, "properties": ["fill"] },
            { "name": "late", "since": next, "properties": ["text"] },
        ], "variants": [{ "name": "quiet", "since": 3 }] },
    ] });
    assert_eq!(since_of(&manifest, "old"), Some(2));
    assert_eq!(since_of(&manifest, "old.quiet"), Some(3));
    assert_eq!(since_of(&manifest, "old.root.fill"), Some(2));
    assert_eq!(since_of(&manifest, "old.late"), Some(next));
    assert_eq!(since_of(&manifest, "old.root.text"), None);
    assert_eq!(since_of(&manifest, "new"), None);

    let ok = "**Old** (`old`, manifest version 2), its `quiet` (`old.quiet`, since manifest\n\
              version 3) and (`old.late`, the next release's manifest version).";
    assert_eq!(versions_said_wrongly(ok, &manifest), Vec::<String>::new());
    let stale = format!(
        "(`old`, the next release's manifest version) (`old.quiet`, manifest version 2) \
         (`old.late`, manifest version {next}) (`new`, manifest version 1) \
         (the next release's manifest version)"
    );
    assert_eq!(versions_said_wrongly(&stale, &manifest).len(), 5);
}
