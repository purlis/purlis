// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which of a harness's installed plugins a chat really has** (#4477; ADR 0099 as amended
//! 2026-11-01). A harness's record says what is on the machine. What a chat has is what the
//! harness decides at the chat's start from its settings, read here the way it reads them.
//!
//! Claude Code, measured on 2.1.316 from the `init` event of a chat in a throwaway
//! `CLAUDE_CONFIG_DIR`:
//! - a plugin loads only where `enabledPlugins[id]` is literally `true`: absent is off, and so is
//!   a list of versions;
//! - later wins: the user's `settings.json`, the chat folder's `.claude/settings.json`, its
//!   `.claude/settings.local.json`, the repository top's `.claude/settings.local.json`, then
//!   `--settings`; a parent folder's `.claude/settings.json` is not read;
//! - `--setting-sources user` drops the two project files, both ways;
//! - an enabled plugin loads only where its marketplace is known, in
//!   `plugins/known_marketplaces.json` or a settings file's `extraKnownMarketplaces`;
//! - a running chat keeps what it started with, and a resumed one reads these again. So a chat's
//!   answer is read at its start and held (`hoststart::ChatSkills::plugins`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{ClaudeCode, Env, split};

/// **Where a chat reads its settings**, for one chat or for a chat that would start.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatFolder {
    /// The folder it starts in, where it is known.
    pub cwd: Option<PathBuf>,
    /// Whether it loads that folder's settings files: not where a sandboxed chat's start screened
    /// them out (`--setting-sources user`).
    pub reads_project: bool,
}

/// **Why a chat does not have a plugin that is on this machine.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OffWhy {
    /// The person's settings have it off, or do not name it.
    TurnedOff,
    /// The chat folder's settings turn it off.
    ProjectOff,
    /// It is installed only for another folder, and nothing here turns it on.
    ElsewhereOnly(PathBuf),
    /// It is on, and its marketplace is not known.
    MarketplaceGone(String),
    /// Only the chat folder's settings turn it on, and the chat does not read them.
    ProjectUnread,
    /// It was installed after the chat started.
    AfterStart,
    /// purlis's own `--settings` keep it off: a plugin purlis replaced.
    PurlisOff,
}

/// **One installed plugin a chat does not have.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginOff {
    /// `<name>@<marketplace>`.
    pub id: String,
    /// The name its skills are prefixed with.
    pub name: String,
    pub why: OffWhy,
}

impl PluginOff {
    /// Why, as the plugin's closing line says it: of the chat in front where `chat`, else of a
    /// chat that would start, which has nothing to restart.
    pub fn said(&self, title: &str, chat: bool) -> String {
        let name = &self.name;
        let then = if chat {
            "then restart this chat"
        } else {
            "then start a new chat"
        };
        match &self.why {
            OffWhy::TurnedOff => {
                format!("{title} has the {name} plugin turned off. Turn it on, {then}.")
            }
            OffWhy::ProjectOff => format!(
                "This project's {title} settings turn the {name} plugin off. Change \
                 enabledPlugins in its .claude settings files, {then}."
            ),
            OffWhy::ElsewhereOnly(folder) => format!(
                "The {name} plugin is installed for another folder, {}. Install it for your \
                 user to have it here, {then}.",
                folder.display()
            ),
            OffWhy::MarketplaceGone(market) => {
                format!(
                    "{title} cannot load the {name} plugin: its marketplace, {market}, is gone."
                )
            }
            OffWhy::ProjectUnread => format!(
                "Only this project's settings turn the {name} plugin on, and this chat does not \
                 read them. The chat's tab says why."
            ),
            OffWhy::AfterStart => {
                format!(
                    "The {name} plugin was installed after this chat started. Restart this chat."
                )
            }
            OffWhy::PurlisOff => {
                format!("purlis keeps the {name} plugin off: it is one purlis replaced.")
            }
        }
    }

    /// Why, in the few words its folded line has room for.
    pub fn line(&self, title: &str) -> String {
        match &self.why {
            OffWhy::TurnedOff => format!("Off in {title}"),
            OffWhy::ProjectOff => "Off in this project".to_owned(),
            OffWhy::ElsewhereOnly(folder) => format!("Installed for {}", folder.display()),
            OffWhy::MarketplaceGone(_) => format!("{title} cannot load it"),
            OffWhy::ProjectUnread => "Project settings unread".to_owned(),
            OffWhy::AfterStart => "Installed after this chat started".to_owned(),
            OffWhy::PurlisOff => "Replaced by purlis".to_owned(),
        }
    }

    /// The command that puts it right, where one does.
    pub fn command(&self) -> Option<String> {
        match &self.why {
            OffWhy::TurnedOff => Some(format!("claude plugin enable {}", self.id)),
            OffWhy::ElsewhereOnly(_) => {
                Some(format!("claude plugin install {} --scope user", self.id))
            }
            OffWhy::ProjectOff
            | OffWhy::MarketplaceGone(_)
            | OffWhy::ProjectUnread
            | OffWhy::AfterStart
            | OffWhy::PurlisOff => None,
        }
    }
}

/// A plugin a chat has or does not have, at the moment its settings were read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plugins {
    /// The name of each one it has.
    pub on: Vec<String>,
    /// Each it does not. A name with an install that is on is never here: skills are named by
    /// the plugin's name, so one copy on gives the chat that name's skills.
    pub off: Vec<PluginOff>,
}

impl Plugins {
    /// Whether the skills named under plugin `name` are not the chat's.
    pub fn is_off(&self, name: &str) -> bool {
        self.off.iter().any(|one| one.name == name)
    }
}

/// `enabledPlugins` of the settings text `text`, by id: only a literal `true` is on. A file that
/// is no JSON says nothing, as does one that is not there.
fn said_in(text: Option<&str>) -> BTreeMap<String, bool> {
    text.and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        .and_then(|doc| doc.get("enabledPlugins").cloned())
        .and_then(|plugins| plugins.as_object().cloned())
        .map(|plugins| {
            plugins
                .iter()
                .map(|(id, on)| (id.clone(), on.as_bool() == Some(true)))
                .collect()
        })
        .unwrap_or_default()
}

/// Whether the settings text `text` declares marketplace `market`.
fn declares(text: Option<&str>, market: &str) -> bool {
    text.and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        .is_some_and(|doc| {
            doc.pointer(&format!("/extraKnownMarketplaces/{market}"))
                .is_some()
        })
}

/// A file's text, or nothing where it cannot be read.
fn text_of(path: &Path) -> Option<String> {
    super::read(path).ok().flatten()
}

/// The folder a plugin is installed only for, where every install of it is for another folder.
fn only_elsewhere(installs: &serde_json::Value, cwd: Option<&Path>) -> Option<PathBuf> {
    let installs = installs.as_array()?;
    let mut folder = None;
    for install in installs {
        let scope = install.get("scope").and_then(serde_json::Value::as_str);
        let at = install
            .get("projectPath")
            .and_then(serde_json::Value::as_str);
        match (scope, at) {
            (Some("project" | "local"), Some(at)) if cwd != Some(Path::new(at)) => {
                folder = folder.or_else(|| Some(PathBuf::from(at)));
            }
            _ => return None,
        }
    }
    folder
}

/// **What a Claude Code chat in `at` has of the plugins `env`'s record holds**, or `None` where it
/// cannot tell: no folder to look in, or a record it cannot read. A record that is not there is
/// an answer: nothing is installed.
pub(super) fn claude(adapter: &ClaudeCode, env: &Env<'_>, at: &ChatFolder) -> Option<Plugins> {
    use super::Adapter as _;
    let record = adapter.record(env)?;
    let config = env.dir("CLAUDE_CONFIG_DIR", ".claude")?;
    let doc = match super::read(&record).ok()? {
        Some(text) => serde_json::from_str::<serde_json::Value>(&text).ok()?,
        None => return Some(Plugins::default()),
    };
    let user = text_of(&config.join("settings.json"));
    let project: Vec<String> = at
        .cwd
        .as_deref()
        .map(crate::harness::claude::project::files)
        .unwrap_or_default()
        .iter()
        .filter_map(|(path, _)| text_of(path))
        .collect();
    let known = text_of(&config.join("plugins/known_marketplaces.json"));
    let known_doc = known
        .as_deref()
        .map(serde_json::from_str::<serde_json::Value>);
    let user_said = said_in(user.as_deref());
    let project_said: BTreeMap<String, bool> = project
        .iter()
        .flat_map(|text| said_in(Some(text)))
        .collect();
    let mut out = Plugins::default();
    for (id, installs) in doc
        .get("plugins")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flatten()
    {
        if !super::id_ok(id) {
            continue;
        }
        let (name, market) = split(id);
        if super::purlis_s_own(adapter, id) {
            // purlis's own plugin is not the person's to read of; one it pins off is in no chat.
            if adapter.pinned().iter().any(|pin| pin.id == id && !pin.on) {
                out.off.push(PluginOff {
                    id: id.clone(),
                    name: name.to_owned(),
                    why: OffWhy::PurlisOff,
                });
            }
            continue;
        }
        let in_project = project_said.get(id).copied();
        let read_project = in_project.filter(|_| at.reads_project);
        let on = read_project
            .or_else(|| user_said.get(id).copied())
            .unwrap_or(false);
        let why = if on {
            // Files it cannot read as JSON say nothing of what is known: only a marketplace
            // nothing declares is said to be gone.
            let declared = declares(user.as_deref(), market)
                || (at.reads_project && project.iter().any(|one| declares(Some(one), market)));
            let in_file = match &known_doc {
                Some(Ok(doc)) => doc.get(market).is_some(),
                Some(Err(_)) => true,
                None => false,
            };
            (!declared && !in_file).then(|| OffWhy::MarketplaceGone(market.to_owned()))
        } else if in_project == Some(true) && !at.reads_project {
            Some(OffWhy::ProjectUnread)
        } else if read_project == Some(false) {
            Some(OffWhy::ProjectOff)
        } else if !user_said.contains_key(id)
            && let Some(folder) = only_elsewhere(installs, at.cwd.as_deref())
        {
            Some(OffWhy::ElsewhereOnly(folder))
        } else {
            Some(OffWhy::TurnedOff)
        };
        match why {
            None => out.on.push(name.to_owned()),
            Some(why) => out.off.push(PluginOff {
                id: id.clone(),
                name: name.to_owned(),
                why,
            }),
        }
    }
    let on = out.on.clone();
    out.off.retain(|one| !on.contains(&one.name));
    Some(out)
}

#[cfg(test)]
mod tests;
