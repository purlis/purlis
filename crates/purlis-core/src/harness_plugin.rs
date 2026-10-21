// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Which of a harness's own plugins a project has on (charter-app#274, ADR 0050).
//!
//! **Harness-neutral, with one adapter per harness.** A [`Plugin`] is what one harness has
//! installed on this machine, by the id that harness itself uses. A project says, per harness and
//! per plugin, on or off, in `[harness_plugins.<harness>]` of `charter.toml` (Shared) and
//! `charter.local.toml` (Local). An [`Adapter`] knows one harness: what it has installed, whether
//! it can take a set of plugins for one chat, and which plugins charter fixes whatever a file says.
//!
//! # The precedence, and where it comes from
//!
//! #272's ([`crate::extension::project`], ADR 0048), per key: **Local, then the workspace, then
//! Shared**. A chat in a workspace also reads `settings.harness_plugins.<harness>` in that
//! workspace's `workspace.json` (charter-app#282, [`Choices::read_in`]), the layer between the
//! project's two files, through the same reader. With no layer naming a plugin it is **not
//! set**. Not set means the harness decides the way it always did, from its own user and project
//! settings. This is the one place it differs from an extension, which is on by default. A
//! harness plugin is not charter's to turn on: it is the operator's own install, and charter says
//! nothing about it until a project does.
//!
//! **What this machine has installed comes first**, the way this machine's approval comes first
//! for an extension. A file that names a plugin this machine does not have is listed as such and
//! handed to nothing. A file can choose among installed plugins; it cannot install one.
//!
//! **Pins come before everything** ([`Adapter::pinned`]). Claude Code's are written into every
//! chat's `--settings`. `purlis@inline` is always on, because it carries the hooks and the
//! Bash guard, and a file a chat can write must not be able to switch the guard off.
//! `charter@charter` is always off, by the operator's ruling of 2026-09-23. Every id the bundled
//! plugin had before it was renamed (`charter-app@inline` until #406, `charter@inline` and the
//! installed `charter@charter-app` until #1266) is always off too, so a file that still names
//! one is told the new id, and an old copy never loads beside the new one. A file
//! that says otherwise is refused by the settings tab's save ([`refusals`]) and, if it gets there
//! another way, ignored with a sentence ([`Effective::ignored`]).
//!
//! # A harness whose adapter cannot apply
//!
//! It still lists what it has installed, and says [`not_supported`]: "plugins for <harness> are not
//! supported yet", with the measured reason (the sentence is ADR 0050's). The "yet" waits on the
//! harness gaining a per-session switch; charter's own channel to every harness is the MCP server,
//! HP-7 (#674), which superseded per-project plugins for Codex and opencode. A chat on it is handed
//! nothing, and every choice a file makes for it is said to be ignored. It is never left out of the
//! list.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::extension::project::{Source, toml_key};
use crate::profiles::{COMMITTED_FILE, LOCAL_FILE};
use crate::settings::Refusal;

/// The table both files hold choices in: `[harness_plugins.<harness>]`, `"<id>" = true|false`.
pub const TABLE: &str = "harness_plugins";

/// What one chat is handed: each plugin on (`true`) or off (`false`), by the harness's own id.
/// A plugin it does not name is left to the harness.
pub type Chosen = BTreeMap<String, bool>;

/// One plugin a harness has installed on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    /// The harness's own id for it, which is what a file names and what the harness is handed.
    pub id: String,
    /// The plane's word for the harness: a profile's `kind`.
    pub harness: &'static str,
    /// What a person calls it.
    pub name: String,
    /// Where charter read it from, in a few words.
    pub source: String,
}

/// Whether an adapter can hand one chat a set of plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// For one chat alone, with nothing written anywhere.
    PerChat,
    /// Not yet, and the measured reason, as the end of a sentence.
    NotYet(&'static str),
}

/// A plugin charter fixes whatever a project file says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pin {
    pub id: &'static str,
    pub on: bool,
    /// Why, as a clause that starts with the id: "<id> is always on: …".
    pub why: &'static str,
}

/// Where an adapter reads the harness's own files from: the chat's environment first, because
/// a profile may point a harness at another account's directory (`CLAUDE_CONFIG_DIR`,
/// `CODEX_HOME`), then this process's, then the home directory.
#[derive(Debug, Clone)]
pub struct Env<'a> {
    /// The chat's own variables, as the profile and charter set them.
    pub chat: &'a [(String, String)],
    pub home: Option<PathBuf>,
    /// Whether a variable the chat does not set is looked up in this process. Off in a test,
    /// so the machine running it cannot reach the answer.
    pub process: bool,
}

impl<'a> Env<'a> {
    /// A chat's environment, over this process's.
    pub fn of(chat: &'a [(String, String)]) -> Self {
        Self {
            chat,
            home: crate::profiles::home(),
            process: true,
        }
    }

    fn var(&self, name: &str) -> Option<String> {
        crate::envvar::lookup(name, |name| {
            self.chat
                .iter()
                .rev()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
                .or_else(|| self.process.then(|| std::env::var(name).ok()).flatten())
        })
        .filter(|value| !value.is_empty())
    }

    /// `name`'s directory, or `fallback` under the home directory.
    fn dir(&self, name: &str, fallback: &str) -> Option<PathBuf> {
        self.var(name)
            .map(PathBuf::from)
            .or_else(|| self.home.as_ref().map(|home| home.join(fallback)))
    }
}

/// One harness, as far as its plugins go.
pub trait Adapter: Sync {
    /// The plane's word for it: a profile's `kind`, and the key under [`TABLE`].
    fn harness(&self) -> &'static str;
    /// What a person calls it: its built-in declaration's `title`
    /// ([`crate::harness::Harness::title`]), the one place a title is written.
    fn title(&self) -> &'static str;
    /// Where it records what it has installed, under `env`: what the settings tab names, so a
    /// listing is never mistaken for another account's.
    fn record(&self, env: &Env<'_>) -> Option<PathBuf>;
    /// Everything it has installed on this machine, read from [`Self::record`] and never
    /// written, or why it could not be read.
    fn installed(&self, env: &Env<'_>) -> Result<Vec<Plugin>, String>;
    /// The skills it brings on its own on this machine, under `env`: the person's skill folders
    /// and its plugins' skills, read and never written (SK-5, #2768). What a skill a persona
    /// names is checked against, so a chat that starts without one says so. A plugin whose skills
    /// cannot be listed is there as a whole, so nothing it has is said to be missing.
    fn skills(&self, env: &Env<'_>) -> Vec<purlis_skills::OnMachine>;
    /// What the project at `root` holds of its own that the harness loads beside what this
    /// machine installed, read and never written: listed so a reader sees it, and never
    /// handed to a chat. Nothing, for a harness that loads nothing from a project.
    fn in_project(&self, _root: &Path) -> Vec<Plugin> {
        Vec::new()
    }
    /// What the workspace `workspace` of the project at `root` holds of its own that the
    /// harness loads for a chat started there, beside [`Self::in_project`]: read the same way,
    /// listed the same way. Nothing, for a harness that loads nothing from a workspace.
    fn in_workspace(&self, _root: &Path, _workspace: &str) -> Vec<Plugin> {
        Vec::new()
    }
    fn support(&self) -> Support;
    /// What charter fixes for it, whatever a file says.
    ///
    /// The default is an empty slice, and cargo-mutants' `Vec::leak(Vec::new())` is an empty
    /// slice too: every caller only iterates it, so no input tells them apart
    /// (`.cargo/mutants.toml` excludes that one mutant).
    fn pinned(&self) -> &'static [Pin] {
        &[]
    }
}

/// Every harness charter knows, in the registry's order ([`crate::profiles::KINDS`]).
///
/// Each kind in [`crate::profiles::KINDS`], made a harness by
/// [`crate::harness::Harness::of_kind`]: its plugin adapter is what its
/// [`crate::harness::HarnessAdapter::plugins`] names. This module keeps no list of harnesses of
/// its own, but charter keeps more than one: `KINDS`, `of_kind`'s match and
/// [`crate::harness::Harness::ALL`] each name every harness, and a harness added to one is
/// added to each.
pub fn adapters() -> impl Iterator<Item = &'static dyn Adapter> {
    crate::profiles::KINDS
        .iter()
        .filter_map(|kind| adapter(kind.word))
}

/// The adapter for the harness `kind` names.
pub fn adapter(kind: &str) -> Option<&'static dyn Adapter> {
    crate::harness::Harness::of_kind(kind).map(|harness| harness.adapter().plugins())
}

/// "plugins for <harness> are not supported yet — <why>" (ADR 0050), or none for an adapter
/// that applies.
pub fn not_supported(adapter: &dyn Adapter) -> Option<String> {
    match adapter.support() {
        Support::PerChat => None,
        // ADR 0050 fixes the sentence; the module header says what the "yet" waits on.
        Support::NotYet(why) => Some(format!(
            "plugins for {} are not supported yet — {why}",
            adapter.title()
        )),
    }
}

// ------------------------------------------------------------------------------------------
// Claude Code
// ------------------------------------------------------------------------------------------

/// Claude Code: `installed_plugins.json`, and `enabledPlugins` in the chat's `--settings`.
pub struct ClaudeCode;

pub static CLAUDE_CODE: ClaudeCode = ClaudeCode;

/// Claude Code's pins: the app's own plugin on, and off every plugin it replaced — the Python
/// charter's, and its own under each id it had before it was renamed (#406, #1266). Each
/// matches on the whole id: `charter@inline` and `charter@charter` are both *named* `charter`.
///
/// Built once, because each sentence names its ids from [`crate::names`] rather than spelling
/// them, and a pin's sentence lives as long as the program.
static CLAUDE_PINS: std::sync::LazyLock<Vec<Pin>> = std::sync::LazyLock::new(|| {
    let said = |text: String| -> &'static str { Box::leak(text.into_boxed_str()) };
    let own = crate::plugin::LOADED_AS;
    let mut pins = vec![
        Pin {
            id: own,
            on: true,
            why: said(format!(
                "{own} is always on: it is purlis's own plugin, and it carries purlis's hooks \
                 and the Bash guard"
            )),
        },
        Pin {
            id: crate::plugin::SUPERSEDED,
            on: false,
            why: said(format!(
                "{} is always off: it is the Python charter's plugin, and a chat the app starts \
                 carrying it too would have two sets of hooks and two handoff skills",
                crate::plugin::SUPERSEDED
            )),
        },
    ];
    pins.extend(crate::plugin::FORMERLY.iter().map(|&(old, new)| Pin {
        id: old,
        on: false,
        why: said(if new == own {
            format!(
                "{old} is always off: it is an id purlis's own plugin had before it was \
                 renamed, and purlis's own plugin is {own}"
            )
        } else {
            format!(
                "{old} is always off: it is the id the copy `purlis plugin install` wrote had \
                 before the plugin was renamed; that copy is {new} now, and a chat the app \
                 starts loads {own}"
            )
        }),
    }));
    pins
});

impl Adapter for ClaudeCode {
    fn harness(&self) -> &'static str {
        "claude"
    }

    fn title(&self) -> &'static str {
        crate::harness::Harness::ClaudeCode.title()
    }

    /// `plugins/installed_plugins.json` under the chat's `CLAUDE_CONFIG_DIR`, else `~/.claude`:
    /// the record `claude plugin list` reads. Measured on the operator's 2.1.x install: version
    /// 2, `plugins` keyed by `<name>@<marketplace>`, each a list of installs with a `scope`
    /// (`user`, `project`, `local`). One plugin installed in several scopes is one plugin here.
    fn record(&self, env: &Env<'_>) -> Option<PathBuf> {
        env.dir("CLAUDE_CONFIG_DIR", ".claude")
            .map(|dir| dir.join("plugins").join("installed_plugins.json"))
    }

    fn installed(&self, env: &Env<'_>) -> Result<Vec<Plugin>, String> {
        let Some(path) = self.record(env) else {
            return Ok(Vec::new());
        };
        let Some(text) = read(&path)? else {
            return Ok(Vec::new());
        };
        let doc: serde_json::Value = serde_json::from_str(&text)
            .map_err(|err| format!("{} is not JSON purlis can read: {err}", path.display()))?;
        let Some(plugins) = doc.get("plugins").and_then(serde_json::Value::as_object) else {
            return Err(format!("{} holds no plugins table", path.display()));
        };
        let mut out: Vec<Plugin> = plugins
            .iter()
            .filter(|(id, _)| id_ok(id))
            .map(|(id, installs)| {
                let mut scopes: Vec<&str> = Vec::new();
                for scope in installs
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|one| one.get("scope").and_then(serde_json::Value::as_str))
                {
                    if !scopes.contains(&scope) {
                        scopes.push(scope);
                    }
                }
                let (name, marketplace) = split(id);
                let mut source = vec![marketplace];
                source.extend(scopes);
                Plugin {
                    id: id.clone(),
                    harness: self.harness(),
                    name: name.to_owned(),
                    source: source.join(", "),
                }
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    fn skills(&self, env: &Env<'_>) -> Vec<purlis_skills::OnMachine> {
        skills::claude(self, env)
    }

    fn support(&self) -> Support {
        Support::PerChat
    }

    fn pinned(&self) -> &'static [Pin] {
        CLAUDE_PINS.as_slice()
    }
}

// ------------------------------------------------------------------------------------------
// Codex
// ------------------------------------------------------------------------------------------

/// Codex: `[plugins."<id>"]` in its `config.toml`.
pub struct Codex;

pub static CODEX: Codex = Codex;

impl Adapter for Codex {
    fn harness(&self) -> &'static str {
        "codex"
    }

    fn title(&self) -> &'static str {
        crate::harness::Harness::Codex.title()
    }

    /// `[plugins."<name>@<marketplace>"]` in `$CODEX_HOME/config.toml`, else `~/.codex`, which
    /// is where Codex records an installed plugin and whether it is on (its plugin docs, and the
    /// operator's 0.147.0 install).
    fn record(&self, env: &Env<'_>) -> Option<PathBuf> {
        env.dir("CODEX_HOME", ".codex")
            .map(|dir| dir.join("config.toml"))
    }

    fn installed(&self, env: &Env<'_>) -> Result<Vec<Plugin>, String> {
        let Some(path) = self.record(env) else {
            return Ok(Vec::new());
        };
        let Some(text) = read(&path)? else {
            return Ok(Vec::new());
        };
        let doc: toml::Table = text
            .parse()
            .map_err(|err| format!("{} is not TOML purlis can read: {err}", path.display()))?;
        let mut out: Vec<Plugin> = doc
            .get("plugins")
            .and_then(toml::Value::as_table)
            .into_iter()
            .flatten()
            .filter(|(id, _)| id_ok(id))
            .map(|(id, table)| {
                let on = table
                    .get("enabled")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(true);
                let (name, marketplace) = split(id);
                Plugin {
                    id: id.clone(),
                    harness: self.harness(),
                    name: name.to_owned(),
                    source: format!(
                        "{marketplace}, {} in config.toml",
                        if on { "on" } else { "off" }
                    ),
                }
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// Measured on codex-cli 0.147.0 (charter-app#274), in a throwaway `CODEX_HOME` holding a
    /// copy of a real config and plugin cache, with `codex debug prompt-input` showing which of
    /// the plugin's skills reached the model. `enabled = false` in `config.toml` took them out.
    /// `-c` did nothing in any shape its parser takes: not `plugins.<id>.enabled=false`, not
    /// `plugins.<id>={enabled=false}`, not `plugins={"<id>"={enabled=false}}`, and not the
    /// other way round over a file that says false. A per-session switch would have to be one
    /// of those. The only other switch is `--disable plugins`, which turns them all off.
    fn skills(&self, env: &Env<'_>) -> Vec<purlis_skills::OnMachine> {
        skills::codex(self, env)
    }

    fn support(&self) -> Support {
        Support::NotYet(
            "Codex 0.147.0 turns a plugin on or off only in its own config.toml, and ignores \
             the same key given for one session with -c (measured), and purlis never writes \
             Codex's config",
        )
    }
}

// ------------------------------------------------------------------------------------------
// opencode
// ------------------------------------------------------------------------------------------
/// opencode: its config's `plugin` list, and its plugin directories.
pub struct Opencode;

pub static OPENCODE: Opencode = Opencode;

/// The files opencode loads from a plugin directory.
const OPENCODE_SCRIPTS: [&str; 4] = ["js", "ts", "mjs", "mts"];

/// The most folders above a project the list walks through when the project is no git top of
/// its own ([`Opencode::in_project`]). opencode walks on to `/`; a path deeper than this is
/// not one a project sits under.
const MOST_LEVELS_ABOVE: usize = 64;

impl Adapter for Opencode {
    fn harness(&self) -> &'static str {
        "opencode"
    }

    fn title(&self) -> &'static str {
        crate::harness::Harness::Opencode.title()
    }

    /// Its global configuration directory, `$XDG_CONFIG_HOME/opencode`, else
    /// `~/.config/opencode`: the npm packages in `opencode.json`'s `plugin` list, and every
    /// script in `plugin/` and `plugins/` (opencode's plugin docs name `plugins/`; the operator's
    /// install has `plugin/`). `opencode.jsonc` is not read: charter has no JSONC reader, and a
    /// plugin listed only there is not listed here.
    fn record(&self, env: &Env<'_>) -> Option<PathBuf> {
        env.var("XDG_CONFIG_HOME")
            .map(|base| PathBuf::from(base).join("opencode"))
            .or_else(|| env.home.as_ref().map(|home| home.join(".config/opencode")))
    }

    /// What [`Self::record`] holds, then what the home folder's `~/.opencode/` holds (#1590),
    /// which opencode reads beside its global directory: its two configs and its `plugin/` and
    /// `plugins/` scripts, read as a project's are ([`opencode_dot`]). A config at the top of
    /// the home folder, `~/opencode.json`, is no global one, and is not listed.
    fn installed(&self, env: &Env<'_>) -> Result<Vec<Plugin>, String> {
        let mut out = Vec::new();
        if let Some(dir) = self.record(env) {
            let config = dir.join("opencode.json");
            if let Some(text) = read(&config)? {
                let doc: serde_json::Value = serde_json::from_str(&text).map_err(|err| {
                    format!("{} is not JSON purlis can read: {err}", config.display())
                })?;
                out.extend(npm_plugins(&doc).into_iter().map(|id| Plugin {
                    id: id.to_owned(),
                    harness: self.harness(),
                    name: id.to_owned(),
                    source: "npm, in opencode.json".to_owned(),
                }));
            }
            if let Some(dir) = Folder::open(&dir, true) {
                out.extend(opencode_scripts(
                    self.harness(),
                    &dir,
                    "",
                    "a file in the plugin directory",
                    false,
                ));
            }
        }
        if let Some(home) = env
            .home
            .as_deref()
            .and_then(|home| Folder::open(home, true))
        {
            out.extend(opencode_dot_at(
                self.harness(),
                &home,
                "~/",
                "the home folder's",
            ));
        }
        Ok(out)
    }

    /// What opencode loads from the project itself, beside the global ones (ADR 0058): the npm
    /// plugins each of the project's own configs names (#1590), then the scripts in its
    /// `.opencode/plugin/` and `.opencode/plugins/` (#1371). A chat can write all of it, so it is
    /// listed for a reader to see, and never handed to anything.
    ///
    /// **A project that is no git top of its own** (#1590): opencode walks up from it to the git
    /// top above it, or to `/` outside a repository, so the list goes on upward the same way,
    /// each folder above the project named by its path, at most [`MOST_LEVELS_ABOVE`] of them.
    /// The home folder's own `.opencode` is [`Self::installed`]'s, and is not listed twice.
    fn in_project(&self, root: &Path) -> Vec<Plugin> {
        let Some(level) = Folder::open(root, true) else {
            return Vec::new();
        };
        let mut out = opencode_level(self.harness(), &level, "", "this project's");
        if level.is_git_top() {
            return out;
        }
        let Ok(start) = root.canonicalize() else {
            return out;
        };
        let home = crate::profiles::home().and_then(|home| home.canonicalize().ok());
        for above in start.ancestors().skip(1).take(MOST_LEVELS_ABOVE) {
            let Some(level) = Folder::open(above, false) else {
                break;
            };
            let shown = above.display().to_string();
            let prefix = if shown.ends_with(std::path::MAIN_SEPARATOR) {
                shown.clone()
            } else {
                format!("{shown}{}", std::path::MAIN_SEPARATOR)
            };
            let whose = format!("the {shown} folder's");
            out.extend(opencode_configs(
                self.harness(),
                &level,
                &prefix,
                &whose,
                "",
            ));
            if home.as_deref() != Some(above) {
                out.extend(opencode_dot_at(self.harness(), &level, &prefix, &whose));
            }
            if level.is_git_top() {
                break;
            }
        }
        out
    }

    /// What opencode loads for a chat started in the workspace, in one of its clones or in one
    /// of their worktrees (#1590), level by level the way opencode walks ([`OPENCODE_CONFIGS`]):
    /// the workspace's folder, then each folder above it up to the git top or the project,
    /// whichever comes first (the project's own is [`Self::in_project`]'s), then each clone,
    /// which is its own git top, then each clone's worktrees under the workspace's
    /// `.worktrees/<repo>/`, each its own git top too. Each is id'd by its place in the project
    /// and says whose it is.
    ///
    /// **A link found is not followed** (#1590, #1828): the workspace's folder and each clone
    /// are reached as [`crate::repos::clones`] reaches them, and each folder is read through the
    /// one handle purlis opened ([`Folder`]), so a `.opencode`, plugin or worktree folder that is
    /// a link is named and not followed ([`opencode_level`]), even when it became one after it
    /// was looked at. A workspace purlis cannot reach that way lists nothing.
    fn in_workspace(&self, root: &Path, workspace: &str) -> Vec<Plugin> {
        let Ok(dir) = crate::worktree::confine::workspace_dir(root, workspace) else {
            return Vec::new();
        };
        let Some(held) = Folder::open(&dir, false) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut level = dir.as_path();
        while level != root {
            let Ok(place) = level.strip_prefix(root) else {
                break;
            };
            let here = if level == dir {
                None
            } else {
                match Folder::open(level, false) {
                    Some(here) => Some(here),
                    None => break,
                }
            };
            let here = here.as_ref().unwrap_or(&held);
            let place = place.display().to_string();
            let whose = if level == dir {
                format!("workspace {workspace}'s")
            } else {
                format!("the {place} folder's")
            };
            out.extend(opencode_level(
                self.harness(),
                here,
                &format!("{place}/"),
                &whose,
            ));
            // opencode stops at the git top: a workspace that is a repository of its own is
            // where a chat in it stops looking.
            if here.is_git_top() {
                break;
            }
            let Some(up) = level.parent() else {
                break;
            };
            level = up;
        }
        let repos = crate::repos::clones(root, workspace)
            .map(|clones| clones.repos)
            .unwrap_or_default();
        for repo in &repos {
            let Some(clone) = Folder::open(&repo.path, false) else {
                continue;
            };
            out.extend(opencode_level(
                self.harness(),
                &clone,
                &format!("workspaces/{workspace}/{}/", repo.name),
                &format!("clone {}'s", repo.name),
            ));
        }
        out.extend(opencode_worktrees(
            self.harness(),
            root,
            workspace,
            &held,
            &repos,
        ));
        out
    }

    fn skills(&self, env: &Env<'_>) -> Vec<purlis_skills::OnMachine> {
        skills::opencode(self, env)
    }

    fn support(&self) -> Support {
        Support::NotYet(
            "opencode has no switch that turns one plugin off: it loads every plugin from every \
             config and plugin directory together, and a project's `\"plugin\": []` did not \
             remove one another config named (measured, opencode 1.18.23)",
        )
    }
}

/// The configs opencode reads in a folder of a project, in the order it looks for them
/// (measured, opencode 1.18.33): these two in the folder itself, then the same two in its
/// `.opencode/`.
///
/// **Which folders opencode reads them in** (#1590, read from opencode 1.18.33's source:
/// `ConfigPaths.files` and `ConfigPaths.directories` in `packages/opencode/src/config/paths.ts`,
/// over `FSUtil.up` in `packages/core/src/fs-util.ts`). It walks from the chat's own folder up
/// to its worktree, both included: the git top in a repository, `/` outside one. In each folder
/// on the way it reads these two configs, and a `.opencode/` there for its two configs and its
/// `plugin/` and `plugins/` scripts (`{plugin,plugins}/*.{ts,js}`). `up` asks only whether a
/// path exists, so a `.opencode` that is a link is walked into. Beside those it reads the global
/// directory and a `~/.opencode`. So a chat in a workspace also loads the workspace's own, and
/// those of each folder between it and the git top; a chat in a clone or a worktree, its own,
/// since each is its own git top ([`Opencode::in_workspace`]).
///
/// **The git top is the one git takes** (#1828): a `.git` folder holding `HEAD`, or a `.git`
/// file that starts `gitdir:` ([`Folder::is_git_top`]). Any other `.git` is no repository to
/// git, and opencode walks on past it.
const OPENCODE_CONFIGS: [&str; 2] = ["opencode.json", "opencode.jsonc"];

/// The npm plugins a config's `plugin` list names, each one purlis can show, in name order.
fn npm_plugins(doc: &serde_json::Value) -> Vec<&str> {
    let mut npm: Vec<&str> = doc
        .get("plugin")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .filter(|id| id_ok(id))
        .collect();
    npm.sort_unstable();
    npm
}

/// One folder opencode reads a project's own plugins in (#1590, [`OPENCODE_CONFIGS`]): its two
/// configs, and a `.opencode/` there for its two configs and its scripts. Each id starts with
/// `prefix`, the folder's place in the project, and each source says `whose` it is.
///
/// **A link found is not followed.** A chat can write all of it, so a `.opencode`, `plugin` or
/// `plugins` that is a link is named as one purlis does not follow, and what opencode loads
/// through it is not listed: opencode follows it, so it is named rather than left out. Each is
/// read through the folder handle purlis opened (#1828, [`Folder`]), never by its path again.
fn opencode_level(harness: &'static str, level: &Folder, prefix: &str, whose: &str) -> Vec<Plugin> {
    let mut out = opencode_configs(harness, level, prefix, whose, "");
    out.extend(opencode_dot_at(harness, level, prefix, whose));
    out
}

/// What the `.opencode` in `level` holds ([`opencode_dot`]), or the link it is, named.
fn opencode_dot_at(
    harness: &'static str,
    level: &Folder,
    prefix: &str,
    whose: &str,
) -> Vec<Plugin> {
    match level.child(".opencode", false) {
        Child::Folder(dot) => opencode_dot(harness, &dot, prefix, whose, true),
        Child::Link => vec![unfollowed(harness, prefix, ".opencode")],
        Child::Absent => Vec::new(),
    }
}

/// A `.opencode` folder's two configs and its scripts, read through `dot`, the handle purlis
/// opened on it, each id'd under `<prefix>.opencode/`.
fn opencode_dot(
    harness: &'static str,
    dot: &Folder,
    prefix: &str,
    whose: &str,
    chat_written: bool,
) -> Vec<Plugin> {
    let mut out = opencode_configs(harness, dot, prefix, whose, ".opencode/");
    out.extend(opencode_scripts(
        harness,
        dot,
        &format!("{prefix}.opencode/"),
        &format!("a file in {whose} .opencode plugin directory"),
        chat_written,
    ));
    out
}

/// A clone's worktrees under the workspace's `.worktrees/<repo>/` (#1590), each its own git
/// top, read through `workspace`, the handle purlis opened on the workspace's folder: the
/// worktrees git lists for each of `repos` ([`crate::worktree::list`], read only), each id'd
/// `workspaces/<ws>/.worktrees/<repo>/<piece>/`. A link on the way there is named and not
/// followed, and so is any link in a repo's worktree folder, where a chat may be started.
fn opencode_worktrees(
    harness: &'static str,
    root: &Path,
    workspace: &str,
    held: &Folder,
    repos: &[crate::repos::Repo],
) -> Vec<Plugin> {
    use crate::worktree::DIR_NAME;
    let at = format!("workspaces/{workspace}/");
    let pieces = match held.child(DIR_NAME, false) {
        Child::Folder(pieces) => pieces,
        Child::Link => return vec![unfollowed(harness, &at, DIR_NAME)],
        Child::Absent => return Vec::new(),
    };
    let at = format!("{at}{DIR_NAME}/");
    let mut out = Vec::new();
    for repo in repos {
        let folder = match pieces.child(&repo.name, false) {
            Child::Folder(folder) => folder,
            Child::Link => {
                out.push(unfollowed(harness, &at, &repo.name));
                continue;
            }
            Child::Absent => continue,
        };
        let listed: BTreeSet<String> = crate::worktree::list(root, workspace, &repo.name)
            .map(|pieces| pieces.into_iter().map(|piece| piece.piece).collect())
            .unwrap_or_default();
        let at = format!("{at}{}/", repo.name);
        let mut names = folder.names();
        names.sort();
        for name in names {
            match folder.child(&name, false) {
                Child::Link => out.push(unfollowed(harness, &at, &name)),
                Child::Folder(piece) if listed.contains(&name) => out.extend(opencode_level(
                    harness,
                    &piece,
                    &format!("{at}{name}/"),
                    &format!("the {}/{name} branch folder's", repo.name),
                )),
                _ => {}
            }
        }
    }
    out
}

/// A folder opencode reads that is a link, which purlis names and does not follow.
fn unfollowed(harness: &'static str, prefix: &str, name: &str) -> Plugin {
    Plugin {
        id: format!("{prefix}{name}"),
        harness,
        name: name.to_owned(),
        source: "a link purlis does not follow: the plugins opencode loads through it are not \
                 listed"
            .to_owned(),
    }
}

/// **The npm plugins each of a folder's own opencode configs names** (#1590), read through
/// `folder` at `place` (`""` for the folder itself, `".opencode/"` for its `.opencode`), each
/// id'd `<prefix><place><config>:<package>`, so a project's entry never collides with the global
/// entry of the same package, nor with another config's.
///
/// **Named more, never less.** A config purlis cannot read (JSONC with a comment or a trailing
/// comma, which it has no reader for, text that is not JSON, or a link) may still name plugins
/// opencode loads, so the config itself is listed, by its place in the project, as one it cannot
/// read. A config that is not there lists nothing.
fn opencode_configs(
    harness: &'static str,
    folder: &Folder,
    prefix: &str,
    whose: &str,
    place: &str,
) -> Vec<Plugin> {
    let mut out = Vec::new();
    for config in OPENCODE_CONFIGS {
        let config_at = format!("{place}{config}");
        let text = match folder.read(config) {
            Ok(None) => continue,
            Ok(Some(text)) => Ok(text),
            Err(()) => Err(()),
        };
        match text.and_then(|text| serde_json::from_str::<serde_json::Value>(&text).map_err(drop)) {
            Ok(doc) => {
                out.extend(npm_plugins(&doc).into_iter().map(|id| Plugin {
                    id: format!("{prefix}{config_at}:{id}"),
                    harness,
                    name: id.to_owned(),
                    source: format!("npm, in {whose} {config_at}"),
                }));
            }
            _ => out.push(Plugin {
                id: format!("{prefix}{config_at}"),
                harness,
                name: config_at.clone(),
                source: "a config purlis cannot read: the plugins it names are not listed"
                    .to_owned(),
            }),
        }
    }
    out
}

/// Every script opencode loads from `dir`'s `plugin/` and `plugins/` (its plugin docs name
/// `plugins/`; the operator's install has `plugin/`), in name order, each id'd
/// `<prefix><folder>/<file>`. A folder that cannot be read lists nothing.
///
/// **A symbolic link is listed like a file** (#1590): opencode's scan follows links (measured,
/// opencode 1.18.33), so a link to a script elsewhere is loaded as one. A link whose target is
/// not there is listed too, since its target may be there when opencode starts; a link to a
/// directory is no script. Where a chat can write `dir` (`chat_written`), a `plugin` or `plugins`
/// that is itself a link is named and not followed ([`unfollowed`]). Only names are listed: no
/// script is read.
fn opencode_scripts(
    harness: &'static str,
    dir: &Folder,
    prefix: &str,
    source: &str,
    chat_written: bool,
) -> Vec<Plugin> {
    let mut out = Vec::new();
    for folder in ["plugin", "plugins"] {
        let scripts = match dir.child(folder, !chat_written) {
            Child::Folder(scripts) => scripts,
            Child::Link => {
                out.push(unfollowed(harness, prefix, folder));
                continue;
            }
            Child::Absent => continue,
        };
        let mut files: Vec<String> = scripts
            .names()
            .into_iter()
            .filter(|name| {
                Path::new(name)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| OPENCODE_SCRIPTS.contains(&ext))
            })
            .filter(|name| match scripts.kind(name, false) {
                Some(Kind::File) => true,
                Some(Kind::Link) => scripts.kind(name, true) != Some(Kind::Folder),
                _ => false,
            })
            .collect();
        files.sort();
        out.extend(files.into_iter().map(|file| Plugin {
            id: format!("{prefix}{folder}/{file}"),
            harness,
            name: file,
            source: source.to_owned(),
        }));
    }
    out
}

/// A file's text, `None` when it is not there, or why it could not be read.
fn read(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("purlis could not read {}: {err}", path.display())),
    }
}

/// The most of a project's own config purlis reads to list its plugins.
const MOST_PROJECT_CONFIG_BYTES: u64 = 1024 * 1024;

/// A file a chat can write, read only where it is a plain file of its own (train 77 review),
/// from what opening it gave: `None` where nothing is there, and an error for a link, a pipe, a
/// device, anything past [`MOST_PROJECT_CONFIG_BYTES`] or a file that will not read. A link is
/// never followed, so a chat cannot have this read a file elsewhere, nor one that never ends.
fn read_opened(opened: std::io::Result<std::fs::File>) -> Result<Option<String>, ()> {
    use std::io::Read;
    let file = match opened {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    let meta = file.metadata().map_err(drop)?;
    if !meta.is_file() || meta.len() > MOST_PROJECT_CONFIG_BYTES {
        return Err(());
    }
    let mut text = String::new();
    file.take(MOST_PROJECT_CONFIG_BYTES)
        .read_to_string(&mut text)
        .map_err(drop)?;
    Ok(Some(text))
}

/// What a name in a [`Folder`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Folder,
    File,
    Link,
    Other,
}

/// What a name in a [`Folder`] opened as.
enum Child {
    /// A folder, held open.
    Folder(Folder),
    /// A symbolic link where none is followed: named, never read through.
    Link,
    /// Nothing there, or nothing purlis opens as a folder.
    Absent,
}

/// **A folder held open, and read by name through that one handle** (#1828). A chat can write
/// what opencode reads in a project, so purlis never asks about a path and then opens it again:
/// a `.opencode` or a plugin folder is opened once, without following a link at its last step,
/// and everything in it is opened relative to that handle. A link put in its place after it was
/// opened is never read through. Off unix, by path, with the same answers where nothing changes
/// in between.
#[cfg(unix)]
struct Folder(rustix::fd::OwnedFd);

#[cfg(unix)]
impl Folder {
    fn flags(follow: bool) -> rustix::fs::OFlags {
        use rustix::fs::OFlags;
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
        if follow {
            flags
        } else {
            flags | OFlags::NOFOLLOW
        }
    }

    /// `path` opened as a folder, through a link at its last step only when `follow`.
    fn open(path: &Path, follow: bool) -> Option<Self> {
        rustix::fs::open(path, Self::flags(follow), rustix::fs::Mode::empty())
            .ok()
            .map(Self)
    }

    /// `name` in this folder opened as a folder, through a link only when `follow`.
    fn child(&self, name: &str, follow: bool) -> Child {
        match rustix::fs::openat(
            &self.0,
            name,
            Self::flags(follow),
            rustix::fs::Mode::empty(),
        ) {
            Ok(fd) => Child::Folder(Self(fd)),
            Err(_) if !follow && self.kind(name, false) == Some(Kind::Link) => Child::Link,
            Err(_) => Child::Absent,
        }
    }

    /// What `name` is, or what it leads to when `follow`.
    fn kind(&self, name: &str, follow: bool) -> Option<Kind> {
        use rustix::fs::{AtFlags, FileType};
        let flags = if follow {
            AtFlags::empty()
        } else {
            AtFlags::SYMLINK_NOFOLLOW
        };
        let stat = rustix::fs::statat(&self.0, name, flags).ok()?;
        Some(match FileType::from_raw_mode(stat.st_mode) {
            FileType::Directory => Kind::Folder,
            FileType::RegularFile => Kind::File,
            FileType::Symlink => Kind::Link,
            _ => Kind::Other,
        })
    }

    /// `name`'s text, read as [`read_opened`] reads, never through a link.
    fn read(&self, name: &str) -> Result<Option<String>, ()> {
        use rustix::fs::OFlags;
        let opened = rustix::fs::openat(
            &self.0,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        );
        read_opened(opened.map(std::fs::File::from).map_err(Into::into))
    }

    /// Every name in this folder that is text, in no order.
    fn names(&self) -> Vec<String> {
        let Ok(dir) = rustix::fs::Dir::read_from(&self.0) else {
            return Vec::new();
        };
        dir.filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_str().ok().map(str::to_owned))
            .filter(|name| name != "." && name != "..")
            .collect()
    }
}

/// Off unix, a folder by its path ([`Folder`]'s unix half holds it open).
#[cfg(not(unix))]
struct Folder(PathBuf);

#[cfg(not(unix))]
impl Folder {
    fn open(path: &Path, follow: bool) -> Option<Self> {
        let here = Self(path.to_path_buf());
        (follow || !std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_symlink()))
            .then_some(here)
            .filter(|here| here.0.is_dir())
    }

    fn child(&self, name: &str, follow: bool) -> Child {
        let path = self.0.join(name);
        if !follow && self.kind(name, false) == Some(Kind::Link) {
            return Child::Link;
        }
        if path.is_dir() {
            Child::Folder(Self(path))
        } else {
            Child::Absent
        }
    }

    fn kind(&self, name: &str, follow: bool) -> Option<Kind> {
        let path = self.0.join(name);
        let meta = if follow {
            std::fs::metadata(path)
        } else {
            std::fs::symlink_metadata(path)
        }
        .ok()?;
        let kind = meta.file_type();
        Some(if kind.is_symlink() {
            Kind::Link
        } else if kind.is_dir() {
            Kind::Folder
        } else if kind.is_file() {
            Kind::File
        } else {
            Kind::Other
        })
    }

    fn read(&self, name: &str) -> Result<Option<String>, ()> {
        let path = self.0.join(name);
        if self.kind(name, false) == Some(Kind::Link) {
            return Err(());
        }
        read_opened(std::fs::File::open(path))
    }

    fn names(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.0) else {
            return Vec::new();
        };
        entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect()
    }
}

impl Folder {
    /// Whether this folder is a git top as git takes one (#1828): a `.git` folder holding
    /// `HEAD`, or a `.git` file that starts `gitdir:`. Any other `.git` (an empty folder, a
    /// stray file) is no repository to git, so opencode walks on past it. A `.git` that is a
    /// link is not followed, and the walk goes on: it lists more, never less.
    fn is_git_top(&self) -> bool {
        match self.child(".git", false) {
            Child::Folder(git) => git.kind("HEAD", false).is_some(),
            Child::Link => false,
            Child::Absent => self
                .read(".git")
                .is_ok_and(|text| text.is_some_and(|text| text.starts_with("gitdir:"))),
        }
    }
}

/// `<name>@<marketplace>` in two, the marketplace empty when there is none.
fn split(id: &str) -> (&str, &str) {
    id.rsplit_once('@').unwrap_or((id, ""))
}

/// The longest id charter reads or hands on.
const MOST_ID_BYTES: usize = 200;

/// Whether `id` could be a plugin's id: one line, short, and nothing in it draws as nothing.
pub fn id_ok(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= MOST_ID_BYTES && !id.contains(crate::panel::undrawable)
}

// ------------------------------------------------------------------------------------------
// What a project's files say, and the precedence
// ------------------------------------------------------------------------------------------

/// What one layer says: by harness, then by plugin id.
type Said = BTreeMap<String, BTreeMap<String, bool>>;

/// What a project's two files say, by harness and then by plugin id — and, in a workspace, what
/// that workspace's `workspace.json` says (charter-app#282), the layer between them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choices {
    shared: Said,
    /// Empty outside a workspace, and for a workspace whose manifest says nothing.
    workspace: Said,
    local: Said,
    /// The workspace these were read in, when they were.
    workspace_name: Option<String>,
    /// Why `charter.local.toml` is not among these layers, when it is there and git would carry
    /// it (charter-app#319) — kept only when it named a plugin of some harness.
    local_left_out: Option<String>,
    /// The harnesses the left-out Local file named a plugin of: the groups that say why.
    local_unread: BTreeSet<String>,
    /// The project these were read from ([`Self::read`]), whose own plugin files a survey
    /// lists ([`Adapter::in_project`]); none for choices made from text.
    project: Option<PathBuf>,
}

impl Choices {
    /// The two files' text: `None` for a file that is not there.
    pub fn from_text(shared: Option<&str>, local: Option<&str>) -> Self {
        let said = |text: &str| {
            text.parse::<toml::Table>()
                .map(|top| said_in(&top))
                .unwrap_or_default()
        };
        Self {
            shared: shared.map(said).unwrap_or_default(),
            local: local.map(said).unwrap_or_default(),
            ..Self::default()
        }
    }

    /// The same choices, in the workspace `name` whose `workspace.json` has this text (`None`:
    /// it has none). Its `settings.harness_plugins` go between Shared and Local. A manifest with
    /// none, or one that is not JSON, says nothing, which leaves the project's answer as it was.
    pub fn in_workspace(self, name: &str, manifest: Option<&str>) -> Self {
        self.with_workspace(
            name,
            manifest.and_then(crate::settings::workspace::table_in),
        )
    }

    fn with_workspace(mut self, name: &str, settings: Option<toml::Table>) -> Self {
        self.workspace = settings.as_ref().map(said_in).unwrap_or_default();
        self.workspace_name = Some(name.to_owned());
        self
    }

    /// The plane at `root`'s two files, as [`crate::settings::layer_text`] hands them: a file
    /// that cannot be read says nothing, and neither does a `charter.local.toml` git would carry
    /// (charter-app#308).
    pub fn read(root: &Path) -> Self {
        use crate::settings::{Which, layer_text};
        Self::from_layers(
            &layer_text(root, Which::Shared),
            &layer_text(root, Which::Local),
        )
        .of_project(root)
    }

    /// The same choices, read from the project at `root`, whose own plugin files a survey
    /// lists ([`Adapter::in_project`]). [`Self::read`] is the files' text read there.
    #[must_use]
    pub fn of_project(mut self, root: &Path) -> Self {
        self.project = Some(root.to_path_buf());
        self
    }

    /// The two files as [`crate::settings::layer_text`] hands them — [`Self::from_text`], and,
    /// when the Local file was left out having named plugins, why and for which harnesses
    /// (charter-app#319).
    pub fn from_layers(
        shared: &crate::settings::LayerText,
        local: &crate::settings::LayerText,
    ) -> Self {
        let mut unread = BTreeSet::new();
        let why = local.left_out_where(|top| {
            unread = said_in(top)
                .into_iter()
                .filter(|(_, plugins)| !plugins.is_empty())
                .map(|(harness, _)| harness)
                .collect();
            !unread.is_empty()
        });
        Self {
            local_left_out: why,
            local_unread: unread,
            ..Self::from_text(shared.text(), local.text())
        }
    }

    /// Why `charter.local.toml` is not among `harness`'s choices, when it is there, git would
    /// carry it, and it named a plugin of `harness`: the ignore check's sentence, which the
    /// Settings tab says too, at the Project level (charter-app#319). A settings tab says it in
    /// that harness's group, so a plugin set in Local and not applied is never shown without its
    /// reason.
    pub fn local_left_out(&self, harness: &str) -> Option<&str> {
        self.local_unread
            .contains(harness)
            .then_some(self.local_left_out.as_deref())
            .flatten()
    }

    /// [`Self::read`], in `workspace` when there is one — the same reader
    /// [`crate::extension::project::Choices::read_in`] is. A name that is not one of the plane's
    /// workspaces reads as a workspace with no settings.
    pub fn read_in(root: &Path, workspace: Option<&str>) -> Self {
        let project = Self::read(root);
        match workspace {
            None => project,
            Some(name) => {
                project.with_workspace(name, crate::settings::workspace::read(root, name))
            }
        }
    }

    /// The workspace's file as a sentence names it — `workspaces/<ws>/workspace.json` — or
    /// `None` outside a workspace.
    pub fn workspace_file(&self) -> Option<String> {
        self.workspace_name
            .as_deref()
            .map(crate::settings::workspace::named)
    }

    /// Whether any layer turns any of `harness`'s plugins on or off.
    fn names_any(&self, harness: &str) -> bool {
        [&self.shared, &self.workspace, &self.local]
            .iter()
            .any(|file| file.get(harness).is_some_and(|said| !said.is_empty()))
    }

    /// Every layer, the one with the last word first: its source, the file a sentence names,
    /// where the table sits in that file, and what it says for `harness`.
    fn layers(&self, harness: &str) -> [Layer<'_>; 3] {
        [
            Layer {
                source: Source::Local,
                file: LOCAL_FILE.to_owned(),
                at: "",
                said: self.local.get(harness),
            },
            Layer {
                source: Source::Workspace,
                file: self.workspace_file().unwrap_or_default(),
                at: WORKSPACE_AT,
                said: self.workspace.get(harness),
            },
            Layer {
                source: Source::Shared,
                file: COMMITTED_FILE.to_owned(),
                at: "",
                said: self.shared.get(harness),
            },
        ]
    }
}

/// Where `[harness_plugins]` sits in a workspace's `workspace.json`, as a key path names it.
const WORKSPACE_AT: &str = "settings.";

/// One layer of [`Choices`], for one harness.
struct Layer<'c> {
    source: Source,
    /// The file a sentence names.
    file: String,
    /// Where the table sits in that file.
    at: &'static str,
    said: Option<&'c BTreeMap<String, bool>>,
}

/// Every well-formed `"<id>" = true|false` under each `[harness_plugins.<harness>]` in `top` — a
/// whole TOML file, or a workspace's `settings` read as one. What is not well formed is left out
/// here and refused by [`refusals`].
fn said_in(top: &toml::Table) -> Said {
    top.get(TABLE)
        .and_then(toml::Value::as_table)
        .into_iter()
        .flatten()
        .filter_map(|(harness, plugins)| {
            let plugins = plugins
                .as_table()?
                .iter()
                .filter(|(id, _)| id_ok(id))
                .filter_map(|(id, on)| Some((id.clone(), on.as_bool()?)))
                .collect();
            Some((harness.clone(), plugins))
        })
        .collect()
}

/// A value a file set that charter did not use: which file, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ignored {
    pub source: Source,
    pub why: String,
}

/// One plugin, in this project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effective {
    pub id: String,
    pub name: String,
    /// Where charter found it installed, or empty when it is not.
    pub origin: String,
    /// On, off, or `None` for not set: the harness decides.
    pub wanted: Option<bool>,
    /// Which layer decided [`Self::wanted`], or none (and for a pin, none).
    pub source: Source,
    /// Whether this machine has it installed for this harness.
    pub installed: bool,
    /// Why charter fixes it whatever a file says ([`Pin::why`]), or none for a plugin a project
    /// may choose.
    pub pinned: Option<&'static str>,
    pub ignored: Vec<Ignored>,
}

/// **The precedence, in one place**, for one harness: every plugin it has installed, every one
/// a project file names for it, and every pin, with what this project has it at and which file
/// decided that. In id order.
pub fn resolve(adapter: &dyn Adapter, installed: &[Plugin], choices: &Choices) -> Vec<Effective> {
    let harness = adapter.harness();
    let layers = choices.layers(harness);
    let mut ids: BTreeSet<&str> = installed.iter().map(|it| it.id.as_str()).collect();
    for said in layers.iter().filter_map(|layer| layer.said) {
        ids.extend(said.keys().map(String::as_str));
    }
    ids.extend(adapter.pinned().iter().map(|pin| pin.id));
    let cannot = not_supported(adapter);

    ids.into_iter()
        .map(|id| {
            let here = installed.iter().find(|it| it.id == id);
            // Local, then the workspace, then Shared: the first that names it decides.
            let said: Vec<(&Layer<'_>, bool)> = layers
                .iter()
                .filter_map(|layer| Some((layer, *layer.said?.get(id)?)))
                .collect();
            let (mut wanted, mut source) =
                said.first().map_or((None, Source::Default), |(layer, on)| {
                    (Some(*on), layer.source)
                });
            let mut ignored = Vec::new();
            let pin = adapter.pinned().iter().find(|pin| pin.id == id);
            for (layer, on) in said {
                let (file, at) = (&layer.file, layer.at);
                let key = format!("{at}{TABLE}.{harness}.{}", toml_key(id));
                if let Some(pin) = pin.filter(|pin| pin.on != on) {
                    ignored.push(Ignored {
                        source: layer.source,
                        why: format!("{file} sets {key} to {on}, and {}", pin.why),
                    });
                } else if let Some(cannot) = &cannot {
                    ignored.push(Ignored {
                        source: layer.source,
                        why: format!("{file} sets {key}, and {cannot}; purlis hands it nothing"),
                    });
                }
            }
            if let Some(pin) = pin {
                wanted = Some(pin.on);
                source = Source::Default;
            }
            Effective {
                id: id.to_owned(),
                name: here.map_or_else(|| split(id).0.to_owned(), |it| it.name.clone()),
                origin: here.map(|it| it.source.clone()).unwrap_or_default(),
                wanted,
                source,
                installed: here.is_some(),
                pinned: pin.map(|pin| pin.why),
                ignored,
            }
        })
        .collect()
}

/// **What a chat on this harness is handed**: every pin, and every installed plugin a file
/// turned on or off. Nothing for a harness whose adapter cannot apply, and nothing not set: the
/// harness decides those as it always did.
pub fn chosen(adapter: &dyn Adapter, all: &[Effective]) -> Chosen {
    if adapter.support() != Support::PerChat {
        return Chosen::new();
    }
    all.iter()
        .filter(|it| it.pinned.is_some() || it.installed)
        .filter_map(|it| Some((it.id.clone(), it.wanted?)))
        .collect()
}

/// What a chat of `kind`, started in the plane at `root` — in `workspace`, when it is in one —
/// with `env`, is handed.
///
/// **The harness's record is read only when a layer names one of its plugins**: with nothing
/// chosen, the answer is the pins whatever is installed, so a project that says nothing costs no
/// read of anybody's home directory. Where the record cannot be read, the pins alone: a listing
/// charter could not read is not a reason to start a chat without its guard.
pub fn for_start(kind: &str, root: &Path, workspace: Option<&str>, env: &Env<'_>) -> Chosen {
    let Some(adapter) = adapter(kind) else {
        return Chosen::new();
    };
    let choices = Choices::read_in(root, workspace);
    let installed = if choices.names_any(adapter.harness()) {
        adapter.installed(env).unwrap_or_default()
    } else {
        Vec::new()
    };
    chosen(adapter, &resolve(adapter, &installed, &choices))
}

/// One harness in the settings tab.
#[derive(Clone)]
pub struct Group {
    pub adapter: &'static dyn Adapter,
    /// Where its listing was read from ([`Adapter::record`]).
    pub record: Option<PathBuf>,
    /// Why the harness's own record could not be read, if it could not.
    pub trouble: Option<String>,
    pub plugins: Vec<Effective>,
}

/// Every harness, with what it has installed and what `choices` — a project's, or a project's in
/// one workspace ([`Choices::read_in`]) — have each at.
/// What the project holds of its own that the harness loads ([`Adapter::in_project`]) is listed
/// beside what this machine installed, and in a workspace, what the workspace and its clones hold
/// of their own ([`Adapter::in_workspace`], #1590).
pub fn survey(choices: &Choices, env: &Env<'_>) -> Vec<Group> {
    adapters()
        .map(|adapter| {
            let (mut installed, trouble) = match adapter.installed(env) {
                Ok(installed) => (installed, None),
                Err(why) => (Vec::new(), Some(why)),
            };
            if let Some(root) = &choices.project {
                installed.extend(adapter.in_project(root));
                if let Some(workspace) = &choices.workspace_name {
                    installed.extend(adapter.in_workspace(root, workspace));
                }
            }
            Group {
                adapter,
                record: adapter.record(env),
                trouble,
                plugins: resolve(adapter, &installed, choices),
            }
        })
        .collect()
}

/// Everything in `text`'s `[harness_plugins]` that charter would not read or would not honour,
/// as `file` holds it, one sentence each, in the file's order.
pub fn refusals(text: &str, file: &str) -> Vec<String> {
    keyed(text, file).into_iter().map(|one| one.why).collect()
}

/// [`refusals`], each with the key it is about (#1292).
pub fn keyed(text: &str, file: &str) -> Vec<Refusal> {
    let Ok(top) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    keyed_in(&top, file, "")
}

/// [`refusals`], of a table already read — a whole TOML file, or a workspace's `settings` read as
/// one (charter-app#282) — whose `[harness_plugins]` sits at `at` in `file` (`"settings."` in a
/// `workspace.json`), so each sentence names the key where it is written.
pub fn refusals_in(top: &toml::Table, file: &str, at: &str) -> Vec<String> {
    keyed_in(top, file, at)
        .into_iter()
        .map(|one| one.why)
        .collect()
}

/// [`refusals_in`], each with the key it is about **in `top`** (#1292): `at` is left out, so a
/// workspace's is the key under its `settings`. A plugin id is one step, dots and all.
pub fn keyed_in(top: &toml::Table, file: &str, at: &str) -> Vec<Refusal> {
    let Some(table) = top.get(TABLE) else {
        return Vec::new();
    };
    let shape = "each harness is [harness_plugins.<harness>], holding \"<plugin id>\" = true or \
                 false";
    let Some(table) = table.as_table() else {
        return vec![Refusal::at(
            format!("{at}{TABLE} in {file} is not a table — {shape}"),
            &[TABLE],
        )];
    };
    let mut out = Vec::new();
    for (harness, plugins) in table {
        let Some(adapter) = adapter(harness) else {
            let known: Vec<&str> = adapters().map(|it| it.harness()).collect();
            out.push(Refusal::at(
                format!(
                    "[{at}{TABLE}.{}] in {file} is not a harness purlis knows — one of: {}",
                    toml_key(harness),
                    known.join(", ")
                ),
                &[TABLE, harness.as_str()],
            ));
            continue;
        };
        let Some(plugins) = plugins.as_table() else {
            out.push(Refusal::at(
                format!("{at}{TABLE}.{harness} in {file} is not a table — {shape}"),
                &[TABLE, harness.as_str()],
            ));
            continue;
        };
        for (id, on) in plugins {
            let key = format!("{at}{TABLE}.{harness}.{}", toml_key(id));
            let keyed = [TABLE, harness.as_str(), id.as_str()];
            if !id_ok(id) {
                out.push(Refusal::at(
                    format!(
                        "{key} in {file} is not a plugin id — one line of at most \
                         {MOST_ID_BYTES} bytes, with nothing invisible in it"
                    ),
                    &keyed,
                ));
                continue;
            }
            let Some(on) = on.as_bool() else {
                out.push(Refusal::at(
                    format!("{key} in {file} is not true or false"),
                    &keyed,
                ));
                continue;
            };
            if let Some(pin) = adapter
                .pinned()
                .iter()
                .find(|pin| pin.id == id && pin.on != on)
            {
                out.push(Refusal::at(
                    format!("{key} in {file} cannot be {on}: {}", pin.why),
                    &keyed,
                ));
            }
        }
    }
    out
}

mod skills;

#[cfg(test)]
mod tests;
