// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Every memory write a shipped text teaches is spelled the way that runs without a prompt
//! (#3880).
//!
//! #3829 pre-allowed a chat's own memory writes by name only (ADR 0064, amended 2026-10-29):
//! `purlis persona remember <its persona> …` and `purlis workspace remember|note -w <its
//! workspace> …`, beside the `memory_add` and `persona_remember` tools. The unnamed spellings
//! (`purlis persona remember "<fact>"`, `purlis workspace remember "<fact>"`) stop on the
//! harness's prompt, so a chat that does what a skill, a briefing or a doc page says stops on
//! the person for its own work.
//!
//! **What is read:** the string literals of shipped source (outside comments and test modules,
//! as [`every_command_charter_names_is_one_it_has`](super::every_command_charter_names_is_one_it_has)
//! reads them), every shipped `SKILL.md`, and the pages `purlis docs show` serves
//! (`crates/purlis-core/docs`).
//!
//! **What is refused:** `purlis|charter|{program} persona remember` not followed by a persona
//! (a name, `<placeholder>` or `{placeholder}`), `… workspace remember|note` not followed by
//! `-w`, and `… ws remember|note` at all (the rules name `workspace`).

use super::every_command_charter_names_is_one_it_has::{
    literals, repo, shipped_skills, shipped_sources,
};

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Every unnamed memory-write spelling in `text`, as written.
fn unnamed(text: &str) -> Vec<String> {
    static WRITE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r"(?:purlis|charter|\{program\})\s+(persona\s+remember|workspace\s+remember|workspace\s+note|ws\s+remember|ws\s+note)\b\s*([^\s`]*)",
        )
        .expect("the pattern compiles")
    });
    WRITE
        .captures_iter(text)
        .filter(|c| {
            let next = &c[2];
            // A text wraps where it likes, so the words are read across a line break.
            let words: Vec<&str> = c[1].split_whitespace().collect();
            match words.join(" ").as_str() {
                "persona remember" => !next
                    .chars()
                    .next()
                    .is_some_and(|first| first.is_ascii_alphanumeric() || "<{".contains(first)),
                "workspace remember" | "workspace note" => next != "-w",
                _ => true,
            }
        })
        .map(|c| c[0].split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

/// The pages `purlis docs show` serves.
fn shipped_docs(root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(root.join("crates/purlis-core/docs"))
        .expect("the docs directory")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    out.sort();
    out
}

#[test]
fn every_memory_write_a_shipped_text_teaches_names_the_chats_own_scope() {
    let root = repo();
    let mut found = Vec::new();
    let mut read = 0;
    let mut note = |file: &Path, text: &str| {
        let rel = file
            .strip_prefix(&root)
            .unwrap_or(file)
            .display()
            .to_string();
        for spelling in unnamed(text) {
            found.push(format!("{rel}: `{spelling}`"));
        }
    };
    // A `*_tests.rs` beside its module is a test file, as `tests.rs` is.
    for file in shipped_sources(&root)
        .into_iter()
        .filter(|file| !file.to_string_lossy().ends_with("_tests.rs"))
    {
        let src = std::fs::read_to_string(&file).expect("a source file reads");
        for text in literals(&src) {
            note(&file, &text);
        }
        read += 1;
    }
    let pages: Vec<PathBuf> = shipped_skills(&root)
        .into_iter()
        .chain(shipped_docs(&root))
        .collect();
    assert!(pages.len() > 10, "the walk found the pages: {pages:?}");
    for file in &pages {
        note(file, &std::fs::read_to_string(file).expect("a page reads"));
        read += 1;
    }
    assert!(read > 60, "only {read} files were read");
    assert!(
        found.is_empty(),
        "these texts teach a memory write that stops on the harness's prompt — teach the \
         `memory_add` / `persona_remember` tool first, then `purlis persona remember <persona> …` \
         or `purlis workspace remember|note -w <workspace> …`, with the chat's own name written \
         in where the text knows it:\n{}",
        found.join("\n")
    );
}

#[test]
fn a_named_spelling_passes_and_an_unnamed_one_is_found() {
    for named in [
        "`purlis persona remember devops \"x\"`",
        "`purlis persona remember <persona> --shared \"x\"`",
        "`{program} persona remember {name} \"x\"`",
        "`purlis workspace remember -w alpha \"x\"`",
        "`purlis workspace note -w {ws} \"x\"`",
        "`purlis persona recall devops`",
        "`purlis persona remember\n  devops \"x\"`",
        "`purlis workspace\nremember -w alpha \"x\"`",
    ] {
        assert_eq!(unnamed(named), Vec::<String>::new(), "{named}");
    }
    for bare in [
        "`purlis persona remember \"x\"`",
        "`purlis persona remember --shared \"x\"`",
        "`purlis persona remember`",
        "`charter workspace remember \"x\"`",
        "`{program} workspace note \"…\"`",
        "`purlis workspace remember --no-sync`",
        "`purlis ws note -w alpha \"x\"`",
        "`purlis persona remember` /\n`purlis workspace remember`",
        "`purlis\nworkspace remember \"x\"`",
    ] {
        assert!(!unnamed(bare).is_empty(), "{bare}");
    }
}
