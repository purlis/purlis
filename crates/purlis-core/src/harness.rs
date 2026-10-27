// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Which harness a session is running, and the arguments that start or resume one.
//!
//! Every measured value here is a reading of one harness version, and is read from that
//! harness's built-in declaration (`harness_declaration/*.toml`, FD-14b): the facts are data,
//! and only what arms a harness is code. Nothing here reads a harness's output: the arguments are decided before the
//! program starts, and a resume is decided from what the app itself recorded.

use std::fmt;
use std::sync::LazyLock;

pub mod adapter;
pub mod asked;
pub mod asks;
pub mod claude;
pub mod codex;
pub mod hooked;
pub mod model;
pub mod opencode;
#[cfg(test)]
pub(crate) mod testing;

pub use adapter::{HarnessAdapter, SkillStart, SkillToStart};

/// **purlis's skill budget for one chat** (SK-12, #2775; ADR 0102 §1 as amended 2026-10-21): 30
/// skills and 6,000 characters of their descriptions, on every harness. Under the smallest
/// harness limit measured on 2026-10-20 (Claude Code's 1% of a 200k-token window, about 8,000
/// characters for names, descriptions and their lines together), with room for 30 names and the
/// lines they sit on, so no harness has to cut anything. Each adapter adds its own cap on one
/// description and whether it lists a skill's path ([`HarnessAdapter::skill_budget`]); whether a
/// manual-only skill is free follows from what it reads ([`Harness::skill_budget`]).
pub(crate) const SKILL_BUDGET: crate::skills::Budget = crate::skills::Budget {
    skills: 30,
    chars: 6_000,
    per_description: None,
    lists_paths: false,
};
pub use claude::SMART_CLOSE_ALLOW;

/// A harness session's id, held to a shape that cannot be read as a flag.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(String);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SessionIdError {
    #[error(
        "a session id is 1 to 128 characters of letters, digits, `_` or `-`, starting with a letter or a digit; this one is {0:?}"
    )]
    Malformed(String),
}

impl SessionId {
    /// Takes `text` as a session id, or refuses it.
    ///
    /// The shape is the Python charter's (`charter/frame/state.py:1128`), so both
    /// implementations take and refuse the same ids. It starts with a letter or a digit,
    /// which is what makes an id off disk safe to put in argv: it can never be read as a flag.
    pub fn new(text: impl Into<String>) -> Result<Self, SessionIdError> {
        let text = text.into();
        // Length first: a value that is far too long is refused without walking it.
        if text.is_empty() || text.len() > MOST {
            return Err(SessionIdError::Malformed(text));
        }
        let mut bytes = text.bytes();
        let starts = bytes.next().is_some_and(|b| b.is_ascii_alphanumeric());
        let rest = bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        if starts && rest {
            Ok(Self(text))
        } else {
            Err(SessionIdError::Malformed(text))
        }
    }

    /// A fresh id for a harness whose session id charter chooses.
    pub fn fresh() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The longest session id both implementations take: the Python charter's regex allows a
/// first character and 127 more.
const MOST: usize = 128;

/// A harness the app knows how to start and resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Harness {
    ClaudeCode,
    Codex,
    /// Armed through a plugin the app ships ([`crate::opencode`], ADR 0058).
    Opencode,
}

impl Harness {
    /// Every harness charter starts, each named once.
    pub const ALL: [Harness; 3] = [Harness::ClaudeCode, Harness::Codex, Harness::Opencode];

    /// The harness a command is, judged by the name it is invoked under, or none for a
    /// program that is not one — a shell, or a harness charter has not measured.
    pub fn of_command(program: &str) -> Option<Self> {
        // The file name, so an absolute path to the same binary is the same harness. The
        // word is matched whole: `claude-something` is not Claude Code.
        let named = std::path::Path::new(program).file_name()?.to_str()?;
        Self::ALL
            .into_iter()
            .find(|harness| harness.declared().program == named)
    }

    /// The harness a profile's DECLARED `kind` names, or none for a kind this app does not
    /// start.
    ///
    /// Not [`Self::of_command`], and the difference is the whole reason both exist.
    /// `of_command` INFERS a harness from a program name and cannot tell a shell from a
    /// harness charter has not measured — both are `None`, deliberately, and no refusal may
    /// rest on telling them apart. A `kind` is not inferred: the operator wrote one of three
    /// words in `charter.local.toml` and charter's own validator already refused anything
    /// else. So this answer IS knowable, and a launch may refuse on it.
    ///
    /// All three kinds `profiles` reads are started since #371, when opencode joined.
    pub fn of_kind(kind: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|harness| harness.declared().name == kind)
    }

    /// **The harness of a chat that names `kind`** in [`crate::hookwire::HARNESS_ENV`], where
    /// purlis ships it: by the registry's name the chat carries (`claude-code`,
    /// [`crate::profiles::Kind::registry`]) or by its kind's word. The one rule for a chat that
    /// names none: it is a terminal's own Claude Code, with purlis's plugin. A kind purlis ships
    /// no adapter for is none.
    /// A chat that names an empty one names none ([`crate::hookwire::chat_harness`] reads it so).
    pub fn of_chat(kind: Option<&str>) -> Option<Self> {
        kind.map_or(Some(Self::ClaudeCode), Self::of_registry_or_kind)
    }

    /// **The harness the registry's name `name` is** (`claude-code`, `codex`, `opencode`;
    /// [`crate::profiles::Kind::registry`]), which is what a chat's start puts into
    /// [`crate::hookwire::HARNESS_ENV`], or the kind's word a profile is written with. The one
    /// rule from a registry's name to a harness; none for a name purlis ships no adapter for.
    pub fn of_registry_or_kind(name: &str) -> Option<Self> {
        let word = crate::profiles::KINDS
            .iter()
            .find(|known| known.registry == name)
            .map_or(name, |known| known.word);
        Self::of_kind(word)
    }

    /// This harness's built-in declaration (FD-14b, #966): every fact measured on it is read
    /// from there, and only what arms it (its adapter) is code.
    pub fn declared(self) -> &'static crate::harness_declaration::Declaration {
        crate::harness_declaration::of(self)
    }

    /// Whether this harness picks a conversation back up by its id (its declaration's
    /// `resumes_by_id`): what ADR 0076 §5 calls resuming natively, which lets an idle run be
    /// hibernated rather than ended.
    pub fn resumes_by_id(self) -> bool {
        self.holds("resumes_by_id")
    }

    /// Whether this harness's declaration answers `capability` yes.
    fn holds(self, capability: &str) -> bool {
        self.declared().capability(capability).holds()
    }

    /// The word the plane calls this harness by (`charter.local.toml`'s `kind`).
    pub fn name(self) -> &'static str {
        &self.declared().name
    }

    /// Whether this harness tells a hook which process it is running under.
    ///
    /// Claude Code sets `$CLAUDE_PID` in every hook's environment, and that is the whole of
    /// what tells `/clear` (a new conversation from the same process, ADR 0024 C6) from a
    /// `claude` started inside the chat's own shell (a new conversation from a different
    /// one, C5). Codex names no such variable, so it gets the narrower rule: the first
    /// report of a chat is adopted and a later different id is ignored. opencode names none
    /// either, and gets the same rule.
    pub fn reports_its_process(self) -> bool {
        self.holds("reports_its_process")
    }

    /// The variables of the app's own environment this harness reads for itself, passed to a
    /// chat that runs it on top of [`crate::chatenv::PASSED`]. A trailing `*` is a prefix.
    ///
    /// Each harness's own configuration namespace, and nothing else: a name in it that reads
    /// as a credential is still held back unless the operator lists it
    /// ([`crate::chatenv::inherited`]), and a harness's identity is never passed at all.
    pub fn env_passed(self) -> &'static [String] {
        &self.declared().env
    }

    /// Whether charter chooses this harness's session id and hands it over at the start, so
    /// the link exists before the harness does.
    ///
    /// Measured live on Claude Code 2.1.272 (C1): `--session-id` is reported at SessionStart as
    /// given. Codex names no flag to choose an id (openai/codex#14482), so its id is only ever
    /// what it reports itself — through a hook, inside its first turn (X1). opencode has no
    /// flag that takes an id for a new session: `-s` resumes one that exists, and its id is
    /// what its plugin reports, at the first prompt (ADR 0058).
    pub fn chooses_session_id(self) -> bool {
        self.declared().session.chosen_by == crate::harness_declaration::ChosenBy::Charter
    }

    /// Whether this harness finds a conversation by the directory it ran in, so a chat whose
    /// directory moves can no longer be resumed by its id (charter#367, D10).
    ///
    /// - **Claude Code** files each transcript under `~/.claude/projects/<encoded cwd>/`, and
    ///   `--resume <id>` looks there, under the directory it is started in. After
    ///   `charter workspace rename` the chat starts in the new directory and its conversation
    ///   stays under the old one. charter does not move the harness's files (ADR 0050).
    /// - **Codex** keeps its rollouts by date under `~/.codex/sessions/`, and
    ///   `codex resume <id>` finds one by its id from any directory.
    /// - **opencode** keys a session by its project, which is the repository's first commit
    ///   (or `global` outside git), not by a path, and `-s <id>` names the session itself.
    pub fn keeps_conversations_by_directory(self) -> bool {
        self.holds("keeps_conversations_by_directory")
    }

    /// The arguments that start a new session under `id`, or none for a harness that
    /// chooses its own id.
    pub fn new_session_argv(self, id: &SessionId, name: &str) -> Vec<String> {
        self.declared().new_session_argv(id, name)
    }

    /// Whether `args` already name a session themselves, so charter adds none of its own.
    ///
    /// The operator's flag wins: two `--resume` on one command line is not a harness charter
    /// has measured, and the one the operator typed is the one they meant. The words are the
    /// declaration's `named_by` (`charter/harness/claude_code.py:298`,
    /// `charter/harness/codex.py:261`, and `opencode --help` on 1.18.23); Codex's are
    /// subcommands rather than flags, which is why they carry no dashes.
    pub fn session_named_in(self, args: &[String]) -> bool {
        self.declared().session_named_in(args)
    }

    /// The arguments that bring the conversation `id` back, or none where charter has not
    /// measured how this harness resumes.
    pub fn resume_argv(self, id: &SessionId, name: &str) -> Option<Vec<String>> {
        self.declared().resume_argv(id, name)
    }
}

/// What a harness can tell charter about its own state, and how it is asked to.
/// When a chat just opened on a harness can have a curation prompt typed into it
/// ([`Harness::ready_to_type`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadyToType {
    /// On the first `SessionStart` hook report that began a session, once the terminal is raw.
    WhenItReportsItsStart,
    /// Once the terminal is raw and the harness has then written nothing for a quiet period.
    WhenRawAndQuiet,
}

/// The biggest paste a harness draws whole in its input ([`Harness::longest_paste_drawn_whole`]):
/// at most `lines` lines and at most `chars` characters. `usize::MAX` is no limit measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawnWhole {
    pub lines: usize,
    pub chars: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateHooks {
    /// Armed on this session alone, by the arguments and the environment given.
    ///
    /// Nothing outside this session is changed: no config folder is written, and a harness
    /// the operator runs in a terminal is untouched.
    ThisSessionOnly {
        args: Vec<String>,
        /// Variables the session's own process needs, beside the ones the app always sets.
        env: Vec<(String, String)>,
        /// Events it cannot report, by the word `charter hook` takes. Empty is the whole set.
        cannot_report: Vec<&'static str>,
    },
    /// Charter has not measured how this program reports anything, or it is not a harness.
    None,
}

/// What the app ships that a chat is armed with: its own `charter`, and its own plugin.
#[derive(Debug, Clone, Copy)]
pub struct Kit<'a> {
    /// The `charter` every hook runs.
    pub binary: &'a std::path::Path,
    /// The bundled Claude Code plugin ([`crate::plugin`]), where the app found it. Its
    /// [`crate::opencode::shim_in`] is the plugin an opencode chat loads.
    pub plugin: Option<&'a std::path::Path>,
    /// The persona the chat runs as, in its project: what an adapter that can hands the chat
    /// of that persona's own (#1451: its MCP servers and its denied tools,
    /// [`crate::personaverbs::chatstart`]). `None` for a chat on no persona.
    pub persona: Option<As<'a>>,
    /// The plane whose skills the chat gets beside the built-in ones, and where it works in it
    /// (ADR 0099): `None` while the `skill-studio` flag is off, and then a chat gets the built-in
    /// skills alone, as it always did.
    pub skills: Option<crate::skills::PlaneSkills<'a>>,
    /// **A grade's chat** (SK-16, #2779; the operator's ruling of 2026-10-26): armed with
    /// purlis's MCP server alone, and that server's read tools alone, where the harness can be
    /// told so. A grade runs on such a harness only (`skills::grades::chat_refused`).
    pub graded: bool,
    /// The set's folder the host laid out before arming the chat
    /// ([`crate::skills::lay_out_for_chat`]): the plane's skills of its set, and on a route that
    /// names the built-in skills their own (`purlis-<name>`,
    /// [`crate::skills::Route::names_built_ins_its_own`]) those too. `None` where it laid out
    /// none: a harness that needs the folder is then handed none of what it holds.
    pub laid_out: Option<&'a std::path::Path>,
}

/// The persona a chat runs as, and the project that defines it.
#[derive(Debug, Clone, Copy)]
pub struct As<'a> {
    /// The project's root, which the persona's definition and this machine's approvals are
    /// read under.
    pub root: &'a std::path::Path,
    pub persona: &'a str,
}

impl Harness {
    /// How this harness is asked to report its state and run charter's guard, with what the
    /// app ships in `kit`, for a chat that will run in `cwd`.
    ///
    /// `cwd` decides one thing only: whether charter may also fill Claude Code's status line
    /// for this chat ([`crate::footerclaim`]). It is the chat's own directory because project
    /// settings are read from the session's own directory and the host does not walk up.
    ///
    /// `plugins` is the harness's own plugins the project turned on or off for this chat
    /// ([`crate::harness_plugin::chosen`], charter-app#274), by the harness's id. Only an
    /// adapter that applies per chat hands it anything, so for Codex it is always empty; it is
    /// taken here all the same, so no harness can be armed past it.
    pub fn state_hooks(
        self,
        kit: Kit<'_>,
        cwd: Option<&std::path::Path>,
        plugins: &crate::harness_plugin::Chosen,
        sandbox: Option<&crate::sandbox::Applied>,
    ) -> StateHooks {
        // The adapter refuses a sandbox compiled for another harness ([`HarnessAdapter::arm`]).
        self.adapter().arm(kit, cwd, plugins, sandbox)
    }

    /// Why a chat of this harness started with `command` and `env` would run without charter's
    /// hooks — `None` when it would not. Only opencode has such a switch charter can see
    /// ([`crate::opencode::disarmed_by`]): Claude Code loads `--plugin-dir` whatever else it is
    /// told, and Codex's session flags are charter's own.
    pub fn disarmed_by(self, command: &[String], env: &[(String, String)]) -> Option<String> {
        self.adapter().disarmed_by(command, env)
    }

    /// What the app arms a chat of this harness with, as `charter doctor` says it.
    pub fn armed_with(self) -> String {
        self.adapter().armed_with()
    }

    /// The arguments that start this harness's program as an ACP agent, for a chat at level 3
    /// over [`crate::acp`], or `None` where charter does not run it over ACP yet (HP-3 and HP-4,
    /// #670 and #783): the `[levels] acp` argv of its declaration, after the program.
    ///
    /// opencode's ACP agent is opencode itself, `opencode acp` (ADR 0080 §3, measured on
    /// 1.18.33: protocol version 1, `loadSession`, and a session run with `fs` and `terminal`
    /// both off). Codex and Claude Code reach ACP through ACP adapter programs, which HP-3 and
    /// HP-4 decide.
    pub fn acp_args(self) -> Option<&'static [String]> {
        // The declared argv starts with the program, which the chat's own start names.
        let argv = self.declared().levels.acp.as_deref()?;
        Some(argv.get(1..).unwrap_or_default())
    }

    /// The adapter that arms this harness at level 2 (ADR 0073 §3): the enum is their
    /// registry, and every harness has one.
    pub fn adapter(self) -> &'static dyn HarnessAdapter {
        match self {
            Self::ClaudeCode => &claude::ADAPTER,
            Self::Codex => &codex::ADAPTER,
            Self::Opencode => &opencode::ADAPTER,
        }
    }

    /// [`HarnessAdapter::skill_start_text`]: what Use skill… types to start `skill` (SK-12).
    pub fn skill_start_text(self, skill: &SkillToStart<'_>) -> String {
        self.adapter().skill_start_text(skill)
    }

    /// [`HarnessAdapter::skill_name`]: what this harness calls a skill of a chat's set (SK-12).
    pub fn skill_name(self, name: &str, origin: crate::skills::Origin) -> String {
        self.adapter().skill_name(name, origin)
    }

    /// [`HarnessAdapter::typed_start_named`]: how a skill Use skill… typed is confirmed sent.
    pub fn typed_start(self) -> crate::skillspan::Confirmed {
        if self.adapter().typed_start_named() {
            crate::skillspan::Confirmed::ByName
        } else {
            crate::skillspan::Confirmed::ByPrompt
        }
    }

    /// [`HarnessAdapter::skill_budget`]: one chat's skill budget on this harness (SK-12). A
    /// manual-only skill is free on every one, which keeps it out of the listing by its own key
    /// or purlis's translation ([`Self::skill_frontmatter`], SK-8).
    pub fn skill_budget(self) -> crate::skills::Budget {
        self.adapter().skill_budget()
    }

    /// [`HarnessAdapter::skill_frontmatter`]: what this harness reads of a skill's frontmatter
    /// (SK-8).
    pub fn skill_frontmatter(self) -> crate::skills::Honours {
        self.adapter().skill_frontmatter()
    }

    /// [`HarnessAdapter::manual_beside`]: the file beside a `SKILL.md` this harness keeps one of
    /// its own skills manual-only by, where it has one (SK-39).
    pub fn manual_beside(self) -> Option<crate::skills::ManualBeside> {
        self.adapter().manual_beside()
    }

    /// How a chat on this harness comes to know charter's skills, for that chat alone and with
    /// nothing written (ADR 0063). [`Self::state_hooks`] is where each route is taken.
    ///
    /// - **Claude Code** loads the bundled plugin with `--plugin-dir`, and a plugin's `skills/`
    ///   comes with it.
    /// - **opencode** discovers every `SKILL.md` under the paths in its config's `skills.paths`
    ///   (`packages/opencode/src/skill/index.ts`, v1.18.32), and a plugin's `config` hook is
    ///   handed "the live merged config" to change. The shim appends the bundle's directory
    ///   there, measured on 1.18.32: `opencode debug skill` listed charter's skills beside a
    ///   path the operator's own config named.
    /// - **Codex** has no such switch. codex-cli 0.147.0 finds skills only under
    ///   `$CODEX_HOME/skills`, `~/.agents/skills`, a trusted project's `.codex/skills`, the
    ///   `.agents/skills` folders between the project root and the directory it runs in,
    ///   `/etc/codex/skills`, and installed plugins (`codex-rs/ext/skills/src/host_roots.rs` at
    ///   `rust-v0.147.0`). `skills.config` only turns a skill it found on or off. Each root is
    ///   the operator's home, their tree or Codex's plugin cache, which ADR 0050 keeps charter
    ///   from writing, so a Codex chat is briefed on the skills instead.
    pub fn skills(self) -> crate::skills::Route {
        match self {
            Self::ClaudeCode => crate::skills::Route::Plugin,
            Self::Opencode => crate::skills::Route::Config,
            Self::Codex => crate::skills::Route::Briefing,
        }
    }

    /// **What a running chat on this harness takes in by itself** when the project's skills
    /// change, through the route purlis hands them by (ADR 0099 §5); everything else waits for a
    /// restart on the same conversation, which the chat's tab offers (#2770).
    ///
    /// - The plugin and config routes (Claude Code, opencode): nothing. Claude Code reads a
    ///   `--plugin-dir` plugin's skills at the start (measured twice on 2.1.304), and opencode
    ///   caches discovery and the body per directory (measured on 1.18.33).
    /// - The MCP route: everything, as its tools ask for the chat's set afresh at each call.
    /// - The briefing route (Codex): nothing. The `SessionStart` briefing lists each skill with
    ///   the `SKILL.md` purlis wrote for it in the set's folder, once (SK-8), and that folder
    ///   never changes while the chat runs, so an edit waits for a restart as on the other
    ///   routes.
    pub fn skills_reload(self) -> crate::skills::Reload {
        match self.skills() {
            // `skill_find` and `skill_load` work the chat's set out at each call (SK-13).
            crate::skills::Route::Mcp => crate::skills::Reload::Everything,
            // `Own` is no harness's route: it is how a harness takes its own skills, which purlis
            // hands nothing of.
            crate::skills::Route::Plugin
            | crate::skills::Route::Config
            | crate::skills::Route::Briefing
            | crate::skills::Route::Own => crate::skills::Reload::Nothing,
        }
    }

    /// What a chat's pane sends for Shift+Enter: the bytes this harness reads as "a new line
    /// in what I am typing", rather than "send it" (SI-4).
    ///
    /// A terminal has no Shift+Enter of its own — xterm.js 6.0.0 sends a bare CR for it, the
    /// same as Enter, so every harness submitted on it. ESC CR is Alt/Option+Enter, the
    /// sequence each of these harnesses already reads as a newline. Measured, not assumed: in a
    /// pseudo-terminal, `hello`, ESC CR, `world` left both words in the input on two lines,
    /// unsent, on Claude Code 2.1.283, codex-cli 0.147.0 and opencode 1.18.23.
    ///
    /// A harness's and not the pane's, because it is a fact about the program reading the
    /// keys; a chat that runs no harness — a shell — keeps the terminal's own Enter.
    pub fn newline(self) -> &'static str {
        // A declaration that names none keeps the terminal's own Enter, a bare CR.
        self.declared().terminal.newline.as_deref().unwrap_or("\r")
    }

    /// Whether this harness reports `SessionStart` when it starts, before anyone has typed a
    /// thing — the moment charter types a curation action's prompt into a Claude Code chat it
    /// has just opened (ADR 0061), knowing from a hook rather than from the harness's output
    /// that the program is there to read it.
    ///
    /// Measured, not assumed: Claude Code fires it at launch. Codex fires it inside the FIRST
    /// TURN (codex-cli 0.147.0, see [`Harness::unreported`]), and opencode's shim asks for it at
    /// the first prompt because opencode names no hook for a session's creation (ADR 0058). A
    /// prompt typed on either one's `SessionStart` would land after the operator had already
    /// sent something, so that report never types one ([`Harness::ready_to_type`] says what
    /// does).
    pub fn reports_its_start_before_the_first_prompt(self) -> bool {
        self.holds("reports_its_start_before_the_first_prompt")
    }

    /// How charter tells that a chat it has just opened on this harness is ready to have a
    /// curation prompt typed into it (ADR 0061), or `None` where it has no way to tell. Every
    /// harness is named, so one added later is given an answer on purpose, once measured.
    ///
    /// Claude Code reports `SessionStart` at launch, so it is typed into on that hook. Codex
    /// says nothing until the first prompt, so it is typed into once its terminal has gone raw
    /// and it has then written nothing for a while — the kernel's line discipline and the
    /// moment bytes last arrived, never what it drew. Measured on codex-cli 0.147.0: it took a
    /// paste at any moment after going raw, even mid-draw.
    ///
    /// opencode has neither. Measured on 1.18.32, it draws its input box, goes raw, and then
    /// boots silently before that box takes a key: 0.83 s idle, 1.4 s with every core busy,
    /// more than 8 s on a machine at load 50 — and a paste in that silence vanished. No quiet
    /// period long enough to cover it is short enough to wait for (ADR 0061, amended
    /// 2026-09-27).
    pub fn ready_to_type(self) -> Option<ReadyToType> {
        self.declared().ready_to_type()
    }

    /// The biggest paste this harness draws whole in its input: a paste with more lines or
    /// more characters is drawn as a placeholder the operator cannot read. A curation prompt is
    /// typed to be read before it is sent (ADR 0061, and its amendment of 2026-09-27), so one
    /// that would be drawn as a placeholder is not typed.
    ///
    /// Measured, each with no setting a chat can be started with to stop it:
    ///
    /// - Claude Code 2.1.283: `[Pasted text #N +M lines]` over 800 characters or at 4 lines or
    ///   more. A line feed counts among the characters, a trailing one starts a line, and a
    ///   character is a character, not a byte.
    /// - Codex 0.147.0: `[Pasted Content N chars]` over 1,000 characters, line feeds among
    ///   them, however many lines (200 were drawn whole).
    /// - opencode 1.18.32: `[Pasted ~N lines]` at 3 lines or more, or over 150 characters.
    ///   opencode is never typed into ([`Harness::ready_to_type`]); its limit is here so that a
    ///   harness is judged the moment it can be.
    pub fn longest_paste_drawn_whole(self) -> DrawnWhole {
        // A declaration that measured none limits nothing; every built-in names one, and a
        // test holds each to its measurement.
        self.declared().longest_paste_drawn_whole()
    }

    /// Why `pasted`, handed to this harness as one paste, would be drawn as a placeholder in
    /// its input — or `None` when it is drawn whole. `pasted` is the text inside the paste
    /// (`curation::pasted`), and the rule is its declaration's
    /// ([`crate::harness_declaration::Declaration::why_drawn_as_a_placeholder`]), for
    /// `curate`'s refusal and `charter persona lint`'s warning alike.
    pub fn why_drawn_as_a_placeholder(self, pasted: &str) -> Option<String> {
        self.declared().why_drawn_as_a_placeholder(pasted)
    }

    /// `reference` in this harness's own syntax ([`HarnessAdapter::reference`]).
    pub fn reference(self, reference: &crate::reference::Reference) -> String {
        self.adapter().reference(reference)
    }

    /// What the operator calls this harness: `Claude Code`, `Codex`, `opencode`.
    pub fn title(self) -> &'static str {
        &self.declared().title
    }

    /// What a chat on this harness cannot tell purlis, in a sentence the chat shows — or
    /// none, where it can tell purlis everything the board asks.
    ///
    /// Said ON the chat, not left for the operator to infer from a quiet sidebar: a Codex
    /// chat waiting on an approval looks exactly like one that is working, and a chat that
    /// reports nothing looks like purlis is broken rather than like the harness is
    /// different.
    ///
    /// **Made from the harness's card** (ADR 0073 §3, #1134): each *no* its card says about
    /// what a chat tells purlis, in [`Self::unsaid`]'s words or the card's, so the two never
    /// disagree ([`crate::harness_card::Card::unreported`]).
    pub fn unreported(self) -> Option<&'static str> {
        static SAID: LazyLock<Vec<(Harness, Option<String>)>> = LazyLock::new(|| {
            Harness::ALL
                .iter()
                .map(|&harness| {
                    let card = crate::harness_card::built_in_card(harness);
                    (harness, card.unreported)
                })
                .collect()
        });
        SAID.iter()
            .find(|(harness, _)| *harness == self)
            .and_then(|(_, said)| said.as_deref())
    }

    /// This harness's own words for what its chats cannot tell purlis
    /// ([`Self::unreported`]): a clause for a *no* of its card, said only while the card says
    /// it, and the facts of its adapter no card line holds. Each was measured on the version
    /// its declaration's `tested` names.
    pub(crate) fn unsaid(self) -> &'static [crate::harness_card::Unsaid] {
        use crate::harness_card::Unsaid::{Also, Lacks};
        match self {
            Self::ClaudeCode => &[],
            Self::Codex => &[
                Lacks {
                    line: "reports_its_start_before_the_first_prompt",
                    words: "it says nothing until your first prompt",
                },
                Also("it says nothing at all until you trust purlis's hooks when Codex asks"),
                Lacks {
                    line: "reports_waiting",
                    words: "it says when it stops mid-turn for your approval and for nothing else",
                },
                Lacks {
                    line: "reports_its_process",
                    words: "a later conversation you start inside it is not followed",
                },
            ],
            Self::Opencode => &[
                Lacks {
                    line: "reports_its_start_before_the_first_prompt",
                    words: "it says nothing until your first prompt",
                },
                Also("it says nothing when it quits"),
                Also(
                    "once you answer its permission prompt, the chat reads waiting until the \
                     turn ends",
                ),
                Lacks {
                    line: "reports_its_process",
                    words: "a session you open inside it with /new is not followed",
                },
                // FD-18: opencode's payload names no child agent.
                Also("its helpers are not shown under it"),
            ],
        }
    }
}

fn words<const N: usize>(argv: [&str; N]) -> Vec<String> {
    argv.map(str::to_owned).to_vec()
}

#[cfg(test)]
mod tests;
