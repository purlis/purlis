// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The skills each harness brings on its own on this machine (SK-5, #2768), for
//! [`super::Adapter::skills`]: what a skill a persona names is checked against. Read, never
//! written. Where each harness looks is the research of 2026-10-20
//! (`refs/skills-research/harness-skill-mechanics.md` in the smart-ide workspace).

use std::path::Path;

use purlis_skills::OnMachine;

use super::{Adapter, Env, split};

/// The prefix Claude Code names a skill synced from claude.ai by (`anthropic-skills:<name>`).
const SYNCED_PLUGIN: &str = "anthropic-skills";

/// Claude Code: the person's `skills/` under its config folder (each also by
/// [`SYNCED_PLUGIN`] where it sits in `skills/synced/`), and each installed plugin's `skills/`,
/// from its `installPath` in `installed_plugins.json`. A plugin with no path to read is there as a
/// whole.
pub(super) fn claude(adapter: &super::ClaudeCode, env: &Env<'_>) -> Vec<OnMachine> {
    let mut out = Vec::new();
    if let Some(config) = env.dir("CLAUDE_CONFIG_DIR", ".claude") {
        let own = config.join("skills");
        out.extend(in_folder(&own, None));
        out.extend(in_folder(&own.join("synced"), None));
        out.extend(in_folder(&own.join("synced"), Some(SYNCED_PLUGIN)));
    }
    let Some(record) = adapter.record(env) else {
        return out;
    };
    let Ok(Some(text)) = super::read(&record) else {
        return out;
    };
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(&text) else {
        return out;
    };
    for (id, installs) in doc
        .get("plugins")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flatten()
    {
        let (name, _) = split(id);
        let paths: Vec<&str> = installs
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|one| one.get("installPath").and_then(serde_json::Value::as_str))
            .collect();
        if paths.is_empty() {
            out.push(OnMachine::Plugin(name.to_owned()));
        }
        for path in paths {
            out.extend(in_folder(&Path::new(path).join("skills"), Some(name)));
        }
    }
    out
}

/// Codex: `$CODEX_HOME/skills` (else `~/.codex/skills`) and `~/.agents/skills`, and each plugin
/// its `config.toml` records, there as a whole: where Codex keeps a plugin's files is not read.
pub(super) fn codex(adapter: &super::Codex, env: &Env<'_>) -> Vec<OnMachine> {
    let mut out = Vec::new();
    if let Some(home) = env.dir("CODEX_HOME", ".codex") {
        out.extend(in_folder(&home.join("skills"), None));
    }
    if let Some(home) = env.home.as_deref() {
        out.extend(in_folder(&home.join(".agents/skills"), None));
    }
    out.extend(
        adapter
            .installed(env)
            .unwrap_or_default()
            .into_iter()
            .map(|plugin| OnMachine::Plugin(plugin.name)),
    );
    out
}

/// opencode: `~/.claude/skills` and `~/.agents/skills`, which it reads too, and `skill/` and
/// `skills/` in its global config folder, in `~/.opencode/` and in `$OPENCODE_CONFIG_DIR`. Its
/// plugins name no skills.
pub(super) fn opencode(adapter: &super::Opencode, env: &Env<'_>) -> Vec<OnMachine> {
    let mut out = Vec::new();
    let mut configs = Vec::new();
    if let Some(home) = env.home.as_deref() {
        out.extend(in_folder(&home.join(".claude/skills"), None));
        out.extend(in_folder(&home.join(".agents/skills"), None));
        configs.push(home.join(".opencode"));
    }
    configs.extend(adapter.record(env));
    configs.extend(env.var("OPENCODE_CONFIG_DIR").map(std::path::PathBuf::from));
    for config in configs {
        for folder in ["skill", "skills"] {
            out.extend(in_folder(&config.join(folder), None));
        }
    }
    out
}

/// Every skill in `dir`, by its folder's name and by the name its `SKILL.md` gives where that
/// differs, as `plugin`'s.
fn in_folder(dir: &Path, plugin: Option<&str>) -> Vec<OnMachine> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for folder in entries.filter_map(Result::ok).map(|entry| entry.path()) {
        let Ok(text) = std::fs::read_to_string(folder.join("SKILL.md")) else {
            continue;
        };
        let mut names: Vec<String> = folder
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .into_iter()
            .collect();
        if let Some((_, name)) = purlis_skills::frontmatter(&text)
            .into_iter()
            .find(|(key, value)| key == "name" && !value.is_empty())
            && !names.contains(&name)
        {
            names.push(name);
        }
        out.extend(names.into_iter().map(|name| OnMachine::Skill {
            plugin: plugin.map(str::to_owned),
            name,
        }));
    }
    out
}

#[cfg(test)]
mod tests;
