// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Harness declarations: a harness as data ([ADR 0073], FD-14).
//!
//! A **harness declaration** says how to start one harness, what it can do and which levels it
//! runs at. A project declares a harness charter does not ship in `harnesses/<name>.toml`, and
//! the three charter ships (Claude Code, Codex, opencode) are declarations too, in this
//! module's directory, read by the same reader in the same format. `docs/plane-format.md`'s
//! `harnesses/<name>.toml` entry is the schema; this module is its reader.
//!
//! **A declaration is data and never a command.** `program` is a bare name found on `PATH`,
//! never a path or a shell string; the session templates take `{id}` and `{name}` and nothing
//! else; the environment it names is a namespace of the harness's own, and never charter's or
//! one that reads as a credential (ADR 0073 §5, ADR 0022's refusals). A project declaration's
//! program runs on this machine only once the operator has approved it, and again after any
//! change to it ([`crate::profiletrust::declaration_approval_needed`], V24b).
//!
//! **This machine may declare a harness too** (ADR 0073 §5, #968): a `[harnesses.<name>]`
//! table in `charter.local.toml`, in the same schema ([`Origin::Local`]). It is asked
//! about with its profile, in the one question every program that file names gets, which shows
//! its words and is asked again when they change; and it is read only where git would not
//! carry that file ([`read`]), as the profiles there are. One that takes a built-in's name
//! **replaces that built-in's measured facts on this machine and never how it is run or
//! armed**: the built-in's program, levels, adapter, session words and env namespace stand, as
//! a harness purlis ships is started with no question, and purlis types into it one line only,
//! as into any harness it did not measure ([`Declaration::why_drawn_as_a_placeholder`]).
//!
//! **Level 2 is charter code** (V24c). A project declaration that says `hooks = true` is
//! refused: the hooks a harness reports through are armed by an adapter charter ships
//! ([`crate::harness::HarnessAdapter`]), and only a built-in has one. A declared harness runs
//! at level 1, its terminal alone.
//!
//! [ADR 0073]: ../../../docs/adr/0073-a-harness-is-declared-as-data-and-a-chat-runs-it-at-one-of-three-levels.md

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use sha2::Digest as _;

use crate::harness::{DrawnWhole, Harness, SessionId};
use crate::shown;

/// The directory a project declares its harnesses in, at the project root.
pub const DIR: &str = "harnesses";

/// The table of `charter.local.toml` a machine declares its own harnesses in, one
/// `[harnesses.<name>]` table each (ADR 0073 §5, #968).
pub const LOCAL_TABLE: &str = "harnesses";

/// The record of the declarations this clone's operator approved, under `.charter/`.
pub const APPROVED: &str = "harness-declarations-approved.json";

/// The most a declaration file may be. It is read on the path that decides whether a chat
/// starts, so a planted giant is a launch that never finishes.
const MAX_BYTES: u64 = 64 * 1024;

/// The most characters of a declaration's `title` a window draws: the harness card's label
/// (HP-19), on the picker and the tab strip.
pub const MOST_TITLE: usize = 60;

/// The most characters of a declaration's `tested`: a fact on the harness card.
pub const MOST_TESTED: usize = 80;

/// The most characters of the reason in a capability's `"no: <reason>"`: a line on the card.
pub const MOST_REASON: usize = 200;

/// Why `value`, a declaration's words a window draws, may not be drawn, or nothing: it is one
/// line of at most `most` characters with nothing invisible in it — no control, bidi or
/// zero-width character, which [`shown::one_line`] would escape — so a committed file can
/// neither forge a second line, reorder the card's own words nor blow up a first-hour surface.
fn drawn_refusal(value: &str, most: usize) -> Option<String> {
    (shown::one_line(value, shown::NO_CLIP) != value || value.chars().count() > most).then(|| {
        format!(
            "is {}, and words a window draws are one line of at most {most} characters with no \
             control, bidi or zero-width character in them. Shorten it to one plain line.",
            shown::short(value)
        )
    })
}

/// Every harness capability charter reads today (ADR 0073 §6). A ticket that reads a new one
/// adds it here and to `docs/plane-format.md`.
pub const CAPABILITIES: [&str; 7] = [
    "reports_its_process",
    "reports_its_start_before_the_first_prompt",
    "keeps_conversations_by_directory",
    "reports_waiting",
    "resumes_by_id",
    "per_chat_plugins",
    MCP_SERVERS,
];

/// The capability that says a harness takes MCP servers: a chat on it can be handed purlis's
/// own, and a harness purlis has no adapter for then gets its skills through it (ADR 0063 as
/// amended 2026-10-20, [`crate::skills::route_of`]).
pub const MCP_SERVERS: &str = "mcp_servers";

/// Where a declaration came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Shipped with charter, in this module's directory.
    BuiltIn,
    /// The project's `harnesses/<name>.toml`, committed.
    Project,
    /// A `[harnesses.<name>]` table in `charter.local.toml`: this machine's own, a harness
    /// for it alone or the replacement of a built-in's facts ([`Declaration::replaces`]).
    Local,
}

/// Who chooses a new session's id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChosenBy {
    /// The harness, and charter learns it only from what the harness reports.
    Harness,
    /// purlis, handed over by `[session] new` before the harness starts: `chosen_by =
    /// "purlis"`, or `"charter"`, its old spelling. The variant keeps the old name for now.
    Charter,
}

/// How a declared harness starts, names and resumes a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub chosen_by: ChosenBy,
    /// The words that start a new session, with `{id}` and `{name}` filled in.
    pub new: Vec<String>,
    /// The words that bring a session back, or none where the declaration says no way.
    pub resume: Option<Vec<String>>,
    /// The flags (or subcommands) by which the operator names a session themselves.
    pub named_by: Vec<String>,
}

/// When a chat just opened can have a curation prompt typed into it (ADR 0061).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadyToType {
    /// On the harness's first `SessionStart` report: a hook, so level 2 only.
    OnStart,
    /// Once its terminal is raw and it has then written nothing for a quiet period.
    RawAndQuiet,
    /// Never: there is no way to tell.
    Never,
}

/// What a declared harness's terminal does with what is typed and pasted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terminal {
    /// The bytes it reads as a new line in its input, or none for the terminal's own Enter.
    pub newline: Option<String>,
    /// The biggest paste it draws whole, or none where nobody measured it.
    pub paste_drawn_whole: Option<DrawnWhole>,
    pub ready_to_type: ReadyToType,
}

/// The levels a declared harness offers (ADR 0073 §1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Levels {
    /// Level 1: the terminal alone. Every declaration offers it.
    pub terminal: bool,
    /// Level 2: its own hooks, armed by an adapter charter ships.
    pub hooks: bool,
    /// Level 3 over ACP: the argv that starts its ACP agent, which HP-2's client speaks to.
    pub acp: Option<Vec<String>>,
}

/// One harness capability's answer. **Silence never reads as yes**: a capability a
/// declaration does not answer is [`Answer::Unknown`], which charter treats as no.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Yes,
    /// No, and the reason, as the declaration says it.
    No(String),
    Unknown,
}

impl Answer {
    /// Whether charter may rely on it: only a yes.
    pub fn holds(&self) -> bool {
        matches!(self, Self::Yes)
    }
}

/// A kind of sign-in a profile of a harness may have (#2030): the four words a declaration's
/// `[sign_in] kinds` lists. Not a forge's sign-in ([`crate::forge::route::SignIn`]), which
/// never reaches a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SignInKind {
    /// A plan (Claude Pro or Max, ChatGPT, Copilot), signed in through the harness's own flow
    /// and kept by the harness in its own folder. purlis never reads, copies, stores or relays
    /// it.
    Subscription,
    /// A provider's key, a vault value handed to the chat at start.
    ApiKey,
    /// A cloud's credentials (Bedrock, Vertex, Azure), vault values handed over the same way.
    Cloud,
    /// A local model, with nothing to sign in.
    Local,
}

impl SignInKind {
    /// Every kind, in the order a declaration and a listing name them.
    pub const ALL: [SignInKind; 4] = [Self::Subscription, Self::ApiKey, Self::Cloud, Self::Local];

    /// The word a declaration writes and a listing prints.
    pub fn word(self) -> &'static str {
        match self {
            Self::Subscription => "subscription",
            Self::ApiKey => "api-key",
            Self::Cloud => "cloud",
            Self::Local => "local",
        }
    }

    fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.word() == word)
    }
}

/// How the profiles of a declared harness sign in (`[sign_in]`, #2031).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignInFacts {
    /// The kinds it supports, as the declaration lists them. None listed: none is offered.
    pub kinds: Vec<SignInKind>,
    /// Whether its credential files can be kept from the commands it runs: only a yes counts,
    /// and silence is [`Answer::Unknown`], which reads as no.
    pub kept_from_commands: Answer,
    /// The plans a subscription is signed in to, where the harness serves several and purlis
    /// names the one (`[sign_in] plans`, #2039): an allow-list, so a provider the harness adds
    /// later is not offered until it is declared. None: its own vendor's plan alone.
    pub plans: Vec<Plan>,
}

/// One plan a subscription of a harness is signed in to (`[sign_in] plans`, #2039).
///
/// **A Claude plan is never one of them, but in Claude Code's own declaration**
/// ([`ANTHROPIC`]): Anthropic's terms let no other harness carry it, and a project may declare
/// harnesses of its own, so the rule is purlis's and no declaration lifts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The harness's own id for the provider, as its sign-in takes it (`openai`).
    pub provider: String,
    /// What people call the plan (`ChatGPT`).
    pub title: String,
}

/// The provider whose plan only Claude Code's own declaration may list ([`Plan`]).
pub const ANTHROPIC: &str = "anthropic";

/// The name of Claude Code's own declaration (`claude.toml`), the one that may list
/// [`ANTHROPIC`]. Written out: it is asked while the built-ins are being read.
const CLAUDE_CODE: &str = "claude";

/// The most characters of a plan's provider id.
const MOST_PROVIDER: usize = 64;

/// The variables that set where a declared harness keeps its own files (`[folders]`, #2031):
/// what purlis sets to a profile's folder. None declared: purlis sets none. A harness may keep
/// its files in the folder a variable names (Claude Code, Codex) or in a folder of its own name
/// under it (opencode, under each XDG folder); `{config}` and `{data}` in a
/// [`Conversation::File`] are the folder the variable names, either way.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Folders {
    /// Where it keeps its settings and its sign-in, and its data unless `data_env` is given.
    pub config_env: Option<String>,
    /// Where it keeps its data (conversations, and for some its credentials), where that is a
    /// variable of its own.
    pub data_env: Option<String>,
}

/// How a declared harness's conversation leaves a device and comes back (`[conversation]`,
/// #2031), for a chat that moves; it is resumed by `[session] resume`. A declaration that
/// says nothing has no way, and its chats do not move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conversation {
    /// Copied as a file: `file` is where the session `{id}` lives, under `{config}/` or
    /// `{data}/`. A `*` part is kept as it was found, and `{cwd_key}` is made again from the
    /// directory the chat runs in where it comes back ([`cwd_key`]).
    File {
        file: String,
        /// The session it was forked from has to come with it.
        with_forked_from: bool,
    },
    /// Through the harness's own commands: `export` prints session `{id}`, and `import` takes
    /// the file it printed, named after its words.
    Export {
        export: Vec<String>,
        import: Vec<String>,
    },
}

/// One harness, declared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    /// The word a profile's `kind` names.
    pub name: String,
    /// What the operator calls it.
    pub title: String,
    /// A bare name found on `PATH`.
    pub program: String,
    /// The harness's own environment namespace: names, or prefixes ending `_*`.
    pub env: Vec<String>,
    /// The versions its facts were measured on, as written (TS1 settles the rest).
    pub tested: Option<String>,
    /// Where the operator logs in, said by a refusal that will not hold their credential.
    pub login: Option<String>,
    pub session: Session,
    pub terminal: Terminal,
    pub levels: Levels,
    /// How its profiles sign in.
    pub sign_in: SignInFacts,
    /// Where it keeps its own files.
    pub folders: Folders,
    /// How a conversation is exported and resumed, or none where the declaration says no way.
    pub conversation: Option<Conversation>,
    capabilities: BTreeMap<String, Answer>,
    pub origin: Origin,
    /// `harnesses/<name>.toml`, or `built-in`.
    pub file: String,
    /// `sha256:<hex>` of the file's bytes: what an approval is of.
    pub digest: String,
    /// The file as written, for `charter harness show`.
    pub text: String,
}

impl Declaration {
    /// The answer the declaration gives for `capability`, and [`Answer::Unknown`] where it
    /// gives none.
    pub fn capability(&self, capability: &str) -> Answer {
        self.capabilities
            .get(capability)
            .cloned()
            .unwrap_or(Answer::Unknown)
    }

    /// The sign-in a profile of this harness has, as far as purlis can see it, or none where
    /// it cannot tell (#2031). Until a profile names its own (#2034, #2036), what purlis sees
    /// is: a harness that supports one kind alone has that one; a profile of a harness purlis
    /// ships whose `env` sets, or whose `[chat_env] pass` (`passed`) passes, the variables that
    /// switch it to a cloud has `cloud`, or a provider's key has `api-key`, where only one of
    /// the two is seen. Nothing else is a sign-in purlis can see: a subscription is the
    /// harness's own, and never read.
    pub fn profile_sign_in(
        &self,
        env: &[(String, String)],
        passed: &[String],
    ) -> Option<SignInKind> {
        if let [only] = self.sign_in.kinds.as_slice() {
            return Some(*only);
        }
        let harness = match self.origin {
            Origin::BuiltIn => Harness::of_kind(&self.name),
            Origin::Local => self.replaces(),
            Origin::Project => None,
        }?;
        let (keys, clouds) = sign_in_variables(harness);
        let seen = |names: &[&str]| {
            names.iter().any(|name| {
                env.iter().any(|(set, value)| {
                    set == name && !matches!(value.as_str(), "" | "0" | "false")
                }) || passed.iter().any(|entry| overlap(entry, name))
            })
        };
        let supported = |kind| self.sign_in.kinds.contains(&kind);
        match (seen(keys), seen(clouds)) {
            (true, false) if supported(SignInKind::ApiKey) => Some(SignInKind::ApiKey),
            (false, true) if supported(SignInKind::Cloud) => Some(SignInKind::Cloud),
            _ => None,
        }
    }

    /// The words that start a new session under `id` and `name`, or none where the harness
    /// chooses its own id.
    pub fn new_session_argv(&self, id: &SessionId, name: &str) -> Vec<String> {
        filled(&self.session.new, id, name)
    }

    /// The words that bring session `id` back, or none where the declaration names no way.
    pub fn resume_argv(&self, id: &SessionId, name: &str) -> Option<Vec<String>> {
        self.session
            .resume
            .as_ref()
            .map(|words| filled(words, id, name))
    }

    /// Whether `args` already name a session, so charter adds none of its own: a flag in
    /// `named_by`, alone or with its value attached (`--resume=<id>`).
    pub fn session_named_in(&self, args: &[String]) -> bool {
        args.iter().any(|arg| {
            self.session.named_by.iter().any(|word| {
                arg == word
                    || (word.starts_with('-')
                        && arg.starts_with(word.as_str())
                        && arg.as_bytes().get(word.len()) == Some(&b'='))
            })
        })
    }

    /// When a chat just opened on this harness can have a curation prompt typed into it
    /// (ADR 0061), from `[terminal] ready_to_type`, or none where the declaration says there is
    /// no way to tell. A built-in's answer ([`Harness::ready_to_type`]) and a declared
    /// harness's (#967) are this one.
    pub fn ready_to_type(&self) -> Option<crate::harness::ReadyToType> {
        use crate::harness::ReadyToType as Typed;
        match self.terminal.ready_to_type {
            ReadyToType::OnStart => Some(Typed::WhenItReportsItsStart),
            ReadyToType::RawAndQuiet => Some(Typed::WhenRawAndQuiet),
            ReadyToType::Never => None,
        }
    }

    /// The biggest paste this harness draws whole in its input, from
    /// `[terminal] paste_drawn_whole`: a declaration that measured none limits nothing.
    pub fn longest_paste_drawn_whole(&self) -> DrawnWhole {
        self.terminal.paste_drawn_whole.unwrap_or(DrawnWhole {
            lines: usize::MAX,
            chars: usize::MAX,
        })
    }

    /// Why `pasted`, handed to this harness as one paste, would be drawn as a placeholder in
    /// its input, or `None` when it is drawn whole. `pasted` is the text inside the paste
    /// (`curation::pasted`). The one rule every harness is judged by, built-in or declared
    /// ([`Harness::why_drawn_as_a_placeholder`]).
    ///
    /// **A harness the project or this machine declares is typed into one line only**, and so
    /// is this machine's replacement of a built-in's facts. purlis cannot tell
    /// whether its program turned bracketed paste on, and one that did not reads each line
    /// break of a paste as Enter: what is above it would be sent before the person read it.
    /// A built-in's measurement says it takes a paste whole; a declaration's word is not one.
    pub fn why_drawn_as_a_placeholder(&self, pasted: &str) -> Option<String> {
        let most = self.longest_paste_drawn_whole();
        let lines = pasted.split('\n').count();
        let chars = pasted.chars().count();
        let title = &self.title;
        if self.origin != Origin::BuiltIn && lines > 1 {
            return Some(format!(
                "{title} is typed into only a prompt on one line, as purlis cannot tell that it \
                 takes a paste whole and a line break could send what is above it, and this one \
                 is {lines} lines"
            ));
        }
        if lines > most.lines {
            return Some(format!(
                "{title} draws a paste of more than {} lines as a placeholder, and this one is \
                 {lines} lines",
                most.lines
            ));
        }
        (chars > most.chars).then(|| {
            format!(
                "{title} draws a paste over {} characters as a placeholder, and this one is \
                 {chars}",
                most.chars
            )
        })
    }

    /// Whether charter ships an adapter for this harness's hooks (level 2). Only a built-in
    /// can, and this machine's replacement of one, which keeps the built-in's levels: a
    /// project's `hooks = true` is refused.
    pub fn has_adapter(&self) -> bool {
        self.levels.hooks && (self.origin == Origin::BuiltIn || self.replaces().is_some())
    }

    /// The harness purlis ships whose facts this declaration replaces on this machine: a
    /// `[harnesses.<name>]` table in `charter.local.toml` under a built-in's name (#968). Its
    /// adapter, its levels and its program stay the built-in's; `None` for every other.
    pub fn replaces(&self) -> Option<Harness> {
        if self.origin != Origin::Local {
            return None;
        }
        Harness::of_kind(&self.name)
    }
}

/// The variables by which a profile of `harness` would be signed in with a provider's key,
/// and those that switch it to a cloud: what [`Declaration::profile_sign_in`] looks for in a
/// profile's `env` and in `[chat_env] pass`. Names only; a value is never read.
fn sign_in_variables(harness: Harness) -> (&'static [&'static str], &'static [&'static str]) {
    match harness {
        Harness::ClaudeCode => (
            &["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"],
            &[
                "CLAUDE_CODE_USE_BEDROCK",
                "CLAUDE_CODE_USE_VERTEX",
                "CLAUDE_CODE_USE_FOUNDRY",
            ],
        ),
        Harness::Codex => (
            &["OPENAI_API_KEY", "CODEX_API_KEY"],
            &["AZURE_OPENAI_API_KEY"],
        ),
        Harness::Opencode => (
            &[
                "ANTHROPIC_API_KEY",
                "OPENAI_API_KEY",
                "OPENROUTER_API_KEY",
                "GEMINI_API_KEY",
                "GOOGLE_GENERATIVE_AI_API_KEY",
            ],
            &[
                "AWS_ACCESS_KEY_ID",
                "AWS_PROFILE",
                "AWS_BEARER_TOKEN_BEDROCK",
                "GOOGLE_APPLICATION_CREDENTIALS",
                "GOOGLE_CLOUD_PROJECT",
                "AZURE_API_KEY",
                "AZURE_RESOURCE_NAME",
            ],
        ),
    }
}

/// The folder name Claude Code keeps a working directory's transcripts under, which
/// `{cwd_key}` in a [`Conversation::File`] stands for: the directory's absolute path with every
/// character other than an ASCII letter or digit written as `-` (`/Users/a/my.app` is
/// `-Users-a-my-app`). It is made again from the directory a chat runs in on the device it
/// comes back to, never copied.
pub fn cwd_key(dir: &Path) -> String {
    dir.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn filled(words: &[String], id: &SessionId, name: &str) -> Vec<String> {
    words
        .iter()
        .map(|word| word.replace("{id}", id.as_str()).replace("{name}", name))
        .collect()
}

/// A declaration charter will not read, and the one sentence saying why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// `harnesses/<file>`, or the directory itself, or `charter.local.toml [harnesses.<name>]`.
    pub file: String,
    pub reason: String,
}

/// Every harness a project has: the built-ins first, then the project's in file-name order,
/// and every declaration refused.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Declarations {
    pub declared: Vec<Declaration>,
    pub refused: Vec<Refused>,
}

impl Declarations {
    pub fn get(&self, name: &str) -> Option<&Declaration> {
        self.declared.iter().find(|d| d.name == name)
    }

    /// The project's own, without the built-ins.
    pub fn projects(&self) -> impl Iterator<Item = &Declaration> {
        self.declared.iter().filter(|d| d.origin == Origin::Project)
    }

    /// Every harness added to the ones purlis ships: the project's own and this machine's
    /// (`charter.local.toml`), without a replacement of a built-in's facts.
    pub fn added(&self) -> impl Iterator<Item = &Declaration> {
        self.declared
            .iter()
            .filter(|d| d.origin != Origin::BuiltIn && d.replaces().is_none())
    }

    /// This machine's replacement of `harness`'s facts, where `charter.local.toml` declares one
    /// (#968).
    pub fn replacement(&self, harness: Harness) -> Option<&Declaration> {
        self.declared.iter().find(|d| d.replaces() == Some(harness))
    }
}

/// The harnesses purlis ships, in REGISTRY order: the order the built-ins are listed in and a
/// refusal names them in (`claude`, `opencode`, `codex`), which is not alphabetical.
const REGISTRY: [Harness; 3] = [Harness::ClaudeCode, Harness::Opencode, Harness::Codex];

/// The file that declares `harness`: the one link between a harness purlis arms in code (its
/// adapter, [`Harness::adapter`]) and the facts measured on it, which are all in the file.
fn built_in_text(harness: Harness) -> &'static str {
    match harness {
        Harness::ClaudeCode => include_str!("harness_declaration/claude.toml"),
        Harness::Codex => include_str!("harness_declaration/codex.toml"),
        Harness::Opencode => include_str!("harness_declaration/opencode.toml"),
    }
}

static BUILT_IN: LazyLock<Vec<Declaration>> = LazyLock::new(|| {
    REGISTRY
        .iter()
        .map(|&harness| {
            parse(built_in_text(harness), Origin::BuiltIn, "built-in").unwrap_or_else(|why| {
                panic!("a built-in declaration purlis ships is refused: {why}")
            })
        })
        .collect()
});

/// The declarations charter ships, in registry order (`claude`, `opencode`, `codex`).
pub fn builtins() -> &'static [Declaration] {
    &BUILT_IN
}

/// The built-in declaration of `harness`: where every fact [`Harness`] answers is read from
/// (FD-14b, #966).
pub fn of(harness: Harness) -> &'static Declaration {
    let at = REGISTRY
        .iter()
        .position(|&one| one == harness)
        .expect("every harness purlis ships is in its registry");
    &BUILT_IN[at]
}

/// The built-in declaration named `name`.
pub fn builtin(name: &str) -> Option<&'static Declaration> {
    builtins().iter().find(|d| d.name == name)
}

/// Every harness the project at `root` has: the built-ins, then each `harnesses/*.toml`, then
/// each this machine declares in `charter.local.toml` (a replacement of a built-in's facts in
/// the built-in's place), and each declaration refused with its reason. Never fails.
///
/// **This machine's are read only where git would not carry `charter.local.toml`**, through
/// the reader of that file's layer ([`crate::settings::local_layer_holding`], which asks git
/// only of a file that holds a `[harnesses]` table), as nothing else in the file is: a card, a picker, the doctor and a start all read through here, so none of them
/// shows or types by a declaration a start would leave out. Where git would carry it, the
/// tables are refused with git's reason.
pub fn read(root: &Path) -> Declarations {
    read_layer(
        root,
        crate::settings::local_layer_holding(root, LOCAL_TABLE),
    )
}

/// [`read`] of the local layer as `local` holds it: the seam a test gives git's answer
/// through.
pub fn read_layer(root: &Path, local: crate::settings::LayerText) -> Declarations {
    use crate::settings::LayerText;
    let mut out = read_project(root);
    match local {
        LayerText::Text(text) => with_local(&mut out, Ok(Some(text))),
        // Said only of a file that holds the table: any other has nothing here to refuse.
        LayerText::LeftOut { why, text } if declares_locally(&text) => out.refused.push(Refused {
            file: format!("{} [{LOCAL_TABLE}]", crate::profiles::LOCAL_FILE),
            reason: why,
        }),
        LayerText::LeftOut { .. } | LayerText::Nothing => {}
    }
    out
}

/// Whether `charter.local.toml`'s text holds a `[harnesses]` key at all.
fn declares_locally(text: &str) -> bool {
    text.parse::<toml::Table>()
        .is_ok_and(|top| top.contains_key(LOCAL_TABLE))
}

/// [`read`] with `charter.local.toml`'s text as `local` gives it (as
/// the profiles' loader reads it): what the file would declare, before it is written. **The unchecked read**: it does not ask
/// whether git would carry the file, so nothing that shows or starts a chat calls it.
pub fn read_with(root: &Path, local: io::Result<Option<String>>) -> Declarations {
    let mut out = read_project(root);
    with_local(&mut out, local);
    out
}

/// The built-ins and the project's `harnesses/*.toml`.
fn read_project(root: &Path) -> Declarations {
    let mut out = Declarations {
        declared: builtins().to_vec(),
        refused: Vec::new(),
    };
    let dir = root.join(DIR);
    let entries = match std::fs::symlink_metadata(&dir) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return out,
        Ok(meta) if meta.is_dir() => match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) => {
                out.refused.push(Refused {
                    file: format!("{DIR}/"),
                    reason: format!(
                        "{DIR}/ could not be read ({}), so no harness it declares was \
                         loaded — the built-in harnesses still are.",
                        shown::short(&e.to_string())
                    ),
                });
                return out;
            }
        },
        _ => {
            out.refused.push(Refused {
                file: format!("{DIR}/"),
                reason: format!(
                    "{DIR} is not a directory, so no harness it declares was loaded — a \
                     project declares each harness in {DIR}/<name>.toml."
                ),
            });
            return out;
        }
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    files.sort();
    for path in files {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let file = format!("{DIR}/{}", shown::short(&format!("{stem}.toml")));
        match read_one(root, &path).and_then(|text| {
            let found = parse(&text, Origin::Project, &file)?;
            if found.name != stem {
                return Err(format!(
                    "{file} declares '{}', and a declaration's file is named after the \
                     harness it declares. Rename the file {}.toml.",
                    shown::short(&found.name),
                    shown::short(&found.name)
                ));
            }
            if out.get(&found.name).is_some() {
                return Err(format!(
                    "{file} declares '{}', which is a harness purlis ships — a project adds \
                     harnesses and never replaces one, because a built-in's declaration \
                     decides how its chats are armed. Give it a name of its own.",
                    shown::short(&found.name)
                ));
            }
            if crate::guardcmd::HARNESSES.contains(&found.name.as_str()) {
                return Err(format!(
                    "{file} declares '{}', which is the name purlis gives a harness it ships — \
                     a chat on it would be taken for that harness and armed as one. Give it a \
                     name of its own.",
                    shown::short(&found.name)
                ));
            }
            Ok(found)
        }) {
            Ok(found) => out.declared.push(found),
            Err(reason) => out.refused.push(Refused { file, reason }),
        }
    }
    out
}

/// `out` with each `[harnesses.<name>]` table of `charter.local.toml`'s text (#968): a
/// harness for this machine alone after the project's, or a built-in's facts replaced in the
/// built-in's place. A file that cannot be read or is not TOML adds nothing here, and the
/// profiles' read says so once ([`crate::profiles::derive_declared`]).
fn with_local(out: &mut Declarations, local: io::Result<Option<String>>) {
    let Ok(Some(text)) = local else {
        return;
    };
    let Ok(top) = text.parse::<toml::Table>() else {
        return;
    };
    let Some(tables) = top.get(LOCAL_TABLE) else {
        return;
    };
    let local_file = crate::profiles::LOCAL_FILE;
    let Some(tables) = tables.as_table() else {
        out.refused.push(Refused {
            file: local_file.to_owned(),
            reason: format!(
                "{LOCAL_TABLE} in {local_file} is not a table, so no harness it declares was \
                 read — the file holds one [{LOCAL_TABLE}.<name>] table per harness. Write it \
                 that way."
            ),
        });
        return;
    };
    for (key, value) in tables {
        let file = format!("{local_file} [{LOCAL_TABLE}.{}]", shown::short(key));
        match local_one(key, value, &file, out) {
            // A replacement keeps the built-in's place: the order a picker draws.
            Ok(found) => match out.declared.iter_mut().find(|d| d.name == found.name) {
                Some(slot) => *slot = found,
                None => out.declared.push(found),
            },
            Err(reason) => out.refused.push(Refused { file, reason }),
        }
    }
}

/// One `[harnesses.<key>]` table as a declaration from this machine, or why it is not one.
fn local_one(
    key: &str,
    value: &toml::Value,
    file: &str,
    out: &Declarations,
) -> Result<Declaration, String> {
    let Some(table) = value.as_table() else {
        return Err(format!(
            "{file} is not a table, so it declares nothing. Write it as [{LOCAL_TABLE}.{}] with \
             the keys of a harness declaration.",
            shown::short(key)
        ));
    };
    // Named after the harness it declares, as a project's file is, so the name may be left out.
    let mut table = table.clone();
    table
        .entry("name")
        .or_insert_with(|| toml::Value::String(key.to_owned()));
    let text = toml::to_string(&table).map_err(|e| {
        format!(
            "{file} is not a harness declaration purlis reads ({}), so it declares nothing. \
             Fix the table.",
            shown::short(&e.to_string())
        )
    })?;
    let found = parse(&text, Origin::Local, file)?;
    if found.name != key {
        return Err(format!(
            "{file} declares '{}', and a table is named after the harness it declares. Name \
             the table [{LOCAL_TABLE}.{}].",
            shown::short(&found.name),
            shown::short(&found.name)
        ));
    }
    if found.replaces().is_some() {
        return Ok(found);
    }
    if let Some(theirs) = out.projects().find(|d| d.name == found.name) {
        return Err(format!(
            "{file} declares '{}', which this project declares in {} — {} adds a harness or \
             replaces one purlis ships, and never the project's, whose approval is for its own \
             words. Give it a name of its own.",
            shown::short(&found.name),
            theirs.file,
            crate::profiles::LOCAL_FILE
        ));
    }
    if crate::guardcmd::HARNESSES.contains(&found.name.as_str()) {
        return Err(format!(
            "{file} declares '{}', which is the name purlis gives a harness it ships — a chat \
             on it would be taken for that harness and armed as one. Give it a name of its own.",
            shown::short(&found.name)
        ));
    }
    Ok(found)
}

/// One declaration file's text, through the gates every read of a committed file that decides
/// a launch takes: no link on the way, a plain file, a bound, and text.
fn read_one(root: &Path, path: &Path) -> Result<String, String> {
    let name = shown::short(&path.display().to_string());
    let unreadable =
        |why: String| format!("{name} could not be read ({why}), so it declares nothing.");
    let mut open = crate::contain::open_no_link(root, path)
        .map_err(|e| unreadable(shown::short(&e.to_string())))?;
    let meta = open
        .metadata()
        .map_err(|e| unreadable(shown::short(&e.to_string())))?;
    if !meta.file_type().is_file() {
        return Err(unreadable("it is not a plain file".to_owned()));
    }
    if meta.len() > MAX_BYTES {
        return Err(unreadable(format!("it is over {MAX_BYTES} bytes")));
    }
    let mut text = String::new();
    io::Read::read_to_string(&mut open, &mut text)
        .map_err(|e| unreadable(shown::short(&e.to_string())))?;
    Ok(text)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    name: String,
    title: Option<String>,
    program: String,
    #[serde(default)]
    env: Vec<String>,
    tested: Option<String>,
    login: Option<String>,
    #[serde(default)]
    session: RawSession,
    #[serde(default)]
    terminal: RawTerminal,
    /// Absent and present apart: a replacement of a built-in's facts takes the built-in's.
    levels: Option<RawLevels>,
    /// Absent and present apart, as `levels` is: a replacement takes the built-in's.
    sign_in: Option<RawSignIn>,
    folders: Option<RawFolders>,
    conversation: Option<RawConversation>,
    #[serde(default)]
    capabilities: BTreeMap<String, String>,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawSignIn {
    #[serde(default)]
    kinds: Vec<String>,
    kept_from_commands: Option<String>,
    #[serde(default)]
    plans: Vec<RawPlan>,
}

#[derive(serde::Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawPlan {
    provider: String,
    title: String,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawFolders {
    config_env: Option<String>,
    data_env: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConversation {
    file: Option<String>,
    with_forked_from: Option<bool>,
    export: Option<Vec<String>>,
    import: Option<Vec<String>>,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawSession {
    chosen_by: Option<String>,
    new: Option<Vec<String>>,
    resume: Option<Vec<String>>,
    #[serde(default)]
    named_by: Vec<String>,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawTerminal {
    newline: Option<String>,
    paste_drawn_whole: Option<RawPaste>,
    ready_to_type: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPaste {
    lines: Option<u32>,
    chars: Option<u32>,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawLevels {
    terminal: Option<bool>,
    #[serde(default)]
    hooks: bool,
    acp: Option<Vec<String>>,
}

/// `text` as a declaration from `origin`, named `file` in every refusal — or the one sentence
/// saying why it is not one. The first failure wins, so one file gets one sentence.
pub fn parse(text: &str, origin: Origin, file: &str) -> Result<Declaration, String> {
    let raw: Raw = toml::from_str(text).map_err(|e| {
        format!(
            "{file} is not a harness declaration purlis reads ({}), so it declares nothing. \
             Fix the file.",
            shown::short(e.message())
        )
    })?;
    let at = |what: &str| format!("{file}'s {what}");
    // This machine's replacement of a built-in's facts (#968): asked of a local table only, as
    // the built-ins are what is being read when one of them is parsed.
    let replaces = match origin {
        Origin::Local => builtin(&raw.name),
        Origin::BuiltIn | Origin::Project => None,
    };
    if let Some(title) = &raw.title {
        if let Some(why) = drawn_refusal(title, MOST_TITLE) {
            return Err(format!("{} {why}", at("title")));
        }
        // Read once the built-ins are: they are what this asks about (`word_launcher`'s reason).
        // A replacement may keep the title of the built-in it replaces, and no other's.
        if origin != Origin::BuiltIn
            && builtins().iter().any(|built_in| {
                built_in.title.eq_ignore_ascii_case(title.trim())
                    && replaces.is_none_or(|replaced| replaced.name != built_in.name)
            })
        {
            return Err(format!(
                "{} is {}, which is a harness purlis ships — a card and a picker that named it \
                 would read as that harness. Give it a title of its own.",
                at("title"),
                shown::short(title)
            ));
        }
    }
    if let Some(why) = raw
        .tested
        .as_deref()
        .and_then(|tested| drawn_refusal(tested, MOST_TESTED))
    {
        return Err(format!("{} {why}", at("tested")));
    }
    if !crate::profiles::name_ok(&raw.name) {
        return Err(format!(
            "{} {} is not a name purlis accepts — letters, digits, '_' and '-', starting \
             with a letter or digit. Rename it.",
            at("name"),
            shown::short(&raw.name)
        ));
    }
    if !bare_word(&raw.program) {
        return Err(format!(
            "{} {} is not a bare program name — a declaration names a program found on PATH, \
             never a path or a shell string, because it runs on a click. Name the program \
             alone; a profile in charter.local.toml can point a chat at a path on this \
             machine.",
            at("program"),
            shown::short(&raw.program)
        ));
    }
    if let Some(replaced) = replaces
        && raw.program != replaced.program
    {
        return Err(format!(
            "{} is {}, and a replacement of {}'s facts runs its program, {} — a profile in \
             {} points a chat at another program, and asks before it runs it. Write program \
             = \"{}\".",
            at("program"),
            shown::short(&raw.program),
            replaced.title,
            replaced.program,
            crate::profiles::LOCAL_FILE,
            replaced.program
        ));
    }
    if origin != Origin::BuiltIn
        && replaces.is_none()
        && let Some(why) = launcher(&raw.program)
    {
        return Err(format!(
            "{} {} is {why} — a declaration names the harness's own program, and purlis \
             does not run one that would run whatever its words say. Name the harness's \
             program.",
            at("program"),
            shown::short(&raw.program)
        ));
    }
    for name in &raw.env {
        env_ok(name).map_err(|why| format!("{} {}: {why}", at("env entry"), shown::short(name)))?;
    }
    let session = match replaces {
        Some(replaced) => replacement_session(raw.session, replaced, &at)?,
        None => session(raw.session, &at, origin)?,
    };
    let terminal = terminal(raw.terminal, &at)?;
    let levels = raw.levels.map(|raw| Levels {
        terminal: raw.terminal.unwrap_or(true),
        hooks: raw.hooks,
        acp: raw.acp,
    });
    let levels = match (replaces, levels) {
        (None, levels) => levels.unwrap_or(Levels {
            terminal: true,
            hooks: false,
            acp: None,
        }),
        // A replacement runs at the built-in's levels, through its adapter: its arming.
        (Some(replaced), None) => replaced.levels.clone(),
        (Some(replaced), Some(levels)) if levels == replaced.levels => levels,
        (Some(replaced), Some(_)) => {
            return Err(format!(
                "{} differs from {}'s — a replacement changes the facts purlis measured on a \
                 harness it ships, and never how purlis runs and arms it. Remove [levels].",
                at("[levels]"),
                replaced.title
            ));
        }
    };
    if !levels.terminal {
        return Err(format!(
            "{} says terminal = false — every harness purlis runs starts in its terminal \
             (level 1), and a chat in a tab always does. Remove the line.",
            at("[levels]")
        ));
    }
    if levels.hooks && origin != Origin::BuiltIn && replaces.is_none() {
        return Err(format!(
            "{} says hooks = true — level 2 needs an adapter purlis ships to arm a harness's \
             hooks, and a declaration cannot carry one. Remove the line; the harness runs in \
             its terminal.",
            at("[levels]")
        ));
    }
    if terminal.ready_to_type == ReadyToType::OnStart && !levels.hooks {
        return Err(format!(
            "{} is on-start, which waits for a hook to report the start, and this harness \
             has no hooks purlis arms. Write raw-and-quiet or never.",
            at("[terminal] ready_to_type")
        ));
    }
    // A replacement's is the built-in's own, read with the built-in.
    if replaces.is_none()
        && let Some(acp) = &levels.acp
    {
        let program_ok = acp.first().is_some_and(|program| {
            bare_word(program) && (origin == Origin::BuiltIn || launcher(program).is_none())
        });
        if !program_ok {
            return Err(format!(
                "{} is not a command purlis runs — a list of words whose first is a bare \
                 program name found on PATH, and not a shell, an interpreter, a launcher or a \
                 harness purlis ships. Write it as one.",
                at("[levels] acp")
            ));
        }
        template(&acp[1..], at("[levels] acp"), origin)?;
    }
    let mut capabilities = BTreeMap::new();
    for (key, value) in raw.capabilities {
        if !CAPABILITIES.contains(&key.as_str()) {
            return Err(format!(
                "{} {} is not a harness capability purlis reads — one of: {}. Remove it.",
                at("[capabilities]"),
                shown::short(&key),
                CAPABILITIES.join(", ")
            ));
        }
        let answer = answer(&value, &format!("{} {key}", at("[capabilities]")))?;
        capabilities.insert(key, answer);
    }
    if capabilities.get("resumes_by_id") == Some(&Answer::Yes) && session.resume.is_none() {
        return Err(format!(
            "{} says resumes_by_id = \"yes\" and [session] has no resume — say how it \
             resumes, or answer no.",
            at("[capabilities]")
        ));
    }
    // What of this machine's environment a chat on a harness purlis ships is started with is
    // purlis's own list too: a replacement leaves it out or restates it. `[chat_env]` in the
    // same file is where this machine passes more.
    let env = match replaces {
        Some(replaced) if raw.env.is_empty() || raw.env == replaced.env => replaced.env.clone(),
        Some(replaced) => {
            return Err(format!(
                "{} differs from {}'s — a replacement changes the facts purlis measured on a \
                 harness it ships, and never what its chats are started with. [chat_env] in \
                 {} passes more of this machine's environment to a chat. Remove env.",
                at("env"),
                replaced.title,
                crate::profiles::LOCAL_FILE
            ));
        }
        None => raw.env,
    };
    let sign_in = match (replaces, raw.sign_in) {
        (None, raw_sign_in) => sign_in(raw_sign_in.unwrap_or_default(), &raw.name, &at)?,
        (Some(replaced), None) => replaced.sign_in.clone(),
        (Some(replaced), Some(raw_sign_in)) => kept_as_built(
            sign_in(raw_sign_in, &raw.name, &at)?,
            &replaced.sign_in,
            "[sign_in]",
            replaced,
            &at,
        )?,
    };
    let folders = match (replaces, raw.folders) {
        (None, raw_folders) => folders(raw_folders.unwrap_or_default(), &env, origin, &at)?,
        (Some(replaced), None) => replaced.folders.clone(),
        (Some(replaced), Some(raw_folders)) => kept_as_built(
            folders(raw_folders, &replaced.env, Origin::BuiltIn, &at)?,
            &replaced.folders,
            "[folders]",
            replaced,
            &at,
        )?,
    };
    let conversation = match (replaces, raw.conversation) {
        (None, None) => None,
        (None, Some(raw_conversation)) => {
            Some(conversation(raw_conversation, &folders, origin, &at)?)
        }
        (Some(replaced), None) => replaced.conversation.clone(),
        (Some(replaced), Some(raw_conversation)) => kept_as_built(
            Some(conversation(
                raw_conversation,
                &replaced.folders,
                Origin::BuiltIn,
                &at,
            )?),
            &replaced.conversation,
            "[conversation]",
            replaced,
            &at,
        )?,
    };
    Ok(Declaration {
        title: raw.title.unwrap_or_else(|| raw.name.clone()),
        name: raw.name,
        program: raw.program,
        env,
        tested: raw.tested,
        login: raw.login,
        session,
        terminal,
        levels,
        sign_in,
        folders,
        conversation,
        capabilities,
        origin,
        file: file.to_owned(),
        digest: format!(
            "sha256:{}",
            crate::extension::hex(&sha2::Sha256::digest(text.as_bytes()))
        ),
        text: text.to_owned(),
    })
}

/// One answer in the grammar of a harness capability (ADR 0073 §6): `"yes"`, `"no: <the
/// reason>"` or `"unknown"`, the reason one line a card can draw. `what` names the key.
fn answer(value: &str, what: &str) -> Result<Answer, String> {
    match value {
        "yes" => Ok(Answer::Yes),
        "unknown" => Ok(Answer::Unknown),
        other => match other.strip_prefix("no:").map(str::trim) {
            Some(reason) if !reason.is_empty() => match drawn_refusal(reason, MOST_REASON) {
                Some(why) => Err(format!("{what}'s reason {why}")),
                None => Ok(Answer::No(reason.to_owned())),
            },
            _ => Err(format!(
                "{what} is {}, and a capability is \"yes\", \"no: <the reason>\" or \
                 \"unknown\". Write one of them.",
                shown::short(other)
            )),
        },
    }
}

/// `given`, the facts of a replacement of `replaced` under `table`, when they are the
/// built-in's own word for word: what its profiles sign in with, where it keeps its files and
/// how its conversations move are how purlis runs a harness it ships, and a replacement
/// changes the facts measured on it alone (#968).
fn kept_as_built<T: PartialEq>(
    given: T,
    built: &T,
    table: &str,
    replaced: &Declaration,
    at: &dyn Fn(&str) -> String,
) -> Result<T, String> {
    if given == *built {
        return Ok(given);
    }
    Err(format!(
        "{} differs from {}'s — a replacement changes the facts purlis measured on a harness \
         it ships, and never how its profiles sign in, where it keeps its files or how its \
         conversations move. Remove {table}.",
        at(table),
        replaced.title
    ))
}

/// A declaration's `[sign_in]`, of the harness `name`: each kind one of the four words, once;
/// whether its credentials can be kept from its commands, unknown where it does not say; and
/// the plans a subscription signs in to ([`plans`]).
fn sign_in(raw: RawSignIn, name: &str, at: &dyn Fn(&str) -> String) -> Result<SignInFacts, String> {
    let mut kinds = Vec::new();
    for word in &raw.kinds {
        let Some(kind) = SignInKind::of_word(word) else {
            return Err(format!(
                "{} holds {}, and a sign-in kind is one of: {}. Write one of them.",
                at("[sign_in] kinds"),
                shown::short(word),
                SignInKind::ALL.map(SignInKind::word).join(", ")
            ));
        };
        if kinds.contains(&kind) {
            return Err(format!(
                "{} names {} twice. Remove one.",
                at("[sign_in] kinds"),
                kind.word()
            ));
        }
        kinds.push(kind);
    }
    let kept_from_commands = match raw.kept_from_commands.as_deref() {
        None => Answer::Unknown,
        Some(value) => answer(value, &at("[sign_in] kept_from_commands"))?,
    };
    let plans = plans(raw.plans, &kinds, name, at)?;
    Ok(SignInFacts {
        kinds,
        kept_from_commands,
        plans,
    })
}

/// `[sign_in] plans` of the harness `name`: only beside a `subscription` kind; each provider
/// the harness's own id, a word its sign-in is typed with, named once; each title words a
/// window draws; and [`ANTHROPIC`] in Claude Code's own declaration alone.
fn plans(
    raw: Vec<RawPlan>,
    kinds: &[SignInKind],
    name: &str,
    at: &dyn Fn(&str) -> String,
) -> Result<Vec<Plan>, String> {
    let here = at("[sign_in] plans");
    if !raw.is_empty() && !kinds.contains(&SignInKind::Subscription) {
        return Err(format!(
            "{here} names the plans a subscription signs in to, and [sign_in] kinds lists no \
             subscription. List subscription in kinds, or remove plans."
        ));
    }
    let mut plans: Vec<Plan> = Vec::new();
    for RawPlan { provider, title } in raw {
        let id_shaped = provider.len() <= MOST_PROVIDER
            && provider
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            && provider
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "-_.".contains(c));
        if !id_shaped {
            return Err(format!(
                "{here} holds {}, which is not a provider's id: lower-case letters, digits, \
                 '-', '_' and '.', starting with a letter or digit, at most {MOST_PROVIDER}. \
                 Write the id the harness's own sign-in takes.",
                shown::short(&provider)
            ));
        }
        if provider == ANTHROPIC && name != CLAUDE_CODE {
            return Err(format!(
                "{here} names {ANTHROPIC}, and a Claude plan is signed in only through Claude \
                 Code's own sign-in. Remove it."
            ));
        }
        if plans.iter().any(|plan| plan.provider == provider) {
            return Err(format!("{here} names {provider} twice. Remove one."));
        }
        if let Some(why) = drawn_refusal(&title, MOST_TITLE) {
            return Err(format!("{here}' title {why}"));
        }
        plans.push(Plan { provider, title });
    }
    Ok(plans)
}

/// A declaration's `[folders]`: each a variable's whole name, and for a harness purlis does
/// not ship one under `env`'s refusals and inside its own namespace, `env`; the two apart. A
/// built-in's are purlis's own measured words (opencode's are XDG folders, which `env` refuses
/// a project).
fn folders(
    raw: RawFolders,
    env: &[String],
    origin: Origin,
    at: &dyn Fn(&str) -> String,
) -> Result<Folders, String> {
    for (key, name) in [("config_env", &raw.config_env), ("data_env", &raw.data_env)] {
        let Some(name) = name else { continue };
        let what = at(&format!("[folders] {key}"));
        if name.ends_with('*') {
            return Err(format!(
                "{what} {} is a prefix, and a folder is one variable's whole name. Name it.",
                shown::short(name)
            ));
        }
        if origin == Origin::BuiltIn {
            continue;
        }
        env_ok(name).map_err(|why| format!("{what} {}: {why}", shown::short(name)))?;
        if !env.iter().any(|ours| overlap(name, ours)) {
            return Err(format!(
                "{what} is {}, which is not in its env namespace — a declaration's folders are \
                 variables of the harness's own. Add it to env, or name one there.",
                shown::short(name)
            ));
        }
    }
    if raw.config_env.is_some() && raw.config_env == raw.data_env {
        return Err(format!(
            "{} names one variable for config_env and data_env. Leave data_env out where they \
             are one folder.",
            at("[folders]")
        ));
    }
    if raw.data_env.is_some() && raw.config_env.is_none() {
        return Err(format!(
            "{} gives data_env and no config_env — config_env is where the harness keeps its \
             settings and its sign-in. Name it.",
            at("[folders]")
        ));
    }
    Ok(Folders {
        config_env: raw.config_env,
        data_env: raw.data_env,
    })
}

/// The most characters of a `[conversation] file`.
const MOST_FILE: usize = 200;

/// A declaration's `[conversation]`: a `file` under one of its folders, or its own `export`
/// and `import` commands, whose words take the shapes every word after a program does; which
/// one is the keys it gives.
fn conversation(
    raw: RawConversation,
    folders: &Folders,
    origin: Origin,
    at: &dyn Fn(&str) -> String,
) -> Result<Conversation, String> {
    match (raw.file, raw.export, raw.import) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => Err(format!(
            "{} gives a file and an export or import — a conversation moves as a file, or \
             through the harness's own export and import. Keep one.",
            at("[conversation]")
        )),
        (Some(file), None, None) => {
            if let Some(why) = file_refusal(&file, folders) {
                return Err(format!(
                    "{} is {}, which {why}. Write it as {{config}}/ or {{data}}/ and a path \
                     of plain parts, with * for a part kept as it is, {{cwd_key}} for the \
                     working directory's folder and {{id}} for the session's id.",
                    at("[conversation] file"),
                    shown::short(&file)
                ));
            }
            Ok(Conversation::File {
                file,
                with_forked_from: raw.with_forked_from.unwrap_or(false),
            })
        }
        (None, export, import) => {
            if raw.with_forked_from.is_some() {
                return Err(format!(
                    "{} gives with_forked_from and no file — it says a file comes with the \
                     session's own. Give file, or remove it.",
                    at("[conversation]")
                ));
            }
            let (Some(export), Some(import)) = (export, import) else {
                return Err(format!(
                    "{} gives neither a file nor both export and import: the words that print \
                     session {{id}}, and the words that take what they printed.",
                    at("[conversation]")
                ));
            };
            template(&export, at("[conversation] export"), origin)?;
            template(&import, at("[conversation] import"), origin)?;
            if !export.iter().any(|word| word.contains("{id}")) {
                return Err(format!(
                    "{} names no {{id}}, so it could not export any one session. Put {{id}} \
                     where the harness takes it.",
                    at("[conversation] export")
                ));
            }
            if import.is_empty() || import.iter().any(|word| word.contains("{id}")) {
                return Err(format!(
                    "{} are the words before the file it takes, and hold no {{id}}. Write \
                     them.",
                    at("[conversation] import")
                ));
            }
            Ok(Conversation::Export { export, import })
        }
    }
}

/// Why `file` may not be a `[conversation] file`, or nothing: it starts at a folder the
/// declaration names (`{config}/`, `{data}/`), names `{id}`, and every part after is plain
/// (letters, digits, `.`, `_`, `-`, `{id}`), or `*` alone (a part kept as it was found), or
/// `{cwd_key}` alone; never empty, `.` or `..`, and no other glob.
fn file_refusal(file: &str, folders: &Folders) -> Option<&'static str> {
    if file.chars().count() > MOST_FILE {
        return Some("is over 200 characters");
    }
    let Some((folder, rest)) = file.split_once('/') else {
        return Some("does not start at {config}/ or {data}/");
    };
    let named = match folder {
        "{config}" => folders.config_env.is_some(),
        "{data}" => folders.data_env.is_some(),
        _ => return Some("does not start at {config}/ or {data}/"),
    };
    if !named {
        return Some("starts at a folder its [folders] does not name");
    }
    if !rest.contains("{id}") {
        return Some("names no {id}");
    }
    let plain = |part: &str| {
        if part == "*" || part == "{cwd_key}" {
            return true;
        }
        let bare = part.replace("{id}", "").replace('*', "");
        // A `*` inside a part stands for what varies in a name (`rollout-*-{id}.jsonl`), never
        // for more than one part.
        !part.is_empty()
            && part != "."
            && part != ".."
            && !part.contains("**")
            && bare
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    };
    (!rest.split('/').all(plain)).then_some("holds a part that is not plain")
}

/// The `[session]` of this machine's replacement of `replaced`'s facts (#968): **the
/// built-in's own**, left out or restated word for word. The words purlis adds to the command
/// line of a harness it ships run with no question asked, because they are purlis's and no
/// file's; a table in a file a chat may have written adds none. A profile in
/// `charter.local.toml` is how a word is added to a harness's command, and it asks first.
fn replacement_session(
    raw: RawSession,
    replaced: &Declaration,
    at: &dyn Fn(&str) -> String,
) -> Result<Session, String> {
    if raw.chosen_by.is_none()
        && raw.new.is_none()
        && raw.resume.is_none()
        && raw.named_by.is_empty()
    {
        return Ok(replaced.session.clone());
    }
    // Read by the rules the built-in's own words were, as they have to be those words.
    let given = session(raw, at, Origin::BuiltIn)?;
    if given != replaced.session {
        return Err(format!(
            "{} differs from {}'s — a replacement changes the facts purlis measured on a \
             harness it ships, and never the words purlis runs it with. A profile in {} adds \
             a word to its command, and asks before it runs. Remove [session].",
            at("[session]"),
            replaced.title,
            crate::profiles::LOCAL_FILE
        ));
    }
    Ok(given)
}

fn session(
    raw: RawSession,
    at: &dyn Fn(&str) -> String,
    origin: Origin,
) -> Result<Session, String> {
    let chosen_by = match raw.chosen_by.as_deref() {
        None | Some("harness") => ChosenBy::Harness,
        // "charter" is the old spelling, read through the rename's window (#1756).
        Some("purlis" | "charter") => ChosenBy::Charter,
        Some(other) => {
            return Err(format!(
                "{} is {}, and it is \"harness\" or \"purlis\". Write one of them.",
                at("[session] chosen_by"),
                shown::short(other)
            ));
        }
    };
    let new = raw.new.unwrap_or_default();
    template(&new, at("[session] new"), origin)?;
    let gives_id = new.iter().any(|word| word.contains("{id}"));
    match chosen_by {
        ChosenBy::Charter if !gives_id => {
            return Err(format!(
                "{} says chosen_by = \"purlis\" and [session] new hands over no {{id}} — say \
                 how the id is handed over, or write chosen_by = \"harness\".",
                at("[session]")
            ));
        }
        ChosenBy::Harness if gives_id => {
            return Err(format!(
                "{} hands over an {{id}}, and chosen_by is \"harness\" — purlis chooses an id \
                 only for a harness that takes one. Write chosen_by = \"purlis\".",
                at("[session] new")
            ));
        }
        _ => {}
    }
    if let Some(resume) = &raw.resume {
        template(resume, at("[session] resume"), origin)?;
        if !resume.iter().any(|word| word.contains("{id}")) {
            return Err(format!(
                "{} names no {{id}}, so it could not bring any one session back. Put {{id}} \
                 where the harness takes it.",
                at("[session] resume")
            ));
        }
    }
    if raw.named_by.iter().any(String::is_empty) {
        return Err(format!(
            "{} holds an empty word. Remove it.",
            at("[session] named_by")
        ));
    }
    Ok(Session {
        chosen_by,
        new,
        resume: raw.resume,
        named_by: raw.named_by,
    })
}

/// The bytes a declaration's `[terminal] newline` may be: the keys terminals read as a new
/// line in a prompt's input (Enter, line feed, both, Alt+Enter, a backslash before Enter, and
/// Shift+Enter in the kitty keyboard encoding, CSI 13;2u).
/// It is typed into the program's terminal, so it is never free text (#1119).
const NEWLINES: [&str; 6] = ["\r", "\n", "\r\n", "\x1b\r", "\\\r", "\x1b[13;2u"];

fn terminal(raw: RawTerminal, at: &dyn Fn(&str) -> String) -> Result<Terminal, String> {
    let ready_to_type = match raw.ready_to_type.as_deref() {
        None | Some("never") => ReadyToType::Never,
        Some("raw-and-quiet") => ReadyToType::RawAndQuiet,
        Some("on-start") => ReadyToType::OnStart,
        Some(other) => {
            return Err(format!(
                "{} is {}, and it is \"on-start\", \"raw-and-quiet\" or \"never\". Write one \
                 of them.",
                at("[terminal] ready_to_type"),
                shown::short(other)
            ));
        }
    };
    match raw.newline.as_deref() {
        None => {}
        Some("") => {
            return Err(format!(
                "{} is empty. Remove the line, and the chat keeps the terminal's own Enter.",
                at("[terminal] newline")
            ));
        }
        Some(newline) if !NEWLINES.contains(&newline) => {
            return Err(format!(
                "{} is {}, which is not one of the keys a terminal reads as a new line: \"\\r\", \
                 \"\\n\", \"\\r\\n\", \"\\u001b\\r\", \"\\\\\\r\" or \"\\u001b[13;2u\". Write one of \
                 them, or remove the line and the chat keeps the terminal's own Enter.",
                at("[terminal] newline"),
                shown::short(&format!("{newline:?}"))
            ));
        }
        Some(_) => {}
    }
    let paste_drawn_whole = match raw.paste_drawn_whole {
        None => None,
        Some(RawPaste { lines: Some(0), .. })
        | Some(RawPaste { chars: Some(0), .. })
        | Some(RawPaste {
            lines: None,
            chars: None,
        }) => {
            return Err(format!(
                "{} measures nothing — give lines, chars or both, each at least 1, and leave \
                 out the one that has no limit.",
                at("[terminal] paste_drawn_whole")
            ));
        }
        Some(RawPaste { lines, chars }) => Some(DrawnWhole {
            lines: lines.map_or(usize::MAX, |n| n as usize),
            chars: chars.map_or(usize::MAX, |n| n as usize),
        }),
    };
    Ok(Terminal {
        newline: raw.newline,
        paste_drawn_whole,
        ready_to_type,
    })
}

/// Every word after a program — a session template's, a resume's, an ACP command's — held to
/// the one shape charter allows (ruling V66c), and checked against the launchers too.
///
/// **Why an allowlist** (rulings V66, V66c): every word here is appended to a program's
/// command line on a click, out of a committed file. A list of refused shapes missed a
/// response file (`@args`), a glued value (`-I.`), a bare relative filename, a path after `=`;
/// so a word is taken only in a shape a reader can take at a glance for a flag or a value:
///
/// - a flag: `-x`, or `--name` (letters, digits and `-`);
/// - `--name=value`, the value plain: letters, digits and `-_.:,+`, no leading `.` and no
///   drive letter (`C:`), and
///   `{id}` or `{name}` inside it (`--title={name}`);
/// - a subcommand: lowercase letters, digits and `-`, no dot (`exec`, `run`);
/// - a bare `{id}` or `{name}`, for a built-in only, which passes them that way today.
fn template(words: &[String], what: String, origin: Origin) -> Result<(), String> {
    for word in words {
        if let Some(why) = word_refusal(word, origin) {
            return Err(format!(
                "{what} holds {}, which {why}. A word after the program is a flag (-x, \
                 --name), --name=<a plain value>, or a lowercase subcommand, and takes {{id}} \
                 and {{name}} only inside a flag's value (--session={{id}}). Rewrite it.",
                shown::short(word)
            ));
        }
    }
    Ok(())
}

/// Why `word` may not be a word after the program, or nothing (ruling V66c).
fn word_refusal(word: &str, origin: Origin) -> Option<&'static str> {
    if word.is_empty() {
        return Some("is empty");
    }
    if origin == Origin::BuiltIn && (word == "{id}" || word == "{name}") {
        return None;
    }
    // A session id never reads as a flag ([`SessionId::new`]), so this machine's own table
    // hands one over bare, as a built-in does (`codex resume <id>`). A chat's name it does not.
    if origin == Origin::Local && word == "{id}" {
        return None;
    }
    let literal = word.replace("{id}", "").replace("{name}", "");
    if literal.contains('{') || literal.contains('}') {
        return Some("is a placeholder purlis does not fill");
    }
    // `{name}` is the chat's name, which the operator types: at the start of a word it could
    // make the word a flag (`--yolo`).
    if word.starts_with("{name}") {
        return Some("starts with {name}, and a chat's name could make it a flag");
    }
    if word == "{id}" {
        return Some("is a bare {id}, which a project's declaration hands over as a flag's value");
    }
    if word.contains('/') || word.starts_with('~') || word.starts_with('.') {
        return Some("is a path");
    }
    if is_assignment(word) {
        return Some("is a VAR=value assignment");
    }
    if word.contains('@') {
        return Some("names a file to read more words from (@file)");
    }
    if word
        .chars()
        .any(|c| c.is_whitespace() || !c.is_ascii() || ";|&$`<>()[]*?!#'\"\\^".contains(c))
    {
        return Some("holds shell syntax or a space");
    }
    let flag_name = |name: &str| {
        let mut chars = name.chars();
        chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '-')
    };
    if let Some(long) = word.strip_prefix("--") {
        let (name, value) = match long.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            None => (long, None),
        };
        if !flag_name(name) {
            return Some("is not a flag purlis takes: a flag's name is letters, digits and '-'");
        }
        // A bare `--name` is a flag; `--name=value` has its value judged.
        let value = value?;
        let plain = value.replace("{id}", "").replace("{name}", "");
        // No `%`: some programs decode `%2F` into a path, and no measured harness passes one.
        // No drive letter: `C:` is a path on Windows.
        let drive = {
            let mut chars = value.chars();
            chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.next() == Some(':')
        };
        if value.is_empty()
            || value.starts_with('.')
            || drive
            || !plain.chars().all(|c| {
                c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | ',' | '+')
            })
        {
            return Some(
                "gives a flag a value that is not plain: letters, digits and -_.:,+ with no \
                 leading dot and no drive letter",
            );
        }
        return word_launcher(&plain, origin);
    }
    if let Some(short) = word.strip_prefix('-') {
        let mut chars = short.chars();
        let one = chars.next().is_some_and(|c| c.is_ascii_alphanumeric()) && chars.next().is_none();
        return (!one).then_some("glues a value to a short flag, or is not a flag purlis takes");
    }
    let subcommand = word
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !subcommand {
        return Some(
            "is not a flag, a flag's value or a lowercase subcommand (a bare filename, a dot or \
             a capital is none of them)",
        );
    }
    word_launcher(word, origin)
}

/// What the approval dialog says beside `word` when it names a file or folder at the project
/// root (`root`), or nothing: a flag's value or a subcommand-shaped word the program may
/// resolve in the chat's folder. The shapes allow it; the operator sees it (ruling of
/// 2026-10-03, V66c's residual).
pub fn names_in_project(root: &Path, word: &str) -> Option<String> {
    let named = match word.strip_prefix("--") {
        Some(long) => long.split_once('=')?.1,
        None if word.starts_with('-') => return None,
        None => word,
    };
    if named.is_empty() || named.contains('{') {
        return None;
    }
    let found = std::fs::symlink_metadata(root.join(named)).ok()?;
    let what = if found.is_dir() { "folder" } else { "file" };
    Some(format!(
        "{} names the {what} {} in this project; the program may read it",
        shown::readable(word, usize::MAX),
        shown::readable(named, usize::MAX)
    ))
}

/// Why a subcommand or a flag's value names a program that would run whatever follows it, or
/// nothing: the launcher check, over every word, as defence in depth (ruling V66c). `exec` is a
/// subcommand (`codex exec`), and no program on PATH.
///
/// A built-in's words are charter's own, and are not asked: the built-ins are what
/// [`builtins`] is being made from when they are read, so asking it would wait on itself.
fn word_launcher(word: &str, origin: Origin) -> Option<&'static str> {
    if origin == Origin::BuiltIn || word.is_empty() || word == "exec" {
        return None;
    }
    let lower = word.to_ascii_lowercase();
    let family = INTERPRETER_FAMILIES.iter().any(|family| {
        lower
            .strip_prefix(family)
            .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit() || c == '.'))
    });
    (family
        || LAUNCHERS.contains(&lower.as_str())
        || builtins()
            .iter()
            .any(|d| d.program.eq_ignore_ascii_case(word)))
    .then_some("names a shell, an interpreter, a launcher or a harness purlis ships")
}

/// `NAME=…` with `NAME` a variable's name: what `env` and a shell read as an assignment.
fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A program name found on `PATH`: letters, digits, `.`, `_`, `+` and `-`, starting with a
/// letter or a digit — so never a path, a flag, `~`, or a shell string.
fn bare_word(word: &str) -> bool {
    let mut chars = word.chars();
    word.len() <= 128
        && chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
}

/// Shells, interpreters and launchers: programs that run whatever their arguments say, so a
/// declaration naming one would be a script in a file (ruling V66). Matched without case.
const LAUNCHERS: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "fish",
    "dash",
    "ksh",
    "mksh",
    "csh",
    "tcsh",
    "ash",
    "busybox",
    "nu",
    "xonsh",
    "elvish",
    "env",
    "sudo",
    "doas",
    "su",
    "nohup",
    "exec",
    "xargs",
    "time",
    "nice",
    "timeout",
    "script",
    "watch",
    "parallel",
    "node",
    "nodejs",
    "deno",
    "bun",
    "npx",
    "npm",
    "pnpm",
    "pnpx",
    "yarn",
    "uv",
    "uvx",
    "pipx",
    "pip",
    "perl",
    "ruby",
    "php",
    "lua",
    "luajit",
    "tclsh",
    "wish",
    "expect",
    "osascript",
    "pwsh",
    "powershell",
    "cmd",
    "java",
    "awk",
    "gawk",
    "mawk",
    "nawk",
    "sed",
    "make",
    "docker",
    "podman",
    "ssh",
    "open",
    "xdg-open",
    "launchctl",
    "systemd-run",
    "flatpak",
    "snap",
    "nix",
    "nix-shell",
    "guix",
    "arch",
    "caffeinate",
    "xcrun",
    "sandbox-exec",
    "stdbuf",
    "chrt",
    "taskset",
    "strace",
    "ltrace",
    "dtrace",
    "dtruss",
    "tmux",
    "screen",
    "go",
    "cargo",
    "rustc",
    "swift",
    "swiftc",
    "dotnet",
    "bunx",
    "corepack",
    "rscript",
    "r",
    "julia",
    "tsx",
    "ts-node",
    "qjs",
    "jshell",
    "groovy",
    "scala",
    "kotlin",
    "kotlinc",
    "elixir",
    "iex",
    "mix",
    "erl",
    "escript",
    "ghci",
    "runghc",
    "stack",
    "cabal",
    "racket",
    "guile",
    "sbcl",
    "clisp",
    "ocaml",
    "utop",
    "vim",
    "nvim",
    "vi",
    "emacs",
    "ex",
    "ed",
    "nano",
    "gdb",
    "lldb",
    "git",
    "just",
    "bundle",
    "bundler",
    "rake",
    "gem",
    "poetry",
    "pipenv",
    "conda",
    "mamba",
    "micromamba",
    "mise",
    "rtx",
    "asdf",
    "direnv",
    "volta",
    "fnm",
    "nvm",
    "nodenv",
    "pyenv",
    "rbenv",
    "goenv",
    "jenv",
    "sdk",
    "zx",
    "nushell",
    "yash",
    "rc",
    "osh",
    "oil",
    "ion",
    "find",
    "chroot",
    "unshare",
    "nsenter",
    "firejail",
    "bwrap",
    "proot",
    "setsid",
    "runuser",
    "pkexec",
    "gosu",
    "su-exec",
    "entr",
    "nodemon",
    "watchexec",
    "foreman",
    "honcho",
    "pm2",
    "at",
    "batch",
    "crontab",
    "toybox",
    "automator",
    "shortcuts",
    "login",
    // The product itself, by every name it has had (RN-3): `crate::cliname::RECOGNISED`.
    "purlis",
    "charter",
    "edm",
];

/// Interpreters that come in versioned names (`python3.12`, `ruby3`, `pwsh-preview`): a
/// program starting with one is refused, and a word that is one plus a version.
const INTERPRETER_FAMILIES: &[&str] = &[
    "python", "pypy", "ruby", "perl", "php", "node", "lua", "bash", "zsh", "pwsh", "julia",
];

/// Why `program` may not be a project declaration's program, or nothing.
///
/// **Also under a versioned or suffixed name** (#1119, D-1119-1): `ksh93`, `tclsh8.6`,
/// `luajit-2.1` and `bash.exe` are the launcher they name ([`unversioned`]), so a name check
/// of the bare word alone would let each through. It only refuses more: a name that is no
/// launcher's, versioned or not (`aider2`), is as it was.
fn launcher(program: &str) -> Option<&'static str> {
    let lower = program.to_ascii_lowercase();
    let base = unversioned(&lower);
    if builtins()
        .iter()
        .any(|d| d.program.eq_ignore_ascii_case(program) || d.program.eq_ignore_ascii_case(base))
    {
        return Some(
            "the program of a harness purlis ships, which runs only armed, through its adapter",
        );
    }
    let interpreter = INTERPRETER_FAMILIES
        .iter()
        .any(|family| lower.starts_with(family));
    (interpreter || LAUNCHERS.contains(&lower.as_str()) || LAUNCHERS.contains(&base))
        .then_some("a shell, an interpreter or a launcher")
}

/// `program` without a Windows `.exe` and then a version at its end (digits and dots, after
/// an optional `-` or `_`): `tclsh8.6` is `tclsh`, `luajit-2.1.exe` is `luajit`. A name that
/// is all version is kept whole. Trailing dots go first: Windows drops them from a file's
/// name, so `bash.exe.` runs `bash.exe`.
fn unversioned(program: &str) -> &str {
    let program = program.trim_end_matches('.');
    let name = program.strip_suffix(".exe").unwrap_or(program);
    let bare = name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    let bare = bare.strip_suffix(['-', '_']).unwrap_or(bare);
    if bare.is_empty() { name } else { bare }
}

/// Variables that change what a process loads or runs, or where it finds programs: a namespace
/// reaching one would let a declaration steer the harness's own process (ruling V66).
const INJECTORS: &[&str] = &[
    "LD_*",
    "DYLD_*",
    "NODE_OPTIONS",
    "NODE_PATH",
    "PYTHONPATH",
    "PYTHONHOME",
    "PYTHONSTARTUP",
    "PERL5LIB",
    "PERL5OPT",
    "PERLLIB",
    "RUBYOPT",
    "RUBYLIB",
    "JAVA_TOOL_OPTIONS",
    "BASH_ENV",
    "ENV",
    "PATH",
    "HOME",
    "SHELL",
    "ZDOTDIR",
    "PROMPT_COMMAND",
    "IFS",
    "GIT_*",
    // The JDK's own launcher options, the shell's exported functions and where every tool
    // reads its configuration (git's `core.sshCommand` among it).
    "JDK_JAVA_OPTIONS",
    "BASH_FUNC_*",
    "XDG_CONFIG_HOME",
];

/// Variables that set what a chat's tools trust, connect through or run on the operator's
/// behalf (#1013): TLS trust, proxies, the SSH agent and the programs a tool starts (a pager,
/// an editor, a browser, a compiler or its wrapper). Each is the exact name programs read; a
/// prefix only where every name under it is the class's own (D-1013-3).
const STEERS: &[&str] = &[
    // TLS trust.
    "SSL_*",
    "NODE_EXTRA_CA_CERTS",
    "NODE_TLS_REJECT_UNAUTHORIZED",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "AWS_CA_BUNDLE",
    "PIP_CERT",
    "PIP_CLIENT_CERT",
    "PIP_TRUSTED_HOST",
    "CARGO_HTTP_*",
    "DENO_CERT",
    "DENO_TLS_CA_STORE",
    "NODE_USE_SYSTEM_CA",
    "PYTHONHTTPSVERIFY",
    "HTTPLIB2_CA_CERTS",
    "YARN_CA_FILE_PATH",
    "YARN_ENABLE_STRICT_SSL",
    "YARN_CAFILE",
    "YARN_STRICT_SSL",
    "GOINSECURE",
    "GONOSUMDB",
    "GONOSUMCHECK",
    "GOSUMDB",
    // Where a fetching tool reads its own configuration, trust and proxy included.
    "CURL_HOME",
    "WGETRC",
    "NIX_SSL_CERT_FILE",
    "GRPC_DEFAULT_SSL_ROOTS_FILE_PATH",
    // Proxies (`overlap` ignores case, so `http_proxy` too).
    "HTTP_*",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "FTP_PROXY",
    "SOCKS_PROXY",
    "GRPC_PROXY",
    "RSYNC_PROXY",
    // npm's whole configuration: its proxy, CA, `strict-ssl` and `script-shell` among it.
    "NPM_CONFIG_*",
    "YARN_HTTP_PROXY",
    "YARN_HTTPS_PROXY",
    "PIP_PROXY",
    "GOPROXY",
    "GONOPROXY",
    "NODE_USE_ENV_PROXY",
    "GLOBAL_AGENT_*",
    // The SSH agent and what asks for its passphrase.
    "SSH_*",
    "SUDO_ASKPASS",
    "GPG_AGENT_INFO",
    // Programs a tool runs for you.
    "PAGER",
    "MANPAGER",
    "LESSOPEN",
    "LESSCLOSE",
    "EDITOR",
    "VISUAL",
    "SUDO_EDITOR",
    "FCEDIT",
    "BROWSER",
    "TERMINAL",
    "CC",
    "CXX",
    "CPP",
    "LD",
    "AR",
    "FC",
    "RUSTC_*",
    "RUSTDOC",
    // `CARGO_BUILD_RUSTC`, its wrappers and `CARGO_BUILD_RUSTDOC`; a target's linker and
    // runner (`CARGO_TARGET_<triple>_RUNNER`).
    "CARGO_BUILD_RUSTC*",
    "CARGO_BUILD_RUSTDOC",
    "CARGO_TARGET_*",
    "CCACHE_PREFIX",
    "COMSPEC",
    // Go's `-toolexec` and the toolchain it fetches and runs.
    "GOFLAGS",
    "GOTOOLCHAIN",
];

/// Whether two names-or-prefixes (`NAME`, `PREFIX*`) could name one variable, ignoring case.
fn overlap(one: &str, other: &str) -> bool {
    let (one, other) = (one.to_ascii_uppercase(), other.to_ascii_uppercase());
    match (one.strip_suffix('*'), other.strip_suffix('*')) {
        (None, None) => one == other,
        (None, Some(prefix)) => one.starts_with(prefix),
        (Some(prefix), None) => other.starts_with(prefix),
        (Some(a), Some(b)) => a.starts_with(b) || b.starts_with(a),
    }
}

/// Why `name` may not be in a declaration's environment namespace, or nothing.
fn env_ok(name: &str) -> Result<(), String> {
    let base = name.strip_suffix('*').unwrap_or(name);
    let shaped = base.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && base
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && (base.len() == name.len() || base.ends_with('_'));
    if !shaped {
        return Err(
            "a namespace entry is a variable's name in capitals, digits and '_', or a prefix \
             ending '_*', such as GEMINI_*. Write it that way."
                .to_owned(),
        );
    }
    // The product's own variables under either of their names (RN-2d, V93k).
    if crate::names::ENV_PREFIX
        .spellings()
        .any(|prefix| base.starts_with(prefix) || prefix.starts_with(base))
    {
        return Err(
            "it reaches purlis's own variables, which purlis sets itself. Remove it.".to_owned(),
        );
    }
    if crate::hookwire::harness_internal(name) {
        return Err(
            "it is a harness's internal variable, which stays out of a chat's environment. \
             Remove it."
                .to_owned(),
        );
    }
    if crate::profiles::named_like_a_credential(base)
        || crate::chatenv::CREDENTIALS
            .iter()
            .any(|credential| overlap(name, credential))
    {
        return Err(
            "it is named like a credential, or reaches a family of credentials (a forge's, a \
             cloud's, a model provider's or a registry's), and a declaration never passes one \
             into a chat. Remove it."
                .to_owned(),
        );
    }
    if INJECTORS.iter().any(|injector| overlap(name, injector)) {
        return Err(
            "it reaches a variable that changes what a process loads, runs or finds on PATH, \
             and a declaration never sets how the harness's own process starts. Remove it."
                .to_owned(),
        );
    }
    if STEERS.iter().any(|steer| overlap(name, steer)) {
        return Err(
            "it reaches a variable that sets what a chat's tools trust, connect through or \
             runs (TLS trust, a proxy, the SSH agent, or a pager, editor, browser or compiler), \
             and a declaration never steers those. Remove it."
                .to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod profile_facts_tests;
