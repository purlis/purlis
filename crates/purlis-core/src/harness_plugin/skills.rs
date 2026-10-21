// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where each harness finds the skills it brings on its own on this machine (SK-5, #2768), for
//! [`super::Adapter::skills_at`]. An adapter reads only its own record of what is installed, as
//! [`super::Adapter::installed`] does, and names folders; the host lists what is in them
//! ([`crate::skills::on_machine`]). Where each harness looks is the research of 2026-10-20
//! (`refs/skills-research/harness-skill-mechanics.md` in the smart-ide workspace).

use std::path::{Path, PathBuf};

use super::{Adapter, Env, split};

/// One place a harness finds skills.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillsAt {
    /// A folder of skills, a folder per skill: the person's own (`plugin` `None`), or the ones a
    /// harness names by a plugin's prefix.
    Folder {
        dir: PathBuf,
        plugin: Option<String>,
    },
    /// An installed plugin's folder. Its skills are in its `skills/`, or wherever the open plugin
    /// manifest at `manifest` inside it names under `skills`. With `versions`, the folder holds a
    /// folder per installed version, each laid out so.
    Plugin {
        plugin: String,
        dir: PathBuf,
        manifest: &'static str,
        versions: bool,
    },
    /// An installed plugin whose folder purlis cannot find: a skill named under it is said to be
    /// unchecked, never missing.
    Unlisted(String),
}

/// The prefix Claude Code names a skill synced from claude.ai by (`anthropic-skills:<name>`).
const SYNCED_PLUGIN: &str = "anthropic-skills";

/// Claude Code: the person's `skills/` under its config folder, the skills synced from claude.ai
/// in its `skills/synced/` (by their own name and by [`SYNCED_PLUGIN`]'s), and each installed
/// plugin at the `installPath` its `installed_plugins.json` records.
pub(super) fn claude(adapter: &super::ClaudeCode, env: &Env<'_>) -> Vec<SkillsAt> {
    let mut out = Vec::new();
    if let Some(config) = env.dir("CLAUDE_CONFIG_DIR", ".claude") {
        let own = config.join("skills");
        out.push(folder(own.join("synced"), Some(SYNCED_PLUGIN)));
        out.push(folder(own.join("synced"), None));
        out.push(folder(own, None));
    }
    let Some(Ok(Some(text))) = adapter.record(env).map(|record| super::read(&record)) else {
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
            out.push(SkillsAt::Unlisted(name.to_owned()));
        }
        out.extend(paths.into_iter().map(|path| SkillsAt::Plugin {
            plugin: name.to_owned(),
            dir: PathBuf::from(path),
            manifest: ".claude-plugin/plugin.json",
            versions: false,
        }));
    }
    out
}

/// Codex: `$CODEX_HOME/skills` (else `~/.codex/skills`) and `~/.agents/skills`, and each plugin
/// its `config.toml` records, in its cache at `plugins/cache/<marketplace>/<plugin>/<version>/`
/// with the manifest `.codex-plugin/plugin.json` (the cache as Codex 0.163.0 lays it out on the
/// operator's machine; the manifest's `skills` key is `core-plugins/src/manifest.rs`'s).
pub(super) fn codex(adapter: &super::Codex, env: &Env<'_>) -> Vec<SkillsAt> {
    let mut out = Vec::new();
    let home = env.dir("CODEX_HOME", ".codex");
    if let Some(home) = &home {
        out.push(folder(home.join("skills"), None));
    }
    if let Some(home) = env.home.as_deref() {
        out.push(folder(home.join(".agents/skills"), None));
    }
    for plugin in adapter.installed(env).unwrap_or_default() {
        let (name, marketplace) = split(&plugin.id);
        out.push(match &home {
            Some(home) if !marketplace.is_empty() => SkillsAt::Plugin {
                plugin: name.to_owned(),
                dir: home.join("plugins/cache").join(marketplace).join(name),
                manifest: ".codex-plugin/plugin.json",
                versions: true,
            },
            _ => SkillsAt::Unlisted(name.to_owned()),
        });
    }
    out
}

/// opencode: `~/.claude/skills` and `~/.agents/skills`, which it reads too, and `skill/` and
/// `skills/` in `~/.opencode/`, its global config folder and `$OPENCODE_CONFIG_DIR`. Its plugins
/// name no skills.
pub(super) fn opencode(adapter: &super::Opencode, env: &Env<'_>) -> Vec<SkillsAt> {
    let mut out = Vec::new();
    let mut configs = Vec::new();
    if let Some(home) = env.home.as_deref() {
        out.push(folder(home.join(".claude/skills"), None));
        out.push(folder(home.join(".agents/skills"), None));
        configs.push(home.join(".opencode"));
    }
    configs.extend(adapter.record(env));
    configs.extend(env.var("OPENCODE_CONFIG_DIR").map(PathBuf::from));
    for config in configs {
        for name in ["skill", "skills"] {
            out.push(folder(config.join(name), None));
        }
    }
    out
}

fn folder(dir: impl AsRef<Path>, plugin: Option<&str>) -> SkillsAt {
    SkillsAt::Folder {
        dir: dir.as_ref().to_path_buf(),
        plugin: plugin.map(str::to_owned),
    }
}

#[cfg(test)]
mod tests;
