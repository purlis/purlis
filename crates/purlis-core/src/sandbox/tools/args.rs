// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The options a declared tool runs with** (#3421, the person's ruling of 2026-10-29): the
//! person controls them, with `args` beside the tool's vault, and a forge command line's
//! subcommands that show or change its credentials or settings never run with a vault's value
//! from a chat, whatever `args` says.
//!
//! ```toml
//! [sandbox.personas.devops]
//! tools = { kubectl = { vault = "devops", as = "KUBECONFIG file", key = "kubeconfig",
//!                       args = ["get", "describe", "pods", "deployments", "--namespace=*"] } }
//! ```
//!
//! **Each pattern is for one whole argument** ([`matches`]): `*` stands for any run of
//! characters, every other character for itself. A run goes ahead only where every argument
//! matches one pattern, in any order ([`permits`]); `[]` runs the tool with no argument. **An
//! option is allowed only by a pattern that spells one**: an argument starting with `-` matches
//! only a pattern starting with `-`, so `"*"` lets in operands and no option. A pattern holds no
//! place, so `"*"` lets in every operand, a command's own verb among them: a list that keeps a
//! tool to some of its commands names its operands rather than `"*"`.
//!
//! Absent, a tool runs with whatever arguments the chat gives it, as before; the built-in
//! refusals ([`refused_forge`]) apply either way.

use std::path::Path;

/// The key of a tool's table that holds its patterns.
pub const KEY: &str = "args";

/// The most patterns one tool holds: enough for a command line's own vocabulary, and short
/// enough that the line a person allows stays readable whole.
pub const MOST: usize = 64;

/// The longest pattern read, in bytes.
pub const LONGEST: usize = 256;

/// **A tool's `args`**, as written, or why they are not taken: not an array, an entry that is
/// not a string, an empty pattern or one with a control character, one longer than
/// [`LONGEST`], or more than [`MOST`] of them.
pub fn of(value: &toml::Value) -> Result<Vec<String>, String> {
    let example = "args = [\"get\", \"-n\", \"*\"]";
    let Some(list) = value.as_array() else {
        return Err(format!(
            "args is not an array of patterns — write {example}"
        ));
    };
    if list.len() > MOST {
        return Err(format!(
            "args holds {} patterns, and a tool holds at most {MOST}",
            list.len()
        ));
    }
    list.iter()
        .map(|one| {
            let Some(pattern) = one.as_str() else {
                return Err(format!(
                    "args holds {one}, which is not a string — write {example}"
                ));
            };
            if pattern.is_empty() {
                return Err(
                    "args holds an empty pattern, which no argument is written as".to_owned(),
                );
            }
            if pattern.chars().any(char::is_control) {
                return Err(format!(
                    "args holds {pattern:?}, which has a control character"
                ));
            }
            if pattern.len() > LONGEST {
                return Err(format!("args holds a pattern longer than {LONGEST} bytes"));
            }
            Ok(pattern.to_owned())
        })
        .collect()
}

/// **How `args` reads at the end of its tool's line** ([`super::Tool::whole`]): each pattern
/// quoted, so a pattern holding the words that join them is never read as another line.
pub fn spelled(args: &[String]) -> String {
    if args.is_empty() {
        return ", only with no arguments".to_owned();
    }
    format!(", only with arguments matching {}", quoted(args))
}

/// Each pattern quoted, joined with commas.
fn quoted(args: &[String]) -> String {
    let each: Vec<String> = args.iter().map(|one| format!("{one:?}")).collect();
    each.join(", ")
}

/// **Whether `arg` matches `pattern`**: the pattern's `*` stands for any run of characters (none
/// included) and every other character for itself, over the whole argument; an argument that
/// starts with `-` matches only a pattern that starts with `-`.
pub fn matches(pattern: &str, arg: &str) -> bool {
    if arg.starts_with('-') && !pattern.starts_with('-') {
        return false;
    }
    globbed(pattern.as_bytes(), arg.as_bytes())
}

/// `*`-only globbing over bytes, whole: the greedy match with one point to come back to, which
/// takes time linear in the pattern times the argument at worst.
fn globbed(pattern: &[u8], text: &[u8]) -> bool {
    let (mut p, mut t) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        if p < pattern.len() && pattern[p] == b'*' {
            star = Some((p, t));
            p += 1;
        } else if p < pattern.len() && pattern[p] == text[t] {
            p += 1;
            t += 1;
        } else if let Some((at, from)) = star {
            p = at + 1;
            t = from + 1;
            star = Some((at, from + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|b| *b == b'*')
}

/// **Whether a run of `tool` with `args` is one its declaration allows**: where it has `args`,
/// each argument matches one of them; or the sentence naming the first that does not.
pub fn permits(tool: &str, patterns: Option<&[String]>, args: &[String]) -> Result<(), String> {
    let Some(patterns) = patterns else {
        return Ok(());
    };
    let Some(odd) = args
        .iter()
        .find(|arg| !patterns.iter().any(|pattern| matches(pattern, arg)))
    else {
        return Ok(());
    };
    let allowed = if patterns.is_empty() {
        "no arguments".to_owned()
    } else {
        format!("only arguments matching {}", quoted(patterns))
    };
    Err(format!(
        "{tool} runs with {allowed}, and {odd:?} is not one of them, so nothing ran. To allow it, \
         add a pattern to the project's tools.{tool}.args and allow the persona's tools again on \
         this machine, or run the command in your own terminal, where purlis limits nothing."
    ))
}

/// The forge command lines purlis knows the credential and settings commands of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Forge {
    Gh,
    Glab,
}

impl Forge {
    fn of(name: &str) -> Option<Self> {
        match name {
            "gh" => Some(Self::Gh),
            "glab" => Some(Self::Glab),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Gh => "gh",
            Self::Glab => "glab",
        }
    }

    /// Whether `command` shows or changes the forge's credentials or the program's own
    /// settings: never run with a vault's value from a chat.
    fn refuses(self, command: &str) -> bool {
        const BOTH: &[&str] = &[
            "auth",
            "config",
            "alias",
            "extension",
            "extensions",
            "ext",
            "ssh-key",
            "gpg-key",
        ];
        const GLAB: &[&str] = &["token", "deploy-key"];
        BOTH.contains(&command) || (self == Self::Glab && GLAB.contains(&command))
    }
}

/// The options a forge's run may start with alone: each prints and runs nothing.
const ONLY_PRINTS: &[&str] = &["--help", "-h", "--version"];

/// **Why a run of a forge command line with `args` never runs with a vault's value**, if it
/// does not: by `names`, the tool's name and its program's file names as found and as their
/// links resolve, where one is `gh` or `glab` (lower case, a `.exe` aside). The run must start
/// with its command, not an option (only [`ONLY_PRINTS`] aside); and none of
/// [`Forge::refuses`], `gh repo … deploy-key`, or `api` naming a host (`--hostname`) or an
/// endpoint by a whole URL runs. Each applies with or without the tool's `args`.
pub fn refused_forge(names: &[&Path], args: &[String]) -> Option<String> {
    let forge = names
        .iter()
        .map(|name| super::file_name_of(name))
        .find_map(|name| Forge::of(&name))?;
    let terminal = "run it in your own terminal, where purlis limits nothing";
    let name = forge.name();
    let first = args.first()?;
    if first.starts_with('-') {
        if args.iter().all(|arg| ONLY_PRINTS.contains(&arg.as_str())) {
            return None;
        }
        return Some(format!(
            "{name} runs with a vault's value only when its first argument is its command, and \
             {first:?} is an option, so nothing ran: put the command first, or {terminal}."
        ));
    }
    let command = first.to_ascii_lowercase();
    let deploy_key = forge == Forge::Gh
        && command == "repo"
        && args
            .iter()
            .any(|arg| arg.eq_ignore_ascii_case("deploy-key"));
    if forge.refuses(&command) || deploy_key {
        let said = if deploy_key {
            "repo deploy-key"
        } else {
            command.as_str()
        };
        return Some(format!(
            "{name} {said} shows or changes the forge's credentials or {name}'s own settings, so \
             a chat never runs it with a vault's value; {terminal}."
        ));
    }
    if command == "api"
        && args
            .iter()
            .any(|arg| arg == "--hostname" || arg.starts_with("--hostname=") || arg.contains("://"))
    {
        return Some(format!(
            "{name} api names another host here (--hostname, or an endpoint as a whole URL), \
             and a chat runs it with a vault's value only against the forge's own; {terminal}."
        ));
    }
    None
}
