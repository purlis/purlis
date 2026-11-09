// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The words a chat is told, settled once** (spec #5714's Q28 and Q31): the losing side of
//! each settled pair ([`LOSING`]) and the words the glossary avoids ([`AVOID`]), with how a text
//! is read for them.
//!
//! One copy, read by two checks: the shipped-skill check
//! (`tests/it/every_shipped_skill_passes_the_built_in_checks.rs`) holds every built-in skill to
//! them, and the binary's `main/tests/briefing_words.rs` holds the briefing, every tool's
//! description and the MCP server's own texts to them, which only the binary can reach.

/// **Words the glossary avoids** (#5723, spec #5714's Q31): `CONTEXT.md`'s `_Avoid_` words that
/// drifted into what a chat is told, each beside the word it says instead. Matched on the prose
/// only, as whole words in any case and across a line break, so a tool's name (`ask_operator`),
/// a command and a path, which are code, keep theirs. "the user" is two words, because "user"
/// alone is also a harness's word (`user` scope, `$USER`). "plane" stays where a command or a
/// path says it, until ADR 0072's rename.
pub(crate) const AVOID: &[(&[&str], &str)] = &[
    (&["control", "plane"], "the plane"),
    (&["sub-agent"], "a helper"),
    (&["sub-agents"], "helpers"),
    (&["subagent"], "a helper"),
    (&["subagents"], "helpers"),
    (&["the", "user"], "the person"),
    (&["operator"], "the person"),
    (&["operators"], "people"),
];

/// Each avoided word `prose` says, with what it says instead, once per time it is said.
pub(crate) fn avoided(prose: &str) -> Vec<(String, &'static str)> {
    let lower = prose.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
        .filter(|word| !word.is_empty())
        .collect();
    let mut out = Vec::new();
    for at in 0..words.len() {
        for (avoid, instead) in AVOID {
            if words[at..].starts_with(avoid) {
                out.push((avoid.join(" "), *instead));
            }
        }
    }
    out
}

/// **The losing side of each settled pair** (#5722, spec #5714's Q28): words a skill, the
/// briefing or a tool description said that another place contradicted, each beside the ruling
/// that settled it. Matched in any case, with runs of spaces and line breaks read as one space.
/// (e), (g) and (j) are said in words a phrase cannot hold, so they are read in review.
pub(crate) const LOSING: &[(&str, &str)] = &[
    (
        "(a) a website login is the `browser_login` tool, never a login run by its name",
        "run it by its name",
    ),
    (
        "(b) a chat commits in a branch folder with `purlis worktree commit`, in a terminal too",
        "commits with plain `git commit`",
    ),
    (
        "(c) a helper does read-only work; writing work is a purlis task",
        "right tool for work this chat's own persona owns",
    ),
    (
        "(c) a helper does read-only work; writing work is a purlis task",
        "with work this chat's own persona owns: your harness's own helper",
    ),
    (
        "(d) a chat does what it can itself, and dispatches when the work needs another \
         persona's charter, memory or vault",
        "rather than doing it yourself",
    ),
    (
        "(f) the app saves a sandboxed chat's change to the project, a terminal chat runs \
         `purlis save`",
        "commit it.",
    ),
    (
        "(h) a chat never asks for a secret's value: it raises a Secret ask",
        "supply the value",
    ),
    (
        "(i) purlis asks the person through `ask_operator`, not one harness's question tool",
        "askuserquestion",
    ),
    (
        "(i) purlis asks the person through `ask_operator`, not one harness's question tool",
        "request_user_input",
    ),
];

/// Each ruling `text` says the losing side of, once per phrase found.
pub(crate) fn losing_sides(text: &str) -> Vec<&'static str> {
    let text = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    LOSING
        .iter()
        .filter(|(_, phrase)| text.contains(&phrase.to_lowercase()))
        .map(|(ruling, _)| *ruling)
        .collect()
}

/// Whether `line` opens or closes a fence: three backticks or tildes.
fn fence(line: &str) -> Option<char> {
    let line = line.trim_start();
    ['`', '~']
        .into_iter()
        .find(|mark| line.chars().take(3).filter(|c| c == mark).count() == 3)
}

/// The paragraphs of `page` outside its fenced blocks, code spans and all, each with the line it
/// starts on.
pub(crate) fn paragraphs(page: &str) -> Vec<(usize, String)> {
    let mut fenced: Option<char> = None;
    let mut paragraph: Option<(usize, String)> = None;
    let mut out = Vec::new();
    for (at, line) in page.lines().enumerate() {
        let ends = fence(line).is_some() || (fenced.is_none() && line.trim().is_empty());
        if ends {
            out.extend(paragraph.take());
        }
        if let Some(mark) = fence(line) {
            match fenced {
                None => fenced = Some(mark),
                Some(open) if open == mark => fenced = None,
                Some(_) => {}
            }
        } else if fenced.is_none() && !line.trim().is_empty() {
            let (_, text) = paragraph.get_or_insert_with(|| (at + 1, String::new()));
            text.push_str(line);
            text.push('\n');
        }
    }
    out.extend(paragraph);
    out
}

/// The prose of `page`: each of its [`paragraphs`] with its code spans taken out.
pub(crate) fn prose_paragraphs(page: &str) -> Vec<(usize, String)> {
    paragraphs(page)
        .into_iter()
        .map(|(line, paragraph)| {
            let prose = paragraph
                .split('`')
                .step_by(2)
                .collect::<Vec<_>>()
                .join(" ");
            (line, prose)
        })
        .collect()
}
