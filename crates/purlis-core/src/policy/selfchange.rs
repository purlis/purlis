// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat cannot change the policy that governs it** (M75 slice 16, #7025; spec §2.5, Q309
//! point 5): a chat's direct write to the plane root's `policy.cedar`, or to the `[policy]`
//! tables of the project's settings files, is held and names the rule, [`RULE`].
//!
//! **This is one of the three things outside the policy** (`fixed:policy-self-change`): no rule,
//! mode, template or lock lifts it, and the session host is not asked. A hook that reads a tool
//! call asks [`by_tool`] or [`by_command`], and a [`Held`] is refused to the chat, whatever the
//! family modes say.
//!
//! **What it reads is the call's own words, and only that**: a path the tool names, or a word of
//! a shell command, resolved against the folder the call runs in. It is a guard against a
//! chat's honest or careless call, **not a barrier**: a shell can spell a path no static reading
//! sees, so what stops a chat that tries is its sandbox, which holds the same file
//! ([`crate::sandbox::policy_file_held`]). Where the command cannot be read (an unparsed line, a
//! substitution, a string a shell runs) and it names a policy file, it is held.
//!
//! **An unsandboxed chat has no second hold.** The sandbox is what holds a chat that spells a
//! path this cannot read; a chat that runs without one (the person's choice, or a harness with
//! no backend) is held by this reading alone, and so only against an honest or careless call.
//!
//! **A branch folder's own copy is never held**: the chat edits it and commits it, and what it
//! loosens waits for the person's press (M75-14, M75-15).

use std::path::Path;

use serde_json::Value;

use crate::pypath;

/// The rule a held write names.
pub const RULE: &str = "policy-self-change";

/// The project policy's rules, at the plane root.
pub const POLICY_FILE: &str = "policy.cedar";

/// What a chat's call would write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// The plane root's `policy.cedar`.
    File,
    /// The project's settings, where `[policy]` and its tables are.
    Table,
}

impl Held {
    /// What the chat is told: the rule, what was held, and the road that is open.
    pub fn said(self) -> String {
        let what = match self {
            Held::File => "the plane root's `policy.cedar`",
            Held::Table => "the project's settings file, where `[policy]` is",
        };
        format!(
            "purlis holds a chat's write to the policy that governs it ({RULE}): {what} is \
             changed by the person alone, under every mode, so nothing was written. Change the \
             copy in your branch folder and commit it with `purlis worktree commit`: a change \
             that loosens the policy waits for the person's press in the window, and one that \
             tightens it applies once it is merged."
        )
    }
}

/// The settings files' names, under every spelling.
fn settings_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = crate::names::PLANE_MANIFEST.spellings().collect();
    names.extend(crate::names::LOCAL_SETTINGS.spellings());
    names
}

/// The plane root as the kernel names it.
fn real_root(root: &Path) -> String {
    pypath::realpath(&root.to_string_lossy())
}

/// `word` as a path, against `cwd`, with every link resolved.
fn resolved(cwd: &str, word: &str) -> String {
    if pypath::is_abs(word) {
        pypath::realpath(word)
    } else {
        pypath::realpath(&pypath::join(cwd, word))
    }
}

/// Whether two resolved paths are one, folding case where the file system does (macOS).
fn same(fold: bool, a: &str, b: &str) -> bool {
    if fold {
        a.to_lowercase() == b.to_lowercase()
    } else {
        a == b
    }
}

/// Whether this machine's file system folds case, as the sandbox's denials do.
fn folds() -> bool {
    crate::sandbox::Os::this() == crate::sandbox::Os::MacOs
}

/// What `path` (already resolved) is at the plane root `real`, where it is one of the held files.
fn held_at(fold: bool, real: &str, path: &str) -> Option<Held> {
    let at = |name: &str| {
        same(
            fold,
            path,
            &format!("{}/{name}", real.trim_end_matches('/')),
        )
    };
    if at(POLICY_FILE) {
        Some(Held::File)
    } else if settings_names().into_iter().any(at) {
        Some(Held::Table)
    } else {
        None
    }
}

/// **Whether a file tool's call writes a held file**: `tool` is `Write`, `Edit` or `MultiEdit`
/// and `paths` the files its input names, in the folder `cwd`. A write of a settings file is held
/// only where it changes `[policy]`, which is read by working out the file the tool would leave
/// ([`crate::proposed::apply_edit`]); an edit that cannot be worked out, or leaves a file that
/// does not parse, is held.
pub fn by_tool(
    root: &Path,
    cwd: &str,
    tool: &str,
    input: &Value,
    paths: &[String],
) -> Option<Held> {
    by_tool_folding(folds(), root, cwd, tool, input, paths)
}

/// [`by_tool`], with case folded or not. A tool that is not one of the three file tools is held
/// on the path alone: what it would write cannot be worked out.
fn by_tool_folding(
    fold: bool,
    root: &Path,
    cwd: &str,
    tool: &str,
    input: &Value,
    paths: &[String],
) -> Option<Held> {
    let known = matches!(tool, "Write" | "Edit" | "MultiEdit");
    let real = real_root(root);
    for path in paths {
        let at = resolved(cwd, path);
        match held_at(fold, &real, &at) {
            Some(Held::File) => return Some(Held::File),
            Some(Held::Table) if !known || changes_the_policy_table(tool, input, &at) => {
                return Some(Held::Table);
            }
            _ => {}
        }
    }
    None
}

/// Whether the edit leaves `[policy]` other than it is in the settings file at `at`. On a file
/// system that folds case `at` may be spelled other than the file is, so it is read as spelled.
fn changes_the_policy_table(tool: &str, input: &Value, at: &str) -> bool {
    let before = std::fs::read_to_string(at).ok();
    let Ok(after) = crate::proposed::apply_edit(tool, input, before.as_deref()) else {
        return true;
    };
    let Ok(after) = after.parse::<toml::Table>() else {
        return true;
    };
    let before = match before.as_deref() {
        None => toml::Table::new(),
        Some(text) => match text.parse::<toml::Table>() {
            Ok(table) => table,
            Err(_) => return true,
        },
    };
    before.get("policy") != after.get("policy")
}

/// The programs that only read what they are given.
const READERS: [&str; 17] = [
    "cat", "head", "tail", "less", "more", "bat", "grep", "egrep", "fgrep", "rg", "diff", "cmp",
    "wc", "stat", "ls", "file", "shasum",
];

/// The `git` subcommands that only read.
const READING_GIT: [&str; 8] = [
    "diff", "show", "log", "status", "blame", "ls-files", "cat-file", "grep",
];

/// Whether the invocation only reads: a reader, or `git` with a reading subcommand and no
/// `--output`. Its redirections are read apart ([`redirect_targets`]).
fn only_reads(base: &str, args: &[String]) -> bool {
    if base == "git" {
        // The subcommand is the first word that is no option and no option's value.
        let mut words = args.iter();
        let sub = loop {
            match words.next().map(String::as_str) {
                Some("-C" | "-c" | "--git-dir" | "--work-tree") => {
                    words.next();
                }
                Some(word) if word.starts_with('-') => {}
                other => break other,
            }
        };
        // `--output=<file>` (or `--output <file>`) makes a reader write: `git diff --output=f`.
        // Any `--out…` is taken so an abbreviation cannot slip by. `git` has no `-o` there.
        let writes = args.iter().any(|arg| arg.starts_with("--out"));
        return !writes && sub.is_some_and(|sub| READING_GIT.contains(&sub));
    }
    READERS.contains(&base)
}

/// Whether a word is, or starts with, a redirection (`>`, `>>`, `2>`, `&>`).
fn redirects(word: &str) -> bool {
    word.trim_start_matches(|c: char| c.is_ascii_digit() || c == '&')
        .starts_with('>')
}

/// The word without a leading redirection, and, for `--flag=value`, the value.
fn operands(word: &str) -> Vec<&str> {
    let bare = word
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '&')
        .trim_start_matches('>');
    let mut out = vec![bare];
    if let Some((_, value)) = bare.split_once('=') {
        out.push(value);
    }
    out
}

/// A word the shell would expand to a name this reads no further than naming a policy file.
fn maybe_a_policy_file(word: &str) -> bool {
    word.to_ascii_lowercase().contains("cedar") && word.contains(['$', '*', '?', '[', '{', '~'])
}

/// The marks of text a shell expands before it runs.
const EXPANDED: [&str; 5] = ["$(", "`", "<(", ">(", "<<"];

/// **Whether a shell command writes a held file.** Read word by word; a program that only reads
/// is passed; a word that resolves to a held file (or a folder that is the plane root, beside a
/// word named `policy.cedar`) is a write. A command that names a policy file and cannot be read
/// is held.
pub fn by_command(root: &Path, cwd: &str, command: &str) -> Option<Held> {
    by_command_folding(folds(), root, cwd, command)
}

/// The files a segment redirects its output to (`> f`, `>> f`, `2>f`, `&> f`, `>| f`).
fn redirect_targets(toks: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, tok) in toks.iter().enumerate() {
        if !redirects(tok) {
            continue;
        }
        let bare = tok
            .trim_start_matches(|c: char| c.is_ascii_digit() || c == '&')
            .trim_start_matches('>')
            .trim_start_matches('|');
        if !bare.is_empty() {
            out.push(bare.to_owned());
        } else if let Some(next) = toks.get(i + 1) {
            out.push(next.clone());
        }
    }
    out
}

/// [`by_command`], with case folded or not.
fn by_command_folding(fold: bool, root: &Path, cwd: &str, command: &str) -> Option<Held> {
    let lowered = command.to_ascii_lowercase();
    let policy = lowered.contains(".cedar");
    let settings = settings_names().iter().any(|name| lowered.contains(name));
    if !policy && !settings {
        return None;
    }
    let unread = if policy { Held::File } else { Held::Table };
    let (segments, parsed) = crate::shellseg::segment_argv_parsed(command);
    if !parsed || EXPANDED.iter().any(|mark| command.contains(mark)) {
        return Some(unread);
    }
    let real = real_root(root);
    let mut here = cwd.to_owned();
    for toks in &segments {
        let invocation = crate::shellwrap::split_env_chdir(toks);
        let base = crate::shellwrap::base_lower(&invocation.prog);
        let args = invocation.argv.get(1..).unwrap_or(&[]);
        if runs_a_string(&invocation, &base) {
            return Some(unread);
        }
        let at = if invocation.chdir.is_empty() {
            here.clone()
        } else {
            resolved(&here, &invocation.chdir)
        };
        // A redirection writes whatever the command is, and wherever it stands in the segment
        // (`> policy.cedar` alone has no program at all).
        if let Some(held) = redirect_targets(toks)
            .iter()
            .find_map(|word| held_at(fold, &real, &resolved(&at, word)))
        {
            return Some(held);
        }
        if matches!(base.as_str(), "cd" | "pushd") {
            if let Some(folder) = args.iter().find(|arg| !arg.starts_with('-')) {
                here = resolved(&here, folder);
            }
            continue;
        }
        if only_reads(&base, args) {
            continue;
        }
        // `git -C <dir>` runs where `<dir>` is.
        let at = if base == "git" {
            git_dir(&at, args)
        } else {
            at
        };
        let words: Vec<&str> = args.iter().flat_map(|arg| operands(arg)).collect();
        if let Some(held) = words
            .iter()
            .find_map(|word| held_at(fold, &real, &resolved(&at, word)))
        {
            return Some(held);
        }
        if words.iter().any(|word| maybe_a_policy_file(word)) {
            return Some(Held::File);
        }
        // A copy or a move into the root, of a file named as a held one: the rules, or the
        // settings (`cp x/purlis.toml {root}/`).
        let into_root = words
            .iter()
            .any(|word| same(fold, &resolved(&at, word), &real));
        let named = words.iter().find_map(|word| {
            let name = word.rsplit('/').next()?;
            if same(fold, name, POLICY_FILE) {
                Some(Held::File)
            } else if settings_names().iter().any(|one| same(fold, name, one)) {
                Some(Held::Table)
            } else {
                None
            }
        });
        if into_root && named.is_some() {
            return named;
        }
    }
    None
}

/// The folder `git` runs in: where `at` is, moved by each `-C <dir>` in turn.
fn git_dir(at: &str, args: &[String]) -> String {
    let mut here = at.to_owned();
    let mut words = args.iter();
    while let Some(word) = words.next() {
        if word == "-C"
            && let Some(dir) = words.next()
        {
            here = resolved(&here, dir);
        }
    }
    here
}

/// Whether the invocation runs a string or a script whose words are not read here.
fn runs_a_string(invocation: &crate::shellwrap::Invocation, base: &str) -> bool {
    let named = |word: &str| {
        matches!(
            word,
            "sh" | "bash" | "zsh" | "dash" | "ksh" | "fish" | "eval" | "source" | "." | "xargs"
        )
    };
    named(base)
        || invocation
            .taken
            .iter()
            .any(|word| named(crate::shellwrap::base_lower(word).as_str()))
        || !crate::shellwrap::shell_scripts_of(&invocation.prog, &invocation.argv).is_empty()
}

#[cfg(test)]
mod tests;
