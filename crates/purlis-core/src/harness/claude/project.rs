// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which of a project's own Claude Code settings a sandboxed chat loads** (ADR 0067 as
//! amended 2026-10-15).
//!
//! A sandboxed chat's sandbox and permissions come from purlis and from the person, not from
//! files in the project. So each sandboxed Claude Code chat's start reads the project settings
//! files Claude Code would load for it ([`files`]) and keeps them only while every key in them
//! is one of [`KEPT`]: rules that can only make a chat ask or refuse more, the project's
//! choice of the plugins the person installed, and how the chat looks.
//!
//! Anything else, or a file that is there and cannot be read as settings, and the chat starts
//! on the person's own settings and purlis's alone ([`Screened::setting_sources`]). Its
//! project's deny and ask rules still apply: purlis carries them on its own `--settings`
//! ([`Screened::deny`], [`Screened::ask`]). Its tab says which keys were not applied
//! ([`Screened::note`]), and that the project's instructions, skills, agents and commands are
//! not loaded either, since Claude Code loads those with the project's settings.
//!
//! A chat that is not sandboxed is not read here and loads what it always did.

use std::path::{Path, PathBuf};

/// The keys a project's settings file may hold for a sandboxed chat to load it as it is. Small
/// and explicit on purpose: a key Claude Code adds later is not kept until it is ruled in.
pub const KEPT: [&str; 6] = [
    "$schema",
    "permissions",
    "enabledPlugins",
    "model",
    "outputStyle",
    "theme",
];

/// The `permissions` rules a project's file may hold: those that only ever narrow.
pub const KEPT_RULES: [&str; 2] = ["deny", "ask"];

/// The largest settings file read. A bigger one is not read, and so not kept.
const LARGEST: u64 = 1024 * 1024;

/// How many key names a note lists before it says how many more.
const LISTED: usize = 12;

/// What a sandboxed chat's start found in its project's Claude Code settings files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Screened {
    /// Each key the files held that is not kept, by its name (`permissions.<rule>` for one under
    /// `permissions`), sorted and once each.
    pub dropped: Vec<String>,
    /// Each file that is there and could not be read as settings, as the note names it.
    pub unreadable: Vec<String>,
    /// Whether the chat's folder is known. A chat whose folder is not known is not checked, so
    /// its project's files are not loaded.
    pub unchecked: bool,
    /// The project's deny rules, as its files hold them: carried on purlis's own `--settings`
    /// when the files are not loaded.
    pub deny: Vec<String>,
    /// The project's ask rules, the same way.
    pub ask: Vec<String>,
}

impl Screened {
    /// Whether the project's own settings files are loaded as they are.
    pub fn loads_the_project(&self) -> bool {
        self.dropped.is_empty() && self.unreadable.is_empty() && !self.unchecked
    }

    /// The words that limit Claude Code to the person's own settings, or none where the
    /// project's files are loaded. `--settings` and an administrator's settings apply whatever
    /// this says.
    pub fn setting_sources(&self) -> Vec<String> {
        if self.loads_the_project() {
            Vec::new()
        } else {
            vec!["--setting-sources".to_owned(), "user".to_owned()]
        }
    }

    /// What the chat's tab says when the project's files are not loaded, and nothing otherwise.
    /// Said in the window only.
    pub fn note(&self) -> Option<String> {
        if self.loads_the_project() {
            return None;
        }
        let why = if self.unchecked {
            "purlis could not tell which folder this chat runs in, so this project's Claude Code \
             settings were not checked"
                .to_owned()
        } else {
            let mut parts = Vec::new();
            if !self.dropped.is_empty() {
                parts.push(format!(
                    "they hold settings beyond deny and ask rules ({})",
                    listed(&self.dropped)
                ));
            }
            if !self.unreadable.is_empty() {
                parts.push(format!(
                    "{} could not be read as settings",
                    listed(&self.unreadable)
                ));
            }
            format!(
                "This project's Claude Code settings are not applied in a sandboxed chat: {}",
                parts.join(", and ")
            )
        };
        Some(format!(
            "{why}. This chat runs on your own Claude Code settings and purlis's, with the \
             project's deny and ask rules. It does not load the project's CLAUDE.md, rules, \
             skills, agents or commands either. Keep only deny and ask rules, plugins, model, \
             output style or theme in the project's .claude settings, and restart the chat, to \
             load them."
        ))
    }
}

/// `names`, each as it can be shown, at most [`LISTED`] of them and how many more.
fn listed(names: &[String]) -> String {
    let shown: Vec<String> = names
        .iter()
        .take(LISTED)
        .map(String::as_str)
        .map(shown)
        .collect();
    let more = names.len().saturating_sub(LISTED);
    if more == 0 {
        shown.join(", ")
    } else {
        format!("{} and {more} more", shown.join(", "))
    }
}

/// A name read from a project's file, as a note can show it: printable characters only, and
/// not too long. The file is the project's, so what it holds is never shown as it is.
fn shown(name: &str) -> String {
    let mut out: String = name
        .chars()
        .filter(|c| c.is_ascii_graphic() || *c == ' ')
        .take(48)
        .collect();
    if out.is_empty() {
        out.push('?');
    }
    out
}

/// The project settings files Claude Code loads for a chat in `cwd`: the folder's own
/// `.claude/settings.json` and `.claude/settings.local.json`, and the local file at the top of
/// the repository the folder is in, and at the top of its main checkout where it is a linked
/// worktree. Each with the name a note gives it.
pub fn files(cwd: &Path) -> Vec<(PathBuf, String)> {
    let mut out = vec![
        (
            cwd.join(".claude/settings.json"),
            ".claude/settings.json".to_owned(),
        ),
        (
            cwd.join(".claude/settings.local.json"),
            ".claude/settings.local.json".to_owned(),
        ),
    ];
    let mut add = |dir: &Path| {
        let path = dir.join(".claude/settings.local.json");
        if !out.iter().any(|(had, _)| *had == path) {
            out.push((
                path,
                "the repository's .claude/settings.local.json".to_owned(),
            ));
        }
    };
    if let Some(top) = cwd.ancestors().find(|dir| dir.join(".git").exists()) {
        add(top);
        if let Some(main) = main_checkout(&top.join(".git")) {
            add(&main);
        }
    }
    out
}

/// The top of the main checkout a linked worktree's `.git` file points into, read from its
/// `gitdir:` line and that folder's `commondir`; none for a `.git` that is a folder.
fn main_checkout(dot_git: &Path) -> Option<PathBuf> {
    if !dot_git.is_file() {
        return None;
    }
    let text = std::fs::read_to_string(dot_git).ok()?;
    let gitdir = text.lines().find_map(|line| line.strip_prefix("gitdir:"))?;
    let gitdir = dot_git.parent()?.join(gitdir.trim());
    let common = std::fs::read_to_string(gitdir.join("commondir")).ok()?;
    let common = gitdir.join(common.trim()).canonicalize().ok()?;
    common.parent().map(Path::to_path_buf)
}

/// What a sandboxed chat in `cwd` finds in its project's Claude Code settings files.
pub fn screen(cwd: Option<&Path>) -> Screened {
    let Some(cwd) = cwd else {
        return Screened {
            unchecked: true,
            ..Screened::default()
        };
    };
    let mut out = Screened::default();
    for (path, name) in files(cwd) {
        match read(&path) {
            Read::Absent => {}
            Read::Unreadable => {
                if !out.unreadable.contains(&name) {
                    out.unreadable.push(name);
                }
            }
            Read::Settings(settings) => take(&settings, &mut out),
        }
    }
    out.dropped.sort();
    out.dropped.dedup();
    out
}

/// What one file held.
enum Read {
    Absent,
    Unreadable,
    Settings(serde_json::Map<String, serde_json::Value>),
}

fn read(path: &Path) -> Read {
    match std::fs::metadata(path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Read::Absent,
        Err(_) => return Read::Unreadable,
        Ok(meta) if !meta.is_file() || meta.len() > LARGEST => return Read::Unreadable,
        Ok(_) => {}
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return Read::Unreadable;
    };
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(serde_json::Value::Object(settings)) => Read::Settings(settings),
        _ => Read::Unreadable,
    }
}

/// Sorts what `settings` holds into `out`: each key kept or dropped, and its deny and ask rules.
fn take(settings: &serde_json::Map<String, serde_json::Value>, out: &mut Screened) {
    for (key, value) in settings {
        if !KEPT.contains(&key.as_str()) {
            out.dropped.push(key.clone());
            continue;
        }
        match key.as_str() {
            "permissions" => {
                let Some(permissions) = value.as_object() else {
                    out.dropped.push(key.clone());
                    continue;
                };
                for (rule, rules) in permissions {
                    let bucket = match rule.as_str() {
                        "deny" => &mut out.deny,
                        "ask" => &mut out.ask,
                        _ => {
                            out.dropped.push(format!("permissions.{rule}"));
                            continue;
                        }
                    };
                    match strings(rules) {
                        Some(rules) => {
                            for one in rules {
                                if !bucket.contains(&one) {
                                    bucket.push(one);
                                }
                            }
                        }
                        None => out.dropped.push(format!("permissions.{rule}")),
                    }
                }
            }
            "enabledPlugins" => {
                let plugins_only = value
                    .as_object()
                    .is_some_and(|plugins| plugins.values().all(serde_json::Value::is_boolean));
                if !plugins_only {
                    out.dropped.push(key.clone());
                }
            }
            "$schema" | "model" | "outputStyle" | "theme" => {
                if !value.is_string() {
                    out.dropped.push(key.clone());
                }
            }
            _ => out.dropped.push(key.clone()),
        }
    }
}

/// Every item of `value`, where it is a list of strings.
fn strings(value: &serde_json::Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_owned))
        .collect()
}

#[cfg(test)]
mod tests;
