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

use crate::support::shipped::{shipped, sources};

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

/// Every site in `files` that starts git outside test code. `name` gives each file the name it
/// is reported by.
fn sites(files: &[PathBuf], name: impl Fn(&Path) -> String) -> Vec<Site> {
    let mut out = Vec::new();
    for source in shipped(files) {
        for (i, line) in source.text.lines().enumerate() {
            let code = line.trim_start();
            if source.shipped[i] && !code.starts_with("//") && starts_git(line) {
                out.push(Site {
                    file: name(&source.path),
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
        sources(&dir, &["rs"], &mut files);
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
    sources(dir.path(), &["rs"], &mut files);
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
