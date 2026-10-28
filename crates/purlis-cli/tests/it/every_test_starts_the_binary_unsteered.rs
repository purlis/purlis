// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Every test in this folder starts the binary through `support` (#1284), so none inherits a
//! variable that chooses a project.
//!
//! A suite run inside a chat inherits that chat's project, workspace, persona and homes under
//! both names (V93k). A test that sets `CHARTER_ROOT` and inherits `PURLIS_ROOT` hands the binary
//! two answers, and the purlis one wins. `support::purlis()` and `support::command()` start every
//! process with those variables removed, and the test sets its own after.
//!
//! A source scan, so a new test cannot forget it:
//! - the binary's path (`CARGO_BIN_EXE_…`) is named in `tests/support/` alone;
//! - a process is started with a bare `Command::new` only when its program is a string literal
//!   (`"git"`, `"/bin/sh"`): any other program, the binary, a copy of it or a path a test built,
//!   goes through `support::command`;
//! - a file that names the binary's path (`support::PURLIS`, `support::CHARTER`), to write it
//!   into a hook or hand it to a shell, starts every process through `support`, since any of
//!   them may start the binary;
//! - `Command` is never imported under another name, which would start a process the scan
//!   cannot see.

use crate::support;

use std::path::{Path, PathBuf};

/// The text that names a binary cargo built, spelled in two halves so this file does not.
const BIN_EXE: &str = concat!("CARGO_BIN", "_EXE_");

/// A bare process start (or the function itself, handed on as a value).
const NEW: &str = concat!("Command", "::new");

/// `Command` under another name, which the scan for [`NEW`] would not read.
const RENAMED: &str = concat!("Command", " as ");

/// The names of the binary's path in `support`.
const PATHS: [&str; 2] = [
    concat!("support::", "PURLIS"),
    concat!("support::", "CHARTER"),
];

/// What is wrong with `source` (a test file at `name`), one line per finding.
fn findings(name: &str, source: &str) -> Vec<String> {
    let code: Vec<(usize, &str)> = source
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
        .map(|(at, line)| (at + 1, line))
        .collect();
    let mut found = Vec::new();
    for (at, line) in &code {
        if line.contains(BIN_EXE) {
            found.push(format!(
                "{name}:{at}: names the binary's path itself; use `support::purlis()`"
            ));
        }
        if line.contains(RENAMED) {
            found.push(format!(
                "{name}:{at}: names `Command` otherwise, so its starts are not read; use \
                 `support::command`"
            ));
        }
    }
    let names_the_path = code
        .iter()
        .any(|(_, line)| PATHS.iter().any(|path| line.contains(path)));
    // Joined, so a program on the line after `Command::new(` is read too.
    let mut joined = String::new();
    let mut line_of = Vec::new();
    for (at, line) in &code {
        joined.push_str(line);
        joined.push('\n');
        line_of.resize(joined.len(), *at);
    }
    for (offset, _) in joined.match_indices(NEW) {
        let at = line_of[offset];
        let after = joined[offset + NEW.len()..].trim_start();
        let program = after.strip_prefix('(').map(str::trim_start);
        if names_the_path {
            found.push(format!(
                "{name}:{at}: starts a process with a bare `{NEW}` in a file that hands the \
                 binary's path on; use `support::command`"
            ));
        } else if !program.is_some_and(|program| program.starts_with('"')) {
            found.push(format!(
                "{name}:{at}: starts a program that is not a string literal with a bare \
                 `{NEW}`; use `support::command` (or `support::purlis()`)"
            ));
        }
    }
    found
}

/// Every `.rs` file below `dir`, `support/` left out.
fn test_files(dir: &Path, into: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("the tests folder") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "support") {
                test_files(&path, into);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            into.push(path);
        }
    }
}

#[test]
fn every_test_in_the_folder_starts_the_binary_through_support() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut files = Vec::new();
    test_files(&tests, &mut files);
    files.sort();

    let mut found = Vec::new();
    let mut through_support = 0;
    for file in &files {
        let source = std::fs::read_to_string(file).expect("a test file");
        let name = file
            .strip_prefix(&tests)
            .unwrap_or(file)
            .display()
            .to_string();
        found.extend(findings(&name, &source));
        if source.contains("support::") {
            through_support += 1;
        }
    }

    assert!(found.is_empty(), "{}", found.join("\n"));
    // Not a scan of nothing: the folder holds the suite, and the suite starts the binary.
    assert!(
        files.len() > 60,
        "only {} test files were read",
        files.len()
    );
    assert!(
        through_support > 60,
        "only {through_support} test files start anything through `support`"
    );
}

#[test]
fn a_test_naming_the_binarys_path_or_starting_another_program_barely_is_found() {
    let bin = format!("let out = std::process::{NEW}(env!(\"{BIN_EXE}purlis\"));\n");
    assert_eq!(
        findings("a.rs", &bin).len(),
        2,
        "{:?}",
        findings("a.rs", &bin)
    );

    let built = format!("let out = {NEW}(\n    binary.join(\"copy\"),\n);\n");
    let said = findings("b.rs", &built);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].starts_with("b.rs:1: "), "{said:?}");

    let renamed = format!("use std::process::{{{RENAMED}Run}};\nlet out = Run::new(x);\n");
    let said = findings("g.rs", &renamed);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].starts_with("g.rs:1: "), "{said:?}");

    let handed = format!("let made = paths.iter().map({NEW});\n");
    assert_eq!(
        findings("f.rs", &handed).len(),
        1,
        "{:?}",
        findings("f.rs", &handed)
    );

    let hook = format!(
        "hooks.write(Path::new({path})).unwrap();\nlet git = {NEW}(\"git\");\n",
        path = PATHS[1]
    );
    let said = findings("c.rs", &hook);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].starts_with("c.rs:2: "), "{said:?}");
}

#[test]
fn a_test_starting_git_or_the_binary_through_support_is_not_found() {
    let fine = format!(
        "// {NEW}(env!(\"{BIN_EXE}purlis\")) in a comment\nlet git = {NEW}(\n    \"git\");\n\
         let out = support::purlis().arg(\"root\").output();\n"
    );
    assert_eq!(findings("d.rs", &fine), Vec::<String>::new());

    let hook = format!(
        "hooks.write(Path::new({path})).unwrap();\nlet git = support::command(\"git\");\n\
         let alias = support::command({path});\n",
        path = PATHS[1]
    );
    assert_eq!(findings("e.rs", &hook), Vec::<String>::new());
}

#[test]
fn what_support_starts_is_given_none_of_the_variables_that_choose_a_project() {
    for command in [support::purlis(), support::command("git")] {
        let removed: Vec<String> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect();
        for rest in purlis_core::envvar::SELECTING {
            for name in [format!("PURLIS_{rest}"), format!("CHARTER_{rest}")] {
                assert!(removed.contains(&name), "{name} is passed on: {removed:?}");
            }
        }
    }
    assert_eq!(
        support::purlis().get_program(),
        std::ffi::OsStr::new(support::PURLIS)
    );
}
