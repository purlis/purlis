// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The built-in lints** (SK-8, #2771; decisions 18, 30 and 58): what the live lint and the
//! install review say about a skill's files, behind the one interface a SkillLint answers by
//! (ADR 0101 §1), so SK-14 (#2777) moves them onto the capability unchanged.
//!
//! - **Its frontmatter** ([`crate::change::check`]): a whole skill that keeps its name. These
//!   [`Severity::Blocks`] a save, as does a frontmatter purlis cannot read as YAML; every other
//!   finding warns.
//! - **Its description's length**: too short to tell a chat when to use it.
//! - **Keys nothing reads**, and keys that would widen a chat ([`crate::compat`]).
//! - **Broken skill references**: a `skills:<name>` or `purlis:<name>` in its text that names no
//!   skill there is.
//! - **A built-in skill's name** (SK-37, #3027): a harness or repo skill named like one of
//!   purlis's, which its program loads beside purlis's, so a chat may use either.
//! - **Tools this machine lacks** (decision 30): a program a `` !`command` `` in its text or one
//!   of its scripts calls that this machine does not have. No requirement system: a warning.
//!
//! Every lint here is pure: the caller hands it the skill's files and what it may check them
//! against, and nothing here reads a disk.

use crate::change::Problem;
use crate::compat::{Field, widening_said};
use crate::handed::Refusal;

/// How much a finding stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The skill is not whole, and a save is refused until it is.
    Blocks,
    /// Something a person should know, which a save does not wait on.
    Warns,
}

/// What a lint found.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Lint {
    /// Its frontmatter is not a whole skill that keeps its name ([`crate::change::check`]).
    Frontmatter(Problem),
    /// purlis cannot read its frontmatter, so hands it to no chat, or, for a harness or repo
    /// skill, cannot tell what it asks for.
    Refused(Refusal),
    /// Its description is this many characters, too few to say when to use it.
    ShortDescription(usize),
    /// It sets a key, as written, that nothing reads.
    Unread(String),
    /// It asks for what would widen the chat, which purlis leaves out of a skill it hands.
    LeftOut(Field),
    /// A harness or repo skill that asks for what widens the chat.
    Widens(Vec<Field>),
    /// It names a skill, as written, that there is none of: by its name, and whether it named
    /// one of the project's (or a built-in one).
    NoSuchSkill {
        said: String,
        name: String,
        project: bool,
    },
    /// A harness or repo skill with the name of one of purlis's built-in skills (SK-37): its
    /// program loads it beside purlis's, and purlis cannot stop it.
    BuiltInName,
    /// A program it calls that this machine does not have, and where it calls it.
    Missing { tool: String, place: Place },
    /// What another lint said, a line of its own words, and whose lint it is: an extension that
    /// declares `skill-lint` (ADR 0101). Always a warning: a lint of a third party's never stops
    /// a save.
    Said { by: String, says: String },
}

/// Where a skill calls a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// In a `` !`command` `` of its text.
    Injected,
    /// In one of its scripts, by its path inside the folder.
    Script(String),
}

/// One thing a lint says about a skill. Its [`std::fmt::Display`] is the line the editor shows,
/// which may name a key, as the editor is where keys are typed; [`Finding::shown`] is the line
/// anywhere else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    pub lint: Lint,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.lint {
            Lint::Frontmatter(problem) => write!(f, "{problem}"),
            Lint::Refused(why) if self.severity == Severity::Blocks => {
                write!(f, "purlis hands it to no chat: {why}")
            }
            Lint::LeftOut(Field::Hooks) => f.write_str(
                "its hooks: would run commands in the chat, so purlis leaves them out: a skill \
                 may only narrow a chat",
            ),
            Lint::LeftOut(field) => write!(
                f,
                "its {}: would widen the chat, so purlis leaves it out: a skill may only narrow \
                 a chat, and disallowed-tools: is how it takes tools away",
                field.key()
            ),
            Lint::Unread(key) => {
                write!(
                    f,
                    "its frontmatter's {key}: is read by nothing that runs a skill"
                )
            }
            _ => f.write_str(&self.shown().unwrap_or_default()),
        }
    }
}

impl Finding {
    /// **The line anywhere but the editor**, naming no key, or `None` for a finding only the
    /// editor shows (one about a key as typed).
    pub fn shown(&self) -> Option<String> {
        Some(match &self.lint {
            Lint::Frontmatter(_) | Lint::LeftOut(_) | Lint::Unread(_) => return None,
            Lint::Refused(why) if self.severity == Severity::Blocks => {
                format!("purlis hands it to no chat: {why}")
            }
            Lint::Refused(why) => {
                format!("It may widen the chat: purlis cannot read it, as {why}.")
            }
            Lint::ShortDescription(length) => format!(
                "its description: it is {length} characters, and a chat chooses a skill by what \
                 its description says, so say what it does and when to use it"
            ),
            Lint::Widens(fields) => widening_said(fields),
            Lint::NoSuchSkill {
                said,
                name,
                project,
            } => format!(
                "it names {said}, and {} no skill {name}: a chat told to use it finds none",
                if *project {
                    "this project has"
                } else {
                    "purlis has"
                }
            ),
            Lint::BuiltInName => "it has the name of one of purlis's built-in skills, so a chat \
                                  gets purlis's too and may use either: rename it"
                .to_owned(),
            Lint::Said { by, says } => format!("{by}: {says}"),
            Lint::Missing { tool, place } => format!(
                "{} calls {tool}, which this machine does not have",
                match place {
                    Place::Injected => "its ! command",
                    Place::Script(path) => path,
                }
            ),
        })
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
    /// Where it comes from: purlis hands a built-in or a plane skill over, and a harness reads
    /// a harness or repo skill where it lies ([`crate::Origin::handed`]).
    pub origin: crate::Origin,
    /// The plugin a harness skill comes in, which the harness knows it by the name of
    /// (`<plugin>:<name>`), or `None` for any other skill.
    pub plugin: Option<&'a str>,
}

/// **A lint of a skill's files** (the SkillLint capability, ADR 0101): what the live lint asks, of
/// the built-in lints ([`BuiltInLints`]) and of each extension that declares `skill-lint`, which
/// answers through this same interface.
pub trait SkillLint {
    /// Everything it finds in `skill`, in the order a person reads it.
    fn findings(&self, skill: &SkillFiles<'_>) -> Vec<Finding>;
}

/// The shortest description that can tell a chat when to use a skill.
const FEWEST_DESCRIPTION: usize = 20;

/// **purlis's own lints**, with what they check a skill against.
pub struct BuiltInLints<'a> {
    /// The namespace a skill names a plane skill by: `skills:<name>`.
    pub namespace: &'a str,
    /// The names of the plane's skills a reference may name, beside the skill's own: a set, made
    /// once for every skill a list lints.
    pub plane: &'a std::collections::HashSet<String>,
    /// The names of the built-in skills.
    pub built_in: &'a std::collections::HashSet<String>,
    /// The names the program a skill from outside the plane is read by knows purlis's own
    /// skills by ([`crate::Route::built_in_names`]): what such a skill named so loads beside.
    pub known_as: &'a std::collections::HashSet<String>,
    /// Whether this machine has the program named, on its `PATH` or at the path given.
    pub has: &'a (dyn Fn(&str) -> bool + 'a),
}

impl std::fmt::Debug for BuiltInLints<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuiltInLints")
            .field("namespace", &self.namespace)
            .field("plane", &self.plane)
            .field("built_in", &self.built_in)
            .field("known_as", &self.known_as)
            .finish_non_exhaustive()
    }
}

impl SkillLint for BuiltInLints<'_> {
    fn findings(&self, skill: &SkillFiles<'_>) -> Vec<Finding> {
        let problems = crate::change::check(skill.text, skill.name);
        let whole_frontmatter = !problems.contains(&Problem::NoFrontmatter);
        let mut out: Vec<Finding> = problems
            .into_iter()
            .map(|problem| Finding {
                severity: Severity::Blocks,
                lint: Lint::Frontmatter(problem),
            })
            .collect();
        if !whole_frontmatter {
            return out;
        }
        out.extend(short_description(skill.text));
        out.extend(built_in_name(skill, self.known_as));
        out.extend(keys(skill));
        out.extend(references(skill, self));
        out.extend(tools_missing(skill, self.has));
        out
    }
}

fn warns(lint: Lint) -> Finding {
    Finding {
        severity: Severity::Warns,
        lint,
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
    (length < FEWEST_DESCRIPTION).then(|| warns(Lint::ShortDescription(length)))
}

/// A skill purlis does not hand, named as its program knows one of purlis's own skills
/// (`known_as`): it reads it beside purlis's, which no copy replaces and purlis never turns off
/// ([`crate::SkillSet::with_outside`]). A plugin's skill is known by its plugin's name too, so
/// it never takes one of those names.
fn built_in_name(
    skill: &SkillFiles<'_>,
    known_as: &std::collections::HashSet<String>,
) -> Option<Finding> {
    (!skill.origin.handed() && skill.plugin.is_none() && known_as.contains(skill.name))
        .then(|| warns(Lint::BuiltInName))
}

/// What its frontmatter, read as YAML, says: refused where purlis cannot read it (which blocks a
/// skill purlis hands), each key nothing reads, and what would widen the chat.
fn keys(skill: &SkillFiles<'_>) -> Vec<Finding> {
    let handed = skill.origin.handed();
    let read = match crate::handed::parse(skill.text) {
        Ok(read) => read,
        Err(why) => {
            return vec![Finding {
                severity: if handed {
                    Severity::Blocks
                } else {
                    Severity::Warns
                },
                lint: Lint::Refused(why),
            }];
        }
    };
    let mut out: Vec<Finding> = read
        .unknown()
        .into_iter()
        .map(|key| warns(Lint::Unread(key)))
        .collect();
    let widening: Vec<Field> = read
        .fields()
        .into_iter()
        .filter(|field| field.widens())
        .collect();
    if handed {
        out.extend(
            widening
                .into_iter()
                .map(|field| warns(Lint::LeftOut(field))),
        );
    } else if !widening.is_empty() {
        out.push(warns(Lint::Widens(widening)));
    }
    out
}

/// Each `<namespace>:<name>` in the skill's text that names no skill there is, once each: the
/// broken uses of the one reader of references ([`crate::uses::uses_in`]). A skill naming itself
/// names one there is, wherever it is from.
fn references(skill: &SkillFiles<'_>, lints: &BuiltInLints<'_>) -> Vec<Finding> {
    crate::uses::uses_in(
        skill.text,
        lints.namespace,
        &|one| one == skill.name || lints.plane.contains(one),
        &|one| lints.built_in.contains(one),
    )
    .into_iter()
    .filter(|one| one.broken)
    .map(|one| {
        warns(Lint::NoSuchSkill {
            said: one.said,
            name: one.name,
            project: one.project,
        })
    })
    .collect()
}

/// Each program a `` !`command` `` in the skill's text or one of its scripts calls that this
/// machine does not have, once per place.
fn tools_missing(skill: &SkillFiles<'_>, has: &dyn Fn(&str) -> bool) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut said = |tools: Vec<String>, place: Place| {
        let mut seen: Vec<String> = Vec::new();
        for tool in tools {
            if seen.contains(&tool) || has(&tool) {
                continue;
            }
            seen.push(tool.clone());
            out.push(warns(Lint::Missing {
                tool,
                place: place.clone(),
            }));
        }
    };
    let injected: Vec<String> = injections(skill.text)
        .iter()
        .flat_map(|command| programs(command))
        .collect();
    said(injected, Place::Injected);
    for (path, text) in skill.others {
        let called = script_programs(path, text);
        if !called.is_empty() {
            said(called, Place::Script(path.clone()));
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
