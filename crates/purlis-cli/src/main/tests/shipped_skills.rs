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

/// One built-in skill as it ships: its folder's name, its page, and its description.
struct Skill {
    name: String,
    page: String,
    description: Option<String>,
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
        }
    }
}

/// What a rule checks a skill against: this build's parser and its chat tools.
struct Built {
    cli: clap::Command,
    tools: BTreeSet<&'static str>,
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
];

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
            out.push(uncommented(line).to_owned());
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

/// The words of a shell command, quotes kept on a quoted word so it reads as a value, up to the
/// command's end.
fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(open) => {
                word.push(c);
                if c == open {
                    quote = None;
                }
            }
            None if c == '"' || c == '\'' => {
                word.push(c);
                quote = Some(c);
            }
            None if c.is_whitespace() => {
                if !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
            }
            None if "|;&<>)".contains(c) || (c == '#' && word.is_empty()) => break,
            None if c == '\\' && chars.peek().is_none_or(|n| n.is_whitespace()) => {}
            None => word.push(c),
        }
    }
    // A quote left open is a value cut off, and says nothing.
    if !word.is_empty() && quote.is_none() {
        out.push(word);
    }
    out
}

/// Whether `word` stands for a value the reader fills in, so says nothing of the parser: a
/// `<placeholder>`, a quoted or `$` value, an ellipsis, an `[optional]` or `{a,b}` part.
fn placeholder(word: &str) -> bool {
    word.contains(['<', '[', '{', '$', '"', '\'', '…', '*']) || word.contains("...")
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
            Skill::of(&name, &page)
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

fn problems(page: &str) -> Vec<String> {
    check(&[Skill::of("purlis-example", page)], &Built::now())
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
