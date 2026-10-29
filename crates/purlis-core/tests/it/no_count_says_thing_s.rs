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
//!
//! **Nor does shipped code spell the suffix inline** (#2416): `if n == 1 { "" } else { "s" }`
//! is the count's agreement written out again beside its noun, so the same guard fails on it
//! and the sentence says its count through `counted` instead. What is read is the literal
//! pair, not the condition: a `""` and an `"s"` (or `"es"`), in either order, with nothing but
//! `} else {` between them. [`STILL_INLINE`] is its shrink-only list.

use std::collections::BTreeMap;

use purlis_core::counts::counted;

use crate::shipped_literals;
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

/// Shipped files that still spell a plural suffix inline (`{ "" } else { "s" }`), with how
/// many and why they wait. **Shrink only**, as [`STILL_SAID`].
#[rustfmt::skip]
const STILL_INLINE: &[(&str, usize, &str)] = &[
    // The browser lane is changing browser-engine, which does not depend on purlis-core.
    ("crates/browser-engine/src/drive/settle.rs", 2, "#2431: the browser work is changing this file"),
];

/// A literal's line and text.
type Hit = (usize, String);

/// Which guard an allowance is for: its list's name, the thing it counts (one and many), and
/// the way to fix a new one.
struct Rule {
    list: &'static str,
    one: &'static str,
    many: &'static str,
    fix: &'static str,
}

const THING_S: Rule = Rule {
    list: "STILL_SAID",
    one: "\"thing(s)\" count",
    many: "\"thing(s)\" counts",
    fix: "say it through purlis_core::counts::counted",
};

const INLINE: Rule = Rule {
    list: "STILL_INLINE",
    one: "inline plural suffix",
    many: "inline plural suffixes",
    fix: "say it through purlis_core::counts::counted, or plural for a word alone",
};

/// Where `src` spells a plural suffix inline: each `""`/`"s"` (or `"es"`) pair of `literals`
/// (consecutive, either order) with only `} else {` of code between them, as the line and the
/// code read from the first literal to the second.
fn inline_suffixes(src: &str, literals: &[shipped_literals::Literal]) -> Vec<Hit> {
    let plain = |l: &shipped_literals::Literal| src[l.at..].starts_with('"');
    literals
        .windows(2)
        .filter(|pair| {
            let (a, b) = (&pair[0], &pair[1]);
            let texts = (a.text.as_str(), b.text.as_str());
            let suffix_pair = matches!(texts, ("", "s" | "es") | ("s" | "es", ""));
            let a_end = a.at + a.text.len() + 2;
            suffix_pair
                && plain(a)
                && plain(b)
                && a_end <= b.at
                && src[a_end..b.at].split_whitespace().collect::<String>() == "}else{"
        })
        .map(|pair| {
            let end = pair[1].at + pair[1].text.len() + 2;
            (pair[0].line, src[pair[0].at..end].to_owned())
        })
        .collect()
}

/// Every shipped file's inline plural suffixes, by path relative to `root`.
fn scan_inline(root: &std::path::Path) -> BTreeMap<String, Vec<Hit>> {
    let mut found = BTreeMap::new();
    for (rel, literals) in shipped(root) {
        if !literals.iter().any(|l| l.text.is_empty()) {
            continue;
        }
        let src = std::fs::read_to_string(root.join(&rel)).expect("a shipped file");
        let hits = inline_suffixes(&src, &literals);
        if !hits.is_empty() {
            found.insert(rel, hits);
        }
    }
    found
}

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
    rule: &Rule,
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
                "{file}: allowed {allowed} but says {} — lower its count in {}",
                hits.len(),
                rule.list,
            )),
            std::cmp::Ordering::Less => {
                wrong.push(format!(
                    "{file}: says {}, allowed {allowed} — {}:",
                    counted(hits.len(), rule.one, rule.many),
                    rule.fix,
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
    let wrong = beyond_allowance(&found, STILL_SAID, &THING_S);
    assert!(
        wrong.is_empty(),
        "counts that do not agree with their noun:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn no_shipped_code_spells_a_plural_suffix_inline_beyond_its_allowance() {
    purlis_core::unsteered!();
    let wrong = beyond_allowance(&scan_inline(&repo()), STILL_INLINE, &INLINE);
    assert!(
        wrong.is_empty(),
        "plural suffixes spelled inline:\n{}",
        wrong.join("\n")
    );
}

// ---- the guard's own tests ------------------------------------------------------------ //

#[test]
fn the_guard_fails_on_a_thing_s_count_injected_into_the_tree() {
    purlis_core::unsteered!();
    let (mut found, _) = scan(&repo());
    assert!(beyond_allowance(&found, STILL_SAID, &THING_S).is_empty());
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
        beyond_allowance(&found, STILL_SAID, &THING_S),
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
fn the_inline_guard_fails_on_a_suffix_injected_into_the_tree() {
    purlis_core::unsteered!();
    let mut found = scan_inline(&repo());
    assert!(beyond_allowance(&found, STILL_INLINE, &INLINE).is_empty());
    let tree = tempfile::tempdir().expect("a tree");
    let injected = tree.path().join("crates/injected/src");
    std::fs::create_dir_all(&injected).expect("a crate");
    std::fs::write(
        injected.join("lib.rs"),
        "pub fn said(n: usize) -> String {\n    \
         format!(\"{n} file{}\", if n == 1 { \"\" } else { \"s\" })\n}\n",
    )
    .expect("lib.rs");
    found.extend(scan_inline(tree.path()));
    assert_eq!(
        beyond_allowance(&found, STILL_INLINE, &INLINE),
        [
            "crates/injected/src/lib.rs: says 1 inline plural suffix, allowed 0 — say it \
             through purlis_core::counts::counted, or plural for a word alone:",
            "    crates/injected/src/lib.rs:2: \"\"\" } else { \"s\"\"",
        ],
    );
}

#[test]
fn an_inline_suffix_is_read_in_either_order_and_other_pairs_are_not() {
    purlis_core::unsteered!();
    let said = |src: &str| -> Vec<String> {
        let (literals, _) = outside_tests(src);
        inline_suffixes(src, &literals)
            .into_iter()
            .map(|(_, code)| code)
            .collect()
    };
    for hit in [
        r#"f(if n == 1 { "" } else { "s" })"#,
        r#"f(if n != 1 { "s" } else { "" })"#,
        "f(if n == 1 {\n    \"\"\n} else {\n    \"es\"\n})",
    ] {
        assert_eq!(said(hit).len(), 1, "{hit} should count");
    }
    for miss in [
        r#"f(if n == 1 { "it" } else { "them" })"#,
        r#"f(if n == 1 { "" } else { "x" })"#,
        r#"f("", "s")"#,
        r#"f(if a { "" } else if b { "s" } else { "x" })"#,
        r#"f(counted(n, "file", "files"))"#,
    ] {
        assert!(said(miss).is_empty(), "{miss} should not count");
    }
}

#[test]
fn the_allowance_names_real_files() {
    purlis_core::unsteered!();
    let root = repo();
    for (file, n, why) in STILL_SAID.iter().chain(STILL_INLINE) {
        assert!(*n > 0, "{file}: an entry of 0 ({why}) is a removed entry");
        assert!(root.join(file).is_file(), "{file} is not a file");
    }
}
