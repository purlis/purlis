// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A harness adapter (ADR 0073, FD-13): charter code that arms one harness through the
//! harness's own mechanism, for one chat, with nothing written into the harness's config.
//!
//! **Behaviour, not facts.** ADR 0073 §3 splits what charter knows about a harness in two. Its
//! facts (the flags that start and resume a session, the bytes a newline is, what it reports)
//! become a harness declaration's fields in FD-14, and stay [`super::Harness`]'s methods until
//! then. What is behaviour stays charter code, here: how a chat is armed so that its hooks report
//! in charter's words ([`crate::state::Event::said`] reads them), what would run it unarmed,
//! and which of its own plugins it can be handed.
//!
//! One adapter per harness, and [`super::Harness::adapter`] is their registry:
//! [`super::claude::ADAPTER`], [`super::codex::ADAPTER`] and [`super::opencode::ADAPTER`]. An
//! adapter also compiles a plane's sandbox for its harness, where charter has written that
//! compiler (ADR 0067): [`HarnessAdapter::sandbox_compiler`] and
//! [`HarnessAdapter::sandboxed_line`]. And it says where a project's command rule goes in the
//! harness's own files, or why it has nowhere to put one (#922):
//! [`HarnessAdapter::command_rule`], which `purlis guard` writes through.

use super::{Harness, Kit, StateHooks};

/// One harness, as charter arms it.
pub trait HarnessAdapter: Sync {
    /// The harness this adapter arms.
    fn harness(&self) -> Harness;

    /// How a chat that will run in `cwd` is armed with what the app ships in `kit`, handed
    /// `plugins` (the harness's own plugins the project chose for this chat) and `sandbox`.
    ///
    /// **Fail closed** (ADR 0067). A sandbox compiled for another harness arms nothing, so the
    /// chat is refused rather than started without it. The check is here, in the one way into
    /// an adapter's arming, and not left to a caller: [`Self::arm_under`] can only be handed a
    /// [`Sandbox`], which nothing outside this module can make.
    fn arm(
        &self,
        kit: Kit<'_>,
        cwd: Option<&std::path::Path>,
        plugins: &crate::harness_plugin::Chosen,
        sandbox: Option<&crate::sandbox::Applied>,
    ) -> StateHooks {
        if sandbox.is_some_and(|applied| applied.harness() != self.harness()) {
            return StateHooks::None;
        }
        self.arm_under(kit, cwd, plugins, Sandbox(sandbox))
    }

    /// [`Self::arm`], once `sandbox` is known to be compiled for this harness. Each adapter
    /// writes this; every caller calls [`Self::arm`].
    fn arm_under(
        &self,
        kit: Kit<'_>,
        cwd: Option<&std::path::Path>,
        plugins: &crate::harness_plugin::Chosen,
        sandbox: Sandbox<'_>,
    ) -> StateHooks;

    /// Why a chat started with `command` and `env` would run without the hooks [`Self::arm`]
    /// gave it, or `None` when it would not. The default is a harness with no such switch.
    fn disarmed_by(&self, command: &[String], env: &[(String, String)]) -> Option<String> {
        let _ = (command, env);
        None
    }

    /// This harness's sandbox compiler (ADR 0067), or `None` where charter has not written one
    /// yet, which refuses every chat of it in a sandboxed plane
    /// ([`crate::sandbox::NotStarted::NoCompiler`]).
    fn sandbox_compiler(&self) -> Option<crate::sandbox::Compiler>;

    /// The issue this harness's sandbox is held back for, where charter has a compiler for it
    /// and still refuses a sandboxed start ([`crate::sandbox::NotStarted::HeldBack`]): its own
    /// sandbox cannot be kept to what the compiler says while it runs. `None` for every
    /// harness that starts sandboxed.
    fn sandbox_held_back(&self) -> Option<u32> {
        None
    }

    /// Whether what the harness itself starts, its hooks and its MCP servers, runs inside the
    /// chat's sandbox, as the commands the chat runs do (#1421). `false` where the sandbox
    /// confines only the harness's own tool, so a refusal one of those meets is not the
    /// sandbox's and is never worded as one ([`crate::sandbox::writes_are_sandboxed`]). No
    /// default: each adapter says how its sandbox is applied.
    fn sandbox_holds_what_it_starts(&self) -> bool;

    /// **Whether a grade's chats can run on this harness** (SK-16, ADR 0105, #3310): purlis can
    /// hold one of its chats to the grade's limits there (to purlis's MCP server alone and its
    /// read tools) and read what it spent, the operator's ruling of 2026-10-26. No default: each
    /// adapter says, and every grade's chat on one that says `false` is refused
    /// ([`crate::skills::grades::chat_refused`]).
    fn can_grade(&self) -> bool;

    /// **Why a grade's chat on this harness may not start** as `command` (the program, then the
    /// profile's own words) under the chat's environment `env` would start it, or `None` where it
    /// may: a command, or the person's own settings, that keep the chat from ever asking, so a
    /// grade's "end at the first ask" would never fire. Asked only where [`Self::can_grade`]
    /// says `true`. The default is a harness with no such switch.
    fn grade_refused(
        &self,
        command: &[String],
        env: &crate::harness_plugin::Env<'_>,
    ) -> Option<String> {
        let _ = (command, env);
        None
    }

    /// **The model a grade's chat on this harness runs on**, as `command` (the program, then the
    /// profile's own words) under the chat's environment `env` would start it, where the
    /// harness's words say one (#3469): what its dollar limit is priced from
    /// ([`crate::skills::grades::price`]). `None` where they leave it to the harness's own
    /// default, which the limit then prices at the dearest. Asked only where
    /// [`Self::can_grade`] says `true`; the default is a harness whose model purlis does not
    /// read.
    fn grade_model(
        &self,
        command: &[String],
        env: &crate::harness_plugin::Env<'_>,
    ) -> Option<String> {
        let _ = (command, env);
        None
    }

    /// The chat's whole line under `form`, the sandbox [`Self::sandbox_compiler`] compiled,
    /// from `words`: the program, the profile's `command`, then `armed` (what [`Self::arm`]
    /// gave), then `charters`, charter's own words — where the chat opens, `at`; or the one
    /// sentence saying why it may not start. Only [`crate::sandbox::Applied::line`] asks, with
    /// the form compiled for this harness.
    ///
    /// **Fail closed.** A form this adapter did not compile, a flag in the chat's own words
    /// that would outrank the sandbox, and a wrap without what it needs at `at` each refuse
    /// the chat.
    fn sandboxed_line(
        &self,
        form: &crate::sandbox::Form,
        words: crate::sandbox::Words,
        at: &crate::sandbox::At<'_>,
    ) -> Result<crate::sandbox::Line, String>;

    /// `reference` — a file, a folder or a range of lines of one (FM-9) — in this harness's
    /// own syntax for naming one in its input, as its own file completion writes it. Each
    /// adapter says which version it measured and how; nothing is sent, so nothing here may
    /// hold a line break.
    fn reference(&self, reference: &crate::reference::Reference) -> String;

    /// What the app arms a chat of this harness with, as `charter doctor` says it.
    fn armed_with(&self) -> String;

    /// The harness's own plugins: what it has installed and whether a chat can be handed a
    /// set of them (ADR 0050).
    fn plugins(&self) -> &'static dyn crate::harness_plugin::Adapter;

    /// The name `purlis guard` prints this harness under. It is a recorded contract, kept as
    /// it was printed before the rule writers moved onto the adapters (#922), and it is not
    /// [`Harness::name`].
    fn guard_label(&self) -> &'static str;

    /// Where this harness holds `rule` for the project at `root`: a rule as
    /// [`crate::guardcmd::as_rule`] writes it (`Bash(...)`, or a tool's own rule), in the
    /// committed file, or with `local` in the machine-local one. Or the measured reason this
    /// harness has nowhere to put it, which `purlis guard` says rather than resolves.
    ///
    /// Nothing is read or written here: [`RuleFile::write`] does that, through the same
    /// writers `purlis init` uses ([`crate::scaffold::settings`]).
    fn command_rule(
        &self,
        root: &std::path::Path,
        rule: &str,
        local: bool,
    ) -> Result<RuleFile, String>;

    /// **What a finished tool call of this harness says about a skill starting** (SK-3), read
    /// from the call's tool, by the harness's own name for it, and its input. Pure: nothing is
    /// read but the two. The default is a harness that shows none.
    fn skill_start(&self, tool: &str, input: &serde_json::Value) -> Option<SkillStart> {
        let _ = (tool, input);
        None
    }

    /// **What a chat of this harness types to start `name`, a skill of its set from `origin`,
    /// with the harness's own skill tool, where that start applies what the skill narrows**
    /// (SK-13; ADR 0063 as amended 2026-10-20, §2): the one way a skill `skill_load` is asked for
    /// keeps its narrowing. `None` for a harness whose own start cannot apply it, the default:
    /// a load there says "narrowing not applied". Pure.
    fn narrowed_start(&self, name: &str, origin: crate::skills::Origin) -> Option<String> {
        let _ = (name, origin);
        None
    }

    /// **The name this harness knows a skill of the chat's set by** (SK-12): what its own Skill
    /// tool and its picker call it, and what a run of it records. The default is the skill's
    /// own name, for a harness that does not namespace them.
    fn skill_name(&self, name: &str, origin: crate::skills::Origin) -> String {
        let _ = origin;
        name.to_owned()
    }

    /// **The text that starts `skill` in this harness's own syntax** (SK-12, decision 40), with
    /// the arguments the person gave in its own syntax too (decision 61): what Use skill… types
    /// into the chat, unsent. Pure, and one line: nothing here may hold a line break. Each
    /// adapter says which version it measured and how.
    fn skill_start_text(&self, skill: &SkillToStart<'_>) -> String;

    /// **This harness's skill budget** (SK-12, ADR 0102 §1): how many skills, and how many
    /// characters of their descriptions, one chat's listing may hold, set below the harness's
    /// own limit so it never has to cut anything itself. No default: each adapter says its
    /// harness's limit.
    fn skill_budget(&self) -> crate::skills::Budget;

    /// **What this harness reads of a skill's frontmatter** (SK-8, #2771; decision 18): the
    /// keys it applies to a skill purlis hands it, and how it keeps a manual-only skill out of
    /// its model's choice. What every skill's compatibility badge for it, the `SKILL.md` it is
    /// handed ([`crate::skills::handed_text`]) are worked out from. No default: each adapter
    /// says which version it read and how.
    fn skill_frontmatter(&self) -> crate::skills::Honours;

    /// **The file beside a `SKILL.md` this harness reads to keep one of the skills it finds
    /// itself out of its model's choice** (SK-39), and how a text of it says so: what the badge
    /// of each of its own and its repo skills reads. Pure: the host reads the file. `None`, the
    /// default, for a harness that keeps one out by its frontmatter alone.
    fn manual_beside(&self) -> Option<crate::skills::ManualBeside> {
        None
    }

    /// **Whether this harness names the skill a prompt the person sent started** (SK-12): a
    /// hook it fires says so ([`Self::prompt_started`]), and a skill Use skill… typed is then
    /// confirmed by that name. `false`, the default, for a harness that does not, whose typed
    /// skill is confirmed by the prompt holding its text ([`crate::skillspan::Confirmed`]).
    fn typed_start_named(&self) -> bool {
        false
    }

    /// **The skill a hook's payload says a prompt the person sent started**, by the name this
    /// harness knows it by, or `None` where the payload says no such thing. Pure: nothing is
    /// read but the payload. The default is a harness that says none.
    fn prompt_started(&self, payload: &serde_json::Value) -> Option<String> {
        let _ = payload;
        None
    }
}

/// A skill of a chat's set, as Use skill… starts it: by its own name, from its level, with what
/// the person gave its arguments ([`HarnessAdapter::skill_start_text`]).
#[derive(Debug, Clone, Copy)]
pub struct SkillToStart<'a> {
    /// Its frontmatter's `name`.
    pub name: &'a str,
    pub origin: crate::skills::Origin,
    pub given: &'a crate::skills::Given,
}

/// **`value` as one shell-style word**, as Claude Code's `$N` and opencode's `$1..$n` split a
/// skill's arguments: as it is where it holds no space or quote, in double quotes where it holds
/// a space or a single quote, in single quotes where it holds a double quote, and empty as `""`
/// so it keeps its place. A value with both quotes has its double quotes made single, the one
/// change made to what was typed.
pub(crate) fn shell_word(value: &str) -> String {
    let plain = |c: char| !(c.is_whitespace() || c == '"' || c == '\'');
    if value.is_empty() {
        "\"\"".to_owned()
    } else if value.chars().all(plain) {
        value.to_owned()
    } else if !value.contains('"') {
        format!("\"{value}\"")
    } else if !value.contains('\'') {
        format!("'{value}'")
    } else {
        format!("\"{}\"", value.replace('"', "'"))
    }
}

/// `head` and what `given` gives, as shell-style words after it: what Claude Code's and
/// opencode's slash commands take. A one-line value is handed over as it was typed.
pub(crate) fn slash_with_words(head: &str, given: &crate::skills::Given) -> String {
    use crate::skills::Given;
    match given {
        Given::Nothing => head.to_owned(),
        Given::Raw(line) => format!("{head} {line}"),
        Given::Named(values) => std::iter::once(head.to_owned())
            .chain(values.iter().map(|one| shell_word(&one.value)))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// What a finished tool call says about a skill starting, in a harness's own words (SK-3): the
/// neutral core finds the run it starts from this ([`crate::toolhooks::skill_started`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillStart {
    /// The harness's own Skill tool started the skill it names.
    Named(String),
    /// The call only read: `text` names what it read. A chat briefed on its skills starts one
    /// by reading its `SKILL.md`.
    Reading(String),
}

/// The text field `key` of a tool call's input, where it is one and not blank.
pub(crate) fn input_text<'a>(input: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    input
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

/// A command rule's writer: handed the bucket (`ask` or `allow`) and whether to write nothing.
type RuleWriter = Box<dyn Fn(&str, bool) -> crate::scaffold::settings::Wrote>;

/// One harness's file for a command rule, and the writer that puts the rule there
/// ([`HarnessAdapter::command_rule`]).
pub struct RuleFile {
    path: std::path::PathBuf,
    write: RuleWriter,
}

impl RuleFile {
    /// The file at `path`, written by `write(bucket, dry_run)`.
    pub(super) fn new(
        path: std::path::PathBuf,
        write: impl Fn(&str, bool) -> crate::scaffold::settings::Wrote + 'static,
    ) -> Self {
        Self {
            path,
            write: Box::new(write),
        }
    }

    /// The file the rule goes into.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Put the rule into `bucket` (`ask` or `allow`) of the file, or with `dry_run` say what
    /// that would do with nothing written. The writer touches the one key and never repairs a
    /// file it cannot read.
    pub fn write(&self, bucket: &str, dry_run: bool) -> crate::scaffold::settings::Wrote {
        (self.write)(bucket, dry_run)
    }
}

impl std::fmt::Debug for RuleFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuleFile")
            .field("path", &self.path)
            .finish()
    }
}

/// A chat's sandbox, checked to be compiled for the adapter it is handed to, or none. Only
/// [`HarnessAdapter::arm`] makes one, so an adapter's own arming cannot be reached with a
/// sandbox of another harness.
#[derive(Debug, Clone, Copy)]
pub struct Sandbox<'a>(Option<&'a crate::sandbox::Applied>);

impl<'a> Sandbox<'a> {
    /// The sandbox, compiled for this adapter's harness, or `None` where the plane has none.
    pub fn applied(self) -> Option<&'a crate::sandbox::Applied> {
        self.0
    }
}

/// Why a chat handed a sandbox form its adapter did not compile is not started: the sentence
/// [`HarnessAdapter::sandboxed_line`] refuses it with.
pub(super) fn not_compiled_for(harness: Harness) -> String {
    format!(
        "the sandbox this {} chat was handed was not compiled for it, so nothing was started",
        harness.title()
    )
}

#[cfg(test)]
mod tests;
