// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a shipped build compiles of the source trees**, for the guards that read the source:
//! every file under a tree with one of the given extensions, less the files that are test code by
//! name, the module files declared under `#[cfg(test)]`, and the items `#[cfg(test)]` marks.

use std::path::{Path, PathBuf};

/// One source file a shipped build compiles, with which of its lines are compiled.
pub struct Source {
    pub path: PathBuf,
    pub text: String,
    /// One flag per line of `text`: whether a shipped build compiles it.
    pub shipped: Vec<bool>,
}

/// Every file under `dir` whose extension is one of `extensions`, sorted, past `target`,
/// `node_modules` and dot-folders.
pub fn sources(dir: &Path, extensions: &[&str], into: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if path.is_dir() {
            if name != "target" && name != "node_modules" && !name.starts_with('.') {
                sources(&path, extensions, into);
            }
        } else if path
            .extension()
            .is_some_and(|x| extensions.iter().any(|one| x == *one))
        {
            into.push(path);
        }
    }
}

/// The files of `files` a shipped build compiles, each with its shipped lines. A Rust file is
/// read for `#[cfg(test)]`; any other file is shipped whole unless its name says it is a test.
pub fn shipped(files: &[PathBuf]) -> Vec<Source> {
    let texts: Vec<(PathBuf, String)> = files
        .iter()
        .map(|f| (f.clone(), std::fs::read_to_string(f).unwrap()))
        .collect();
    let test_only: Vec<PathBuf> = texts
        .iter()
        .filter(|(f, _)| is_rust(f))
        .flat_map(|(f, text)| test_modules_declared(f, text))
        .collect();
    texts
        .into_iter()
        .filter(|(f, _)| !test_file_by_name(f) && !test_only.contains(f))
        .map(|(path, text)| {
            let shipped = if is_rust(&path) {
                shipped_lines(&text)
            } else {
                vec![true; text.lines().count()]
            };
            Source {
                path,
                text,
                shipped,
            }
        })
        .collect()
}

fn is_rust(file: &Path) -> bool {
    file.extension().is_some_and(|x| x == "rs")
}

/// Whether a whole file is test code by where it is or what it is called: an integration test,
/// a bench or an example, or a module file named for its tests; and in TypeScript, a test, a
/// story or a spec by its name, or a file in a folder of tests.
pub fn test_file_by_name(file: &Path) -> bool {
    let parts: Vec<String> = file
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    let stem = file.file_stem().unwrap_or_default().to_string_lossy();
    let name = file.file_name().unwrap_or_default().to_string_lossy();
    parts.iter().any(|part| {
        matches!(
            part.as_str(),
            "tests" | "benches" | "examples" | "test" | "__tests__" | "__mocks__"
        )
    }) || stem == "tests"
        || stem.ends_with("_tests")
        || stem.ends_with("_test")
        || [".test.", ".stories.", ".spec."]
            .iter()
            .any(|mark| name.contains(mark))
}

/// The item `#[cfg(test)]` at `lines[at]` applies to: the index of its first line that is not
/// an attribute or a comment.
fn item_after(lines: &[&str], at: usize) -> Option<usize> {
    (at + 1..lines.len()).find(|&i| {
        let t = lines[i].trim_start();
        !(t.is_empty() || t.starts_with("#[") || t.starts_with("//"))
    })
}

/// Whether `line` is an attribute that compiles what follows only for tests: `#[cfg(test)]`,
/// or `#[cfg(all(…))]` with `test` among the conditions it needs. Not `any(test, …)`, which a
/// shipped build can meet.
fn test_only_cfg(line: &str) -> bool {
    let line = line.trim();
    if line == "#[cfg(test)]" {
        return true;
    }
    let Some(inner) = line
        .strip_prefix("#[cfg(all(")
        .and_then(|rest| rest.strip_suffix("))]"))
    else {
        return false;
    };
    let mut depth = 0usize;
    let mut item = String::new();
    let mut items = Vec::new();
    for c in inner.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                items.push(std::mem::take(&mut item));
                continue;
            }
            _ => {}
        }
        item.push(c);
    }
    items.push(item);
    items.iter().any(|item| item.trim() == "test")
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The module files `file` declares under `#[cfg(test)]` (`mod tests;`, `#[path = "…"] mod
/// x;`), as paths.
fn test_modules_declared(file: &Path, text: &str) -> Vec<PathBuf> {
    let lines: Vec<&str> = text.lines().collect();
    let dir = file.parent().unwrap();
    let stem = file.file_stem().unwrap_or_default().to_string_lossy();
    let owns_its_dir = matches!(stem.as_ref(), "lib" | "main" | "mod");
    let mut out = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        if !test_only_cfg(line) {
            continue;
        }
        let Some(item) = item_after(&lines, at) else {
            continue;
        };
        let decl = lines[item].trim();
        let Some(name) = decl
            .trim_start_matches("pub(crate) ")
            .trim_start_matches("pub(super) ")
            .trim_start_matches("pub ")
            .strip_prefix("mod ")
            .and_then(|rest| rest.strip_suffix(';'))
        else {
            continue;
        };
        let path_attr = lines[at + 1..item].iter().find_map(|l| {
            l.trim()
                .strip_prefix("#[path = \"")
                .and_then(|rest| rest.strip_suffix("\"]"))
        });
        match path_attr {
            Some(path) => out.push(dir.join(path)),
            None => {
                let base = if owns_its_dir {
                    dir.to_path_buf()
                } else {
                    dir.join(stem.as_ref())
                };
                out.push(base.join(format!("{name}.rs")));
                out.push(base.join(name).join("mod.rs"));
            }
        }
    }
    out
}

/// The lines of `text` a shipped build compiles: everything but the items `#[cfg(test)]` marks.
/// An item's extent is read from rustfmt's layout: it ends at the first line that closes it at
/// its own indent.
fn shipped_lines(text: &str) -> Vec<bool> {
    let lines: Vec<&str> = text.lines().collect();
    let mut shipped = vec![true; lines.len()];
    let mut at = 0;
    while at < lines.len() {
        if test_only_cfg(lines[at])
            && let Some(item) = item_after(&lines, at)
        {
            // A signature may run over several lines: the item opens a body on the first
            // line that ends in `{`, unless one ends in `;` first.
            let head = (item..lines.len())
                .find(|&i| {
                    let t = lines[i].trim_end();
                    t.ends_with('{') || t.ends_with(';')
                })
                .unwrap_or(item);
            let end = if lines[head].trim_end().ends_with('{') {
                let close = " ".repeat(indent(lines[item])) + "}";
                (head + 1..lines.len())
                    .find(|&i| lines[i].trim_end() == close)
                    .unwrap_or(lines.len() - 1)
            } else {
                head
            };
            for flag in &mut shipped[at..=end] {
                *flag = false;
            }
            at = end + 1;
            continue;
        }
        at += 1;
    }
    shipped
}
