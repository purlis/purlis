// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A shipped skill changes a workspace's charter through purlis's tools** (#5720).
//!
//! A sandboxed chat cannot write a workspace's `workspace.md`: the sandbox refuses it and points
//! to the `workspace_section` tool, which the app answers for the chat. Smart close's step 2
//! told the chat to open the file and edit it, so a chat that followed the skill was refused in
//! the middle of closing; the memory skill's "improve the charter" step had the same gap. So:
//!
//! * no sentence of a shipped skill tells a chat to open or edit `workspace.md`;
//! * each skill that folds lessons into a workspace's charter names the two tools that write
//!   it, `workspace_section` (decisions, glossary) and `workspace_vision` (the goal).

use std::path::{Path, PathBuf};

/// The skills that fold what lasts into a workspace's charter, by their folder.
const FOLDING: [&str; 2] = ["purlis-smart-close", "purlis-tidy-memory"];

/// The tools that write a workspace's charter for a chat.
const TOOLS: [&str; 2] = ["`workspace_section`", "`workspace_vision`"];

fn skills_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/src-tauri/plugin/skills")
}

/// Every `SKILL.md` the app's plugin ships, by its folder's name, with its text.
fn shipped() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = std::fs::read_dir(skills_dir())
        .expect("the skills folder")
        .flatten()
        .filter_map(|entry| {
            let text = std::fs::read_to_string(entry.path().join("SKILL.md")).ok()?;
            Some((entry.file_name().to_string_lossy().into_owned(), text))
        })
        .collect();
    out.sort();
    out
}

/// The sentences of `text`, each on one line: what ends at a `.`, `;` or `:` followed by
/// whitespace or the end, or at a blank line. A `.` inside a word, as in `workspace.md`, ends
/// nothing.
fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for paragraph in text.split("\n\n") {
        let mut start = 0;
        let mut chars = paragraph.char_indices().peekable();
        while let Some((at, c)) = chars.next() {
            let ends =
                ".;:".contains(c) && chars.peek().is_none_or(|(_, next)| next.is_whitespace());
            if ends {
                out.push(paragraph[start..at].to_owned());
                start = at + c.len_utf8();
            }
        }
        out.push(paragraph[start..].to_owned());
    }
    out.into_iter()
        .map(|sentence| sentence.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|sentence| !sentence.is_empty())
        .collect()
}

/// The sentences of `text` that tell a chat to open or edit `workspace.md`: a sentence that
/// names the file and one of those verbs.
fn hand_edits(text: &str) -> Vec<String> {
    const VERBS: [&str; 5] = ["open", "opens", "edit", "edits", "editing"];
    sentences(text)
        .into_iter()
        .filter(|sentence| sentence.contains("workspace.md"))
        .filter(|sentence| {
            sentence
                .split(|c: char| !c.is_ascii_alphabetic())
                .any(|word| VERBS.contains(&word.to_ascii_lowercase().as_str()))
        })
        .collect()
}

#[test]
fn no_shipped_skill_tells_a_chat_to_edit_workspace_md_by_hand() {
    purlis_core::unsteered!();
    let skills = shipped();
    assert!(skills.len() >= 10, "the walk read {} skills", skills.len());
    let found: Vec<String> = skills
        .iter()
        .flat_map(|(name, text)| {
            hand_edits(text)
                .into_iter()
                .map(move |sentence| format!("{name}: {sentence}"))
        })
        .collect();
    assert!(
        found.is_empty(),
        "these skills tell a chat to open or edit workspace.md, which a sandboxed chat is refused; \
         name the `workspace_section` or `workspace_vision` tool instead:\n{}",
        found.join("\n")
    );
}

#[test]
fn smart_close_and_the_memory_skill_name_the_tools_that_write_the_charter() {
    purlis_core::unsteered!();
    let skills = shipped();
    for folder in FOLDING {
        let (_, text) = skills
            .iter()
            .find(|(name, _)| name == folder)
            .unwrap_or_else(|| panic!("{folder} is shipped"));
        for tool in TOOLS {
            assert!(
                text.contains(tool),
                "{folder} folds lessons into a workspace's charter without naming {tool}"
            );
        }
    }
}

#[test]
fn a_sentence_that_opens_workspace_md_is_found_and_one_that_reads_it_is_not() {
    purlis_core::unsteered!();
    let page = "In a workspace, open its\n`workspace.md`. Move what lasts.\n\n\
                Read its memory and its `workspace.md`.\n\nEdit `workspace.md` by hand";
    assert_eq!(
        hand_edits(page),
        [
            "In a workspace, open its `workspace.md`",
            "Edit `workspace.md` by hand"
        ]
    );
}
