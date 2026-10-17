// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **No count says "thing(s)"** (#2358).
//!
//! `docs/ui-copy.md`: counts are digits and plurals agree, *1 file*, *2 files*. A sentence
//! that says "3 file(s)" makes the reader do the agreeing. Shipped code says a count through
//! [`purlis_core::counts::counted`] (and a word the count governs through
//! [`purlis_core::counts::plural`]), so this test fails on a string literal in shipped Rust
//! that writes a word straight into `(s)`, `(es)` or `(ies)`.
//!
//! **What is scanned** is what `no_old_name_is_spelled_outside_the_name_module` scans, through
//! the same lexer (`shipped_literals`): every `.rs` file under `crates/` and `app/src-tauri/`,
//! string literals only, test code left out. A comment may still say "the file(s) it wrote";
//! only what shipped code says to a person or an agent is read.
//!
//! [`STILL_SAID`] lists the shipped files that still say one, each with its exact count and
//! why it waits. The counts are exact, so the list can only shrink.

use std::collections::BTreeMap;

mod shipped_literals;
use shipped_literals::{outside_tests, repo, shipped};

/// Shipped files that still write a word into `(s)`, with how many literals do and why they
/// wait. **Shrink only**: a file that says one more fails until its sentence uses `counted`,
/// and one that says one fewer fails until its count is lowered or its entry removed.
#[rustfmt::skip]
const STILL_SAID: &[(&str, usize, &str)] = &[
    // Files the session-host and browser work is changing in open pull requests; their
    // sentences move to `counted` once those land (#2396).
    ("app/src-tauri/src/planes.rs", 1, "#2396: the session-host work is changing this file"),
    ("crates/purlis-cli/src/main.rs", 1, "#2396: the session-host work is changing this file"),
    ("crates/purlis-core/src/chattools.rs", 2, "#2396: the browser work is changing this file"),
];

/// A literal's line and text.
type Hit = (usize, String);

/// Where `text` writes a word straight into `(s)`, `(es)` or `(ies)`: the suffix found.
fn says_thing_s(text: &str) -> Option<&'static str> {
    ["(s)", "(es)", "(ies)"].into_iter().find(|suffix| {
        text.match_indices(suffix).any(|(at, _)| {
            text[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphabetic())
        })
    })
}

/// Every shipped file's literals that say "thing(s)", by path relative to `root`.
fn scan(root: &std::path::Path) -> (BTreeMap<String, Vec<Hit>>, usize) {
    let mut found = BTreeMap::new();
    let mut scanned = 0;
    for (rel, literals) in shipped(root) {
        scanned += 1;
        let hits: Vec<Hit> = literals
            .into_iter()
            .filter(|l| says_thing_s(&l.text).is_some())
            .map(|l| (l.line, l.text))
            .collect();
        if !hits.is_empty() {
            found.insert(rel, hits);
        }
    }
    (found, scanned)
}

/// What is wrong with `found` against `allowed` ([`STILL_SAID`]'s shape).
fn beyond_allowance(
    found: &BTreeMap<String, Vec<Hit>>,
    allowed: &[(&str, usize, &str)],
) -> Vec<String> {
    let mut wrong = Vec::new();
    let allowance = |file: &str| -> usize {
        allowed
            .iter()
            .filter(|(f, ..)| *f == file)
            .map(|(_, n, _)| n)
            .sum()
    };
    for (file, hits) in found {
        let allowed = allowance(file);
        match allowed.cmp(&hits.len()) {
            std::cmp::Ordering::Equal => {}
            std::cmp::Ordering::Greater => wrong.push(format!(
                "{file}: allowed {allowed} but says {} — lower its count in STILL_SAID",
                hits.len()
            )),
            std::cmp::Ordering::Less => {
                wrong.push(format!(
                    "{file}: says {} \"thing(s)\" count{}, allowed {allowed} — say it through \
                     purlis_core::counts::counted:",
                    hits.len(),
                    if hits.len() == 1 { "" } else { "s" },
                ));
                for (line, text) in hits {
                    let shown: String = text.chars().take(100).collect();
                    wrong.push(format!("    {file}:{line}: \"{shown}\""));
                }
            }
        }
    }
    for (file, _, why) in allowed {
        if !found.contains_key(*file) {
            wrong.push(format!(
                "{file}: allowed ({why}) but says none — remove its entry"
            ));
        }
    }
    wrong
}

#[test]
fn no_shipped_string_says_thing_s_beyond_its_allowance() {
    purlis_core::unsteered!();
    let (found, scanned) = scan(&repo());
    assert!(scanned > 300, "the walk scanned {scanned} shipped files");
    let wrong = beyond_allowance(&found, STILL_SAID);
    assert!(
        wrong.is_empty(),
        "counts that do not agree with their noun:\n{}",
        wrong.join("\n")
    );
}

// ---- the guard's own tests ------------------------------------------------------------ //

#[test]
fn the_guard_fails_on_a_thing_s_count_injected_into_the_tree() {
    purlis_core::unsteered!();
    let (mut found, _) = scan(&repo());
    assert!(beyond_allowance(&found, STILL_SAID).is_empty());
    let tree = tempfile::tempdir().expect("a tree");
    let injected = tree.path().join("crates/injected/src");
    std::fs::create_dir_all(&injected).expect("a crate");
    std::fs::write(
        injected.join("lib.rs"),
        "pub fn said(n: usize) -> String {\n    format!(\"{n} file(s) written\")\n}\n",
    )
    .expect("lib.rs");
    let (stray, _) = scan(tree.path());
    found.extend(stray);
    assert_eq!(
        beyond_allowance(&found, STILL_SAID),
        [
            "crates/injected/src/lib.rs: says 1 \"thing(s)\" count, allowed 0 — say it through \
             purlis_core::counts::counted:",
            "    crates/injected/src/lib.rs:2: \"{n} file(s) written\"",
        ],
    );
}

#[test]
fn a_word_written_into_the_suffix_counts_and_code_and_prose_do_not() {
    purlis_core::unsteered!();
    for hit in [
        "{n} file(s)",
        "{} harness(es) read",
        "archives {n} redundant copy(ies)",
        "skill(s) {}",
        "Wrote {} page(s))",
    ] {
        assert!(says_thing_s(hit).is_some(), "{hit} should count");
    }
    for miss in ["{n} files", "1 file", "the (s) flag", "f (s)", ""] {
        assert_eq!(says_thing_s(miss), None, "{miss} should not count");
    }
}

#[test]
fn comments_and_test_code_are_not_read() {
    purlis_core::unsteered!();
    let src = r#"
// the file(s) it wrote
/// each repo(s)
fn f(s: &str) -> Option<&str> { Some(s) }
const SAID: &str = "{} item(s)";
#[cfg(test)]
mod tests {
    const PINNED: &str = "1 file(s)";
}
"#;
    let (literals, _) = outside_tests(src);
    let said: Vec<&str> = literals
        .iter()
        .filter(|l| says_thing_s(&l.text).is_some())
        .map(|l| l.text.as_str())
        .collect();
    assert_eq!(said, ["{} item(s)"]);
}

#[test]
fn the_allowance_names_real_files() {
    purlis_core::unsteered!();
    let root = repo();
    for (file, n, why) in STILL_SAID {
        assert!(*n > 0, "{file}: an entry of 0 ({why}) is a removed entry");
        assert!(root.join(file).is_file(), "{file} is not a file");
    }
}
