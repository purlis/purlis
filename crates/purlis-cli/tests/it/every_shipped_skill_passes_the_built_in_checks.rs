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
//! * **[`settled`]** (#5722): it says no losing side of a settled pair ([`settled_words::LOSING`]).
//! * **[`glossary`]** (#5723): its prose says none of the words the glossary avoids
//!   ([`settled_words::AVOID`]).
//! * **[`skills`]**: a skill its prose names is one purlis ships.
//! * **[`neighbours`]** (#5730): its description's "Not for" clauses name every neighbour its
//!   `evals/sources.json` declares, each a skill purlis ships.
//! * **[`sandbox`]** (#5735): a sentence naming a way to lower the sandbox or edit a harness's
//!   settings ([`NEVER_OFFERED`]) says never to.
//! * **[`measured`]** (#5731): it carries trigger tests (`evals/triggers.json`) and an
//!   `evals/sources.json` that reads, with its tier.
//!
//! And the set is held to [`SET_RULES`]: **[`descriptions`]**, every description together at
//! most [`MOST_DESCRIPTIONS`] characters (spec #5714's Q37). What a skill names on purpose
//! against a rule is on [`ON_PURPOSE`], with why; the list refuses an entry nothing matches, so
//! it only shrinks. The briefing and the tools' own texts are held to the same settled words by
//! the binary's `main/tests/briefing_words.rs`, which reads [`settled_words`] too.

use super::every_command_charter_names_is_one_it_has::{PROSE, help_of, repo, shipped_skills};
use crate::support;

#[path = "../shared/settled_words.rs"]
mod settled_words;

use purlis_skills::grade::TriggerTest;
use purlis_skills::grade::ci::{SOURCES_PATH, sources_of};
use settled_words::{avoided, losing_sides, paragraphs, prose_paragraphs};

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

/// Codex's limit on a skill's description, in characters.
const DESCRIPTION_LIMIT: usize = 1024;

/// **The built-ins' descriptions together** (spec #5714's Q37): at most this many characters,
/// so Codex's 8,000-character listing keeps every built-in and room for the plane's own skills,
/// and opencode's default budget (8,000) keeps them too. The number is purlis-core's.
const MOST_DESCRIPTIONS: usize = purlis_core::skills::BUILT_IN_DESCRIPTIONS_MOST;

/// One skill as the checks read it.
struct Skill {
    /// Its folder's name, which is its name.
    name: String,
    text: String,
    /// Its trigger tests, from `evals/triggers.json`: none where it has no such file.
    triggers: Vec<TriggerTest>,
    /// Its `evals/sources.json`'s text, where it has one.
    sources: Option<String>,
}

impl Skill {
    /// A skill with no trigger tests and no sources file.
    fn of(name: &str, text: &str) -> Self {
        Self {
            name: name.to_owned(),
            text: text.to_owned(),
            triggers: Vec::new(),
            sources: None,
        }
    }

    /// The same skill, carrying `triggers` and the sources file `sources`.
    fn with(mut self, triggers: Vec<TriggerTest>, sources: Option<String>) -> Self {
        self.triggers = triggers;
        self.sources = sources;
        self
    }
}

/// What a rule found: the line of the skill (1 for its frontmatter), and what is wrong.
type Finding = (usize, String);

/// What the checks ask of this build: the binary's `--help` at each command path, the tools a
/// chat is offered, and the skills purlis ships.
struct Build {
    answers: RefCell<BTreeMap<Vec<String>, Option<String>>>,
    tools: BTreeSet<&'static str>,
    skills: BTreeSet<String>,
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
            skills: shipped().into_iter().map(|skill| skill.name).collect(),
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
    ("settled", settled),
    ("glossary", glossary),
    ("skills", skills),
    ("neighbours", neighbours),
    ("sandbox", sandbox),
    ("save", save),
    ("measured", measured),
];

/// A rule over the whole set: what is wrong, each naming the skills.
type SetRule = fn(&[Skill]) -> Vec<String>;

/// Every rule the shipped skills are held to together, by the name a finding is reported under.
const SET_RULES: &[(&str, SetRule)] = &[("descriptions", descriptions)];

/// What a skill names against a rule on purpose: the skill, the rule, a part of the finding's
/// text, and why. **Shrink only**: an entry nothing matches fails the check.
#[rustfmt::skip]
const ON_PURPOSE: &[(&str, &str, &str, &str)] = &[
    // The upgrade skill says the range view's flags are retired and refused by name, which is
    // what a chat that remembers them needs to read. clap hides them from `--help` (#352).
    ("purlis-upgrade", "commands", "`purlis news` has no --since", "retired flags, named to say so (#352)"),
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

/// The blocks of `text` between its blank lines, fenced code and all, each with the line it
/// starts on: what a phrase may run across, its line breaks included.
fn blocks(text: &str) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut open = false;
    for (at, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            open = false;
            continue;
        }
        match out.last_mut() {
            Some((_, block)) if open => {
                block.push('\n');
                block.push_str(line);
            }
            _ => out.push((at + 1, line.to_owned())),
        }
        open = true;
    }
    out
}

fn settled(skill: &Skill, _: &Build) -> Vec<Finding> {
    blocks(&skill.text)
        .into_iter()
        .flat_map(|(line, block)| {
            losing_sides(&block).into_iter().map(move |ruling| {
                (
                    line,
                    format!("says the losing side of a settled pair, against {ruling}"),
                )
            })
        })
        .collect()
}

fn glossary(skill: &Skill, _: &Build) -> Vec<Finding> {
    prose_paragraphs(&skill.text)
        .into_iter()
        .flat_map(|(line, prose)| {
            avoided(&prose).into_iter().map(move |(word, instead)| {
                (
                    line,
                    format!("says “{word}” where the glossary says {instead}"),
                )
            })
        })
        .collect()
}

/// The skills `text` names in its prose, each with its paragraph's line: a code span the word
/// skill follows ("the `purlis-persona` skill"), and a `purlis-…` word it follows outside code
/// ("the purlis-persona skill").
fn skills_named(text: &str) -> Vec<(usize, String)> {
    let is_skill = |after: &str| {
        after
            .split(|c: char| !c.is_alphanumeric())
            .find(|word| !word.is_empty())
            == Some("skill")
    };
    let mut out = Vec::new();
    for (line, paragraph) in paragraphs(text) {
        let parts: Vec<&str> = paragraph.split('`').collect();
        for at in (1..parts.len().saturating_sub(1)).step_by(2) {
            if parts[at + 1].starts_with(char::is_whitespace) && is_skill(parts[at + 1]) {
                out.push((line, parts[at].trim().to_owned()));
            }
        }
        for prose in parts.iter().step_by(2) {
            for (at, _) in prose.match_indices("purlis-") {
                let starts = prose[..at]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !(c.is_alphanumeric() || "-_/".contains(c)));
                let name: String = prose[at..]
                    .chars()
                    .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
                    .collect();
                let name = name.trim_end_matches('-');
                if starts && is_skill(&prose[at + name.len()..]) {
                    out.push((line, name.to_owned()));
                }
            }
        }
    }
    out
}

fn skills(skill: &Skill, build: &Build) -> Vec<Finding> {
    skills_named(&skill.text)
        .into_iter()
        .filter(|(_, name)| !build.skills.contains(name))
        .map(|(line, name)| {
            (
                line,
                format!("names the skill `{name}`, which purlis does not ship"),
            )
        })
        .collect()
}

/// **The house style's "Not for" clause** (#5730, spec #5714's Q15 and Q30): a skill whose
/// `evals/sources.json` declares neighbours says, after "Not for" in its description, the name
/// of each, so a harness choosing between two skills reads where one ends. Each neighbour is a
/// skill purlis ships, since its near-miss tests are that skill's requests.
fn neighbours(skill: &Skill, build: &Build) -> Vec<Finding> {
    let Some(text) = &skill.sources else {
        return Vec::new();
    };
    // A sources file that does not read is named by `measured`.
    let Ok(sources) = sources_of(text, &skill.name, &skill.triggers) else {
        return Vec::new();
    };
    let (line, description) = description_of(&skill.text).unwrap_or((1, String::new()));
    let neighbours = sources.neighbours;
    let mut out: Vec<Finding> = neighbours
        .iter()
        .filter(|name| !build.skills.contains(*name))
        .map(|name| {
            (
                1,
                format!(
                    "{SOURCES_PATH} declares the neighbour `{name}`, which purlis does not ship"
                ),
            )
        })
        .collect();
    if neighbours.is_empty() {
        return out;
    }
    let description = description.to_lowercase();
    let Some(at) = description.find("not for") else {
        out.push((
            line,
            format!(
                "its description has no “Not for” clause, and its {SOURCES_PATH} names the \
                 neighbours {}",
                neighbours.join(", ")
            ),
        ));
        return out;
    };
    let after = &description[at..];
    for name in &neighbours {
        if !names_word(after, name) {
            out.push((
                line,
                format!("its description's “Not for” clauses never name its neighbour `{name}`"),
            ));
        }
    }
    out
}

/// Whether `text` says `name` as a whole name, not as part of a longer one.
fn names_word(text: &str, name: &str) -> bool {
    let part = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '-' || c == '_');
    text.match_indices(name).any(|(at, _)| {
        !part(text[..at].chars().next_back()) && !part(text[at + name.len()..].chars().next())
    })
}

/// **Remedies a chat never offers** (#5735, spec #5714's Q19, the briefing's `SANDBOXED_NOTE`):
/// each lowers purlis's sandbox or edits a harness's own settings file, which purlis's own
/// settings outrank, so a chat that suggests one sends the person to a refusal or to a rule
/// nothing reads. Matched in any case.
const NEVER_OFFERED: &[&str] = &[
    "/sandbox",
    "excludedcommands",
    "dangerouslydisablesandbox",
    "start without the sandbox",
    "lower the sandbox",
    ".claude/settings.json",
    ".claude/settings.local.json",
    "opencode.json",
    "config.toml",
    "settings file",
];

/// The words that make a sentence about a remedy say not to take it.
const REFUSING: &[&str] = &["never", "not", "no", "cannot", "don't", "nothing"];

/// The sentences of `text` outside its fenced blocks, code spans kept, each on one line with its
/// paragraph's line: a sentence ends at a `.`, `!` or `?` that a space or the paragraph's end
/// follows, so a path's dot does not end one.
fn sentences(text: &str) -> Vec<(usize, String)> {
    paragraphs(text)
        .into_iter()
        .flat_map(|(line, paragraph)| {
            let one = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
            let mut out = Vec::new();
            let mut sentence = String::new();
            let mut chars = one.chars().peekable();
            while let Some(c) = chars.next() {
                sentence.push(c);
                if matches!(c, '.' | '!' | '?') && chars.peek().is_none_or(|n| *n == ' ') {
                    out.push((line, std::mem::take(&mut sentence).trim().to_owned()));
                }
            }
            out.push((line, sentence.trim().to_owned()));
            out.retain(|(_, sentence)| !sentence.is_empty());
            out
        })
        .collect()
}

/// **Never lowers the sandbox or edits a harness's settings** (#5735): a sentence that names one
/// of [`NEVER_OFFERED`] says not to take it, in one of the [`REFUSING`] words. A sentence is
/// read to its end ([`sentences`]), so a refusal in the next one does not cover it.
fn sandbox(skill: &Skill, _: &Build) -> Vec<Finding> {
    sentences(&skill.text)
        .into_iter()
        .filter_map(|(line, sentence)| {
            let lower = sentence.to_lowercase();
            let named = NEVER_OFFERED.iter().find(|r| lower.contains(*r))?;
            let refuses = lower
                .split(|c: char| !(c.is_alphanumeric() || c == '\''))
                .any(|word| REFUSING.contains(&word));
            (!refuses).then(|| {
                (
                    line,
                    format!("offers “{named}” without saying never: “{sentence}”"),
                )
            })
        })
        .collect()
}

/// **A terminal chat saves only on the person's yes** (the review of #5797): `purlis save`
/// publishes everything pending in the plane, not only what the chat changed, so a sentence
/// that tells a terminal chat to run it also says it waits for the person's yes, as
/// purlis-workspace's "Live and `purlis save` publish" does.
fn save(skill: &Skill, _: &Build) -> Vec<Finding> {
    sentences(&skill.text)
        .into_iter()
        .filter_map(|(line, sentence)| {
            let lower = sentence.to_lowercase();
            let about = lower.contains("`purlis save") && lower.contains("terminal");
            let asks = lower
                .split(|c: char| !(c.is_alphanumeric() || c == '\''))
                .any(|word| word == "yes");
            (about && !asks).then(|| {
                (
                    line,
                    format!(
                        "has a terminal chat run `purlis save` without the person's yes: \
                         “{sentence}”"
                    ),
                )
            })
        })
        .collect()
}

/// **Every built-in is measured** (#5731, spec #5714's Q2 and Q3): it carries trigger tests in
/// `evals/triggers.json`, and beside them an `evals/sources.json` a reviewer reads and the CI
/// grader scores by, naming its tier, its neighbours and where each test came from. So no
/// built-in ships without a trigger grade to hold it to its bar.
fn measured(skill: &Skill, _: &Build) -> Vec<Finding> {
    let mut out = Vec::new();
    if skill.triggers.is_empty() {
        out.push((
            1,
            "carries no trigger tests (`evals/triggers.json`), so no grade says whether it \
             starts when it should"
                .to_owned(),
        ));
    }
    match &skill.sources {
        None => out.push((
            1,
            format!(
                "has no {SOURCES_PATH} naming its tier, its neighbours and where each test \
                 came from"
            ),
        )),
        Some(text) if !skill.triggers.is_empty() => {
            if let Err(why) = sources_of(text, &skill.name, &skill.triggers) {
                out.push((
                    1,
                    format!("its trigger tests' sources cannot be read: {why}"),
                ));
            }
        }
        Some(_) => {}
    }
    out
}

fn descriptions(skills: &[Skill]) -> Vec<String> {
    let length = |skill: &Skill| {
        description_of(&skill.text).map_or(0, |(_, description)| description.trim().chars().count())
    };
    let total: usize = skills.iter().map(length).sum();
    if total <= MOST_DESCRIPTIONS {
        return Vec::new();
    }
    let biggest = skills
        .iter()
        .max_by_key(|skill| length(skill))
        .map(|skill| format!("{}'s ({})", skill.name, length(skill)))
        .unwrap_or_default();
    vec![format!(
        "the built-ins' descriptions together are {total} characters, over \
         {MOST_DESCRIPTIONS}: the biggest is {biggest}"
    )]
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
    for (rule, run) in SET_RULES {
        found.extend(
            run(skills)
                .into_iter()
                .map(|what| format!("{rule}: {what}")),
        );
    }
    (found, matched)
}

/// Every skill the app's plugin ships, with its trigger tests and its sources file.
fn shipped() -> Vec<Skill> {
    shipped_skills(&repo())
        .into_iter()
        .map(|path| {
            let folder = path.parent().expect("a skill's folder");
            let name = folder
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let text = std::fs::read_to_string(&path).expect("a skill reads");
            let triggers = std::fs::read_to_string(folder.join("evals/triggers.json"))
                .map(|json| {
                    serde_json::from_str(&json)
                        .unwrap_or_else(|why| panic!("{name}'s evals/triggers.json reads: {why}"))
                })
                .unwrap_or_default();
            let sources = std::fs::read_to_string(folder.join(SOURCES_PATH)).ok();
            Skill::of(&name, &text).with(triggers, sources)
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
    let bad = measured_skill("purlis-bad", BAD);
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
    let page = |length: usize| {
        let text = format!(
            "---\nname: purlis-long\ndescription: purlis {}\n---\n",
            "x".repeat(length - "purlis ".len())
        );
        Skill::of("purlis-long", &text)
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

// ---------------------------------------------------------------------------------------------
// The later rules, each seen to find what it is for

/// A trigger test the skill should start on.
fn should_start(query: &str) -> TriggerTest {
    TriggerTest {
        query: query.to_owned(),
        should_trigger: true,
    }
}

/// An `evals/sources.json` with `tier` (none where `None`), `neighbours`, and a source for each
/// of `queries`.
fn sources_json(tier: Option<&str>, neighbours: &[&str], queries: &[&str]) -> String {
    let prompts: Vec<_> = queries
        .iter()
        .map(|query| serde_json::json!({ "query": query, "source": "Skill text." }))
        .collect();
    let mut file = serde_json::json!({ "neighbours": neighbours, "prompts": prompts });
    if let Some(tier) = tier {
        file["tier"] = tier.into();
    }
    file.to_string()
}

/// A skill measured as a built-in is: one trigger test, and a sources file that reads.
fn measured_skill(name: &str, text: &str) -> Skill {
    Skill::of(name, text).with(
        vec![should_start("a")],
        Some(sources_json(Some("standard"), &[], &["a"])),
    )
}

const GOOD: &str = "---\nname: purlis-example\ndescription: Do a thing in purlis. Use when asked to.\n---\n\n# Example\n\n";

/// What the check finds in a measured example skill whose page is `text`.
fn problems(text: &str) -> Vec<String> {
    check(&[measured_skill("purlis-example", text)], &Build::new()).0
}

#[test]
fn a_measured_skill_that_breaks_no_rule_has_no_problem() {
    assert_eq!(problems(&format!("{GOOD}Do it.\n")), Vec::<String>::new());
}

/// A measured example skill whose description is `length` characters long, filled with `fill`.
fn described(name: &str, length: usize, fill: char) -> Skill {
    let lead = "In purlis ";
    let description = format!("{lead}{}", fill.to_string().repeat(length - lead.len()));
    measured_skill(
        name,
        &format!("---\nname: {name}\ndescription: {description}\n---\n"),
    )
}

#[test]
fn descriptions_over_the_total_are_named_with_the_biggest() {
    // Five of the same length and purlis-b, the biggest, filling the rest exactly.
    let other = (MOST_DESCRIPTIONS - 1_000) / 5;
    assert_eq!(other * 5 + 1_000, MOST_DESCRIPTIONS, "an even split");
    let skills = |b: usize| {
        [
            "purlis-a", "purlis-b", "purlis-c", "purlis-d", "purlis-e", "purlis-f",
        ]
        .map(|name| {
            // Counted in characters, as a harness's listing counts them: an em dash is one.
            let (length, fill) = if name == "purlis-b" {
                (b, '—')
            } else {
                (other, 'a')
            };
            described(name, length, fill)
        })
    };
    assert_eq!(check(&skills(1_000), &Build::new()).0, Vec::<String>::new());
    assert_eq!(
        check(&skills(1_001), &Build::new()).0,
        [format!(
            "descriptions: the built-ins' descriptions together are {} characters, over \
             {MOST_DESCRIPTIONS}: the biggest is purlis-b's (1001)",
            MOST_DESCRIPTIONS + 1
        )]
    );
}

#[test]
fn a_skill_that_says_the_losing_side_of_a_settled_pair_is_named_with_the_ruling() {
    for (page, ruling) in [
        ("Run the login: run it by its\nname with `-s=<s>`.", "(a)"),
        ("Ask them to Supply the value by stdin.", "(h)"),
        ("Ask once, with **AskUserQuestion**.", "(i)"),
        ("Write the persona. Commit it. Personas are shared.", "(f)"),
    ] {
        let found = problems(&format!("{GOOD}{page}\n"));
        assert_eq!(found.len(), 1, "{page}: {found:?}");
        assert!(
            found[0].starts_with("purlis-example:8: settled: says the losing side")
                && found[0].contains(ruling),
            "{found:?}"
        );
    }
    // The winning side of each passes.
    for page in [
        "Call `browser_login`; running the login's name as a command does nothing.",
        "Ask with `ask_operator`.",
        "Commit with `purlis worktree commit`, and commit it there alone.",
    ] {
        assert_eq!(
            problems(&format!("{GOOD}{page}\n")),
            Vec::<String>::new(),
            "{page}"
        );
    }
}

#[test]
fn a_skill_that_says_a_word_the_glossary_avoids_is_named_with_the_word_it_says_instead() {
    for (page, word) in [
        ("A control\nplane orchestrates repos.", "“control plane”"),
        ("Start a sub-agent for it.", "“sub-agent”"),
        ("Two subagents read it.", "“subagents”"),
        ("Tell The User what it says.", "“the user”"),
        ("Prune only with the operator's yes.", "“operator”"),
    ] {
        let found = problems(&format!("{GOOD}{page}\n"));
        assert_eq!(found.len(), 1, "{page}: {found:?}");
        assert!(
            found[0].starts_with("purlis-example:8: glossary: ") && found[0].contains(word),
            "{found:?}"
        );
    }
    let page = "---\nname: purlis-example\ndescription: Ask the operator in purlis.\n---\n";
    assert_eq!(
        problems(page),
        ["purlis-example:1: glossary: says “operator” where the glossary says the person"]
    );
    // A tool's name, a command and a path are code, and keep their words; so does a word that
    // only contains one.
    for page in [
        "Ask the person with `ask_operator`.",
        "```bash\nls ~/control plane/the user\n```",
        "A helper reads; the user-level settings stay; a cooperator is no operator-like word.",
    ] {
        assert_eq!(
            problems(&format!("{GOOD}{page}\n")),
            Vec::<String>::new(),
            "{page}"
        );
    }
}

#[test]
fn a_skill_named_in_prose_that_purlis_does_not_ship_is_named() {
    for (page, name) in [
        ("Use the `secrets` skill for it.", "secrets"),
        ("That is the purlis-secret skill's.", "purlis-secret"),
        ("(the `purlis-persnoa` skill)", "purlis-persnoa"),
    ] {
        assert_eq!(
            problems(&format!("{GOOD}{page}\n")),
            [format!(
                "purlis-example:8: skills: names the skill `{name}`, which purlis does not ship"
            )],
            "{page}"
        );
    }
    for page in [
        "Use the `purlis-secrets` skill, or the purlis-persona skill.",
        "A `secrets` file is no skill; `purlis-secrets` is one.",
        "Run `purlis skill list`; a project skill is the project's.",
    ] {
        assert_eq!(
            problems(&format!("{GOOD}{page}\n")),
            Vec::<String>::new(),
            "{page}"
        );
    }
}

/// What the check finds in an example skill described as `description`, whose sources file
/// declares two neighbours.
fn sourced(description: &str) -> Vec<String> {
    let page = format!("---\nname: purlis-example\ndescription: {description}\n---\n");
    let skill = Skill::of("purlis-example", &page).with(
        vec![should_start("a"), should_start("b")],
        Some(sources_json(
            Some("strict"),
            &["purlis-browser", "purlis-persona"],
            &["a", "b"],
        )),
    );
    check(&[skill], &Build::new()).0
}

#[test]
fn a_description_whose_not_for_clauses_name_every_neighbour_has_no_problem() {
    assert_eq!(
        sourced(
            "Use a credential in purlis. Not for a website login — that is purlis-browser; not \
             for another persona's work — that is `purlis-persona`."
        ),
        Vec::<String>::new()
    );
}

#[test]
fn a_description_with_neighbours_and_no_not_for_clause_is_named_with_them() {
    assert_eq!(
        sourced("Use a credential in purlis, unlike purlis-browser and purlis-persona."),
        [
            "purlis-example:3: neighbours: its description has no “Not for” clause, and its \
             evals/sources.json names the neighbours purlis-browser, purlis-persona"
        ]
    );
}

#[test]
fn a_neighbour_the_not_for_clauses_leave_out_is_named() {
    // Named before "Not for" and as part of a longer name is not named in a clause.
    assert_eq!(
        sourced(
            "Use purlis-persona's vault in purlis. Not for a website login — that is \
             purlis-browser; not for purlis-personas-old."
        ),
        [
            "purlis-example:3: neighbours: its description's “Not for” clauses never name its \
             neighbour `purlis-persona`"
        ]
    );
}

#[test]
fn a_neighbour_purlis_does_not_ship_and_unreadable_sources_are_named() {
    let page =
        "---\nname: purlis-example\ndescription: Do it in purlis. Not for purlis-nope.\n---\n";
    let skill = Skill::of("purlis-example", page).with(
        vec![should_start("a")],
        Some(sources_json(Some("strict"), &["purlis-nope"], &["a"])),
    );
    assert_eq!(
        check(&[skill], &Build::new()).0,
        [
            "purlis-example:1: neighbours: evals/sources.json declares the neighbour \
             `purlis-nope`, which purlis does not ship"
        ]
    );
    let skill =
        Skill::of("purlis-example", page).with(Vec::new(), Some(sources_json(None, &[], &[])));
    assert_eq!(
        check(&[skill], &Build::new()).0,
        [
            "purlis-example:1: measured: carries no trigger tests (`evals/triggers.json`), so \
             no grade says whether it starts when it should"
        ]
    );
}

#[test]
fn a_sentence_that_offers_to_lower_the_sandbox_or_edit_a_harness_s_settings_is_named() {
    for (page, named) in [
        ("Run `/sandbox exclude cargo` and retry.", "/sandbox"),
        ("Add the host to `excludedCommands`.", "excludedcommands"),
        (
            "Ask them to choose Start without the sandbox. Never guess.",
            "start without the sandbox",
        ),
        (
            "Edit `.claude/settings.json` and add `Bash(terraform *)` to its allow list.",
            ".claude/settings.json",
        ),
        (
            "Put the rule in the harness's settings file by hand.",
            "settings file",
        ),
    ] {
        let found = problems(&format!("{GOOD}{page}\n"));
        assert_eq!(found.len(), 1, "{page}: {found:?}");
        assert!(
            found[0].starts_with("purlis-example:8: sandbox: offers") && found[0].contains(named),
            "{found:?}"
        );
    }
}

#[test]
fn a_sentence_that_says_never_to_lower_the_sandbox_or_edit_a_harness_s_settings_is_not_named() {
    for page in [
        "Never suggest `/sandbox` or `excludedCommands`: purlis's own settings outrank them.",
        "The person alone may choose Start without the sandbox; never offer it.\n\nYou do not \
         edit `.claude/settings.json` by hand.",
        "```bash\ncat .claude/settings.json\n```",
        "Settings › Project › Sandbox shows the level.",
    ] {
        assert_eq!(
            problems(&format!("{GOOD}{page}\n")),
            Vec::<String>::new(),
            "{page}"
        );
    }
}

#[test]
fn a_sentence_that_has_a_terminal_chat_save_without_the_person_s_yes_is_named() {
    for page in [
        "Save it: in a sandboxed chat the app saves it; in a terminal chat run `purlis save`.",
        "In a terminal, `purlis save` runs there.",
    ] {
        let found = problems(&format!("{GOOD}{page}\n"));
        assert_eq!(found.len(), 1, "{page}: {found:?}");
        assert!(
            found[0].starts_with("purlis-example:8: save: has a terminal chat run"),
            "{found:?}"
        );
    }
}

#[test]
fn a_sentence_that_saves_from_a_terminal_on_the_person_s_yes_is_not_named() {
    for page in [
        "In a terminal chat, run `purlis save` only on the person's yes (purlis-workspace).",
        "In a sandboxed chat the app saves it.",
        "```bash\npurlis save \"x\"   # in a terminal\n```",
    ] {
        assert_eq!(
            problems(&format!("{GOOD}{page}\n")),
            Vec::<String>::new(),
            "{page}"
        );
    }
}

#[test]
fn a_built_in_without_trigger_tests_sources_or_a_tier_is_named() {
    let page = format!("{GOOD}Do it.\n");
    let found = |skill: Skill| check(&[skill], &Build::new()).0;
    let no_tests = "purlis-example:1: measured: carries no trigger tests (`evals/triggers.json`), \
                    so no grade says whether it starts when it should";
    let no_sources = "purlis-example:1: measured: has no evals/sources.json naming its tier, its \
                      neighbours and where each test came from";
    assert_eq!(
        found(Skill::of("purlis-example", &page)),
        [no_tests, no_sources]
    );
    assert_eq!(
        found(Skill::of("purlis-example", &page).with(vec![should_start("a")], None)),
        [no_sources]
    );
    assert_eq!(
        found(Skill::of("purlis-example", &page).with(
            vec![should_start("a")],
            Some(sources_json(None, &[], &["a"]))
        )),
        [
            "purlis-example:1: measured: its trigger tests' sources cannot be read: \
             evals/sources.json names no tier: say \"strict\" or \"standard\""
        ]
    );
    assert_eq!(
        found(Skill::of("purlis-example", &page).with(
            vec![should_start("a"), should_start("b")],
            Some(sources_json(Some("standard"), &[], &["a"]))
        )),
        [
            "purlis-example:1: measured: its trigger tests' sources cannot be read: \
             evals/sources.json has no source for the prompt \"b\""
        ]
    );
    assert_eq!(
        found(measured_skill("purlis-example", &page)),
        Vec::<String>::new()
    );
}

// ---------------------------------------------------------------------------------------------
// Where a fact goes, said once

/// **The skill that holds the rule for where a fact goes** (#5733, spec #5714's Q29): workspace
/// memory, the workspace's charter, persona memory or shared memory. Written there once; every
/// other text a chat is told points to it rather than saying it again, so the rule cannot drift
/// into four versions.
const REMEMBER: &str = "purlis-remember";

/// The texts that used to restate the rule, each of which now points to [`REMEMBER`].
const POINT_TO_REMEMBER: [&str; 4] = [
    "purlis-persona",
    "purlis-safe-remove",
    "purlis-smart-close",
    "purlis-tidy-memory",
];

/// Each base a fact can be written to that `text` names a way to write to: the tool or command
/// that records a fact there. A text naming two or more is choosing between them, which is the
/// rule; one naming one is only using it.
fn bases_written(text: &str) -> BTreeSet<&'static str> {
    let commands: Vec<String> = code_of(text)
        .into_iter()
        .flat_map(|(_, code)| code.lines().map(str::to_owned).collect::<Vec<_>>())
        .collect();
    // A command shown recording a fact: its words and a quoted argument. One named in passing,
    // as the secrets skill names the commands that persist prose, records nothing.
    let said = |words: &'static str| {
        commands
            .iter()
            .filter(move |line| line.contains(words) && line.contains('"'))
    };
    let prose = text.split_whitespace().collect::<Vec<_>>().join(" ");
    // Each sentence that names the `persona_remember` tool, and whether it gives it `shared`.
    let persona_tool: Vec<bool> = prose
        .match_indices("`persona_remember`")
        .map(|(at, _)| {
            prose[at..]
                .split(". ")
                .next()
                .unwrap_or("")
                .contains("`shared`")
        })
        .collect();
    let mut bases = BTreeSet::new();
    if prose.contains("`memory_add`") || said("purlis workspace remember").next().is_some() {
        bases.insert("workspace memory");
    }
    if prose.contains("`workspace_section`") || prose.contains("`workspace_vision`") {
        bases.insert("the workspace's charter");
    }
    if persona_tool.iter().any(|shared| !shared)
        || said("purlis persona remember").any(|line| !line.contains("--shared"))
    {
        bases.insert("persona memory");
    }
    if persona_tool.iter().any(|shared| *shared)
        || said("purlis persona remember").any(|line| line.contains("--shared"))
    {
        bases.insert("shared memory");
    }
    bases
}

#[test]
fn the_rule_for_where_a_fact_goes_is_written_once_and_the_rest_point_to_it() {
    let skills = shipped();
    let remember = skills
        .iter()
        .find(|skill| skill.name == REMEMBER)
        .unwrap_or_else(|| panic!("purlis ships no `{REMEMBER}`"));
    assert_eq!(
        bases_written(&remember.text).len(),
        4,
        "{REMEMBER} names a way to write to only {:?}",
        bases_written(&remember.text)
    );
    let mut wrong = Vec::new();
    for skill in skills.iter().filter(|skill| skill.name != REMEMBER) {
        let bases = bases_written(&skill.text);
        if bases.len() > 1 {
            wrong.push(format!(
                "{} says where a fact goes ({}) — point to {REMEMBER} instead",
                skill.name,
                bases.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
    }
    for name in POINT_TO_REMEMBER {
        let skill = skills
            .iter()
            .find(|skill| skill.name == name)
            .unwrap_or_else(|| panic!("purlis ships no `{name}`"));
        if !skills_named(&skill.text).iter().any(|(_, n)| n == REMEMBER) {
            wrong.push(format!("{name} does not point to {REMEMBER}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_text_naming_ways_to_write_to_two_bases_is_choosing_between_them() {
    let one = |text: &str| bases_written(text).into_iter().collect::<Vec<_>>();
    assert_eq!(
        one("record it with the `memory_add` tool"),
        ["workspace memory"]
    );
    assert_eq!(
        one("add it with `workspace_section`, or a goal with `workspace_vision`"),
        ["the workspace's charter"]
    );
    assert_eq!(
        one("the `persona_remember` tool with `shared`, to every persona"),
        ["shared memory"]
    );
    assert_eq!(
        one("```bash\npurlis persona remember <persona> --shared \"<fact>\"\n```"),
        ["shared memory"]
    );
    assert_eq!(
        one("record with the `persona_remember` tool. The workspace's own takes `memory_add`."),
        ["persona memory", "workspace memory"]
    );
    assert_eq!(
        one("escape a backtick in `purlis persona remember <persona>` and `note`"),
        Vec::<&str>::new(),
        "a command named in passing records no fact"
    );
    assert_eq!(
        one("purlis persona edit-memory <name> <slug> --shared"),
        Vec::<&str>::new(),
        "curating a memory writes no fact"
    );
}
