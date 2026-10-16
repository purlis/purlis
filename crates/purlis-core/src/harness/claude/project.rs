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
//! **purlis's own entries are kept too**, exactly as `init` writes them
//! ([`crate::scaffold::settings`]): the harness variable in `env`, and the guard hook. A file
//! holding them and nothing else beyond [`KEPT`] is loaded as it is. Anything else under `env`
//! or `hooks`, or the same entries spelt another way, is not.
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

/// What the tab of a sandboxed chat says when purlis could not watch its project's Claude Code
/// settings for changes, so the chat runs without them ([`limited`]). Said in the window only.
pub const UNWATCHED: &str = "purlis could not watch this project's Claude Code settings for \
     changes, so this sandboxed chat runs on your own Claude Code settings and purlis's, with the \
     project's deny and ask rules. It does not load the project's CLAUDE.md, rules, skills, agents \
     or commands either. Restart the chat to try again.";

/// What a chat restarted because its project's Claude Code settings changed is told as it
/// starts again.
pub const RESCREENED: &str = "purlis restarted this chat on its conversation: this project's \
     Claude Code settings changed, and a sandboxed chat does not load them as they are now.";

/// `armed`, the words a Claude Code chat was armed with ([`super::ClaudeCode`]), started on the
/// person's settings and purlis's alone, with the project's deny and ask rules of `screened`
/// carried on its `--settings`: what [`Screened::setting_sources`] gives a project whose files
/// are not loaded, for one whose files cannot be kept watched.
///
/// **Fail closed.** Words with no `--plugin-dir` or no `--settings` JSON are refused, so the
/// chat is not started on its project's files unwatched. Words already limited are as they are.
pub fn limited(mut armed: Vec<String>, screened: &Screened) -> Result<Vec<String>, String> {
    if armed.iter().any(|word| word == "--setting-sources") {
        return Ok(armed);
    }
    let not_armed = || {
        "this Claude Code chat's settings could not be limited to yours and purlis's, so \
         nothing was started"
            .to_owned()
    };
    let at = armed
        .iter()
        .position(|word| word == "--settings")
        .map(|flag| flag + 1)
        .filter(|&at| at < armed.len())
        .ok_or_else(not_armed)?;
    let mut settings: serde_json::Value =
        serde_json::from_str(&armed[at]).map_err(|_| not_armed())?;
    let permissions = settings
        .as_object_mut()
        .ok_or_else(not_armed)?
        .entry("permissions")
        .or_insert_with(|| serde_json::json!({}));
    for (bucket, rules) in [("deny", &screened.deny), ("ask", &screened.ask)] {
        if rules.is_empty() {
            continue;
        }
        let mut had: Vec<serde_json::Value> = permissions
            .get(bucket)
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        had.extend(rules.iter().cloned().map(serde_json::Value::String));
        permissions[bucket] = serde_json::Value::Array(had);
    }
    armed[at] = settings.to_string();
    let plugin = armed
        .iter()
        .position(|word| word == "--plugin-dir")
        .map(|flag| flag + 2)
        .filter(|&after| after <= armed.len())
        .ok_or_else(not_armed)?;
    armed.splice(
        plugin..plugin,
        ["--setting-sources".to_owned(), "user".to_owned()],
    );
    Ok(armed)
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
    // Walked as written and as the folder really is: Claude Code finds the repository by its
    // real path, so a folder reached through a link still has its repository's file read.
    let real = cwd.canonicalize().ok();
    for walked in std::iter::once(cwd).chain(real.as_deref()) {
        if let Some(top) = walked.ancestors().find(|dir| dir.join(".git").exists()) {
            add(top);
            if let Some(main) = main_checkout(&top.join(".git")) {
                add(&main);
            }
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

/// What a sandboxed chat in `cwd` finds in its project's Claude Code settings files, with the
/// plugins this machine has installed for the person read from Claude Code's own record
/// ([`installed_for_the_person`]).
pub fn screen(cwd: Option<&Path>) -> Screened {
    use crate::harness_plugin::Adapter as _;
    let record = crate::harness_plugin::CLAUDE_CODE.record(&crate::harness_plugin::Env::of(&[]));
    screen_with(cwd, installed_for_the_person(record.as_deref()).as_ref())
}

/// The ids Claude Code's record at `record` (`plugins/installed_plugins.json`) holds as
/// installed at user scope; none where it is not there or cannot be read.
pub fn installed_for_the_person(record: Option<&Path>) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(record?).ok()?;
    let doc: serde_json::Value = serde_json::from_str(&text).ok()?;
    let plugins = doc.get("plugins")?.as_object()?;
    Some(
        plugins
            .iter()
            .filter(|(_, installs)| {
                installs.as_array().is_some_and(|installs| {
                    installs.iter().any(|one| {
                        one.get("scope").and_then(serde_json::Value::as_str) == Some("user")
                    })
                })
            })
            .map(|(id, _)| id.clone())
            .collect(),
    )
}

/// [`screen`] with `installed` the plugins installed for the person (none read: none).
pub fn screen_with(cwd: Option<&Path>, installed: Option<&Vec<String>>) -> Screened {
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
            Read::Settings(settings) => take(&settings, installed, &mut out),
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
fn take(
    settings: &serde_json::Map<String, serde_json::Value>,
    installed: Option<&Vec<String>>,
    out: &mut Screened,
) {
    for (key, value) in settings {
        if (key == "env" && purlis_s_own_env(value))
            || (key == "hooks" && purlis_s_own_hooks(value))
        {
            continue;
        }
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
                if !plugins_only(value, installed) {
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

/// Whether `plugins` only chooses among what the person has: each plugin it turns on is one
/// installed for the person (or one purlis itself pins on), and none it turns off is one purlis
/// pins on.
fn plugins_only(plugins: &serde_json::Value, installed: Option<&Vec<String>>) -> bool {
    use crate::harness_plugin::Adapter as _;
    let pinned_on = |id: &str| {
        crate::harness_plugin::CLAUDE_CODE
            .pinned()
            .iter()
            .any(|pin| pin.on && pin.id == id)
    };
    plugins.as_object().is_some_and(|plugins| {
        plugins.iter().all(|(id, on)| match on.as_bool() {
            Some(true) => {
                pinned_on(id) || installed.is_some_and(|installed| installed.contains(id))
            }
            Some(false) => !pinned_on(id),
            None => false,
        })
    })
}

/// The value `init` gives the harness variable for a Claude Code plane
/// ([`crate::scaffold::settings::ensure_harness_env`]).
const INIT_HARNESS: &str = "claude-code";

/// Whether `env` holds the harness variable `init` writes, under any of its spellings, with the
/// value `init` gives it, and nothing else ([`crate::scaffold::settings::ensure_harness_env`]).
fn purlis_s_own_env(env: &serde_json::Value) -> bool {
    env.as_object().is_some_and(|env| {
        env.iter().all(|(name, value)| {
            value.as_str() == Some(INIT_HARNESS)
                && crate::names::ENV_PREFIX
                    .spellings()
                    .any(|prefix| *name == format!("{prefix}HARNESS"))
        })
    })
}

/// Whether `hooks` holds the guard hook `init` writes and nothing else
/// ([`crate::scaffold::settings::guard_hook`]), under either name of purlis's command.
fn purlis_s_own_hooks(hooks: &serde_json::Value) -> bool {
    let guard = |name: &str| {
        serde_json::json!({
            "matcher": "Bash",
            "hooks": [{"type": "command", "command": format!("{name} hook pretooluse"), "timeout": 10}],
        })
    };
    let mut own: Vec<serde_json::Value> = crate::cliname::INSTALLED
        .iter()
        .map(|name| guard(name))
        .collect();
    own.push(crate::scaffold::settings::guard_hook());
    hooks.as_object().is_some_and(|hooks| {
        hooks.iter().all(|(event, entries)| {
            event == "PreToolUse"
                && entries
                    .as_array()
                    .is_some_and(|entries| entries.iter().all(|entry| own.contains(entry)))
        })
    })
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
