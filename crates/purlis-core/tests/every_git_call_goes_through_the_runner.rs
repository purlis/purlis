// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Every git process purlis starts goes through the one runner, `worktree::git` (#810).
//!
//! A repository's own config can name programs, and git runs them: a hook, an fsmonitor, a
//! pager, an alias. A repository a chat can write is a repository whose config a chat can
//! write, so a git purlis runs there, outside any sandbox, must be told on its own command line
//! that none of them runs. The runner says so on every call it makes
//! (`a_planted_repo_config_runs_nothing.rs` proves it), and a call that builds its own
//! `Command::new("git")` says none of it. So outside test code, a git started anywhere but the
//! runner is either on [`ALLOWED`] with the reason it may stay, or this fails.
//!
//! The list may only shrink: an entry whose file no longer has the calls it allows fails too,
//! so a site that is routed comes off the list in the same change.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The sites outside test code that start git without the runner, by file, with how many and
/// why. `crates/purlis-core/docs/git-policy.md` lists the same sites, under "Every git purlis
/// runs goes through one runner".
const ALLOWED: &[(&str, usize, &str)] = &[];

/// The source trees a shipped build compiles, from the workspace root.
const TREES: [&str; 2] = ["crates", "app/src-tauri"];

/// One git process started outside the runner, as this test reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Site {
    file: String,
    line: usize,
}

fn sources(dir: &Path, into: &mut Vec<PathBuf>) {
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
                sources(&path, into);
            }
        } else if path.extension().is_some_and(|x| x == "rs") {
            into.push(path);
        }
    }
}

/// Whether `line` starts a git process by naming the program: `Command::new("git")`, or a
/// path ending in `/git`.
fn starts_git(line: &str) -> bool {
    let mut rest = line;
    while let Some(at) = rest.find("Command::new(\"") {
        let after = &rest[at + "Command::new(\"".len()..];
        if let Some(end) = after.find('"') {
            let program = &after[..end];
            if program == "git" || program.ends_with("/git") {
                return true;
            }
        }
        rest = after;
    }
    false
}

/// Whether a whole file is test code by where it is or what it is called: an integration test,
/// a bench or an example, or a module file named for its tests.
fn test_file_by_name(file: &Path) -> bool {
    let parts: Vec<String> = file
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    let stem = file.file_stem().unwrap_or_default().to_string_lossy();
    parts
        .iter()
        .any(|part| part == "tests" || part == "benches" || part == "examples")
        || stem == "tests"
        || stem.ends_with("_tests")
        || stem.ends_with("_test")
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

/// Every site in `files` that starts git outside test code. `name` gives each file the name it
/// is reported by.
fn sites(files: &[PathBuf], name: impl Fn(&Path) -> String) -> Vec<Site> {
    let texts: Vec<(PathBuf, String)> = files
        .iter()
        .map(|f| (f.clone(), std::fs::read_to_string(f).unwrap()))
        .collect();
    let test_only: Vec<PathBuf> = texts
        .iter()
        .flat_map(|(f, text)| test_modules_declared(f, text))
        .collect();
    let mut out = Vec::new();
    for (file, text) in &texts {
        if test_file_by_name(file) || test_only.contains(file) {
            continue;
        }
        let shipped = shipped_lines(text);
        for (i, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if shipped[i] && !code.starts_with("//") && starts_git(line) {
                out.push(Site {
                    file: name(file),
                    line: i + 1,
                });
            }
        }
    }
    out
}

#[test]
fn every_git_call_outside_tests_goes_through_the_runner_or_is_allowed_with_a_reason() {
    purlis_core::unsteered!();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let mut files = Vec::new();
    for tree in TREES {
        let dir = workspace.join(tree);
        assert!(dir.is_dir(), "{} is not there", dir.display());
        sources(&dir, &mut files);
    }
    assert!(files.len() > 300, "the walk found {} files", files.len());
    let found = sites(&files, |f| {
        f.strip_prefix(&workspace)
            .unwrap_or(f)
            .to_string_lossy()
            .into_owned()
    });

    let mut by_file: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for site in &found {
        by_file.entry(&site.file).or_default().push(site.line);
    }
    let mut wrong = Vec::new();
    for (file, lines) in &by_file {
        match ALLOWED.iter().find(|(allowed, _, _)| allowed == file) {
            None => wrong.push(format!(
                "{file}:{lines:?} starts git without the runner: use `purlis_core::worktree::git`, \
                 or add it to ALLOWED with the reason it may stay"
            )),
            Some((_, count, _)) if lines.len() > *count => wrong.push(format!(
                "{file}:{lines:?} starts git {} times without the runner; ALLOWED gives it {count}",
                lines.len()
            )),
            Some(_) => {}
        }
    }
    for (file, count, reason) in ALLOWED {
        assert!(
            !reason.trim().is_empty(),
            "{file} is allowed with no reason"
        );
        let now = by_file.get(file).map_or(0, Vec::len);
        if now < *count {
            wrong.push(format!(
                "{file} is allowed {count} git calls without the runner and has {now}: \
                 lower its count, or take it off ALLOWED"
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The reader itself, on text whose answer is known: a test that read nothing would pass the
/// walk above.
#[test]
fn a_git_started_in_shipped_code_is_found_and_one_in_a_test_module_is_not() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(src.join("store")).unwrap();
    let shipped = "\
fn lists() {
    let _ = std::process::Command::new(\"git\").arg(\"status\");
}

#[cfg(test)]
fn fixture() {
    std::process::Command::new(\"git\");
}

pub fn later() {
    // Command::new(\"git\") in a comment starts nothing.
    let _ = std::process::Command::new(\"/usr/bin/git\");
    let _ = std::process::Command::new(\"gitk\");
}

#[cfg(test)]
mod tests {
    fn helper() {
        std::process::Command::new(\"git\");
    }

    #[test]
    fn t() {
        purlis_core::unsteered!();
        std::process::Command::new(\"git\");
    }
}

#[cfg(test)]
#[path = \"store_checks.rs\"]
mod checks;

#[cfg(all(test, unix))]
mod on_unix {
    fn f() {
        std::process::Command::new(\"git\");
    }
}

#[cfg(any(test, feature = \"x\"))]
pub fn shipped_with_a_feature() {
    std::process::Command::new(\"git\");
}

#[cfg(test)]
mod store;

#[cfg(test)]
fn a_long_signature(
    first: &str,
) -> std::process::Command {
    std::process::Command::new(\"git\")
}
";
    std::fs::write(src.join("lib.rs"), shipped).unwrap();
    let fixture_only = "fn f() { std::process::Command::new(\"git\"); }\n";
    for test_file in ["store_checks.rs", "store.rs", "thing_tests.rs", "tests.rs"] {
        std::fs::write(src.join(test_file), fixture_only).unwrap();
    }
    std::fs::write(src.join("store/mod.rs"), fixture_only).unwrap();
    std::fs::write(src.join("plain.rs"), fixture_only).unwrap();

    let mut files = Vec::new();
    sources(dir.path(), &mut files);
    let found = sites(&files, |f| {
        f.strip_prefix(dir.path())
            .unwrap()
            .to_string_lossy()
            .into_owned()
    });

    let at = |file: &str, line| Site {
        file: file.into(),
        line,
    };
    assert_eq!(
        found,
        [
            at("src/lib.rs", 2),
            at("src/lib.rs", 12),
            at("src/lib.rs", 42),
            at("src/plain.rs", 1)
        ]
    );
}
