// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One adapter per harness, as far as its plugins and its own skills go** (ADR 0050, as
//! amended by #3467).
//!
//! **purlis does not choose another harness's plugins** (#3467, the operator's rulings of
//! 2026-10-26/27 on spec #2762). A plugin brings more than skills (hooks, MCP servers,
//! subagents, LSP servers, code that runs in the harness), and the three harnesses disagree on
//! each, so whether one is on is the harness's own settings' to say, and a project's old
//! `[harness_plugins]` keys are ignored ([`old`] says so, once). What an adapter still knows:
//!
//! - **purlis's own plugin, pinned** ([`Adapter::pinned`]). Claude Code's pins are written into
//!   every chat's `--settings`: `purlis@inline` always on, because it carries the hooks and the
//!   Bash guard, and `charter@charter`, the Python charter's, always off, by the operator's
//!   ruling of 2026-09-23. Every id the bundled plugin had before it was renamed
//!   (`charter-app@inline` until #406, `charter@inline` and the installed `charter@charter-app`
//!   until #1266) is always off too, so an old copy of purlis's own plugin never loads beside
//!   the new one. Codex and opencode load purlis's plugin their own way (`-c hooks.*`, the
//!   opencode shim) and pin nothing here.
//! - **Where the harness finds its own skills** ([`Adapter::skills_at`], [`harness_skills`],
//!   [`repo_skills`]): the person's own folders, each installed plugin's, and a clone's. The Skill
//!   studio shows a plugin's skills read-only, naming the plugin; the person's own and a repo's
//!   can be turned off for a project in `[harness_skills.<harness>]` (SK-6, #2769).
//!
//! # The precedence, and where it comes from
//!
//! #272's ([`crate::extension::project`], ADR 0048), per key: **Local, then the workspace, then
//! Shared**. A chat in a workspace also reads `settings.harness_skills.<harness>` in that
//! workspace's `workspace.json` ([`Choices::read_in`]), the layer between the project's two
//! files, through the same reader. With no layer naming a skill it is **not set**: the harness
//! has it as it always did.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::extension::project::{Source, toml_key};
use crate::settings::Refusal;

/// The table both files hold a harness's own skills turned on or off in (SK-6, #2769):
/// `[harness_skills.<harness>]`, `"<name>" = true|false`, by the name the harness knows the skill
/// by. Read only while the Skill studio's flag is on.
pub const SKILLS_TABLE: &str = "harness_skills";

/// What one chat is handed: purlis's pins, each plugin on (`true`) or off (`false`) by the
/// harness's own id ([`pins`]), and the harness's own skills the project turned off. A plugin or
/// skill it does not name is left to the harness, which is every plugin but purlis's (#3467).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chosen {
    /// Each pinned plugin, on or off, by the harness's id.
    pub plugins: BTreeMap<String, bool>,
    /// The harness's own skills to leave out of the chat, by the name the harness knows each by
    /// (SK-6, #2769).
    pub skills_off: BTreeSet<String>,
    /// Whether the harness has a skill of its own named [`crate::skills::INDEX`], outside any
    /// plugin, where the chat's environment finds its skills ([`crate::skills::has_own_index`]):
    /// read at the start with that environment, so the adapter that turns it off reads nothing
    /// of this machine itself (#3320).
    pub own_index: bool,
}

impl Chosen {
    /// Nothing chosen: the harness decides every plugin and skill.
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<BTreeMap<String, bool>> for Chosen {
    fn from(plugins: BTreeMap<String, bool>) -> Self {
        Self {
            plugins,
            skills_off: BTreeSet::new(),
            own_index: false,
        }
    }
}

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

    /// `name`'s value in the chat's variables, else in this process's where it may look, unless
    /// it is empty.
    pub(crate) fn var(&self, name: &str) -> Option<String> {
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
    pub(crate) fn dir(&self, name: &str, fallback: &str) -> Option<PathBuf> {
        self.var(name)
            .map(PathBuf::from)
            .or_else(|| self.home.as_ref().map(|home| home.join(fallback)))
    }
}

/// One harness, as far as its plugins and its own skills go.
pub trait Adapter: Sync {
    /// The plane's word for it: a profile's `kind`, and the key under [`SKILLS_TABLE`].
    fn harness(&self) -> &'static str;
    /// What a person calls it: its built-in declaration's `title`
    /// ([`crate::harness::Harness::title`]), the one place a title is written.
    fn title(&self) -> &'static str;
    /// Where it records what it has installed, under `env`, read and never written: where
    /// [`Self::skills_at`] finds each installed plugin.
    fn record(&self, env: &Env<'_>) -> Option<PathBuf>;
    /// Where it finds the skills it brings on its own on this machine, under `env`: the person's
    /// skill folders and each installed plugin's (SK-5, #2768). It reads only its own record of
    /// what is installed ([`Self::record`]); the host lists the skills in each folder
    /// ([`crate::skills::on_machine`]). What a skill a persona names is checked against.
    fn skills_at(&self, env: &Env<'_>) -> Vec<SkillsAt>;
    /// Everything it has installed on this machine, read from [`Self::record`] and never
    /// written, or why it could not be read: what a Try it's scratch chat turns off, so it has
    /// the draft alone (decision 50). Nothing for a harness that cannot turn one plugin off for
    /// one chat (opencode).
    fn installed(&self, _env: &Env<'_>) -> Result<Vec<Plugin>, String> {
        Ok(Vec::new())
    }
    /// Whether the harness takes a plugin turned off for one chat alone, so every one
    /// [`Self::installed`] lists can be: what a grade's chat needs ([`only_purlis_on`], #3468).
    /// The default is a harness that cannot (opencode).
    fn turns_a_plugin_off_for_one_chat(&self) -> bool {
        false
    }
    /// The folders, relative to a clone, it finds a repo's own skills in, each a folder of skill
    /// folders: what a clone a chat works in can carry for it.
    fn repo_skill_dirs(&self) -> &'static [&'static str] {
        &[]
    }
    /// Why a skill the harness knows as `name` cannot be turned off on its own, or `None` where
    /// it can: what a file that names it is refused with. A plugin's skill, `<plugin>:<skill>`,
    /// on every harness ([`plugins_skill`], #3467).
    fn skill_named_alone(&self, name: &str) -> Option<String> {
        plugins_skill(self.title(), name)
    }
    /// What charter fixes for it, whatever a file says.
    ///
    /// The default is an empty slice, and cargo-mutants' `Vec::leak(Vec::new())` is an empty
    /// slice too: every caller only iterates it, so no input tells them apart
    /// (`.cargo/mutants.toml` excludes that one mutant).
    fn pinned(&self) -> &'static [Pin] {
        &[]
    }
}

/// **Why a plugin's skill is not purlis's to switch** (#3467): `name` is `<plugin>:<skill>`, as
/// each harness names one, and a plugin is the harness's own to turn on or off, its skills with
/// it. The Skill studio shows such a skill read-only, naming its plugin.
pub fn plugins_skill(title: &str, name: &str) -> Option<String> {
    name.split_once(':').map(|(plugin, _)| {
        format!(
            "it comes with the plugin {plugin}, which {title} turns on or off itself, so its skills are shown read-only"
        )
    })
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
    /// (`user`, `project`, `local`) and an `installPath`.
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

    /// `enabledPlugins` in the chat's `--settings`, which wins for the keys it names.
    fn turns_a_plugin_off_for_one_chat(&self) -> bool {
        true
    }

    fn skills_at(&self, env: &Env<'_>) -> Vec<SkillsAt> {
        skills::claude(self, env)
    }

    /// A repo's `.claude/skills`, which Claude Code reads as the project's own (its skills docs,
    /// "Choose where skills load").
    fn repo_skill_dirs(&self) -> &'static [&'static str] {
        &[".claude/skills"]
    }

    fn pinned(&self) -> &'static [Pin] {
        CLAUDE_PINS.as_slice()
    }
}

// ------------------------------------------------------------------------------------------
// Codex
// ------------------------------------------------------------------------------------------

/// Codex: its `config.toml`, where it records each installed plugin.
pub struct Codex;

pub static CODEX: Codex = Codex;

impl Adapter for Codex {
    fn harness(&self) -> &'static str {
        "codex"
    }

    fn title(&self) -> &'static str {
        crate::harness::Harness::Codex.title()
    }

    /// `$CODEX_HOME/config.toml`, else `~/.codex/config.toml`, where Codex records an installed
    /// plugin under `[plugins."<name>@<marketplace>"]` (its plugin docs, and the operator's
    /// 0.147.0 install).
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

    /// `-c plugins."<id>".enabled=false`, taken for one session since codex-cli 0.163.0.
    fn turns_a_plugin_off_for_one_chat(&self) -> bool {
        true
    }

    fn skills_at(&self, env: &Env<'_>) -> Vec<SkillsAt> {
        skills::codex(self, env)
    }

    /// A repo's `.agents/skills` and `.codex/skills` (`host_roots.rs` at `rust-v0.163.0`: the
    /// repo roots, and a trusted project's own `.codex/`).
    fn repo_skill_dirs(&self) -> &'static [&'static str] {
        &[".agents/skills", ".codex/skills"]
    }
}

// ------------------------------------------------------------------------------------------
// opencode
// ------------------------------------------------------------------------------------------
/// opencode: its global configuration directory.
pub struct Opencode;

pub static OPENCODE: Opencode = Opencode;

impl Adapter for Opencode {
    fn harness(&self) -> &'static str {
        "opencode"
    }

    fn title(&self) -> &'static str {
        crate::harness::Harness::Opencode.title()
    }

    /// Its global configuration directory, `$XDG_CONFIG_HOME/opencode`, else
    /// `~/.config/opencode`, whose `skill/` and `skills/` hold the person's own skills.
    fn record(&self, env: &Env<'_>) -> Option<PathBuf> {
        env.var("XDG_CONFIG_HOME")
            .map(|base| PathBuf::from(base).join("opencode"))
            .or_else(|| env.home.as_ref().map(|home| home.join(".config/opencode")))
    }

    fn skills_at(&self, env: &Env<'_>) -> Vec<SkillsAt> {
        skills::opencode(self, env)
    }

    /// A repo's `.claude/skills`, `.agents/skills`, and `skill/` and `skills/` in its
    /// `.opencode/` (`skill/index.ts` at `v1.18.33`).
    fn repo_skill_dirs(&self) -> &'static [&'static str] {
        &[
            ".claude/skills",
            ".agents/skills",
            ".opencode/skill",
            ".opencode/skills",
        ]
    }
}

/// A file's text, `None` when it is not there, or why it could not be read.
fn read(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("purlis could not read {}: {err}", path.display())),
    }
}

/// The names of a folder that are text, in the order listed: the only ones opencode's ids are
/// written in.
fn text_names(names: Vec<std::ffi::OsString>) -> Vec<String> {
    names
        .into_iter()
        .filter_map(|name| name.into_string().ok())
        .collect()
}

/// **The most names purlis reads of one folder it lists** (SK-44, #3133): the one limit of the
/// folder-handle reader ([`purlis_skills::held::MOST_NAMES`]).
#[cfg(test)]
pub(crate) use purlis_skills::held::MOST_NAMES;

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
// What a project's files say of the harness's own skills, and the precedence
// ------------------------------------------------------------------------------------------

/// What one layer says: by harness, then by plugin id.
type Said = BTreeMap<String, BTreeMap<String, bool>>;

/// What each of the three layers says of one table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Layers {
    shared: Said,
    /// Empty outside a workspace, and for a workspace whose manifest says nothing.
    workspace: Said,
    local: Said,
}

impl Layers {
    /// Each layer, the one with the last word first.
    fn last_word_first(&self) -> [(&Said, Source); 3] {
        [
            (&self.local, Source::Local),
            (&self.workspace, Source::Workspace),
            (&self.shared, Source::Shared),
        ]
    }

    /// What the first layer that names `harness`'s `key` says, and which layer that is.
    fn answer(&self, harness: &str, key: &str) -> (Option<bool>, Source) {
        self.last_word_first()
            .into_iter()
            .find_map(|(said, source)| {
                said.get(harness)
                    .and_then(|keys| keys.get(key))
                    .map(|on| (Some(*on), source))
            })
            .unwrap_or((None, Source::Default))
    }
}

/// What a project's two files say of the harness's own skills, by harness and then by skill
/// name — and, in a workspace, what that workspace's `workspace.json` says (charter-app#282), the
/// layer between them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choices {
    /// The three layers' `[harness_skills]` (SK-6, #2769), by harness, then by skill name.
    skills: Layers,
}

impl Choices {
    /// The two files' text: `None` for a file that is not there.
    pub fn from_text(shared: Option<&str>, local: Option<&str>) -> Self {
        let said = |text: &str| {
            text.parse::<toml::Table>()
                .map(|top| said_in(&top, SKILLS_TABLE))
                .unwrap_or_default()
        };
        Self {
            skills: Layers {
                shared: shared.map(said).unwrap_or_default(),
                workspace: Said::default(),
                local: local.map(said).unwrap_or_default(),
            },
        }
    }

    /// The same choices, in a workspace whose `workspace.json` has this text (`None`: it has
    /// none). Its `settings.harness_skills` go between Shared and Local. A manifest with none,
    /// or one that is not JSON, says nothing, which leaves the project's answer as it was.
    pub fn in_workspace(self, manifest: Option<&str>) -> Self {
        self.with_workspace(manifest.and_then(crate::settings::workspace::table_in))
    }

    fn with_workspace(mut self, settings: Option<toml::Table>) -> Self {
        self.skills.workspace = settings
            .as_ref()
            .map(|top| said_in(top, SKILLS_TABLE))
            .unwrap_or_default();
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
    }

    /// The two files as [`crate::settings::layer_text`] hands them: [`Self::from_text`] of what
    /// each layer is read as, so a Local file git would carry says nothing.
    pub fn from_layers(
        shared: &crate::settings::LayerText,
        local: &crate::settings::LayerText,
    ) -> Self {
        Self::from_text(shared.text(), local.text())
    }

    /// [`Self::read`], in `workspace` when there is one — the same reader
    /// [`crate::extension::project::Choices::read_in`] is. A name that is not one of the plane's
    /// workspaces reads as a workspace with no settings.
    pub fn read_in(root: &Path, workspace: Option<&str>) -> Self {
        let project = Self::read(root);
        match workspace {
            None => project,
            Some(name) => project.with_workspace(crate::settings::workspace::read(root, name)),
        }
    }

    /// **Whether the project has the harness's own skill `name` on or off** (SK-6, #2769), and
    /// which layer said so: Local, then the workspace, then Shared. `None` where no layer names
    /// it, and the harness has it as it always did.
    pub fn skill(&self, harness: &str, name: &str) -> (Option<bool>, Source) {
        self.skills.answer(harness, name)
    }

    /// Every one of `adapter`'s own skills the project has off, by the harness's name for it:
    /// each name some layer sets whose answer ([`Self::skill`]) is off, and that the harness
    /// can leave out on its own ([`Adapter::skill_named_alone`]): a plugin's skill is the
    /// plugin's, and is handed nothing.
    pub fn skills_off(&self, adapter: &dyn Adapter) -> BTreeSet<String> {
        let harness = adapter.harness();
        self.skills
            .last_word_first()
            .into_iter()
            .filter_map(|(said, _)| said.get(harness))
            .flat_map(BTreeMap::keys)
            .filter(|name| self.skill(harness, name).0 == Some(false))
            .filter(|name| adapter.skill_named_alone(name).is_none())
            .cloned()
            .collect()
    }
}

/// Every well-formed `"<name>" = true|false` under each `[<table>.<harness>]` in `top` — a whole
/// TOML file, or a workspace's `settings` read as one. What is not well formed is left out here
/// and refused by [`refusals`].
fn said_in(top: &toml::Table, table: &str) -> Said {
    top.get(table)
        .and_then(toml::Value::as_table)
        .into_iter()
        .flatten()
        .filter_map(|(harness, names)| {
            let names = names
                .as_table()?
                .iter()
                .filter(|(id, _)| id_ok(id))
                .filter_map(|(id, on)| Some((id.clone(), on.as_bool()?)))
                .collect();
            Some((harness.clone(), names))
        })
        .collect()
}

/// **The pins of `adapter`, as a chat is handed them**: purlis's own plugin on, and each id
/// it replaced off. Nothing for a harness that pins nothing.
pub fn pins(adapter: &dyn Adapter) -> Chosen {
    adapter
        .pinned()
        .iter()
        .map(|pin| (pin.id.to_owned(), pin.on))
        .collect::<BTreeMap<_, _>>()
        .into()
}

/// **What a grade's chat on `adapter`'s harness is handed of its plugins** (P8, #3468): every
/// plugin the harness has installed off, by the harness's id, read from its record under `env`
/// and never written, and purlis's pins over them, so purlis's own plugin is the one on. A
/// grade measures the skill alone. Refused where the harness cannot turn a plugin off for one
/// chat, or its record cannot be read: a grade that could not say which plugins were off does
/// not run. A Try it's scratch chat takes the same set, leniently
/// (`skills::verbs::drafts::alone_on`).
pub fn only_purlis_on(
    adapter: &dyn Adapter,
    env: &Env<'_>,
) -> Result<BTreeMap<String, bool>, String> {
    if !adapter.turns_a_plugin_off_for_one_chat() {
        return Err(format!(
            "{} cannot turn a plugin off for one chat",
            adapter.title()
        ));
    }
    let mut plugins: BTreeMap<String, bool> = adapter
        .installed(env)?
        .into_iter()
        .map(|plugin| (plugin.id, false))
        .collect();
    plugins.extend(pins(adapter).plugins);
    Ok(plugins)
}

/// Whether `id` is purlis's own plugin under any id it has: a pin, or the copy `purlis plugin
/// install` writes, under its current id ([`crate::plugin::FORMERLY`]).
fn purlis_s_own(adapter: &dyn Adapter, id: &str) -> bool {
    adapter.pinned().iter().any(|pin| pin.id == id)
        || crate::plugin::FORMERLY.iter().any(|(_, new)| *new == id)
}

/// The ids in `plugins` that are off and none of purlis's own ([`purlis_s_own`]): the
/// harness's own plugins a chat was handed off, in id order, as a grade's record names them
/// (#3468).
pub fn persons_plugins_off(adapter: &dyn Adapter, plugins: &BTreeMap<String, bool>) -> Vec<String> {
    plugins
        .iter()
        .filter(|(id, on)| !**on && !purlis_s_own(adapter, id))
        .map(|(id, _)| id.clone())
        .collect()
}

/// **What a chat of `kind`, started in the plane at `root` — in `workspace`, when it is in one —
/// is handed**: its harness's pins ([`pins`]), and, only while the Skill studio's flag is on, the
/// harness's own skills the project turned off ([`Choices::skills_off`]). No other plugin is
/// named: the harness decides (#3467).
pub fn for_start(kind: &str, root: &Path, workspace: Option<&str>, env: &Env<'_>) -> Chosen {
    for_start_as(crate::skills::studio_on_here(), kind, root, workspace, env)
}

/// [`for_start`], with the Skill studio's flag `studio` as given.
pub(crate) fn for_start_as(
    studio: bool,
    kind: &str,
    root: &Path,
    workspace: Option<&str>,
    env: &Env<'_>,
) -> Chosen {
    let Some(adapter) = adapter(kind) else {
        return Chosen::new();
    };
    let mut chosen = pins(adapter);
    if studio {
        chosen.skills_off = Choices::read_in(root, workspace).skills_off(adapter);
        chosen.own_index = crate::harness::Harness::of_kind(kind)
            .is_some_and(|harness| crate::skills::has_own_index(harness, env));
    }
    chosen
}

/// Everything in `text`'s `[harness_skills]` that charter would not read or would not honour,
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
/// one (charter-app#282) — whose `[harness_skills]` sits at `at` in `file` (`"settings."` in a
/// `workspace.json`), so each sentence names the key where it is written.
pub fn refusals_in(top: &toml::Table, file: &str, at: &str) -> Vec<String> {
    keyed_in(top, file, at)
        .into_iter()
        .map(|one| one.why)
        .collect()
}

/// [`refusals_in`], each with the key it is about **in `top`** (#1292): `at` is left out, so a
/// workspace's is the key under its `settings`. A skill name is one step, dots and all.
///
/// The old `[harness_plugins]` ([`old::TABLE`]) is refused nothing: it is ignored, and said so
/// once ([`old`]), so a file that still holds it saves as it did (#3467).
pub fn keyed_in(top: &toml::Table, file: &str, at: &str) -> Vec<Refusal> {
    let name = SKILLS_TABLE;
    let Some(table) = top.get(name) else {
        return Vec::new();
    };
    let shape =
        format!("each harness is [{name}.<harness>], holding \"<skill name>\" = true or false");
    let Some(table) = table.as_table() else {
        return vec![Refusal::at(
            format!("{at}{name} in {file} is not a table — {shape}"),
            &[name],
        )];
    };
    let mut out = Vec::new();
    for (harness, skills) in table {
        let Some(adapter) = adapter(harness) else {
            let known: Vec<&str> = adapters().map(|it| it.harness()).collect();
            out.push(Refusal::at(
                format!(
                    "[{at}{name}.{}] in {file} is not a harness purlis knows — one of: {}",
                    toml_key(harness),
                    known.join(", ")
                ),
                &[name, harness.as_str()],
            ));
            continue;
        };
        let Some(skills) = skills.as_table() else {
            out.push(Refusal::at(
                format!("{at}{name}.{harness} in {file} is not a table — {shape}"),
                &[name, harness.as_str()],
            ));
            continue;
        };
        for (id, on) in skills {
            let key = format!("{at}{name}.{harness}.{}", toml_key(id));
            let keyed = [name, harness.as_str(), id.as_str()];
            if !id_ok(id) {
                out.push(Refusal::at(
                    format!(
                        "{key} in {file} is not a skill name — one line of at most \
                         {MOST_ID_BYTES} bytes, with nothing invisible in it"
                    ),
                    &keyed,
                ));
                continue;
            }
            if on.as_bool().is_none() {
                out.push(Refusal::at(
                    format!("{key} in {file} is not true or false"),
                    &keyed,
                ));
                continue;
            }
            if let Some(why) = adapter.skill_named_alone(id) {
                out.push(Refusal::at(
                    format!("{key} in {file} cannot be set: {why}"),
                    &keyed,
                ));
            }
        }
    }
    out
}

mod skills;

pub use skills::SkillsAt;

/// "<name> is one of purlis's built-in skills, which no project turns off" where `name` is one
/// of `built_in`, the built-in skills' names (SK-6, #2769): a built-in skill is protected on every
/// harness, and a name a harness knows its own by is matched as a built-in one's would be.
fn built_in_said(name: &str, built_in: &[String]) -> Option<String> {
    built_in
        .iter()
        .any(|one| one == name)
        .then(|| format!("{name} is one of purlis's built-in skills, which no project turns off"))
}

/// **What `[harness_skills]` in `top` may not say because of a built-in skill** (SK-6, #2769):
/// any harness's key that is one of `built_in`, the built-in skills' names, each refused in a
/// sentence with its key. A built-in skill is never turned off, and opencode's and Claude Code's
/// switches go by the name alone, so one named so would reach purlis's own.
pub fn built_in_refused(
    top: &toml::Table,
    file: &str,
    at: &str,
    built_in: &[String],
) -> Vec<Refusal> {
    top.get(SKILLS_TABLE)
        .and_then(toml::Value::as_table)
        .into_iter()
        .flatten()
        .filter_map(|(harness, names)| Some((harness, names.as_table()?)))
        .flat_map(|(harness, names)| {
            names.keys().filter_map(move |name| {
                let why = built_in_said(name, built_in)?;
                Some(Refusal::at(
                    format!(
                        "{at}{SKILLS_TABLE}.{harness}.{} in {file} cannot be set: {why}",
                        toml_key(name)
                    ),
                    &[SKILLS_TABLE, harness.as_str(), name.as_str()],
                ))
            })
        })
        .collect()
}

/// **The Skill studio's switch** (SK-6, #2769): turns `harness`'s own skill `id` on or off for
/// the chats the studio shows: at the project's root (`workspace` `None`) in
/// `charter.local.toml`, this machine's own, since a harness's skills are the person's own
/// install; in a workspace, in its `workspace.json`, which is the team's for that workspace.
///
/// It writes the least that makes the answer `on`: this layer's key is taken out, and written
/// only where another layer would still say otherwise. So turning one back on where nothing else
/// named it leaves the harness to decide, as it did. Refused, writing nothing, for a skill the
/// harness cannot leave out alone (a plugin's, #3467), a built-in skill's name (`built_in` holds
/// their names), and anything a save of that file refuses.
pub fn switch_in_the_studio(
    root: &Path,
    workspace: Option<&str>,
    harness: crate::harness::Harness,
    id: &str,
    on: bool,
    built_in: &[String],
) -> Result<(), String> {
    use crate::settings::{Edit, Step, Value, Which};
    let adapter = harness.adapter().plugins();
    let kind = adapter.harness();
    if !id_ok(id) {
        return Err("that is no name purlis can write, so nothing was changed".to_owned());
    }
    if let Some(why) = built_in_said(id, built_in).or_else(|| adapter.skill_named_alone(id)) {
        return Err(format!("{why}, so nothing was changed"));
    }
    let path = vec![
        Step::Key(SKILLS_TABLE.to_owned()),
        Step::Key(kind.to_owned()),
        Step::Key(id.to_owned()),
    ];
    let is_on = |choices: &Choices| choices.skill(kind, id).0 != Some(false);
    match workspace {
        None => {
            let read = crate::settings::read(root, Which::Local)?;
            let base = read.exists.then_some(read.text.as_str());
            let without = crate::settings::edited(
                &read.text,
                &[Edit {
                    path: path.clone(),
                    value: None,
                }],
            )?;
            let shared = crate::settings::layer_text(root, Which::Shared);
            let after = Choices::from_text(shared.text(), Some(&without));
            let text = if is_on(&after) == on {
                without
            } else {
                crate::settings::edited(
                    &read.text,
                    &[Edit {
                        path,
                        value: Some(Value::Bool(on)),
                    }],
                )?
            };
            crate::settings::save(root, Which::Local, base, &text).map_err(|why| why.join(" "))
        }
        Some(ws) => {
            let read = crate::settings::workspace::read_file(root, ws)?;
            let base = read.exists.then_some(read.text.as_str());
            let mut settings = crate::settings::workspace::read(root, ws).unwrap_or_default();
            if let Some(harnesses) = settings
                .get_mut(SKILLS_TABLE)
                .and_then(toml::Value::as_table_mut)
                && let Some(ids) = harnesses.get_mut(kind).and_then(toml::Value::as_table_mut)
            {
                ids.remove(id);
            }
            let after = Choices::read(root).with_workspace(Some(settings));
            let value = (is_on(&after) != on).then_some(Value::Bool(on));
            crate::settings::workspace::save(root, ws, base, &[Edit { path, value }])
                .map_err(|why| why.join(" "))
        }
    }
}

mod outside;
pub use outside::{harness_skills, repo_skills};

pub mod old;

#[cfg(test)]
mod tests;
