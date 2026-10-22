// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The built-in lints** (SK-8, #2771; decisions 18, 30 and 58): what the live lint and the
//! install review say about a skill's files, behind the one interface a SkillLint answers by
//! (ADR 0101 §1), so SK-14 (#2777) moves them onto the capability unchanged.
//!
//! - **Its frontmatter** ([`crate::change::check`]): a whole skill that keeps its name. These
//!   [`Level::Blocks`] a save; every other finding warns.
//! - **Its description's length**: too short to tell a chat when to use it.
//! - **Keys nothing reads**, and keys that would widen a chat ([`crate::compat`]).
//! - **Broken skill references**: a `skills:<name>` or `purlis:<name>` in its text that names no
//!   skill there is.
//! - **Tools this machine lacks** (decision 30): a program a `` !`command` `` in its text or one
//!   of its scripts calls that this machine does not have. No requirement system: a warning.
//!
//! Every lint here is pure: the caller hands it the skill's files and what it may check them
//! against, and nothing here reads a disk.

use crate::compat::Field;

/// How much a finding stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// The skill is not whole, and a save is refused until it is.
    Blocks,
    /// Something a person should know, which a save does not wait on.
    Warns,
}

/// One thing a lint says about a skill. Its [`std::fmt::Display`] is the line a person reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub level: Level,
    pub says: String,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.says)
    }
}

/// A skill's files, as a lint is handed them.
#[derive(Debug, Clone, Copy)]
pub struct SkillFiles<'a> {
    /// The skill's name, as its folder holds it.
    pub name: &'a str,
    /// Its `SKILL.md`.
    pub text: &'a str,
    /// Every other text file of its folder: its path inside the folder, and its text.
    pub others: &'a [(String, String)],
    /// Whether purlis hands it over (a built-in skill or the plane's), rather than a harness
    /// reading it where it lies.
    pub handed: bool,
}

/// **A lint of a skill's files**: what the live lint and the install review ask, of the
/// built-in lints ([`BuiltInLints`]) and, once SK-14 lands, of each extension that declares
/// `skill-lint`.
pub trait SkillLint {
    /// Everything it finds in `skill`, in the order a person reads it.
    fn findings(&self, skill: &SkillFiles<'_>) -> Vec<Finding>;
}

/// The shortest description that can tell a chat when to use a skill.
pub const FEWEST_DESCRIPTION: usize = 20;

/// **purlis's own lints**, with what they check a skill against.
pub struct BuiltInLints<'a> {
    /// The namespace a skill names a plane skill by: `skills:<name>`.
    pub namespace: &'a str,
    /// The names of the plane's skills a reference may name.
    pub plane: &'a [String],
    /// The names of the built-in skills.
    pub built_in: &'a [String],
    /// Whether this machine has the program named, on its `PATH` or at the path given.
    pub has: &'a (dyn Fn(&str) -> bool + 'a),
}

impl std::fmt::Debug for BuiltInLints<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuiltInLints")
            .field("namespace", &self.namespace)
            .field("plane", &self.plane)
            .field("built_in", &self.built_in)
            .finish_non_exhaustive()
    }
}

impl SkillLint for BuiltInLints<'_> {
    fn findings(&self, skill: &SkillFiles<'_>) -> Vec<Finding> {
        let blocks = |says: String| Finding {
            level: Level::Blocks,
            says,
        };
        let mut out: Vec<Finding> = crate::change::check(skill.text, skill.name)
            .iter()
            .map(|problem| blocks(problem.to_string()))
            .collect();
        if out
            .iter()
            .any(|f| f.says == crate::change::Problem::NoFrontmatter.to_string())
        {
            return out;
        }
        out.extend(short_description(skill.text));
        out.extend(keys(skill));
        out.extend(references(skill.text, self));
        out.extend(tools_missing(skill, self.has));
        out
    }
}

fn warns(says: String) -> Finding {
    Finding {
        level: Level::Warns,
        says,
    }
}

/// A description too short to say when to use the skill.
fn short_description(text: &str) -> Option<Finding> {
    let pairs = crate::frontmatter(text);
    let description = pairs
        .iter()
        .find(|(key, _)| key == "description")
        .map(|(_, value)| value.trim())
        .filter(|value| !value.is_empty())?;
    let length = description.chars().count();
    (length < FEWEST_DESCRIPTION).then(|| {
        warns(format!(
            "its description: it is {length} characters, and a chat chooses a skill by what \
             its description says, so say what it does and when to use it"
        ))
    })
}

/// The keys nothing reads, and the ones that would widen a chat.
fn keys(skill: &SkillFiles<'_>) -> Vec<Finding> {
    let mut out = Vec::new();
    for (key, _) in crate::read::entries(skill.text) {
        match Field::of(&key) {
            None => out.push(warns(format!(
                "its frontmatter's {key}: is read by nothing that runs a skill"
            ))),
            Some(Field::AllowedTools) if skill.handed => out.push(warns(
                "its allowed-tools: would widen the chat, so purlis leaves it out: a skill may \
                 only narrow a chat, and disallowed-tools: is how it takes tools away"
                    .to_owned(),
            )),
            Some(Field::Hooks) if skill.handed => out.push(warns(
                "its hooks: would run commands in the chat, so purlis leaves them out: a skill \
                 may only narrow a chat"
                    .to_owned(),
            )),
            Some(field) if field.widens() => out.push(warns(format!(
                "its {}: widens the chat: {} apply while it runs",
                field.key(),
                field.said()
            ))),
            Some(_) => {}
        }
    }
    out
}

/// Each `<namespace>:<name>` in `text` that names no skill there is, once each.
fn references(text: &str, lints: &BuiltInLints<'_>) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let mut look = |namespace: &str, known: &[String], whose: &str| {
        for name in named_in(text, namespace) {
            let said = format!("{namespace}:{name}");
            if known.iter().any(|one| *one == name) || seen.contains(&said) {
                continue;
            }
            out.push(warns(format!(
                "it names {said}, and {whose} no skill {name}: a chat told to use it finds none"
            )));
            seen.push(said);
        }
    };
    look(lints.namespace, lints.plane, "this project has");
    for prefix in crate::BUILT_IN_PREFIXES {
        look(prefix, lints.built_in, "purlis has");
    }
    out
}

/// The names `text` gives after `namespace:`, where the namespace is not the end of a longer
/// word and a skill's name follows.
fn named_in(text: &str, namespace: &str) -> Vec<String> {
    let needle = format!("{namespace}:");
    let word = |c: char| c.is_ascii_alphanumeric() || "_.-/".contains(c);
    text.match_indices(&needle)
        .filter(|(at, _)| !text[..*at].chars().next_back().is_some_and(word))
        .filter_map(|(at, _)| {
            let rest = &text[at + needle.len()..];
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
                .collect();
            let name = name.trim_end_matches('-');
            (!name.is_empty() && crate::change::name_problem(name).is_none())
                .then(|| name.to_owned())
        })
        .collect()
}

/// Each program a `` !`command` `` in the skill's text or one of its scripts calls that this
/// machine does not have, once per place.
fn tools_missing(skill: &SkillFiles<'_>, has: &dyn Fn(&str) -> bool) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut said = |tools: Vec<String>, where_: &str| {
        let mut seen: Vec<String> = Vec::new();
        for tool in tools {
            if seen.contains(&tool) || has(&tool) {
                continue;
            }
            out.push(warns(format!(
                "{where_} calls {tool}, which this machine does not have"
            )));
            seen.push(tool);
        }
    };
    let injected: Vec<String> = injections(skill.text)
        .iter()
        .flat_map(|command| programs(command))
        .collect();
    said(injected, "its ! command");
    for (path, text) in skill.others {
        let called = script_programs(path, text);
        if !called.is_empty() {
            said(called, path);
        }
    }
    out
}

/// The commands `` !`…` `` injects into a skill's text as it is started.
fn injections(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("!`") {
        let after = &rest[at + 2..];
        let Some(end) = after.find('`') else {
            break;
        };
        out.push(&after[..end]);
        rest = &after[end + 1..];
    }
    out
}

/// The programs a script calls: its interpreter, and for a shell script every command it runs.
fn script_programs(path: &str, text: &str) -> Vec<String> {
    let first = text.lines().next().unwrap_or_default();
    let interpreter = first.strip_prefix("#!").map(|line| {
        let mut words = line.split_whitespace();
        let program = words.next().unwrap_or_default();
        if program.ends_with("/env") {
            words
                .find(|word| !word.starts_with('-'))
                .unwrap_or_default()
                .to_owned()
        } else {
            program.to_owned()
        }
    });
    let shell = match interpreter.as_deref() {
        Some(program) => ["sh", "bash", "zsh", "dash", "ksh"]
            .iter()
            .any(|shell| program == *shell || program.ends_with(&format!("/{shell}"))),
        None => [".sh", ".bash", ".zsh"]
            .iter()
            .any(|ending| path.ends_with(ending)),
    };
    if interpreter.is_none() && !shell {
        return Vec::new();
    }
    let mut out: Vec<String> = interpreter.into_iter().filter(|p| !p.is_empty()).collect();
    if shell {
        let own = functions(text);
        out.extend(
            shell_lines(text)
                .iter()
                .flat_map(|line| programs(line))
                .filter(|program| !own.contains(program)),
        );
    }
    out
}

/// The functions a shell script defines, `name() {` or `function name`, which it calls as its
/// own.
fn functions(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let name = match line.strip_prefix("function ") {
                Some(rest) => rest.split(|c: char| c.is_whitespace() || c == '(').next()?,
                None => line.split_once("()")?.0.trim(),
            };
            (!name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_-".contains(c)))
            .then(|| name.to_owned())
        })
        .collect()
}

/// A shell script's command lines: comments, here-documents and the shebang taken out, and a
/// line that ends in `\` joined to the next.
fn shell_lines(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut joining = String::new();
    let mut until: Option<String> = None;
    let mut cases = 0usize;
    for line in text.lines() {
        if let Some(end) = &until {
            if line.trim() == end {
                until = None;
            }
            continue;
        }
        let mut line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        // A function's head, `name() {`, runs nothing; nor does a `case` arm's pattern.
        if let Some(at) = line.find("()")
            && line[..at]
                .trim()
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_-".contains(c))
        {
            line = &line[at + 2..];
        }
        if line.starts_with("case ") {
            cases += 1;
        } else if line.starts_with("esac") {
            cases = cases.saturating_sub(1);
        } else if cases > 0
            && let Some(at) = line.find(')')
            && !line[..at].contains('(')
        {
            line = &line[at + 1..];
        }
        if let Some(at) = line.find("<<") {
            let word = line[at + 2..]
                .trim_start_matches(['-', '~'])
                .trim()
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_matches(['\'', '"']);
            if !word.is_empty() {
                until = Some(word.to_owned());
            }
        }
        if let Some(body) = line.strip_suffix('\\') {
            joining.push_str(body);
            joining.push(' ');
            continue;
        }
        joining.push_str(line);
        out.push(std::mem::take(&mut joining));
    }
    if !joining.is_empty() {
        out.push(joining);
    }
    out
}

/// What the shell runs no program for: its keywords and its builtins.
const SHELL_OWN: &[&str] = &[
    "if",
    "then",
    "else",
    "elif",
    "fi",
    "for",
    "while",
    "until",
    "do",
    "done",
    "case",
    "esac",
    "in",
    "function",
    "select",
    "time",
    "!",
    "{",
    "}",
    "[[",
    "]]",
    "[",
    "]",
    "test",
    "echo",
    "printf",
    "cd",
    "export",
    "local",
    "return",
    "exit",
    "set",
    "unset",
    "shift",
    "read",
    "source",
    ".",
    "eval",
    "exec",
    "trap",
    "wait",
    "true",
    "false",
    "break",
    "continue",
    "declare",
    "typeset",
    "readonly",
    "let",
    "alias",
    "unalias",
    "type",
    "command",
    "builtin",
    "hash",
    "pwd",
    "pushd",
    "popd",
    "getopts",
    "umask",
    "ulimit",
    "jobs",
    "fg",
    "bg",
    "kill",
    "shopt",
    "mapfile",
    "readarray",
    "disown",
    "caller",
    "complete",
    "enable",
    "help",
    "logout",
    "times",
    "dirs",
    "suspend",
    "fc",
    "history",
    ":",
    "noglob",
    "nohup",
    "sudo",
];

/// The shell's words a command may start with before the program it runs: `if grep …`,
/// `then jq …`, `sudo make …`.
const LEADING: &[&str] = &[
    "if", "then", "else", "elif", "while", "until", "do", "!", "time", "exec", "sudo", "nohup",
    "noglob", "builtin",
];

/// The programs one command line calls: the first word of each command a pipe, a list or a
/// substitution starts, past what sets a variable for it and the shell's leading words. A
/// variable, an option, a redirection, the skill's own file (`./`, a `$` path) and the shell's
/// own words are no program.
fn programs(line: &str) -> Vec<String> {
    let line = match line.find(" #") {
        Some(at) => &line[..at],
        None => line,
    };
    line.split(['|', ';', '&', '(', ')', '`', '\n'])
        .filter_map(|command| {
            let word = command
                .split_whitespace()
                .map(|word| word.trim_matches(['"', '\'']))
                .find(|word| {
                    !(word.contains('=') && !word.starts_with('=')) && !LEADING.contains(word)
                })?;
            let calls = !word.is_empty()
                && !word.starts_with(['$', '-', '.', '~', '{', '#', '*'])
                && !word.ends_with(['{', '}'])
                && !word.contains(['$', '*', '?', '[', ']', '=', '<', '>'])
                && !word.chars().all(|c| c.is_ascii_digit())
                && !SHELL_OWN.contains(&word);
            calls.then(|| word.to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests;
