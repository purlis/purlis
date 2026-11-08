// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The shipped-skill check** (#5721, spec #5714's seam 1).
//!
//! A built-in skill is copied, not read: a chat runs the command a step shows and calls the
//! tool a step names, and the harness picks the skill by its description alone. So every
//! `SKILL.md` the app's plugin ships is held here, rule by rule, to:
//!
//! * **the commands this binary has**: each `purlis …` (or `charter …`) in a code span or a
//!   fenced block names subcommands and flags that this parser, [`Cli`], answers;
//! * **the tools a chat is handed**: each tool it names is one of [`crate::mcp::every_tool`];
//! * **a description a harness lists**: one the studio's own lint passes, so at most
//!   [`MOST_DESCRIPTION`] characters (Codex cuts there), and saying the word purlis outside its
//!   code, so a person asking about purlis in their own words finds it.
//!
//! Each rule is one function in [`RULES`], given one skill and what the build has, answering
//! what is wrong in words that name the skill and the thing. A later rule is one more entry.
//! Prior art: `every_shipped_skill_names_only_purlis` in purlis-core, which this follows in
//! reading a skill's code.

use super::Cli;
use clap::CommandFactory;
use purlis_skills::change::{MOST_DESCRIPTION, description_problem};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// One built-in skill as it ships: its folder's name, its page, its description, how many
/// trigger tests it carries where its `evals/triggers.json` reads, and its sources file's text
/// where it has one.
struct Skill {
    name: String,
    page: String,
    description: Option<String>,
    tests: Option<usize>,
    sources: Option<String>,
}

impl Skill {
    fn of(name: &str, page: &str) -> Self {
        let description = purlis_skills::frontmatter(page)
            .into_iter()
            .find(|(key, _)| key == "description")
            .map(|(_, value)| value);
        Skill {
            name: name.to_owned(),
            page: page.to_owned(),
            description,
            tests: None,
            sources: None,
        }
    }

    /// The same skill, carrying `tests` trigger tests and no sources file.
    fn with_tests(mut self, tests: usize) -> Self {
        self.tests = Some(tests);
        self
    }

    /// The same skill, carrying `tests` trigger tests whose sources file says `text`.
    fn with_sources(self, text: &str, tests: usize) -> Self {
        let mut skill = self.with_tests(tests);
        skill.sources = Some(text.to_owned());
        skill
    }
}

/// What a rule checks a skill against: this build's parser and its chat tools.
struct Built {
    cli: clap::Command,
    tools: BTreeSet<&'static str>,
    skills: BTreeSet<String>,
}

impl Built {
    fn now() -> Self {
        let mut cli = Cli::command();
        // Built, so `--help` and `--version` are arguments and a global one reaches every
        // subcommand, as they do when the binary parses.
        cli.build();
        Built {
            cli,
            tools: crate::mcp::every_tool().map(|tool| tool.name).collect(),
            skills: shipped().into_iter().map(|skill| skill.name).collect(),
        }
    }
}

/// A rule: what is wrong with one skill, each as a line that names the thing.
type Rule = fn(&Skill, &Built) -> Vec<String>;

/// Every rule the shipped skills are held to. Add a rule here.
const RULES: &[(&str, Rule)] = &[
    ("names a command purlis has", commands_are_real),
    ("names a tool a chat is handed", tools_are_real),
    ("has a description a harness lists", description_is_listable),
    ("says purlis in its description", description_says_purlis),
    ("says each settled pair one way", says_no_losing_side),
    ("uses the glossary's words", uses_the_glossary_s_words),
    ("names a skill purlis ships", skills_are_real),
    ("says what it is not for", names_each_neighbour),
    (
        "never lowers the sandbox or edits a harness's settings",
        keeps_the_sandbox,
    ),
    ("carries trigger tests, sources and a tier", is_measured),
];

/// **Built-ins leaving the bundle**, each with where it goes: the one place a shipped skill is
/// not held to carry trigger tests (#5731). An entry goes when its folder does, and the check
/// fails until it has.
const LEAVING: &[(&str, &str)] = &[(
    "purlis-building-ui",
    "leaves for the plane as its building-purlis-ui skill (#5725)",
)];

/// **Words the glossary avoids** (#5723, spec #5714's Q31): `CONTEXT.md`'s `_Avoid_` words that
/// drifted into what a chat is told, each beside the word it says instead. Matched on the prose
/// only, as whole words in any case and across a line break, so a tool's name (`ask_operator`),
/// a command and a path, which are code, keep theirs. "the user" is two words, because "user"
/// alone is also a harness's word (`user` scope, `$USER`). "plane" stays where a command or a
/// path says it, until ADR 0072's rename.
const AVOID: &[(&[&str], &str)] = &[
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
fn avoided(prose: &str) -> Vec<(String, &'static str)> {
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
const LOSING: &[(&str, &str)] = &[
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
fn losing_sides(text: &str) -> Vec<&'static str> {
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

// ---- reading a skill's code ----------------------------------------------------------- //

/// Whether `line` opens or closes a fence: three or more backticks or tildes.
fn fence(line: &str) -> Option<char> {
    let line = line.trim_start();
    ['`', '~']
        .into_iter()
        .find(|mark| line.chars().take(3).filter(|c| c == mark).count() == 3)
}

/// The code of `page`: each line of a fenced block, up to a shell comment, and each code span of
/// the prose between, read whole where it wraps onto the next line.
fn code(page: &str) -> Vec<String> {
    fn spans(prose: &mut String, out: &mut Vec<String>) {
        out.extend(
            prose
                .split('`')
                .skip(1)
                .step_by(2)
                .map(|span| span.replace('\n', " ")),
        );
        prose.clear();
    }
    let mut fenced: Option<char> = None;
    let mut prose = String::new();
    let mut out = Vec::new();
    let mut continued = false;
    for line in page.lines() {
        if let Some(mark) = fence(line) {
            match fenced {
                None => {
                    spans(&mut prose, &mut out);
                    fenced = Some(mark);
                }
                Some(open) if open == mark => fenced = None,
                Some(_) => {}
            }
            continue;
        }
        if fenced.is_some() {
            // A line ending in `\` goes on on the next, as a shell reads it.
            let line = uncommented(line).trim_end();
            match out.last_mut() {
                Some(last) if continued => {
                    last.pop();
                    last.push(' ');
                    last.push_str(line.trim_start());
                }
                _ => out.push(line.to_owned()),
            }
            continued = line.ends_with('\\');
        } else if line.trim().is_empty() {
            // A blank line ends a paragraph, and a code span never crosses one.
            spans(&mut prose, &mut out);
        } else {
            prose.push_str(line);
            prose.push('\n');
        }
    }
    spans(&mut prose, &mut out);
    out
}

/// The paragraphs of `page` outside its fenced blocks, code spans and all.
fn paragraphs(page: &str) -> Vec<String> {
    let mut fenced: Option<char> = None;
    let mut paragraph = String::new();
    let mut out = Vec::new();
    for line in page.lines() {
        if let Some(mark) = fence(line) {
            match fenced {
                None => {
                    out.push(std::mem::take(&mut paragraph));
                    fenced = Some(mark);
                }
                Some(open) if open == mark => fenced = None,
                Some(_) => {}
            }
        } else if fenced.is_none() {
            if line.trim().is_empty() {
                out.push(std::mem::take(&mut paragraph));
            } else {
                paragraph.push_str(line);
                paragraph.push('\n');
            }
        }
    }
    out.push(paragraph);
    out.retain(|paragraph| !paragraph.is_empty());
    out
}

/// The prose of `page`: what [`code`] leaves, each paragraph with its code spans taken out.
fn prose(page: &str) -> String {
    paragraphs(page)
        .iter()
        .map(|paragraph| {
            paragraph
                .split('`')
                .step_by(2)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The skills `page` names in its prose: a code span the word skill follows ("the
/// `purlis-persona` skill"), and a `purlis-…` word it follows outside code ("the purlis-persona
/// skill").
fn skills_named(page: &str) -> Vec<String> {
    let is_skill = |after: &str| {
        after
            .split(|c: char| !c.is_alphanumeric())
            .find(|word| !word.is_empty())
            == Some("skill")
    };
    let mut out = Vec::new();
    for paragraph in paragraphs(page) {
        let parts: Vec<&str> = paragraph.split('`').collect();
        for at in (1..parts.len().saturating_sub(1)).step_by(2) {
            if parts[at + 1].starts_with(char::is_whitespace) && is_skill(parts[at + 1]) {
                out.push(parts[at].trim().to_owned());
            }
        }
        for text in parts.iter().step_by(2) {
            for (at, _) in text.match_indices("purlis-") {
                let starts = text[..at]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !(c.is_alphanumeric() || "-_/".contains(c)));
                let name: String = text[at..]
                    .chars()
                    .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
                    .collect();
                let name = name.trim_end_matches('-');
                if starts && is_skill(&text[at + name.len()..]) {
                    out.push(name.to_owned());
                }
            }
        }
    }
    out
}

/// `line` up to its shell comment: a `#` that opens a word.
fn uncommented(line: &str) -> &str {
    let at = line
        .char_indices()
        .find(|&(at, c)| {
            c == '#'
                && line[..at]
                    .chars()
                    .next_back()
                    .is_none_or(char::is_whitespace)
        })
        .map_or(line.len(), |(at, _)| at);
    &line[..at]
}

/// The names a skill may call the command line by: the one it ships as, and its alias.
const NAMES: [&str; 2] = ["purlis", "charter"];

/// Each command line in `code` that runs purlis: the words after its name, as a shell splits
/// them, up to the end of the command (a pipe, `;`, `&&`, a redirect, a comment, `)`). The name
/// runs where a command starts: the start of the code, or after `(`, `|`, `;`, `&` or a
/// backtick. Elsewhere it is a word given to another command (`purlis theme export purlis`).
fn commands(code: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for name in NAMES {
        for (at, _) in code.match_indices(name) {
            let before = code[..at].trim_end().chars().next_back();
            let rest = &code[at + name.len()..];
            let starts = before.is_none_or(|c| "(|;&`".contains(c));
            let mut after = rest.chars();
            let runs = after.next() == Some(' ')
                && after
                    .next()
                    .is_some_and(|c| c.is_ascii_lowercase() || c == '-');
            if starts && runs {
                out.push(words(rest));
            }
        }
    }
    out
}

/// The words of a shell command up to its end, as a shell splits and unquotes them
/// (`shell-words`). A quoted value comes back as its text, so a placeholder still reads as one by
/// what it holds.
fn words(text: &str) -> Vec<String> {
    shell_words::split(command_of(text)).unwrap_or_default()
}

/// `text` up to the end of its first command: a pipe, `;`, `&`, `)`, a newline, a comment or a
/// redirect, outside quotes. A `<placeholder>`, a `<` with no space before its `>`, is a word the
/// reader fills in, not a redirect. A quote left open is a value cut off, and says nothing, so the
/// command ends where it opens.
fn command_of(text: &str) -> &str {
    let mut quote: Option<(char, usize)> = None;
    let mut chars = text.char_indices().peekable();
    let mut word_starts = true;
    while let Some((at, c)) = chars.next() {
        match quote {
            Some((open, _)) => {
                if c == open {
                    quote = None;
                } else if open == '"' && c == '\\' {
                    chars.next();
                }
            }
            None => match c {
                '\'' | '"' => quote = Some((c, at)),
                '\\' => {
                    chars.next();
                }
                '<' => match placeholder_ends(&text[at..]) {
                    Some(len) => while chars.next_if(|&(next, _)| next < at + len).is_some() {},
                    None => return &text[..at],
                },
                '|' | ';' | '&' | ')' | '>' | '\n' => return &text[..at],
                '#' if word_starts => return &text[..at],
                _ => {}
            },
        }
        word_starts = c.is_whitespace();
    }
    quote.map_or(text, |(_, at)| &text[..at])
}

/// Where the `<placeholder>` that `text` starts with ends (just past its `>`), if it is one: a
/// `<` and a `>` with no space or other `<` between.
fn placeholder_ends(text: &str) -> Option<usize> {
    let inner = text.strip_prefix('<')?;
    let close = inner.find(|c: char| c == '>' || c == '<' || c.is_whitespace())?;
    (close > 0 && inner[close..].starts_with('>')).then_some(close + 2)
}

/// Whether `word` stands for a value the reader fills in, so says nothing of the parser: a
/// `<placeholder>`, a `$` value, a value that was quoted around spaces, an ellipsis, an
/// `[optional]` or `{a,b}` part.
fn placeholder(word: &str) -> bool {
    word.contains(['<', '[', '{', '$', '"', '\'', '…', '*'])
        || word.contains("...")
        || word.contains(char::is_whitespace)
}

/// What is wrong with `purlis <words>` as this parser reads it: a subcommand it does not have,
/// or a flag the command it reached does not take.
fn wrong_in(cli: &clap::Command, words: &[String]) -> Vec<String> {
    let mut cmd = cli;
    let mut path = vec![cli.get_name().to_owned()];
    let mut out = Vec::new();
    let mut positional = false;
    let mut iter = words.iter();
    while let Some(word) = iter.next() {
        let shown = path.join(" ");
        if word == "--" {
            break;
        }
        if let Some(long) = word.strip_prefix("--") {
            let long = long.split_once('=').map_or(long, |(name, _)| name);
            if placeholder(long) {
                continue;
            }
            match cmd.get_arguments().find(|a| {
                a.get_long() == Some(long)
                    || a.get_all_aliases()
                        .is_some_and(|aliases| aliases.contains(&long))
            }) {
                Some(arg) => {
                    if arg.get_action().takes_values() && !word.contains('=') {
                        iter.next();
                    }
                }
                None => out.push(format!("`{shown}` takes no flag `--{long}`")),
            }
            continue;
        }
        if let Some(shorts) = word
            .strip_prefix('-')
            .filter(|s| s.chars().next().is_some_and(|c| c.is_ascii_alphabetic()))
        {
            for (at, short) in shorts.char_indices() {
                match cmd.get_arguments().find(|a| a.get_short() == Some(short)) {
                    Some(arg) if arg.get_action().takes_values() => {
                        if at + short.len_utf8() == shorts.len() {
                            iter.next();
                        }
                        break;
                    }
                    Some(_) => {}
                    None => {
                        out.push(format!("`{shown}` takes no flag `-{short}`"));
                        break;
                    }
                }
            }
            continue;
        }
        let takes_words = cmd.get_positionals().next().is_some();
        if !positional && cmd.has_subcommands() {
            if let Some(sub) = cmd.find_subcommand(word) {
                path.push(sub.get_name().to_owned());
                cmd = sub;
                continue;
            }
            if placeholder(word) && !takes_words {
                // `purlis persona <verb>`: which one is the reader's, so nothing after it can
                // be read.
                break;
            }
            if !takes_words {
                out.push(format!("`{shown}` has no subcommand `{word}`"));
                break;
            }
        }
        positional = true;
    }
    out
}

/// The tools `code` names: `mcp__purlis__<tool>` anywhere, and a code span that is one
/// snake_case word (or a call of one) in a family the tools have (`dispatch_…`, `browser_…`).
fn tools_named(code: &str, families: &BTreeSet<&str>) -> Vec<String> {
    let prefix = format!("mcp__{}__", purlis_core::chattools::SERVER);
    let mut out: Vec<String> = code
        .match_indices(&prefix)
        .map(|(at, _)| {
            code[at + prefix.len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect()
        })
        .collect();
    let bare = code.split_once('(').map_or(code, |(name, _)| name).trim();
    let snake = bare.contains('_')
        && bare
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if snake
        && bare
            .split_once('_')
            .is_some_and(|(family, _)| families.contains(family))
    {
        out.push(bare.to_owned());
    }
    out
}

/// Words a skill spells like a tool that are not one, each with what it is. **Shrink only.**
const NOT_TOOLS: &[(&str, &str)] = &[(
    "theme_schema",
    "a field of theme_preview's answer: the address a theme.json names in $schema",
)];

// ---- the rules ------------------------------------------------------------------------ //

fn commands_are_real(skill: &Skill, built: &Built) -> Vec<String> {
    code(&skill.page)
        .into_iter()
        .flat_map(|code| commands(&code))
        .flat_map(|words| wrong_in(&built.cli, &words))
        .collect()
}

fn tools_are_real(skill: &Skill, built: &Built) -> Vec<String> {
    let families: BTreeSet<&str> = built
        .tools
        .iter()
        .filter_map(|name| name.split_once('_').map(|(family, _)| family))
        .collect();
    code(&skill.page)
        .into_iter()
        .flat_map(|code| tools_named(&code, &families))
        .filter(|name| !built.tools.contains(name.as_str()))
        .filter(|name| !NOT_TOOLS.iter().any(|(not, _)| not == name))
        .map(|name| format!("names the tool `{name}`, which no chat is handed"))
        .collect()
}

fn description_is_listable(skill: &Skill, _: &Built) -> Vec<String> {
    match &skill.description {
        None => vec!["has no description, and a harness lists a skill by it".to_owned()],
        Some(description) => description_problem(description)
            .map(|why| format!("its description cannot be listed: {why}"))
            .into_iter()
            .collect(),
    }
}

fn description_says_purlis(skill: &Skill, _: &Built) -> Vec<String> {
    let Some(description) = &skill.description else {
        return Vec::new();
    };
    let prose = description.split('`').step_by(2);
    let says = prose
        .flat_map(|text| text.split(|c: char| !c.is_alphanumeric()))
        .any(|word| word == "purlis");
    if says {
        Vec::new()
    } else {
        vec![
            "its description never says purlis outside its code, so a person asking about \
             purlis does not find it"
                .to_owned(),
        ]
    }
}

fn says_no_losing_side(skill: &Skill, _: &Built) -> Vec<String> {
    losing_sides(&skill.page)
        .into_iter()
        .map(|ruling| format!("says the losing side of a settled pair, against {ruling}"))
        .collect()
}

fn uses_the_glossary_s_words(skill: &Skill, _: &Built) -> Vec<String> {
    avoided(&prose(&skill.page))
        .into_iter()
        .map(|(word, instead)| format!("says “{word}” where the glossary says {instead}"))
        .collect()
}

fn skills_are_real(skill: &Skill, built: &Built) -> Vec<String> {
    skills_named(&skill.page)
        .into_iter()
        .filter(|name| !built.skills.contains(name))
        .map(|name| format!("names the skill `{name}`, which purlis does not ship"))
        .collect()
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

/// The sentences of `page` outside its fenced blocks, code spans kept, each on one line: a
/// sentence ends at a `.`, `!` or `?` that a space or the paragraph's end follows, so a path's
/// dot does not end one.
fn sentences(page: &str) -> Vec<String> {
    paragraphs(page)
        .iter()
        .flat_map(|paragraph| {
            let one = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
            let mut out = Vec::new();
            let mut sentence = String::new();
            let mut chars = one.chars().peekable();
            while let Some(c) = chars.next() {
                sentence.push(c);
                if matches!(c, '.' | '!' | '?') && chars.peek().is_none_or(|n| *n == ' ') {
                    out.push(std::mem::take(&mut sentence).trim().to_owned());
                }
            }
            out.push(sentence.trim().to_owned());
            out.retain(|sentence| !sentence.is_empty());
            out
        })
        .collect()
}

/// **Never lowers the sandbox or edits a harness's settings** (#5735): a sentence that names one
/// of [`NEVER_OFFERED`] says not to take it, in one of the [`REFUSING`] words. A sentence is
/// read to its end ([`sentences`]), so a refusal in the next one does not cover it.
fn keeps_the_sandbox(skill: &Skill, _: &Built) -> Vec<String> {
    sentences(&skill.page)
        .into_iter()
        .filter_map(|sentence| {
            let lower = sentence.to_lowercase();
            let named = NEVER_OFFERED.iter().find(|r| lower.contains(*r))?;
            let refuses = lower
                .split(|c: char| !(c.is_alphanumeric() || c == '\''))
                .any(|word| REFUSING.contains(&word));
            (!refuses).then(|| format!("offers “{named}” without saying never: “{sentence}”"))
        })
        .collect()
}

/// **Every built-in is measured** (#5731, spec #5714's Q2 and Q3): it carries trigger tests in
/// `evals/triggers.json`, and beside them a sources file a reviewer reads and the CI grader
/// scores by, naming its trigger bar, its neighbours and where each test came from. So no
/// built-in ships without a trigger grade to hold it to its bar.
fn is_measured(skill: &Skill, _: &Built) -> Vec<String> {
    if LEAVING.iter().any(|(name, _)| *name == skill.name) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let tests = skill.tests.unwrap_or(0);
    if tests == 0 {
        out.push(
            "carries no trigger tests (`evals/triggers.json`), so no grade says whether it \
             starts when it should"
                .to_owned(),
        );
    }
    match &skill.sources {
        None => out.push(
            "has no sources file (`evals/triggers.sources.md`) naming its trigger bar, its \
             neighbours and where each test came from"
                .to_owned(),
        ),
        Some(text) if tests > 0 => {
            if let Err(why) = purlis_skills::grade::ci::sources_of(text, tests) {
                out.push(format!("its trigger tests' sources cannot be read: {why}"));
            }
        }
        Some(_) => {}
    }
    out
}

/// **The house style's "Not for" clause** (#5730, spec #5714's Q15 and Q30): a skill whose
/// sources file declares neighbours says, after "Not for" in its description, the name of each,
/// so a harness choosing between two skills reads where one ends. Each neighbour is a skill
/// purlis ships, since its near-miss tests are that skill's requests.
fn names_each_neighbour(skill: &Skill, built: &Built) -> Vec<String> {
    let Some(text) = &skill.sources else {
        return Vec::new();
    };
    // A sources file that does not read is named by `is_measured`.
    let Ok(sources) = purlis_skills::grade::ci::sources_of(text, skill.tests.unwrap_or(0)) else {
        return Vec::new();
    };
    let neighbours = sources.neighbours;
    let mut out: Vec<String> = neighbours
        .iter()
        .filter(|name| !built.skills.contains(*name))
        .map(|name| format!("declares the neighbour `{name}`, which purlis does not ship"))
        .collect();
    if neighbours.is_empty() {
        return out;
    }
    let description = skill.description.as_deref().unwrap_or("").to_lowercase();
    let Some(at) = description.find("not for") else {
        out.push(format!(
            "its description has no “Not for” clause, and its sources name the neighbours {}",
            neighbours.join(", ")
        ));
        return out;
    };
    let after = &description[at..];
    for name in &neighbours {
        if !names_word(after, name) {
            out.push(format!(
                "its description's “Not for” clauses never name its neighbour `{name}`"
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

/// Each skill, by name, with every line a rule has against it.
fn check(skills: &[Skill], built: &Built) -> Vec<String> {
    let mut out = Vec::new();
    for skill in skills {
        for (rule, of) in RULES {
            for wrong in of(skill, built) {
                out.push(format!("{}: {wrong} ({rule})", skill.name));
            }
        }
    }
    out
}

fn skills_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/src-tauri/plugin/skills")
}

/// Every skill the app's plugin ships, by its folder's name.
fn shipped() -> Vec<Skill> {
    let mut folders: Vec<_> = std::fs::read_dir(skills_dir())
        .expect("the skills folder")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("SKILL.md").is_file())
        .collect();
    folders.sort();
    folders
        .iter()
        .map(|folder| {
            let name = folder.file_name().expect("a folder").to_string_lossy();
            let page = std::fs::read_to_string(folder.join("SKILL.md")).expect("a skill reads");
            let skill = Skill::of(&name, &page);
            let evals = folder.join("evals");
            let tests = std::fs::read_to_string(evals.join("triggers.json"))
                .ok()
                .and_then(|json| serde_json::from_str::<Vec<serde_json::Value>>(&json).ok())
                .map(|tests| tests.len());
            match (
                tests,
                std::fs::read_to_string(evals.join("triggers.sources.md")),
            ) {
                (tests, Ok(text)) => skill.with_sources(&text, tests.unwrap_or(0)),
                (Some(tests), Err(_)) => skill.with_tests(tests),
                (None, Err(_)) => skill,
            }
        })
        .collect()
}

#[test]
fn every_shipped_skill_passes_every_rule() {
    let skills = shipped();
    assert!(skills.len() >= 10, "the walk read {} skills", skills.len());
    let built = Built::now();
    let lines: usize = skills
        .iter()
        .map(|skill| {
            code(&skill.page)
                .into_iter()
                .flat_map(|code| commands(&code))
                .count()
        })
        .sum();
    assert!(
        lines > 50,
        "only {lines} commands read, so the walk is not reading them"
    );
    let wrong = check(&skills, &built);
    assert!(
        wrong.is_empty(),
        "shipped skills that break a rule:\n{}",
        wrong.join("\n")
    );
    for (name, goes) in LEAVING {
        assert!(
            built.skills.contains(*name),
            "`{name}` ({goes}) is shipped no more — take it off LEAVING"
        );
    }
    for (word, why) in NOT_TOOLS {
        assert!(
            skills
                .iter()
                .any(|skill| code(&skill.page).iter().any(|c| c.trim() == *word)),
            "`{word}` ({why}) is named by no skill now — take it off NOT_TOOLS"
        );
        assert!(
            !built.tools.contains(word),
            "`{word}` is a tool now — take it off NOT_TOOLS"
        );
    }
}

// ---- the check's own tests ------------------------------------------------------------ //

/// A sources file for one trigger test that reads: what a measured example skill carries.
const MEASURED: &str = "Trigger bar: standard\nNeighbours: none\n\n1. a\n";

fn problems(page: &str) -> Vec<String> {
    check(
        &[Skill::of("purlis-example", page).with_sources(MEASURED, 1)],
        &Built::now(),
    )
}

const GOOD: &str = "---\nname: purlis-example\ndescription: Do a thing in purlis. Use when asked to.\n---\n\n# Example\n";

#[test]
fn a_skill_that_breaks_no_rule_has_no_problem() {
    let page = format!(
        "{GOOD}Run `purlis dispatch report --outcome done \"what\"`, or call `dispatch_report`.\n\n\
         ```bash\npurlis persona recall steward --query <keywords> | head\n\
         purlis worktree commit -m \"<message>\" --all\n```\n"
    );
    assert_eq!(problems(&page), Vec::<String>::new());
}

#[test]
fn a_subcommand_purlis_does_not_have_is_named_with_its_skill() {
    let page = format!("{GOOD}Run `purlis persona frobnicate x`.\n");
    assert_eq!(
        problems(&page),
        [
            "purlis-example: `purlis persona` has no subcommand `frobnicate` (names a command purlis has)"
        ]
    );
    let page = format!("{GOOD}```sh\ncharter frob add x\n```\n");
    assert_eq!(
        problems(&page),
        ["purlis-example: `purlis` has no subcommand `frob` (names a command purlis has)"]
    );
}

#[test]
fn a_flag_the_command_does_not_take_is_named_with_its_command() {
    let page = format!("{GOOD}Run `purlis dispatch report --verdict done`.\n");
    assert_eq!(
        problems(&page),
        [
            "purlis-example: `purlis dispatch report` takes no flag `--verdict` (names a command purlis has)"
        ]
    );
    let page = format!("{GOOD}Run `purlis worktree commit -Q`.\n");
    assert_eq!(
        problems(&page),
        [
            "purlis-example: `purlis worktree commit` takes no flag `-Q` (names a command purlis has)"
        ]
    );
}

#[test]
fn a_placeholder_is_a_word_and_the_check_goes_on_past_it() {
    let page = format!("{GOOD}Run `purlis dispatch report <id> --verdict done`.\n");
    assert_eq!(
        problems(&page),
        [
            "purlis-example: `purlis dispatch report` takes no flag `--verdict` (names a command purlis has)"
        ]
    );
    let page = format!(
        "{GOOD}```sh\npurlis dispatch report \"<what you did>\" \\\n  --verdict done\n```\n"
    );
    assert_eq!(
        problems(&page),
        [
            "purlis-example: `purlis dispatch report` takes no flag `--verdict` (names a command purlis has)"
        ]
    );
    // A redirect still ends the command.
    for page in [
        "Run `purlis dispatch report done < notes.txt --verdict`.",
        "Run `purlis dispatch report done > out.txt --verdict`.",
        "Run `purlis dispatch report done | tee --verdict`.",
    ] {
        assert_eq!(
            problems(&format!("{GOOD}{page}\n")),
            Vec::<String>::new(),
            "{page}"
        );
    }
}

#[test]
fn a_flag_s_value_and_a_placeholder_are_not_read_as_subcommands() {
    for page in [
        "Run `purlis worktree commit -m frob --all`.",
        "Run `purlis persona <verb> --anything`.",
        "Run `purlis dispatch report --outcome=done \"x\"`.",
        "Run `purlis dispatch report --help`.",
        "Then `purlis` runs it, and purlis is not a command in prose.",
    ] {
        assert_eq!(
            problems(&format!("{GOOD}{page}\n")),
            Vec::<String>::new(),
            "{page}"
        );
    }
}

#[test]
fn a_tool_no_chat_is_handed_is_named() {
    for page in [
        "Call `dispatch_frob`.",
        "Call `mcp__purlis__frob_it` with nothing.",
        "Call `browser_frob(url)`.",
    ] {
        let found = problems(&format!("{GOOD}{page}\n"));
        assert_eq!(found.len(), 1, "{page}: {found:?}");
        assert!(found[0].contains("which no chat is handed"), "{found:?}");
    }
    // A field that is not in a tool's family is no tool, and a real one passes.
    for page in [
        "`should_trigger`",
        "`hard_floors`",
        "`secret_ask`",
        "`theme_schema`",
    ] {
        assert_eq!(
            problems(&format!("{GOOD}{page}\n")),
            Vec::<String>::new(),
            "{page}"
        );
    }
}

#[test]
fn a_description_over_the_limit_or_without_purlis_is_named() {
    let long = format!("Use purlis {}.", "a".repeat(MOST_DESCRIPTION));
    let page = format!("---\nname: purlis-example\ndescription: {long}\n---\n");
    let found = problems(&page);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("longer than 1024 characters"),
        "{found:?}"
    );

    let page =
        "---\nname: purlis-example\ndescription: Work in a repo with `purlis worktree`.\n---\n";
    assert_eq!(
        problems(page),
        [
            "purlis-example: its description never says purlis outside its code, so a person \
          asking about purlis does not find it (says purlis in its description)"
        ]
    );
    let page = "---\nname: purlis-example\n---\n";
    assert_eq!(
        problems(page),
        [
            "purlis-example: has no description, and a harness lists a skill by it (has a \
          description a harness lists)"
        ]
    );
}

#[test]
fn a_comment_a_wrapped_span_and_purlis_as_an_argument_are_read_as_a_shell_and_markdown_would() {
    for page in [
        // A shell comment in a fence is prose.
        "```bash\npurlis persona where          # in a chat purlis started: who asked\n```",
        "```bash\n# use that vault with `purlis secret frob`\n```",
        // A code span that wraps onto the next line is one span.
        "The task's own branch folder (`purlis\nworktree`). purlis cuts a branch folder for it.",
        // `purlis` given to another command is a word, not a command.
        "```bash\npurlis theme export purlis --to \"$TMPDIR/purlis.zip\"\n```",
    ] {
        assert_eq!(
            problems(&format!("{GOOD}{page}\n")),
            Vec::<String>::new(),
            "{page}"
        );
    }
    // And a command after a pipe or inside `$(…)` is still read.
    for page in [
        "`x | purlis frob`",
        "`y=$(purlis frob)`",
        "`a && purlis frob`",
    ] {
        assert_eq!(problems(&format!("{GOOD}{page}\n")).len(), 1, "{page}");
    }
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
            found[0].contains("losing side") && found[0].contains(ruling),
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

/// The settled pairs hold beyond the skills: in what every chat is told at its start, and in
/// every purlis tool's description and arguments, a chat's other two sources of purlis's words.
#[test]
fn the_briefing_and_every_tool_description_say_no_losing_side() {
    use purlis_core::briefing;
    let mut said: Vec<(String, String)> = [
        ("the sandboxed note", briefing::SANDBOXED_NOTE),
        ("the unsandboxed note", briefing::UNSANDBOXED_NOTE),
        (
            "the branch-folder note",
            briefing::BRANCH_FOLDER_COMMIT_NOTE,
        ),
        ("the one route", purlis_core::handoff::ONE_ROUTE),
        ("the one route's tools", briefing::ONE_ROUTE_TOOLS),
        ("the one route's task", briefing::ONE_ROUTE_IS_A_TASK),
        ("the one route's limit", briefing::ONE_ROUTE_ONLY_THEN),
    ]
    .into_iter()
    .map(|(what, text)| (what.to_owned(), text.to_owned()))
    .collect();
    for tool in crate::mcp::every_tool() {
        said.push((
            format!("the `{}` tool", tool.name),
            format!("{} {}", tool.description, (tool.schema)()),
        ));
    }
    let wrong: Vec<String> = said
        .iter()
        .flat_map(|(what, text)| {
            losing_sides(text)
                .into_iter()
                .map(move |ruling| format!("{what} says the losing side, against {ruling}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
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
            found[0].contains(word) && found[0].contains("(uses the glossary's words)"),
            "{found:?}"
        );
    }
    let page = "---\nname: purlis-example\ndescription: Ask the operator in purlis.\n---\n";
    assert_eq!(
        problems(page),
        [
            "purlis-example: says “operator” where the glossary says the person (uses the \
             glossary's words)"
        ]
    );
    // A tool's name, a command and a path are code, and keep their words; so does a word that
    // only contains one.
    for page in [
        "Ask the person with `ask_operator`.",
        "Run `purlis persona use operator`.",
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
        let found = problems(&format!("{GOOD}{page}\n"));
        assert_eq!(
            found,
            [format!(
                "purlis-example: names the skill `{name}`, which purlis does not ship (names a \
                 skill purlis ships)"
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

const SOURCES: &str =
    "Trigger bar: strict\nNeighbours: purlis-browser, purlis-persona\n\n1. a\n2. b\n";

fn sourced(description: &str) -> Vec<String> {
    let page = format!("---\nname: purlis-example\ndescription: {description}\n---\n");
    check(
        &[Skill::of("purlis-example", &page).with_sources(SOURCES, 2)],
        &Built::now(),
    )
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
            "purlis-example: its description has no “Not for” clause, and its sources name the \
             neighbours purlis-browser, purlis-persona (says what it is not for)"
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
            "purlis-example: its description's “Not for” clauses never name its neighbour \
             `purlis-persona` (says what it is not for)"
        ]
    );
}

#[test]
fn a_neighbour_purlis_does_not_ship_and_unreadable_sources_are_named() {
    let page =
        "---\nname: purlis-example\ndescription: Do it in purlis. Not for purlis-nope.\n---\n";
    assert_eq!(
        check(
            &[Skill::of("purlis-example", page)
                .with_sources("Trigger bar: strict\nNeighbours: purlis-nope\n1. a\n", 1)],
            &Built::now(),
        ),
        [
            "purlis-example: declares the neighbour `purlis-nope`, which purlis does not ship \
             (says what it is not for)"
        ]
    );
    assert_eq!(
        check(
            &[Skill::of("purlis-example", page).with_sources("Neighbours: none\n", 0)],
            &Built::now(),
        ),
        [
            "purlis-example: carries no trigger tests (`evals/triggers.json`), so no grade says \
             whether it starts when it should (carries trigger tests, sources and a tier)"
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
            found[0].contains(named)
                && found[0].ends_with("(never lowers the sandbox or edits a harness's settings)"),
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
fn a_built_in_without_trigger_tests_sources_or_a_tier_is_named() {
    let page = format!("{GOOD}Do it.\n");
    let rule = "(carries trigger tests, sources and a tier)";
    assert_eq!(
        check(&[Skill::of("purlis-example", &page)], &Built::now()),
        [
            format!(
                "purlis-example: carries no trigger tests (`evals/triggers.json`), so no grade \
                 says whether it starts when it should {rule}"
            ),
            format!(
                "purlis-example: has no sources file (`evals/triggers.sources.md`) naming its \
                 trigger bar, its neighbours and where each test came from {rule}"
            ),
        ]
    );
    assert_eq!(
        check(
            &[Skill::of("purlis-example", &page).with_tests(1)],
            &Built::now()
        ),
        [format!(
            "purlis-example: has no sources file (`evals/triggers.sources.md`) naming its \
             trigger bar, its neighbours and where each test came from {rule}"
        )]
    );
    assert_eq!(
        check(
            &[Skill::of("purlis-example", &page).with_sources("Neighbours: none\n1. a\n", 1)],
            &Built::now(),
        ),
        [format!(
            "purlis-example: its trigger tests' sources cannot be read: it names no trigger bar \
             (a line `Trigger bar: strict` or `Trigger bar: standard`) {rule}"
        )]
    );
    assert_eq!(
        check(
            &[Skill::of("purlis-example", &page).with_sources(MEASURED, 1)],
            &Built::now()
        ),
        Vec::<String>::new()
    );
}

#[test]
fn a_built_in_leaving_the_bundle_is_not_held_to_carry_trigger_tests() {
    let page = format!("{GOOD}Do it.\n");
    assert_eq!(
        check(&[Skill::of("purlis-building-ui", &page)], &Built::now()),
        Vec::<String>::new()
    );
}

/// What a chat is told at its start in the cases whose words are purlis's own: the gate, the
/// plane root, a record that cannot be resumed, a persona that is gone.
fn briefings() -> Vec<(String, String)> {
    use purlis_core::{active, briefing, sessionrecord};
    let dir = tempfile::tempdir().expect("a folder");
    let root = std::fs::canonicalize(dir.path()).expect("a real path");
    std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("charter.toml");
    for ws in ["alpha", "beta"] {
        std::fs::create_dir_all(root.join("workspaces").join(ws)).expect("a workspace");
        std::fs::write(
            root.join("workspaces").join(ws).join("workspace.md"),
            format!("# {ws}\n\n## Vision\n\nShip {ws}.\n"),
        )
        .expect("workspace.md");
    }
    let outside = root.parent().expect("a parent").to_path_buf();
    /// What is asked, from where, with which variables and in which permission mode.
    type Case<'a> = (
        &'a str,
        &'a std::path::Path,
        &'a [(&'a str, &'a str)],
        &'a str,
    );
    let cases: [Case; 6] = [
        ("the gate", &outside, &[], "default"),
        ("the unattended gate", &outside, &[], "bypassPermissions"),
        (
            "a chat launched at the plane root",
            &outside,
            &[(active::PLANE_ROOT_ENV, "1")],
            "default",
        ),
        ("a chat standing at the plane root", &root, &[], "default"),
        (
            "a record that cannot be resumed",
            &outside,
            &[(sessionrecord::RESUMING_ENV, "nowhere.md")],
            "default",
        ),
        (
            "a persona that is gone",
            &outside,
            &[(active::PERSONA_ENV, "ghost")],
            "default",
        ),
    ];
    cases
        .into_iter()
        .map(|(what, cwd, env, mode)| {
            let lookup = |name: &str| {
                env.iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            };
            let payload = serde_json::json!({ "permission_mode": mode });
            let ask = briefing::Ask {
                root: &root,
                cwd,
                payload: &payload,
                env: &lookup,
                now: chrono::Utc::now(),
            };
            (
                format!("the briefing of {what}"),
                briefing::parts(&ask, None).join("\n\n"),
            )
        })
        .collect()
}

/// The glossary's words hold beyond the skills: in what every chat is told at its start, and in
/// every purlis tool's description and arguments.
#[test]
fn the_briefing_and_every_tool_description_use_the_glossary_s_words() {
    use purlis_core::briefing;
    let mut said = briefings();
    assert!(
        said.iter()
            .any(|(_, text)| text.contains("Confirm the workspace")),
        "the gate was not rendered: {said:?}"
    );
    for (what, text) in [
        ("the sandboxed note", briefing::SANDBOXED_NOTE),
        ("the unsandboxed note", briefing::UNSANDBOXED_NOTE),
        (
            "the branch-folder note",
            briefing::BRANCH_FOLDER_COMMIT_NOTE,
        ),
        ("the one route", purlis_core::handoff::ONE_ROUTE),
        ("the one route's tools", briefing::ONE_ROUTE_TOOLS),
        ("the one route's task", briefing::ONE_ROUTE_IS_A_TASK),
        ("the one route's limit", briefing::ONE_ROUTE_ONLY_THEN),
    ] {
        said.push((what.to_owned(), text.to_owned()));
    }
    for tool in crate::mcp::every_tool() {
        said.push((
            format!("the `{}` tool", tool.name),
            format!("{} {}", tool.description, (tool.schema)()),
        ));
    }
    let wrong: Vec<String> = said
        .iter()
        .flat_map(|(what, text)| {
            avoided(&prose(text))
                .into_iter()
                .map(move |(word, instead)| format!("{what} says “{word}”, not {instead}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

// ---- where a fact goes, said once ----------------------------------------------------- //

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
    let commands: Vec<String> = code(text)
        .iter()
        .flat_map(|code| code.lines().map(str::to_owned).collect::<Vec<_>>())
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
        bases_written(&remember.page).len(),
        4,
        "{REMEMBER} names a way to write to only {:?}",
        bases_written(&remember.page)
    );
    let mut wrong = Vec::new();
    for skill in skills.iter().filter(|skill| skill.name != REMEMBER) {
        let bases = bases_written(&skill.page);
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
        if !skills_named(&skill.page).iter().any(|n| n == REMEMBER) {
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
