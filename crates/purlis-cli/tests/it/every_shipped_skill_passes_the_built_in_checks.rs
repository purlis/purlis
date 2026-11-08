// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every shipped skill passes the built-in checks** (#5721, spec #5714).
//!
//! A built-in skill is read by a harness twice: its description decides whether it starts at
//! all, and its body is copied, a command at a time, once it has. So one check walks every
//! `SKILL.md` the app's plugin ships and holds it to [`RULES`], each a function from a skill to
//! what is wrong with it:
//!
//! * **[`description`]**: the frontmatter has one, it fits in 1,024 characters (Codex cuts a
//!   description there), and it says the word purlis in its prose, not only inside a code span
//!   such as `` `purlis worktree` ``, so a generic request for another tool does not start it
//!   and a purlis one does.
//! * **[`commands`]**: every `purlis …` in a code span or a fenced code block is a command this
//!   build has, at every depth (`purlis worktree commit`), with only the flags that command
//!   takes. Asked of the binary with `--help`, as
//!   [`every_command_charter_names_is_one_it_has`](super::every_command_charter_names_is_one_it_has)
//!   asks it, so an alias clap accepts counts and nothing here can drift from the parser.
//! * **[`tools`]**: every purlis tool a code span names (`` `workspace_section` ``,
//!   `` `mcp__purlis__dispatch_note` ``) is one a chat is offered: the tools `purlis mcp`
//!   lists, read from the same statics it serves.
//!
//! More rules join [`RULES`] as the spec's tickets land (the avoid-list, trigger tests and their
//! tier). What a skill names on purpose against a rule is on [`ON_PURPOSE`], with why; the list
//! refuses an entry nothing matches, so it only shrinks.

use super::every_command_charter_names_is_one_it_has::{PROSE, help_of, repo, shipped_skills};
use crate::support;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

/// Codex's limit on a skill's description, in characters.
const DESCRIPTION_LIMIT: usize = 1024;

/// One skill as the checks read it.
struct Skill {
    /// Its folder's name, which is its name.
    name: String,
    text: String,
}

/// What a rule found: the line of the skill (1 for its frontmatter), and what is wrong.
type Finding = (usize, String);

/// What the checks ask of this build: the binary's `--help` at each command path, and the
/// tools a chat is offered.
struct Build {
    answers: RefCell<BTreeMap<Vec<String>, Option<String>>>,
    tools: BTreeSet<&'static str>,
}

impl Build {
    fn new() -> Self {
        let tools = purlis_core::chattools::TOOLS
            .iter()
            .chain(purlis_core::browsertools::TOOLS.iter())
            .chain(purlis_core::themepreview::TOOLS.iter())
            .chain(purlis_core::skilltools::TOOLS.iter())
            .map(|tool| tool.name)
            .collect();
        Self {
            answers: RefCell::default(),
            tools,
        }
    }

    /// `purlis <path> --help`, or `None` when the binary refuses it.
    fn help(&self, path: &[String]) -> Option<String> {
        self.answers
            .borrow_mut()
            .entry(path.to_vec())
            .or_insert_with(|| {
                let words: Vec<&str> = path.iter().map(String::as_str).collect();
                help_of(support::PURLIS, &words)
            })
            .clone()
    }

    /// The first words of the tools' names, which a code span must open with to be read as
    /// naming one: `dispatch`, `browser`, `workspace` and the rest.
    fn families(&self) -> BTreeSet<&'static str> {
        self.tools
            .iter()
            .filter_map(|name| name.split('_').next())
            .collect()
    }
}

type Rule = fn(&Skill, &Build) -> Vec<Finding>;

/// Every rule a shipped skill is held to, by the name a finding is reported under.
const RULES: &[(&str, Rule)] = &[
    ("description", description),
    ("commands", commands),
    ("tools", tools),
];

/// What a skill names against a rule on purpose: the skill, the rule, a part of the finding's
/// text, and why. **Shrink only**: an entry nothing matches fails the check.
#[rustfmt::skip]
const ON_PURPOSE: &[(&str, &str, &str, &str)] = &[
    // The update skill says the range view's flags are retired and refused by name, which is
    // what a chat that remembers them needs to read. clap hides them from `--help` (#352).
    ("purlis-update", "commands", "`purlis news` has no --since", "retired flags, named to say so (#352)"),
];

/// A snake_case word in a code span that opens with a tool family's word and is not a tool.
#[rustfmt::skip]
const NOT_TOOLS: &[(&str, &str)] = &[
    ("theme_schema", "a field of `purlis theme schema`'s answer, named by the theme skill"),
];

// ---------------------------------------------------------------------------------------------
// Reading a skill

/// The description in `text`'s frontmatter, with the line it is on: a one-line value, quoted
/// or not, or a folded (`>`) or literal (`|`) block under it.
fn description_of(text: &str) -> Option<(usize, String)> {
    let mut lines = text.lines().enumerate();
    if lines.next()?.1.trim() != "---" {
        return None;
    }
    let mut lines = lines
        .take_while(|(_, line)| line.trim() != "---")
        .peekable();
    while let Some((at, line)) = lines.next() {
        let Some(value) = line.strip_prefix("description:") else {
            continue;
        };
        let value = value.trim();
        let value = if value.starts_with(['>', '|']) {
            let mut block = Vec::new();
            while let Some((_, next)) = lines.next_if(|(_, l)| l.starts_with([' ', '\t'])) {
                block.push(next.trim());
            }
            block.join(" ")
        } else {
            value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(value)
                .to_owned()
        };
        return Some((at + 1, value));
    }
    None
}

/// `text` with its code spans taken out.
fn prose_of(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some((before, after)) = rest.split_once('`') {
        out.push_str(before);
        match after.split_once('`') {
            Some((_, more)) => rest = more,
            None => {
                rest = after;
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The code in `text`, each piece with the line it starts on: every line of a fenced code block
/// (a line ending in `\` joined to the next, as a shell joins it), and every code span of the
/// prose, which may wrap across lines inside its paragraph.
fn code_of(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut fence: Option<char> = None;
    let mut paragraph: Vec<(usize, &str)> = Vec::new();
    let mut joined: Option<(usize, String)> = None;
    for (at, line) in text.lines().enumerate() {
        let n = at + 1;
        let trimmed = line.trim_start();
        let mark = ['`', '~']
            .into_iter()
            .find(|mark| trimmed.chars().take(3).filter(|c| c == mark).count() == 3);
        if let Some(mark) = mark {
            spans(&paragraph, &mut out);
            paragraph.clear();
            match fence {
                None => fence = Some(mark),
                Some(open) if open == mark => fence = None,
                Some(_) => {}
            }
            continue;
        }
        if fence.is_some() {
            let (start, mut code) = joined.take().unwrap_or((n, String::new()));
            match line.strip_suffix('\\') {
                Some(head) => {
                    code.push_str(head);
                    code.push(' ');
                    joined = Some((start, code));
                }
                None => {
                    code.push_str(line);
                    out.push((start, code));
                }
            }
        } else if line.trim().is_empty() {
            spans(&paragraph, &mut out);
            paragraph.clear();
        } else {
            paragraph.push((n, line));
        }
    }
    spans(&paragraph, &mut out);
    out
}

/// The code spans of one paragraph: a run of backticks opens one, and the next run of the same
/// length closes it (CommonMark's rule), its line breaks read as spaces.
fn spans(paragraph: &[(usize, &str)], out: &mut Vec<(usize, String)>) {
    let Some(&(first, _)) = paragraph.first() else {
        return;
    };
    let text: Vec<char> = paragraph
        .iter()
        .map(|(_, line)| *line)
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .collect();
    let run = |at: usize| text[at..].iter().take_while(|c| **c == '`').count();
    let mut at = 0;
    while at < text.len() {
        if text[at] != '`' {
            at += 1;
            continue;
        }
        let open = run(at);
        let start = at + open;
        let mut close = None;
        let mut j = start;
        while j < text.len() {
            if text[j] == '`' {
                let length = run(j);
                if length == open {
                    close = Some(j);
                    break;
                }
                j += length;
            } else {
                j += 1;
            }
        }
        let Some(end) = close else {
            at = start;
            continue;
        };
        let line = first + text[..at].iter().filter(|c| **c == '\n').count();
        let code: String = text[start..end]
            .iter()
            .map(|c| if *c == '\n' { ' ' } else { *c })
            .collect();
        out.push((line, code));
        at = end + open;
    }
}

/// Each `purlis …` command in one piece of code, as the words after `purlis`: the first
/// `purlis` of each part the shell runs on its own (split at `|`, `;`, `&&`, `(`, `)`, a
/// backtick and `$(`), up to a comment. A later `purlis` in the same part is an argument, as
/// in `purlis theme export purlis`.
fn commands_in(code: &str) -> Vec<Vec<String>> {
    let code = code
        .replace("$(", "(")
        .replace("&&", ";")
        .replace("||", ";");
    code.split(['|', ';', '(', ')', '`'])
        .filter_map(|part| {
            let words: Vec<&str> = part.split_whitespace().collect();
            let at = words.iter().position(|word| *word == "purlis")?;
            let rest: Vec<String> = words[at + 1..]
                .iter()
                .take_while(|word| !word.starts_with('#') && !word.starts_with("<<"))
                .take_while(|word| !word.starts_with('>'))
                .map(|word| (*word).to_owned())
                .collect();
            (!rest.is_empty()).then_some(rest)
        })
        .collect()
}

/// A word as a command line reads it, without the brackets a usage line puts around an
/// optional part (`[--shared]`) or the punctuation of the sentence it closes.
fn bare(word: &str) -> &str {
    word.trim_start_matches(['[', '('])
        .trim_end_matches([']', ')', ',', '.', ':', ';'])
}

/// Whether `help` lists `flag` (`--to-shared`, `-w`) as one of its options.
fn lists_flag(help: &str, flag: &str) -> bool {
    help.match_indices(flag).any(|(at, _)| {
        let before = help[..at].chars().next_back();
        let after = help[at + flag.len()..].chars().next();
        before.is_none_or(|c| c.is_whitespace() || "[,".contains(c))
            && after.is_none_or(|c| c.is_whitespace() || "=,]<.".contains(c))
    })
}

/// What is wrong with `purlis <words>`: a subcommand the command it follows does not have, or a
/// flag the command does not take. The path is walked while the command has subcommands and
/// the next word is a bare one; the first flag, value or placeholder ends it, and `--` ends
/// the whole line (what follows is another program's).
fn wrong_in(words: &[String], build: &Build) -> Vec<String> {
    let mut path: Vec<String> = Vec::new();
    let mut walking = true;
    let mut out = Vec::new();
    for word in words {
        let word = bare(word);
        if word == "--" {
            break;
        }
        if word.starts_with('-') && word.len() > 1 {
            walking = false;
            let flag = word.split_once('=').map_or(word, |(flag, _)| flag);
            if matches!(flag, "-h" | "--help") {
                continue;
            }
            let Some(help) = build.help(&path) else {
                break;
            };
            if !lists_flag(&help, flag) {
                out.push(format!("`{}` has no {flag}", shown(&path)));
            }
            continue;
        }
        if !walking {
            continue;
        }
        walking = false;
        let subcommand = !word.is_empty()
            && word
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && word.starts_with(|c: char| c.is_ascii_lowercase());
        let Some(help) = build.help(&path) else {
            break;
        };
        if !subcommand || !help.contains("\nCommands:\n") {
            continue;
        }
        let mut deeper = path.clone();
        deeper.push(word.to_owned());
        if build.help(&deeper).is_some() {
            path = deeper;
            walking = true;
        } else if !help.contains("\nArguments:\n") {
            // A command with subcommands and no argument of its own: the word can only have
            // been meant as one of them.
            out.push(format!("`{}` is not a command", shown(&deeper)));
            break;
        }
    }
    out
}

fn shown(path: &[String]) -> String {
    std::iter::once("purlis")
        .chain(path.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The purlis tool a code span names, if it names one: `mcp__purlis__<tool>`, or a snake_case
/// word that opens with a tool family's word (`dispatch_note`, `browser_login`).
fn tool_named<'a>(code: &'a str, families: &BTreeSet<&str>) -> Option<&'a str> {
    let prefixed = code.strip_prefix("mcp__purlis__");
    let name = prefixed.unwrap_or(code);
    let snake = name.contains('_')
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !snake {
        return None;
    }
    let family = name.split('_').next().unwrap_or_default();
    (prefixed.is_some() || families.contains(family)).then_some(name)
}

// ---------------------------------------------------------------------------------------------
// The rules

fn description(skill: &Skill, _: &Build) -> Vec<Finding> {
    let Some((line, text)) = description_of(&skill.text) else {
        return vec![(1, "has no description in its frontmatter".to_owned())];
    };
    let mut out = Vec::new();
    let length = text.chars().count();
    if length > DESCRIPTION_LIMIT {
        out.push((
            line,
            format!("its description is {length} characters, over Codex's {DESCRIPTION_LIMIT}"),
        ));
    }
    let prose = prose_of(&text).to_ascii_lowercase();
    let says_purlis = prose.match_indices("purlis").any(|(at, _)| {
        prose[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
    });
    if !says_purlis {
        out.push((
            line,
            "its description never says purlis outside a code span".to_owned(),
        ));
    }
    out
}

/// Each command the prose of `text` tells a reader to run without a code span, with its line:
/// `purlis <word> [<word>]` after `run`, `try`, `use`, `with` or a colon, the shapes
/// [`every_command_charter_names_is_one_it_has`](super::every_command_charter_names_is_one_it_has)
/// reads in a message. Two words at most, since prose goes on after the command; a word that
/// makes purlis the subject of a sentence (`: purlis cuts …`) is none.
fn prose_commands(text: &str) -> Vec<(usize, Vec<String>)> {
    static SUGGESTION: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r"(?:\b[Rr]un|\b[Tt]ry|\b[Uu]se|\bwith|:)\s+purlis ([a-z][a-z-]*)(?: ([a-z][a-z-]*))?",
        )
        .expect("the pattern compiles")
    });
    let mut fenced = false;
    let mut out = Vec::new();
    for (at, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        for found in SUGGESTION.captures_iter(&prose_of(line)) {
            if PROSE.contains(&&found[1]) {
                continue;
            }
            let words = [found.get(1), found.get(2)]
                .into_iter()
                .flatten()
                .map(|word| word.as_str().to_owned())
                .collect();
            out.push((at + 1, words));
        }
    }
    out
}

fn commands(skill: &Skill, build: &Build) -> Vec<Finding> {
    let prose = prose_commands(&skill.text)
        .into_iter()
        .flat_map(|(line, words)| {
            wrong_in(&words, build)
                .into_iter()
                .map(move |wrong| (line, wrong))
        });
    code_of(&skill.text)
        .into_iter()
        .flat_map(|(line, code)| {
            commands_in(&code)
                .into_iter()
                .flat_map(|words| wrong_in(&words, build))
                .map(move |wrong| (line, wrong))
                .collect::<Vec<_>>()
        })
        .chain(prose)
        .collect()
}

fn tools(skill: &Skill, build: &Build) -> Vec<Finding> {
    let families = build.families();
    code_of(&skill.text)
        .into_iter()
        .filter_map(|(line, code)| {
            let name = tool_named(code.trim(), &families)?;
            let known = build.tools.contains(name) || NOT_TOOLS.iter().any(|(n, _)| *n == name);
            (!known).then(|| (line, format!("`{name}` is not a purlis tool")))
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// The check

/// Every finding of every rule over `skills`, as `<skill>:<line>: <rule>: <what>`, with the
/// [`ON_PURPOSE`] entries each matched.
fn check(skills: &[Skill], build: &Build) -> (Vec<String>, BTreeSet<usize>) {
    let mut found = Vec::new();
    let mut matched = BTreeSet::new();
    for skill in skills {
        for (rule, run) in RULES {
            for (line, what) in run(skill, build) {
                let excused = ON_PURPOSE.iter().position(|(name, r, part, _)| {
                    *name == skill.name && r == rule && what.contains(part)
                });
                match excused {
                    Some(entry) => {
                        matched.insert(entry);
                    }
                    None => found.push(format!("{}:{line}: {rule}: {what}", skill.name)),
                }
            }
        }
    }
    (found, matched)
}

fn shipped() -> Vec<Skill> {
    shipped_skills(&repo())
        .into_iter()
        .map(|path| Skill {
            name: path
                .parent()
                .and_then(|folder| folder.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            text: std::fs::read_to_string(&path).expect("a skill reads"),
        })
        .collect()
}

#[test]
fn every_shipped_skill_passes_the_built_in_checks() {
    let skills = shipped();
    assert!(skills.len() >= 10, "the walk read {} skills", skills.len());
    let build = Build::new();
    assert!(
        build.tools.contains("workspace_section") && build.tools.contains("browser_login"),
        "the tools were read: {:?}",
        build.tools
    );
    let (found, matched) = check(&skills, &build);
    assert!(
        found.is_empty(),
        "these shipped skills fail the built-in checks — fix the skill, or, when it names the \
         thing on purpose, add it to ON_PURPOSE with why:\n{}",
        found.join("\n")
    );
    let stale: Vec<&str> = ON_PURPOSE
        .iter()
        .enumerate()
        .filter(|(at, _)| !matched.contains(at))
        .map(|(_, (_, _, part, _))| *part)
        .collect();
    assert!(
        stale.is_empty(),
        "nothing matches these ON_PURPOSE entries any more — take them off: {stale:?}"
    );
}

/// A skill made to fail every rule, so each is seen to find what it is for.
const BAD: &str = "---\nname: purlis-bad\ndescription: Do real work in a repo with \
                   `purlis worktree`.\n---\n\n# Bad\n\nRun `purlis frob add x`, then \
                   `purlis worktree frob`, or\n`purlis worktree commit --amend -m \"x\"`.\n\n\
                   ```bash\npurlis dispatch report --outcom done \\\n  \"x\"\n\
                   purlis theme export purlis --to x.zip   # fine\n\
                   purlis secret exec v -- sh -c 'x'\n```\n\n\
                   Call the `dispatch_frob` tool, or `mcp__purlis__ask_nobody`, never the \
                   `dispatch_note` one.\n\nOr run purlis frob now, as `purlis worktree` says. purlis cuts it.\n";

#[test]
fn a_skill_made_to_fail_is_found_by_every_rule() {
    let build = Build::new();
    let bad = Skill {
        name: "purlis-bad".to_owned(),
        text: BAD.to_owned(),
    };
    let (found, _) = check(&[bad], &build);
    assert_eq!(
        found,
        [
            "purlis-bad:3: description: its description never says purlis outside a code span",
            "purlis-bad:8: commands: `purlis frob` is not a command",
            "purlis-bad:8: commands: `purlis worktree frob` is not a command",
            "purlis-bad:9: commands: `purlis worktree commit` has no --amend",
            "purlis-bad:12: commands: `purlis dispatch report` has no --outcom",
            "purlis-bad:20: commands: `purlis frob` is not a command",
            "purlis-bad:18: tools: `dispatch_frob` is not a purlis tool",
            "purlis-bad:18: tools: `ask_nobody` is not a purlis tool",
        ]
    );
}

#[test]
fn a_description_over_codexs_limit_is_found_and_one_at_it_is_not() {
    let build = Build::new();
    let page = |length: usize| Skill {
        name: "purlis-long".to_owned(),
        text: format!(
            "---\nname: purlis-long\ndescription: purlis {}\n---\n",
            "x".repeat(length - "purlis ".len())
        ),
    };
    assert!(description(&page(DESCRIPTION_LIMIT), &build).is_empty());
    assert_eq!(
        description(&page(DESCRIPTION_LIMIT + 1), &build),
        [(
            3,
            "its description is 1025 characters, over Codex's 1024".to_owned()
        )]
    );
}

#[test]
fn a_code_span_wraps_inside_its_paragraph_and_a_fenced_line_joins_its_continuation() {
    let text = "Run `purlis worktree\ncommit -m x` now.\n\n```\npurlis a \\\n  b\n```\n";
    assert_eq!(
        code_of(text),
        [
            (1, "purlis worktree commit -m x".to_owned()),
            (5, "purlis a    b".to_owned()),
        ]
    );
}
