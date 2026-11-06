// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A workspace's repo is at `workspaces/<ws>/repos/<repo>` or, made before structure version 6,
//! at `workspaces/<ws>/<repo>`, and one module knows which: `purlis_core::repofolder` (ADR 0114,
//! decision 2). A path to a repo built by hand knows only the old place, and is how the layout
//! drifts. So outside test code, in the core, the CLI and the app, Rust and TypeScript, a
//! hand-built `workspaces/<ws>/<repo>` is either on [`ALLOWED`] with the reason it may stay, or
//! this fails (#5372).
//!
//! **What reads as one.** Two shapes, each with a name in the second place that is not a fixed
//! one (a string literal, or a constant in capitals such as `worktree::DIR_NAME`):
//!
//! - a join: `.join("workspaces").join(ws).join(repo)`, however rustfmt breaks it;
//! - a string: `workspaces/{ws}/{repo}` in a `format!`, or `workspaces/${ws}/${repo}` in a
//!   TypeScript template.
//!
//! A workspace's own store (`workspaces/{ws}/{store}`) has that shape too, and is allowed by
//! name. The list may only shrink: an entry whose file no longer has the sites it allows fails
//! too, so a site that is routed comes off the list in the same change.
//!
//! **What it does not read.** A path built in steps (the workspace's folder into a variable,
//! the repo joined later), a join whose argument holds a call (`.join(repo.as_str())`),
//! `PathBuf::push`, and TypeScript concatenation or `path.join` pass unseen. The guard holds the
//! one-expression shapes a new site is most likely to copy; review holds the rest.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::support::shipped::{shipped, sources};

/// The sites outside test code that build a `workspaces/<ws>/<name>` by hand, by file, with how
/// many and why.
const ALLOWED: &[(&str, usize, &str)] = &[
    (
        "crates/purlis-core/src/repofolder.rs",
        1,
        "the resolver itself: `folder`'s answer for a workspace it cannot anchor",
    ),
    (
        "crates/purlis-core/src/files/kept.rs",
        1,
        "`clone_of` reads a relative path back to its clone, case-folded, at the root and \
         under `repos/` both: the resolver's `locate` for a path as written, not a path to one",
    ),
    (
        "crates/purlis-core/src/scaffold/mod.rs",
        1,
        "`init`'s offer says where `--clone-this-repo` clones today, the root; #5373 moves new \
         clones to `repos/` and this line with them (a recorded answer)",
    ),
    (
        "crates/purlis-core/src/repocmd/clone.rs",
        1,
        "the refusal names where a clone lands today, the root; #5373 moves new clones to \
         `repos/` and this line with them",
    ),
    (
        "app/src-tauri/src/runnercode.rs",
        3,
        "`workspaces/<ws>/<repo>` is the name a transfer carries between machines, which each \
         end finds on its own disk with `repofolder::folder` (`folder_of`), never a path",
    ),
    (
        "crates/purlis-core/src/held.rs",
        1,
        "names a workspace's own store, never a repo",
    ),
    (
        "crates/purlis-core/src/memstore/holding.rs",
        1,
        "names a workspace's own store, never a repo",
    ),
    (
        "crates/purlis-core/src/brokered.rs",
        1,
        "names a workspace's settings file, never a repo",
    ),
    (
        "crates/purlis-core/src/theme/folders.rs",
        1,
        "names a workspace's themes folder, never a repo",
    ),
    (
        "crates/purlis-core/src/wscmd/mod.rs",
        1,
        "names a workspace's own folders (memory, sessions, skills), never a repo",
    ),
    (
        "crates/purlis-core/src/wscmd/rename.rs",
        1,
        "names a workspace's own files a rename rewrote, never a repo",
    ),
];

/// The source trees a shipped build compiles, from the workspace root: the core and the CLI
/// (`crates`), the app's Rust and its TypeScript.
const TREES: [&str; 3] = ["crates", "app/src-tauri", "app/src"];

const EXTENSIONS: [&str; 3] = ["rs", "ts", "tsx"];

/// What to use instead, said in every failure.
const USE: &str = "build it with `purlis_core::repofolder::folder` (an existing repo), \
                   `new_folder` (where a new one goes) or `locate` (from a path back to its repo); \
                   in the app's TypeScript, take the path the core answers";

/// `.join("workspaces").join(<ws>).join(<name>)`, across lines.
static JOINED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\.join\(\s*"workspaces"\s*\)\s*\.join\(([^()]*)\)\s*\.join\(([^()]*)\)"#).unwrap()
});

/// `workspaces/{ws}/{name}` in a format string, or `workspaces/${ws}/${name}` in a template.
static SPELLED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"workspaces/\$?\{([^}/]*)\}/\$?\{([^}/]*)\}").unwrap());

/// Whether `arg`, the name in a repo's place, is a fixed one: a string literal, or a constant
/// in capitals (`DIR_NAME`, `&crate::worktree::DIR_NAME`).
fn fixed(arg: &str) -> bool {
    let arg = arg.trim().trim_start_matches('&');
    if arg.starts_with('"') {
        return true;
    }
    let last = arg.rsplit("::").next().unwrap_or(arg);
    !last.is_empty()
        && last
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// One hand-built repo path, as this test reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Site {
    file: String,
    line: usize,
}

/// Every site in `files` that builds a repo path by hand outside test code. `name` gives each
/// file the name it is reported by.
fn sites(files: &[PathBuf], name: impl Fn(&Path) -> String) -> Vec<Site> {
    let mut out = Vec::new();
    for source in shipped(files) {
        // Only the lines a shipped build compiles, less comments, each kept as a line so a
        // match's offset still says its line.
        let code: String = source
            .text
            .lines()
            .zip(&source.shipped)
            .map(|(line, shipped)| {
                let t = line.trim_start();
                if *shipped && !t.starts_with("//") && !t.starts_with("/*") && !t.starts_with("* ")
                {
                    line
                } else {
                    ""
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut found: Vec<usize> = JOINED
            .captures_iter(&code)
            .chain(SPELLED.captures_iter(&code))
            .filter(|hit| !fixed(&hit[2]))
            .map(|hit| code[..hit.get(0).unwrap().start()].matches('\n').count() + 1)
            .collect();
        found.sort_unstable();
        out.extend(found.into_iter().map(|line| Site {
            file: name(&source.path),
            line,
        }));
    }
    out
}

#[test]
fn a_repo_path_outside_tests_comes_from_the_resolver_or_is_allowed_with_a_reason() {
    purlis_core::unsteered!();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let mut files = Vec::new();
    for tree in TREES {
        let dir = workspace.join(tree);
        assert!(dir.is_dir(), "{} is not there", dir.display());
        sources(&dir, &EXTENSIONS, &mut files);
    }
    assert!(files.len() > 1000, "the walk found {} files", files.len());
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
                "{file}:{lines:?} builds a workspace's repo path by hand, which knows only the \
                 old layout: {USE}; or add it to ALLOWED with the reason it may stay"
            )),
            Some((_, count, _)) if lines.len() > *count => wrong.push(format!(
                "{file}:{lines:?} builds a repo path by hand {} times; ALLOWED gives it {count}: \
                 {USE}",
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
                "{file} is allowed {count} hand-built repo paths and has {now}: lower its \
                 count, or take it off ALLOWED"
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The reader itself, on text whose answer is known: a test that read nothing would pass the
/// walk above.
#[test]
fn a_hand_built_repo_path_in_shipped_code_is_found_and_a_fixed_name_or_a_test_is_not() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    let rust = "\
fn clone(plane: &Path, ws: &str, repo: &str) -> PathBuf {
    plane.join(\"workspaces\").join(ws).join(repo)
}

fn split(plane: &Path, ws: &str, repo: &str) -> PathBuf {
    plane
        .join(\"workspaces\")
        .join(&ws)
        .join(&repo.name)
}

fn fixed(plane: &Path, ws: &str) {
    plane.join(\"workspaces\").join(ws).join(\"workspace.json\");
    plane.join(\"workspaces\").join(ws).join(DIR_NAME);
    plane.join(\"workspaces\").join(ws).join(&crate::worktree::DIR_NAME);
    plane.join(\"workspaces\").join(ws);
    // plane.join(\"workspaces\").join(ws).join(repo) in a comment builds nothing.
}

fn said(ws: &str, repo: &str) -> String {
    format!(\"workspaces/{ws}/{repo} is not a clone\")
}

fn stores(ws: &str) -> String {
    format!(\"workspaces/{ws}/{FILE} and workspaces/{ws}/ and workspaces/{ws}/repos/{FILE}\")
}

#[cfg(test)]
mod tests {
    fn fixture(plane: &Path) {
        plane.join(\"workspaces\").join(\"alpha\").join(name);
    }
}
";
    std::fs::write(src.join("lib.rs"), rust).unwrap();
    let ts = "\
export const at = (plane: string, ws: string, repo: string) => `${plane}/workspaces/${ws}/${repo}`;
export const own = (plane: string, ws: string) => `${plane}/workspaces/${ws}/memory`;
";
    std::fs::write(src.join("Paths.ts"), ts).unwrap();
    std::fs::write(src.join("Paths.test.ts"), ts).unwrap();
    std::fs::write(src.join("Paths.stories.tsx"), ts).unwrap();

    let mut files = Vec::new();
    sources(dir.path(), &EXTENSIONS, &mut files);
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
            at("src/Paths.ts", 1),
            at("src/lib.rs", 2),
            at("src/lib.rs", 7),
            at("src/lib.rs", 21),
        ]
    );
}
