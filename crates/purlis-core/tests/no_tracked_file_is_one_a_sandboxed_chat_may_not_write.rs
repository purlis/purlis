// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #2089: no file this repository tracks is one a sandboxed chat may not write.
//!
//! A chat's sandbox never lets it write what a program run later loads code from (an editor's
//! tasks, an agent's settings, a hook manager's folder; ADR 0067 §5), and purlis's guard
//! refuses a checkout that would write one. A file of that name tracked here would stop every
//! sandboxed chat from bringing a clone of this repository across a change to it. A test
//! fixture that needs such a name is tracked under a neutral one (`<name>.fixture`) and copied
//! to its real name at test time ([`purlis_core::testfixture`]).

use std::path::Path;

use purlis_core::cloneupdate::{Asked, Asks, Denies, Head, Top};

/// [`Asked`], for a checkout that has nothing in it yet: what a fresh clone writes is judged,
/// whatever this checkout already holds.
struct Fresh;

impl Asks for Fresh {
    fn top(&self, dir: &str) -> Top {
        Asked.top(dir)
    }
    fn head(&self, top: &str) -> Head {
        Asked.head(top)
    }
    fn has_index(&self, top: &str) -> bool {
        Asked.has_index(top)
    }
    fn commit(&self, top: &str, name: &str) -> Option<String> {
        Asked.commit(top, name)
    }
    fn guessed(&self, top: &str, word: &str) -> Option<String> {
        Asked.guessed(top, word)
    }
    fn differ(&self, top: &str, from: &str, to: &str) -> Option<Vec<String>> {
        Asked.differ(top, from, to)
    }
    fn replayed(&self, top: &str, onto: &str) -> Option<Vec<String>> {
        Asked.replayed(top, onto)
    }
    fn replayed_from(&self, top: &str, onto: &str, head: &str) -> Option<Vec<String>> {
        Asked.replayed_from(top, onto, head)
    }
    fn tracked(&self, top: &str, commit: &str) -> Option<Vec<String>> {
        Asked.tracked(top, commit)
    }
    fn changed(&self, top: &str, paths: &[String]) -> Option<Vec<String>> {
        Asked.changed(top, paths)
    }
    fn is_there(&self, _: &Path) -> bool {
        false
    }
    fn left_out(&self, top: &str, paths: &[String]) -> Option<Vec<String>> {
        Asked.left_out(top, paths)
    }
}

/// Of `paths`, from the top `top`, those a sandboxed chat may not write into a fresh checkout.
fn denied_among<'a>(top: &str, paths: impl Iterator<Item = &'a str>) -> Vec<String> {
    // No project root: only the later-code names count, never a manifest of a plane this is not.
    let denies = Denies::of(top, "/nonexistent-project", None);
    paths
        .filter(|path| denies.path(path, &Fresh))
        .map(str::to_owned)
        .collect()
}

#[test]
fn a_file_under_a_folder_a_chat_may_not_make_is_denied_whatever_this_checkout_holds() {
    purlis_core::unsteered!();
    // A checkout that already holds the folder, as a developer's own may: a fresh clone does
    // not, and has to make it.
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("tests/fixtures/x/.claude")).unwrap();
    let denied = denied_among(
        &dir.path().to_string_lossy(),
        [
            "tests/fixtures/x/.claude/notes.md",
            "tests/fixtures/x/.CLAUDE/notes.md",
            "tests/fixtures/x/.claude.fixture/settings.json",
            "tests/fixtures/x/opencode.json.fixture",
        ]
        .into_iter(),
    );
    assert_eq!(
        denied,
        [
            "tests/fixtures/x/.claude/notes.md",
            "tests/fixtures/x/.CLAUDE/notes.md"
        ]
    );
}

#[test]
fn no_tracked_path_is_a_name_the_sandbox_denies_a_chat() {
    purlis_core::unsteered!();
    let top = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let top = std::fs::canonicalize(&top).expect("the repository's top");
    let mut listed = std::process::Command::new("git");
    listed
        .args(["ls-files", "-z"])
        .current_dir(&top)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    let out = purlis_core::forklock::output(&mut listed).expect("git runs");
    assert!(out.status.success(), "git ls-files failed");
    let listing = String::from_utf8_lossy(&out.stdout);
    let denied = denied_among(
        &top.to_string_lossy(),
        listing.split('\0').filter(|path| !path.is_empty()),
    );
    assert!(
        denied.is_empty(),
        "tracked under a name a sandboxed chat may not write: {denied:?}. Track a fixture as \
         `<name>.fixture` and copy it with purlis_core::testfixture"
    );
}
