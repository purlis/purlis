// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The string literals of the repository's shipped Rust, lexed just enough to read them:
//! shared by the guards that read what shipped code says
//! (`no_old_name_is_spelled_outside_the_name_module`, `no_count_says_thing_s`), so they agree
//! on what is shipped and what is a test.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Test code, which the guards do not read as shipped: a test may build a project under the
/// old names, or pin a sentence as it used to be. Clearly scoped — each is a shape of path or of source, and
/// nothing shipped matches one.
///
/// * a file under any `tests/` folder (the integration tests, and test modules kept in one);
/// * a file named `tests.rs`, `*_tests.rs` or `tests_*.rs` (a `#[cfg(test)]` module's file);
/// * a file a `#[cfg(test)]` (or `#[cfg(all(test, …))]`) `mod name;` brings in;
/// * an item — a module, a function — under such an attribute, inside any file;
/// * the files named in [`FIXTURE_FILES`].
pub const FIXTURES: &str = "tests/, *tests.rs, tests_*.rs, cfg(test) modules and items, FIXTURE_FILES";

/// Whole files, or folders (ending in `/`), that are test fixtures without matching a shape
/// in [`FIXTURES`], and why.
pub const FIXTURE_FILES: &[(&str, &str)] = &[(
    "crates/extension-probe/",
    "the extension the tests install to prove each capability; never shipped (its lib.rs)",
)];


/// A module a test-only attribute brings in from another file: its name and any `#[path]`.
pub type TestMod = (String, Option<String>);

/// A string literal: the line it starts on, and its text as written (escapes not undone).
#[derive(Debug)]
pub struct Literal {
    pub line: usize,
    pub text: String,
    /// Byte offset in the source.
    pub at: usize,
}

/// `src` lexed just enough: its string literals, and the source with every literal, comment
/// and char literal blanked out (same length, same line breaks), for finding items.
pub fn lex(src: &str) -> (Vec<Literal>, String) {
    let bytes = src.as_bytes();
    let mut code = bytes.to_vec();
    let mut out = Vec::new();
    let blank = |code: &mut Vec<u8>, from: usize, to: usize| {
        for b in &mut code[from..to] {
            if *b != b'\n' {
                *b = b' ';
            }
        }
    };
    let breaks: Vec<usize> = src.match_indices('\n').map(|(at, _)| at).collect();
    let line_at = |at: usize| breaks.partition_point(|&b| b < at) + 1;
    let ident =
        |at: usize| at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
    let mut i = 0;
    while i < bytes.len() {
        let rest = &src[i..];
        if rest.starts_with("//") {
            let end = rest.find('\n').map_or(bytes.len(), |n| i + n);
            blank(&mut code, i, end);
            i = end;
        } else if rest.starts_with("/*") {
            let mut depth = 0;
            let mut j = i;
            while j < bytes.len() {
                if src[j..].starts_with("/*") {
                    depth += 1;
                    j += 2;
                } else if src[j..].starts_with("*/") {
                    depth -= 1;
                    j += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    j += 1;
                }
            }
            blank(&mut code, i, j);
            i = j;
        } else if !ident(i) && raw_open(rest).is_some() {
            let (open, hashes) = raw_open(rest).expect("checked");
            let close = format!("\"{}", "#".repeat(hashes));
            let start = i + open;
            let end = src[start..].find(&close).map_or(bytes.len(), |n| start + n);
            out.push(Literal {
                line: line_at(i),
                text: src[start..end].to_owned(),
                at: i,
            });
            let end = (end + close.len()).min(bytes.len());
            blank(&mut code, i, end);
            i = end;
        } else if bytes[i] == b'"' || (!ident(i) && rest.starts_with("b\"")) {
            let start = i + if bytes[i] == b'"' { 1 } else { 2 };
            let mut j = start;
            while j < bytes.len() && bytes[j] != b'"' {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            out.push(Literal {
                line: line_at(i),
                text: src[start..j.min(bytes.len())].to_owned(),
                at: i,
            });
            let end = (j + 1).min(bytes.len());
            blank(&mut code, i, end);
            i = end;
        } else if bytes[i] == b'\'' {
            // A char literal ('x', '\n', '\'', '\u{..}'); otherwise a lifetime, left as code.
            let len = char_literal(rest);
            if len > 0 {
                blank(&mut code, i, i + len);
            }
            i += len.max(1);
        } else {
            i += rest.chars().next().map_or(1, char::len_utf8);
        }
    }
    (out, String::from_utf8(code).expect("blanking keeps UTF-8"))
}

/// A raw string's opening (`r"`, `r#"`, `br##"` …): its length and its number of `#`s.
pub fn raw_open(rest: &str) -> Option<(usize, usize)> {
    let after_b = rest.strip_prefix('b').unwrap_or(rest);
    let after_r = after_b.strip_prefix('r')?;
    let hashes = after_r.len() - after_r.trim_start_matches('#').len();
    after_r[hashes..]
        .starts_with('"')
        .then_some((rest.len() - after_r.len() + hashes + 1, hashes))
}

/// The length of the char literal `rest` opens, or 0 when the quote opens a lifetime.
pub fn char_literal(rest: &str) -> usize {
    let body = &rest[1..];
    if let Some(esc) = body.strip_prefix('\\') {
        let len = if esc.starts_with("u{") {
            esc.find('}').map_or(1, |n| n + 1)
        } else {
            esc.chars().next().map_or(0, char::len_utf8)
        };
        return if esc[len..].starts_with('\'') {
            len + 3
        } else {
            0
        };
    }
    match body.chars().next() {
        Some(c) if body[c.len_utf8()..].starts_with('\'') => c.len_utf8() + 2,
        _ => 0,
    }
}

/// Whether an attribute (its text between `#[` and `]`) compiles its item for tests only.
pub fn test_only(attr: &str) -> bool {
    let attr: String = attr.chars().filter(|c| !c.is_whitespace()).collect();
    attr.starts_with("cfg(")
        && !attr.contains("not(test")
        && !attr.contains("any(")
        && (attr == "cfg(test)"
            || attr.contains("(test,")
            || attr.contains(",test)")
            || attr.contains(",test,"))
}

/// The byte ranges of `code` (lexed) under a test-only attribute, and the modules such an
/// attribute brings in from other files (`mod x;`), each with any `#[path]` it gives.
pub fn test_items(code: &str, src: &str) -> (Vec<(usize, usize)>, Vec<TestMod>) {
    let b = code.as_bytes();
    let mut ranges = Vec::new();
    let mut files = Vec::new();
    let mut i = 0;
    while let Some(n) = code[i..].find("#[") {
        let at = i + n;
        let Some(close) = attr_end(b, at + 2) else {
            break;
        };
        i = close + 1;
        if !test_only(&code[at + 2..close]) {
            continue;
        }
        // Past any further attributes, to the item; its `{ … }` or its `;`.
        let mut j = close + 1;
        let mut path_attr = None;
        loop {
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if code[j..].starts_with("#[") {
                let Some(end) = attr_end(b, j + 2) else { break };
                // `#[path = "…"]`: its literal is blanked in `code`, so read it from `src`.
                let raw = src[j + 2..end].trim();
                if let Some(p) = raw.strip_prefix("path") {
                    path_attr = p.split('"').nth(1).map(str::to_owned);
                }
                j = end + 1;
            } else {
                break;
            }
        }
        let item_start = j;
        // A test-only field, match arm or struct-literal field is not an item: its attribute
        // hides nothing, and the scan carries on past it.
        if !opens_an_item(&code[item_start..]) {
            continue;
        }
        let Some(stop) = code[j..].find(['{', ';']).map(|n| j + n) else {
            break;
        };
        if b[stop] == b';' {
            let head: Vec<&str> = code[item_start..stop].split_whitespace().collect();
            if let Some(m) = head.iter().position(|w| *w == "mod")
                && let Some(name) = head.get(m + 1)
            {
                files.push(((*name).to_owned(), path_attr));
            }
            ranges.push((at, stop));
            i = stop + 1;
            continue;
        }
        let mut depth = 0usize;
        let mut k = stop;
        while k < b.len() {
            match b[k] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            k += 1;
        }
        ranges.push((at, k));
        i = k.max(close + 1);
    }
    (ranges, files)
}

/// The keywords an item starts with, after any of [`ITEM_PREFIXES`].
pub const ITEM_KEYWORDS: [&str; 11] = [
    "mod",
    "fn",
    "impl",
    "const",
    "static",
    "struct",
    "enum",
    "use",
    "type",
    "trait",
    "macro_rules!",
];

/// What may come before an item's keyword.
pub const ITEM_PREFIXES: [&str; 4] = ["pub", "unsafe", "async", "extern"];

/// Whether `code` (lexed, so an `extern "C"`'s ABI is blank) opens an item.
pub fn opens_an_item(code: &str) -> bool {
    let mut rest = code.trim_start();
    loop {
        let word: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '!')
            .collect();
        if ITEM_KEYWORDS.contains(&word.as_str()) {
            return true;
        }
        if !ITEM_PREFIXES.contains(&word.as_str()) {
            return false;
        }
        rest = rest[word.len()..].trim_start();
        // `pub(crate)`, `pub(in path)`.
        if word == "pub" && rest.starts_with('(') {
            let Some(close) = rest.find(')') else {
                return false;
            };
            rest = rest[close + 1..].trim_start();
        }
    }
}

/// The `]` that closes an attribute whose text starts at `from`, brackets nested.
pub fn attr_end(b: &[u8], from: usize) -> Option<usize> {
    let mut depth = 1;
    for (k, &c) in b.iter().enumerate().skip(from) {
        match c {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(k);
                }
            }
            _ => {}
        }
    }
    None
}


/// The literals in `src` outside test-only items, and the test-only module files `src`
/// brings in.
pub fn outside_tests(src: &str) -> (Vec<Literal>, Vec<TestMod>) {
    let (literals, code) = lex(src);
    let (tests, files) = test_items(&code, src);
    let shipped = literals
        .into_iter()
        .filter(|l| !tests.iter().any(|&(a, b)| a <= l.at && l.at <= b))
        .collect();
    (shipped, files)
}

/// Every shipped Rust file under `root`'s `crates/` and `app/src-tauri/`, by path relative to
/// `root`, with its literals outside test-only items. A fixture file ([`FIXTURES`]) is left out.
pub fn shipped(root: &Path) -> Vec<(String, Vec<Literal>)> {
    let mut files = Vec::new();
    rust_files(&root.join("crates"), &mut files);
    rust_files(&root.join("app/src-tauri"), &mut files);
    files.sort();
    let rel = |p: &Path| {
        p.strip_prefix(root)
            .expect("under the repository")
            .to_string_lossy()
            .replace('\\', "/")
    };
    let mut test_files: Vec<PathBuf> = Vec::new();
    let mut lexed = Vec::new();
    for path in &files {
        let src = std::fs::read_to_string(path).expect("a source file");
        let (literals, mods) = outside_tests(&src);
        for (name, p) in mods {
            test_files.extend(module_file(path, &name, p.as_deref()));
        }
        lexed.push((path.clone(), literals));
    }
    lexed
        .into_iter()
        .filter(|(path, _)| !test_files.contains(path))
        .map(|(path, literals)| (rel(&path), literals))
        .filter(|(rel, _)| !is_fixture_path(rel))
        .collect()
}

// ---- the tree ------------------------------------------------------------------------- //

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository")
}

pub fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "target" && name != "node_modules" && !name.starts_with('.') {
                rust_files(&path, out);
            }
        } else if name.ends_with(".rs") {
            out.push(path);
        }
    }
}

/// The file a `mod name;` (with an optional `#[path]`) declared in `declaring` is.
pub fn module_file(declaring: &Path, name: &str, path_attr: Option<&str>) -> Vec<PathBuf> {
    let dir = declaring.parent().expect("a file has a folder");
    if let Some(p) = path_attr {
        return vec![dir.join(p)];
    }
    let stem = declaring.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let base = if matches!(stem, "mod" | "lib" | "main") {
        dir.to_path_buf()
    } else {
        dir.join(stem)
    };
    vec![
        base.join(format!("{name}.rs")),
        base.join(name).join("mod.rs"),
    ]
}

pub fn is_fixture_path(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    rel.split('/').any(|part| part == "tests")
        || name == "tests.rs"
        || name.ends_with("_tests.rs")
        || name.starts_with("tests_")
        || FIXTURE_FILES
            .iter()
            .any(|(f, _)| *f == rel || (f.ends_with('/') && rel.starts_with(f)))
}

