// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The harness skills a chat's harness has on this machine** (SK-5, #2768): the host's half of
//! looking. The harness's declaration says where it finds skills
//! ([`crate::harness_plugin::Adapter::skills_at`]); this lists the skills in each place, read and
//! never written. What a skill a persona names is checked against.

use std::path::{Path, PathBuf};

use crate::harness_plugin::{Env, SkillsAt};

use super::OnMachine;

/// The folder a plugin's skills are in when its manifest names none.
const DEFAULT_SKILLS: &str = "skills";

/// Every harness skill `harness` has on this machine under `env`.
pub fn on_machine(harness: crate::harness::Harness, env: &Env<'_>) -> Vec<OnMachine> {
    listed(&harness.adapter().plugins().skills_at(env))
}

/// Every skill in `at`, the places a harness finds skills.
pub(crate) fn listed(at: &[SkillsAt]) -> Vec<OnMachine> {
    let mut out = Vec::new();
    for place in at {
        match place {
            SkillsAt::Folder { dir, plugin } => out.extend(in_folder(dir, plugin.as_deref())),
            SkillsAt::Unlisted(plugin) => out.push(OnMachine::Unlisted(plugin.clone())),
            SkillsAt::Plugin {
                plugin,
                dir,
                manifest,
                versions,
            } => {
                let roots = if *versions {
                    folders(dir)
                } else {
                    Some(dir.clone())
                        .filter(|dir| dir.is_dir())
                        .into_iter()
                        .collect()
                };
                if roots.is_empty() {
                    out.push(OnMachine::Unlisted(plugin.clone()));
                }
                for root in roots {
                    for skills in skill_dirs(&root, manifest) {
                        out.extend(in_folder(&root.join(skills), Some(plugin)));
                    }
                }
            }
        }
    }
    out
}

/// The folders a plugin at `root` keeps its skills in: what its manifest names under `skills`, a
/// path or a list of them, else `skills/`.
fn skill_dirs(root: &Path, manifest: &str) -> Vec<String> {
    let named = std::fs::read_to_string(root.join(manifest))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|doc| match doc.get("skills")? {
            serde_json::Value::String(one) => Some(vec![one.clone()]),
            serde_json::Value::Array(many) => Some(
                many.iter()
                    .filter_map(|one| one.as_str().map(str::to_owned))
                    .collect(),
            ),
            _ => None,
        });
    named.unwrap_or_else(|| vec![DEFAULT_SKILLS.to_owned()])
}

/// Every folder in `dir`, by name.
fn folders(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    out.sort();
    out
}

/// Every skill in `dir`, by its folder's name and by the name its `SKILL.md` gives where that
/// differs, as `plugin`'s.
fn in_folder(dir: &Path, plugin: Option<&str>) -> Vec<OnMachine> {
    let mut out = Vec::new();
    for folder in folders(dir) {
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
