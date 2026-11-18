// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A dispatch from an unattended chat** (#1446, spec #1434; decision 20 of the 2026-10-07
//! grill): a chat running with its harness's permission prompts off may dispatch only under a
//! grant that already stands, and what it starts never runs the way it does.
//!
//! # What "unattended" is
//!
//! **The chat's attendance fact** ([`crate::attendance::Fact`], R-110, #6410, T1-22 #6413): set
//! by how purlis started the chat, a task being a chat nobody is at (R-106), and lowered, never
//! raised, by what its own harness's armed hook reported. The app holds it per chat, and every
//! dispatch reader takes it from there ([`crate::attendance::of_held`]): a chat it holds no fact
//! for is unattended. Nothing here reads a hook payload's `permission_mode`.
//!
//! # The one question, for an unattended chat
//!
//! [`covers`] is [`crate::dispatchgrant::covers`] with two differences:
//!
//! - **a grant made for one chat never counts.** Only one made for the person on this machine,
//!   or for everyone in the project, covers the pair. Both were made by a person, at a time
//!   they were there, for every chat of that persona.
//! - **a missing grant is a refusal, never a Notice** ([`Answer::Refused`]). Nobody is there to
//!   press one, and a Notice left waiting is a grant offered to whoever comes back to the
//!   window. So nothing is held, nothing is raised, and no grant can be made from the ask.
//!
//! A chat's own persona needs no grant, and a policy lock refuses, as for any chat.
//!
//! **A refusal for lack of a grant is kept for the person to read afterwards** (#1507,
//! [`crate::dispatchaway`]): an item in the needs-you list, attached to no chat, from which
//! they may make the standing grant for that pair. It is no Notice on the chat and holds no
//! dispatch: the answer here is still a refusal, given at once.
//!
//! # With no sandbox: only a sealed grant (ticket T1.1-73, ruling Q306)
//!
//! A standing grant is a file, and a chat with no sandbox runs as the person and can write it.
//! **So a grant counts only where the app sealed it** ([`crate::secrets::sealed_set`]): one a
//! chat wrote or edited breaks its seal and counts as no grant. Where the seal's key is the
//! app's alone — on macOS, in the app — a chat with its prompts off and no sandbox dispatches
//! under a sealed grant, and under a policy allow the person accepted, as a sandboxed one does.
//! **Elsewhere** (Linux and Windows, where any of the person's programs can read the keyring,
//! or a process that cannot verify a seal) such a chat dispatches to no other persona but under
//! the `dispatch` family's `off` or `log` ([`Refusal::NotSealedHere`]). Its own persona is still
//! its own. **Where the policy asks and no sealed grant covers the pair**, it is refused at once
//! ([`Refusal::Unsandboxed`]) and kept for the person's needs-you list. Whether a chat is
//! sandboxed is the app's record of how it started the chat, never a line the chat sends.
//!
//! # What it starts
//!
//! [`start_of_a_persona_chat`]: the persona chat is a chat like any other, started on a
//! profile as the project and this machine declare it. Nothing of how the asking chat runs
//! goes with it: not a grant it holds, not its opt-out, not its conversation, and never its
//! bypass. A [`crate::start::Start`] has no field for a mode, so the one way a bypass could
//! travel is the asking chat's profile, where that profile's own command is what switches the
//! prompts off ([`bypass_in`]); a persona chat is not started on such a profile taken from the
//! asking chat. The persona chat asks what it asks in its own tab, and waits there, shown as
//! needing the person, like any chat that asks.
//!
//! **Its permission mode travels capped at auto** (#3787, the operator's ruling of 2026-10-29
//! amending this decision): a task of a chat in auto or with its prompts off starts in auto,
//! where its harness's classifier still judges every tool call ([`crate::dispatchmode`]). The
//! level rides the task's record in the app, never the [`crate::start::Start`], and is never a
//! bypass.

use crate::attendance::ClaudeCodeMode;
use crate::dispatchgrant::{self, Covers, InForce};
use crate::sandbox::policy::Locks;
use crate::start::Start;

/// How the asking chat runs, as the app holds it: never as a dispatch says.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Attendance {
    /// A person answers its prompts.
    #[default]
    Attended,
    /// Its harness's permission prompts are off: nobody answers.
    Unattended,
}

/// What [`covers`] answers an unattended chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// The dispatch may start: the same persona, or a grant that stands.
    Covered,
    /// An administrator's policy locks it: the policy's sentence, naming who set it.
    Locked(String),
    /// Nothing starts, nothing is held and no Notice is raised.
    Refused(Refusal),
}

/// Why an unattended chat's dispatch is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The chat was started without a sandbox, the policy asks, and no sealed grant covers the
    /// pair: what is missing. Kept for the needs-you list.
    Unsandboxed(Missing),
    /// No grant that counts covers the pair.
    Missing(Missing),
    /// The person said never to the pair on this machine (#1503): the asking persona and the
    /// target.
    Never(String, String),
    /// The person said never to a persona above this chat in its chain dispatching to the
    /// target: that persona, and the target.
    NeverAbove(String, String),
    /// This machine's record of nevers does not read, so no grant counts.
    NeversUnread,
    /// The project's policy denies the pair (`[dispatch.denies]`): the target.
    Denied(String),
    /// A grant covers the pair, and on this system its seal does not keep it out of reach of
    /// a chat with no sandbox: the target.
    NotSealedHere(String),
}

impl Refusal {
    /// The sentence the asking chat reads.
    pub fn say(&self) -> String {
        match self {
            Self::Missing(missing) => missing.say(),
            Self::Never(asking, target) => dispatchgrant::never_said(asking, target),
            Self::NeverAbove(above, target) => dispatchgrant::never_above_said(above, target),
            Self::NeversUnread => dispatchgrant::NEVERS_UNREAD.to_owned(),
            Self::Denied(target) => dispatchgrant::denied_said(target),
            Self::Unsandboxed(missing) => {
                let target = crate::shown::short(&missing.target);
                format!(
                    "this chat runs with its harness's permission prompts off and with no \
                     sandbox, so nobody is here to answer for a dispatch, and no grant the \
                     person made covers this chat's persona dispatching to {target}. The \
                     person will see this pair in their needs-you list, where one press \
                     grants it; until then, do this work without {target}, or \
                     say in what you leave behind that it is waiting."
                )
            }
            Self::NotSealedHere(target) => {
                let target = crate::shown::short(target);
                format!(
                    "this chat runs with its harness's permission prompts off and with no \
                     sandbox. A grant covers {target}, and on this system purlis cannot keep a \
                     grant out of reach of a chat with no sandbox (only the macOS Keychain \
                     keeps purlis's seal to its app), so a chat with neither a person nor a \
                     sandbox dispatches to no persona but its own here. Run this work in a \
                     sandboxed chat, or in one a person answers."
                )
            }
        }
    }
}

/// **Whether a sealed grant keeps out of reach of a chat with no sandbox here** (ticket
/// T1.1-73): on macOS, in the app, where the Keychain gives the seal's key to the app's binary
/// alone. Not on Linux or Windows, where any of the person's programs can read the keyring, and
/// not in a process that cannot verify a seal.
pub fn sealed_here() -> bool {
    cfg!(target_os = "macos") && crate::secrets::sealed_set::decides()
}

/// The grant an unattended chat lacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    /// The asking chat's persona; none for a chat on no persona.
    pub asking: Option<String>,
    /// The persona it asked for.
    pub target: String,
    /// Whether the project's committed file names the pair and it still does not count on this
    /// machine, because nobody here has reviewed the project's grants (D-1437-R1).
    pub unreviewed: bool,
}

impl Missing {
    /// The sentence the asking chat reads: the pair that is missing, why nothing was asked, and
    /// where a person grants it.
    pub fn say(&self) -> String {
        let target = crate::shown::short(&self.target);
        let Some(asking) = self.asking.as_deref().map(crate::shown::short) else {
            return format!(
                "this chat runs with its harness's permission prompts off and as no persona. \
                 Only a grant made for one chat covers a chat on no persona, and that never \
                 counts for a chat nobody is answering, so nothing lets it dispatch to \
                 {target}. Run this work as a persona a person has allowed to dispatch to \
                 {target}."
            );
        };
        if self.unreviewed {
            return format!(
                "this chat runs with its harness's permission prompts off, so nobody is here \
                 to answer for a dispatch. The project's file lets {asking} chats dispatch to \
                 {target}, and nobody has reviewed the project's dispatch grants on this \
                 machine yet, so that does not count here. A person reviews them in purlis on \
                 this machine, under {SETTINGS}; then dispatch again."
            );
        }
        // A person grants it in Settings (#1465): no dispatch from a chat they are at comes
        // first, so the sentence names the one place.
        format!(
            "this chat runs with its harness's permission prompts off, so nobody is here to \
             answer for a dispatch, and no grant lets {asking} chats dispatch to {target}: \
             none for the person on this machine, and none for this project. A grant made \
             for one chat does not count here. Only a person makes one: they grant it under \
             {SETTINGS}, for themselves on this machine or for everyone in this project. \
             Until then, do this work without {target}, or say in what you leave behind \
             that it is waiting."
        )
    }
}

/// Where the person sees and takes back dispatch grants.
pub const SETTINGS: &str = "Settings › Project › Dispatch";

/// **Whether an unattended chat running as `asking` may dispatch to `target`** under `grants`
/// and `policy`: [`crate::dispatchgrant::covers`], asked without the grants made for one chat,
/// and with "ask the person" answered as a refusal. `sandboxed` is whether the app started
/// the chat inside a sandbox: one it did not dispatches to its own persona and no other.
///
/// `named_by_the_project` is [`answer_of`]'s: whether the project's committed file names the
/// pair, **as the file says it** (#1586). What is in force here cannot tell: a pair the file
/// names that nobody on this machine has reviewed is not in force (D-1437-R1), so the caller
/// asks the file, as the app does.
pub fn covers(
    asking: Option<&str>,
    target: &str,
    grants: &InForce,
    policy: &Locks,
    named_by_the_project: bool,
    sandboxed: bool,
) -> Answer {
    let mut standing = grants.clone();
    standing.chat.clear();
    answer_on(
        dispatchgrant::covers(asking, target, &standing, policy),
        asking,
        target,
        named_by_the_project,
        Reach {
            sandboxed,
            sealed_here: sealed_here(),
        },
    )
}

/// What keeps a grant from an unattended chat: its sandbox, and the seal where the seal keeps
/// out of its reach ([`sealed_here`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reach {
    /// The app started the chat inside a sandbox.
    pub sandboxed: bool,
    /// A sealed grant is out of the chat's reach on this system ([`sealed_here`]).
    pub sealed_here: bool,
}

/// What `answer`, the grant's own for the pair with no chat's grants counted, means for an
/// unattended chat. `named_by_the_project` says whether the project's committed file names the
/// pair, which words the refusal of one this machine has not reviewed. `sandboxed` as for
/// [`covers`]: with no sandbox, only what the grant covers with no grant at all, the chat's
/// own persona, is covered.
pub fn answer_of(
    answer: Covers,
    asking: Option<&str>,
    target: &str,
    named_by_the_project: bool,
    sandboxed: bool,
) -> Answer {
    answer_on(
        answer,
        asking,
        target,
        named_by_the_project,
        Reach {
            sandboxed,
            sealed_here: sealed_here(),
        },
    )
}

/// [`answer_of`], with what keeps a grant from the chat said: a covered pair of two personas
/// stands for a chat with no sandbox only where a sealed grant is out of its reach; an ask with
/// no sealed grant is [`Refusal::Unsandboxed`], kept for the needs-you list.
pub fn answer_on(
    answer: Covers,
    asking: Option<&str>,
    target: &str,
    named_by_the_project: bool,
    reach: Reach,
) -> Answer {
    let sandboxed = reach.sandboxed;
    match answer {
        Covers::Locked(why) => Answer::Locked(why),
        // The person's never, whatever stands beside it: said as it is said to any chat.
        Covers::Never => Answer::Refused(Refusal::Never(
            asking.unwrap_or_default().to_owned(),
            target.to_owned(),
        )),
        Covers::NeverAbove(above) => Answer::Refused(Refusal::NeverAbove(above, target.to_owned())),
        // A chat's own persona never reads as this; for any other, nobody is here to ask.
        Covers::Unread => Answer::Refused(Refusal::NeversUnread),
        Covers::Denied(_) => Answer::Refused(Refusal::Denied(target.to_owned())),
        Covers::Covered if asking == Some(target) => Answer::Covered,
        Covers::Covered if !sandboxed && !reach.sealed_here => {
            Answer::Refused(Refusal::NotSealedHere(target.to_owned()))
        }
        Covers::NeedsGrant if !sandboxed => Answer::Refused(Refusal::Unsandboxed(Missing {
            asking: asking.map(str::to_owned),
            target: target.to_owned(),
            unreviewed: asking.is_some() && named_by_the_project,
        })),
        Covers::Covered => Answer::Covered,
        Covers::NeedsGrant => Answer::Refused(Refusal::Missing(Missing {
            asking: asking.map(str::to_owned),
            target: target.to_owned(),
            unreviewed: asking.is_some() && named_by_the_project,
        })),
    }
}

/// **`answer` as the project's `[policy]` runs the `dispatch` family** (Q308, instruction 50;
/// ticket T1.1-73): under `off` or `log`, a pair nothing covers, one the policy denies, and one
/// refused only because the asking chat has no sandbox ([`Refusal::Unsandboxed`],
/// [`Refusal::NotSealedHere`]), is covered. A lock, the person's nevers, a never above and a
/// nevers record that does not read stand in every mode.
pub fn as_the_policy_runs(answer: Answer, mode: crate::enforcement::Mode) -> Answer {
    match answer {
        Answer::Refused(
            Refusal::Unsandboxed(_)
            | Refusal::Missing(_)
            | Refusal::Denied(_)
            | Refusal::NotSealedHere(_),
        ) if mode != crate::enforcement::Mode::Enforce => Answer::Covered,
        other => other,
    }
}

/// [`as_the_policy_runs`], and under `log` one quiet line in chat `chat`'s enforcement log
/// naming the rule that would have refused (item 8 of the Q308 follow-up). `chat` is the asking
/// chat's id, as the app holds it.
pub fn as_the_policy_runs_for(
    root: &std::path::Path,
    chat: Option<&str>,
    answer: Answer,
    mode: crate::enforcement::Mode,
) -> Answer {
    let rule = match &answer {
        Answer::Refused(Refusal::Denied(_)) => dispatchgrant::DENY_RULE,
        Answer::Refused(
            Refusal::Unsandboxed(_) | Refusal::Missing(_) | Refusal::NotSealedHere(_),
        ) => dispatchgrant::ASK_RULE,
        _ => return answer,
    };
    crate::enforcement::judged(
        root,
        chat,
        crate::enforcement::Family::Dispatch,
        mode,
        rule,
        None,
        answer,
    )
    .unwrap_or(Answer::Covered)
}

/// The words of a harness's command line that start it with its permission prompts off, as a
/// profile's command would carry them: Claude Code's and Codex's.
const BYPASS_FLAGS: [&str; 5] = [
    "--dangerously-skip-permissions",
    "--dangerously-bypass-approvals-and-sandbox",
    "--yolo",
    // Claude Code's flag that narrows hooks to the managed ones, as `disableAllHooks` does
    // ([`crate::footerclaim`]): purlis's guards are not managed.
    "--safe-mode",
    // Codex's low-friction mode: it runs what its own sandbox allows without asking first.
    "--full-auto",
];

/// The flags that name how Codex asks, and the value of each that means "ask nobody": its
/// approval policy by both its spellings and as a configuration override. Claude Code's
/// permission mode is the attendance fact's to read ([`crate::attendance::ClaudeCodeMode`]).
const MODE_FLAGS: [(&str, &str); 4] = [
    ("--ask-for-approval", "never"),
    ("-a", "never"),
    // Codex's approval policy again, set as a configuration override (#1509).
    ("--config", "approval_policy=never"),
    ("-c", "approval_policy=never"),
];

/// The flag Claude Code starts a chat in a permission mode by.
const CLAUDE_CODE_MODE_FLAG: &str = "--permission-mode";

/// The flag Claude Code reads settings by: a file's path, or the settings inline as JSON.
const CLAUDE_CODE_SETTINGS_FLAG: &str = "--settings";

/// Whether `--settings`' `value` is the settings inline, not a file's path: it opens with `{`
/// once the whitespace Claude Code ignores is passed.
fn is_inline(value: &str) -> bool {
    value
        .trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
        .starts_with('{')
}

/// Whether a `permissions.allow` entry allows every Bash command unasked: `Bash` alone, or
/// `Bash(*)`, `Bash(:*)` and the like, where the glob inside is nothing but wildcards. Read
/// without regard to case or whitespace (`bash`, `Bash (*)`, a tab inside the parentheses): a
/// spelling Claude Code reads as the same rule must not slip past, so this reads more as broad,
/// never less.
fn is_broad_bash_allow(rule: &str) -> bool {
    let rule: String = rule
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    match rule.strip_prefix("bash") {
        Some("") => true,
        Some(rest) => rest
            .strip_prefix('(')
            .and_then(|rest| rest.strip_suffix(')'))
            .is_some_and(|glob| glob.chars().all(|c| matches!(c, '*' | ':'))),
        None => false,
    }
}

/// Whether an inline `env` key moves where purlis's guards or their state are read from: the
/// home and config folder Claude Code reads its settings from, and any variable of purlis's own.
fn is_guard_env(name: &str) -> bool {
    let name = name.trim().to_ascii_uppercase();
    name == "HOME"
        || name == "CLAUDE_CONFIG_DIR"
        || crate::names::ENV_PREFIX
            .spellings()
            .any(|prefix| name.starts_with(prefix))
}

/// The inline settings that switch the permission prompts off, as [`settings_without_bypass`]
/// names them; every other key it takes out puts a guard at risk.
const PROMPT_KEYS: [&str; 3] = [
    "permissions.defaultMode",
    "permissions.allow of Bash",
    "sandbox.autoAllowBashIfSandboxed",
];

/// What inline settings carry that [`settings_without_bypass`] takes out.
type Removed = Vec<&'static str>;

/// Inline Claude Code settings `value` without what turns the chat's prompts or guards off, and
/// what was taken out, or `None` where they carry none of it (#7165): a `permissions.defaultMode`
/// that switches the prompts off, a broad `permissions.allow` of Bash (`"Bash"`), a `hooks` key
/// (a hook of the profile's own may answer allow ahead of purlis's guard) and `disableAllHooks`
/// (which would silence purlis's guards), whatever the value of the last two. **Inline settings
/// this cannot read are `Err`** (#7155): Claude Code accepts JSON `serde_json` rejects (comments,
/// trailing commas, a byte-order mark), so none of these can be shown absent, and it is taken
/// out whole.
fn settings_without_bypass(value: &str) -> Option<Result<(String, Removed), ()>> {
    let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(value) else {
        return is_inline(value).then_some(Err(()));
    };
    let mut removed = Removed::new();
    if let Some(permissions) = settings
        .get_mut("permissions")
        .and_then(serde_json::Value::as_object_mut)
    {
        let off = permissions
            .get("defaultMode")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|mode| ClaudeCodeMode::of(mode) == ClaudeCodeMode::BypassPermissions);
        if off {
            permissions.remove("defaultMode");
            removed.push("permissions.defaultMode");
        }
        if let Some(allow) = permissions
            .get_mut("allow")
            .and_then(serde_json::Value::as_array_mut)
        {
            let before = allow.len();
            allow.retain(|rule| !rule.as_str().is_some_and(is_broad_bash_allow));
            if allow.len() != before {
                removed.push("permissions.allow of Bash");
            }
        }
    }
    if let Some(auto) = settings
        .get_mut("sandbox")
        .and_then(serde_json::Value::as_object_mut)
        && auto
            .get(AUTO_ALLOW_KEY)
            .is_some_and(|on| on != &serde_json::Value::Bool(false))
    {
        auto.remove(AUTO_ALLOW_KEY);
        removed.push("sandbox.autoAllowBashIfSandboxed");
    }
    if let Some(object) = settings.as_object_mut() {
        for key in GUARD_KEYS {
            if object.remove(key).is_some() {
                removed.push(key);
            }
        }
        if let Some(env) = object
            .get_mut("env")
            .and_then(serde_json::Value::as_object_mut)
        {
            let before = env.len();
            env.retain(|name, _| !is_guard_env(name));
            if env.len() != before {
                removed.push("env that moves its guards");
            }
        }
    }
    (!removed.is_empty()).then(|| Ok((settings.to_string(), removed)))
}

/// Top-level keys of inline settings that put a hook of the profile's own ahead of, or silence,
/// purlis's guards, whatever their value: `hooks`, `disableAllHooks` and `allowManagedHooksOnly`
/// ([`crate::footerclaim`]).
const GUARD_KEYS: [&str; 3] = ["hooks", "disableAllHooks", "allowManagedHooksOnly"];

/// The sandbox key that lets Bash run unasked wherever the sandbox holds it.
const AUTO_ALLOW_KEY: &str = "autoAllowBashIfSandboxed";

/// What a profile's words do, for the sentence that refuses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum What {
    /// They switch the permission prompts off.
    PromptsOff,
    /// They can silence or outrank purlis's guards, as well as, perhaps, the prompts.
    GuardsOff,
    /// Inline settings purlis cannot read, so it cannot tell.
    Unreadable,
}

impl What {
    /// What follows "starts its harness with" in a sentence that refuses them. `PromptsOff` is
    /// the sentence it always was.
    pub fn clause(self) -> &'static str {
        match self {
            Self::PromptsOff => "the permission prompts off",
            Self::GuardsOff => {
                "settings that can switch the permission prompts or purlis's guards off"
            }
            Self::Unreadable => {
                "inline settings purlis cannot read, so it cannot tell whether they switch the \
                 permission prompts or its guards off"
            }
        }
    }
}

/// What [`bypass_in`] found: the words as shown to a person, and what they do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// As shown to a person.
    pub flag: String,
    /// What they do.
    pub what: What,
}

/// **The words of `command` that start a harness with its permission prompts off**, if any
/// do, as they are shown to a person: the flag, with its value where the value is what
/// switches them off (`-c approval_policy=never`). `command` is a profile's own, as this
/// machine declares it.
///
/// A recognition of the flags purlis knows, never a reading of what a wrapper script does with
/// its words: a profile whose command is a script that adds one is not seen, and neither is a
/// harness setting kept in a file. A profile is declared on this machine and approved by the
/// person before it runs. **A chat's dispatch may name one of them** (`--profile`), and a
/// persona's definition may, so this is asked of whichever profile a dispatch would start on,
/// whoever named it ([`bypass_refusal`]).
///
/// Each word is read as a command line reads it ([`crate::sandbox::codex::split`]): a value
/// may be attached to its flag (`--ask-for-approval=never`, `-anever`,
/// `-capproval_policy=never`) or be the next word.
pub fn bypass_in(command: &[String]) -> Option<String> {
    bypass_found(command).map(|found| found.flag)
}

/// [`bypass_in`], with what the words do ([`What`]).
pub fn bypass_found(command: &[String]) -> Option<Found> {
    bypass_found_beside(command, &[])
}

/// [`bypass_found`] on a line that holds settings purlis wrote itself: `own` are those words,
/// exactly as armed ([`crate::sandbox::Words::armed`]), and a `--settings` whose value is one
/// of them is purlis's own hooks and guards, never read as a profile's. **Only an exact word
/// is exempt**, so a profile's settings that merely look like purlis's are read as any other.
pub fn bypass_found_beside(command: &[String], own: &[String]) -> Option<Found> {
    (0..command.len()).find_map(|at| {
        bypass_at(command, at, own).map(|bypass| Found {
            flag: bypass.shown,
            what: bypass.what,
        })
    })
}

/// **`command` with the words that switch the prompts off taken out** ([`bypass_in`]): what a
/// chat another chat asked for is started on where `[policy] mode` let such a profile through
/// (Q308b), so it still runs at its task's level, capped at auto (#3787, #7104). A flag's value
/// goes with it where it is the next word; inline `--settings` keep every key but the
/// permission mode.
pub fn without_bypass(command: &[String]) -> Vec<String> {
    without_bypass_beside(command, &[])
}

/// [`without_bypass`] on a line that holds settings purlis wrote itself ([`bypass_found_beside`]):
/// those words are kept whole, hooks and all.
pub fn without_bypass_beside(command: &[String], own: &[String]) -> Vec<String> {
    let mut kept = Vec::with_capacity(command.len());
    let mut at = 0;
    while at < command.len() {
        match bypass_at(command, at, own) {
            Some(bypass) => {
                kept.extend(bypass.kept);
                at += bypass.words;
            }
            None => {
                kept.push(command[at].clone());
                at += 1;
            }
        }
    }
    kept
}

/// Words of a command line that switch the prompts off ([`bypass_at`]).
struct Bypass {
    /// As shown to a person.
    shown: String,
    /// How many words they are: one, or two where the value is the next word.
    words: usize,
    /// What stands in their place once the prompts are back on: nothing, or the same inline
    /// `--settings` without the permission mode.
    kept: Vec<String>,
    /// What they do.
    what: What,
}

/// The words at `at` of `command` that switch the prompts off, if they do.
fn bypass_at(command: &[String], at: usize, own: &[String]) -> Option<Bypass> {
    use crate::sandbox::codex::{is_flag, split};
    let word = command.get(at)?;
    if BYPASS_FLAGS.contains(&word.as_str()) {
        return Some(Bypass {
            shown: word.clone(),
            words: 1,
            kept: Vec::new(),
            what: What::PromptsOff,
        });
    }
    if !is_flag(word) {
        return None;
    }
    let (flag, attached) = split(word);
    let value = attached.or_else(|| command.get(at + 1).map(String::as_str))?;
    let words = if attached.is_some() { 1 } else { 2 };
    // **Claude Code's inline settings may carry the permission mode** (the re-review of #7104):
    // `permissions.defaultMode`, and what else would silence a guard ([`settings_without_bypass`]).
    // A settings file's path is not read here.
    if flag == CLAUDE_CODE_SETTINGS_FLAG {
        // Purlis's own armed settings carry its hooks: they are not a profile's to be read.
        if own.iter().any(|word| word == value) {
            return None;
        }
        return Some(match settings_without_bypass(value)? {
            Ok((rewritten, removed)) => Bypass {
                // The mode's own word is the attendance fact's alone to spell.
                shown: format!("{flag} (its {})", removed.join(", its ")),
                words,
                what: if removed.iter().all(|key| PROMPT_KEYS.contains(key)) {
                    What::PromptsOff
                } else {
                    What::GuardsOff
                },
                kept: match attached {
                    Some(_) => vec![format!("{flag}={rewritten}")],
                    None => vec![flag.to_owned(), rewritten],
                },
            },
            // Refused whole: nothing of it is kept, so no mode in it runs.
            Err(()) => Bypass {
                // Not a claim that they switch anything off: only that none can be ruled out.
                shown: format!("{flag} (unreadable)"),
                words,
                kept: Vec::new(),
                what: What::Unreadable,
            },
        });
    }
    // A value may be written quoted and spaced (`approval_policy = "never"`).
    let plain = value.replace(['"', '\'', ' '], "");
    let off = if flag == CLAUDE_CODE_MODE_FLAG {
        ClaudeCodeMode::of(&plain) == ClaudeCodeMode::BypassPermissions
    } else {
        MODE_FLAGS
            .iter()
            .any(|(known, never)| flag == *known && plain == *never)
    };
    off.then(|| Bypass {
        shown: match attached {
            Some(_) => word.clone(),
            None => format!("{flag} {value}"),
        },
        words,
        kept: Vec::new(),
        what: What::PromptsOff,
    })
}

/// **What an unattended chat is told where its handoff would make a workspace** (D-1444-13):
/// nobody is there to see one made, so a chat nobody is at hands off into a workspace that
/// exists, and `--create` is refused. A chat a person is at is not held to this.
pub const NO_WORKSPACE_IS_MADE: &str = "this chat runs with its harness's permission prompts \
     off, so nobody is here to see a workspace made: a handoff from it goes into a workspace \
     that exists, and --create is refused. Hand off into an existing workspace, or make the \
     workspace from a chat someone is at.";

/// Who named the profile a dispatch would start its chat on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedBy<'a> {
    /// The asking chat, in its dispatch (`--profile`).
    TheDispatch,
    /// This persona's own definition.
    ThePersona(&'a str),
    /// Nobody: it is the profile the asking chat itself runs on.
    TheAskingChat,
}

/// **Why no chat is started for another chat on `profile`**, where its own `command` switches
/// the harness's permission prompts off, and `None` where it does not.
///
/// **Whoever named it.** A chat one chat starts for another never runs asking nobody: not on
/// the asking chat's own profile, not on one its dispatch names, and not on one the persona's
/// definition names, which a chat can write. A handoff is held to it as a task is.
pub fn bypass_refusal(profile: &str, command: &[String], by: NamedBy<'_>) -> Option<String> {
    let found = bypass_found(command)?;
    let flag = crate::shown::short(&found.flag);
    let clause = found.what.clause();
    let profile = crate::shown::short(profile);
    Some(match by {
        NamedBy::TheAskingChat => format!(
            "profile '{profile}' starts its harness with {clause} ({flag}), \
             and a task never takes that from the chat that dispatched it. Dispatch \
             from a chat on a profile that asks."
        ),
        NamedBy::TheDispatch => format!(
            "the dispatch names profile '{profile}', which starts its harness with \
             {clause} ({flag}), and purlis starts no chat for another chat on \
             such a profile. Name a profile that asks, or none."
        ),
        NamedBy::ThePersona(persona) => format!(
            "persona '{}' names profile '{profile}', which starts its harness with \
             {clause} ({flag}), and purlis starts no chat for another chat on \
             such a profile. Give the persona a profile that asks, from its view, or name one \
             with --profile.",
            crate::shown::short(persona)
        ),
    })
}

/// **[`bypass_refusal`], as `[policy] mode` runs for the project at `root`** (Q308b): under
/// `off` no chat is refused for its profile switching the prompts off, and under `log` it starts
/// and one line for chat `chat` says it would have been refused.
pub fn bypass_refusal_in(
    root: &std::path::Path,
    chat: Option<&str>,
    profile: &str,
    command: &[String],
    by: NamedBy<'_>,
) -> Option<String> {
    let refused = bypass_refusal(profile, command, by)?;
    crate::enforcement::judged(
        root,
        chat,
        crate::enforcement::Family::HarnessMode,
        crate::enforcement::mode(root, crate::enforcement::Family::HarnessMode),
        crate::enforcement::RULE_ASKS_NOBODY,
        None,
        refused,
    )
}

/// [`start_of_a_persona_chat`], with its refusal of a profile that switches the prompts off
/// asked as `[policy] mode` runs for the project at `root` ([`bypass_refusal_in`]).
pub fn start_of_a_persona_chat_in(
    root: &std::path::Path,
    chat: Option<&str>,
    start: Start,
    target: &str,
    on: Option<Inherited<'_>>,
) -> Result<Start, String> {
    if let Some(Inherited { profile, command }) = on
        && let Some(refused) =
            bypass_refusal_in(root, chat, profile, command, NamedBy::TheAskingChat)
    {
        return Err(refused);
    }
    start_of_a_persona_chat(start, target, None)
}

/// The profile a persona chat starts on, by its name and its command as this machine declares
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inherited<'a> {
    /// The profile's name.
    pub profile: &'a str,
    /// Its command, as this machine declares it.
    pub command: &'a [String],
}

/// **The start of the persona chat a dispatch to `target` opens**, from `start`, the one built
/// from the asking chat's record, or why it is not started.
///
/// Whatever `start` held of the asking chat's own is dropped here, whoever built it: the new
/// chat runs as `target`, holds `target`'s own grants
/// ([`crate::dispatchgrant::grants_for_a_dispatched_chat`]), and has no grant of one chat, no
/// opt-out and no conversation or session record to resume. `on` is the profile it starts on:
/// where that profile's own command switches the prompts off, the chat is not started on it
/// ([`bypass_refusal`], in the words for a profile taken from the asking chat; a caller that
/// knows who named it asks [`bypass_refusal`] first and says so in those words).
pub fn start_of_a_persona_chat(
    start: Start,
    target: &str,
    on: Option<Inherited<'_>>,
) -> Result<Start, String> {
    if let Some(Inherited { profile, command }) = on
        && let Some(refused) = bypass_refusal(profile, command, NamedBy::TheAskingChat)
    {
        return Err(refused);
    }
    let with = dispatchgrant::grants_for_a_dispatched_chat(target);
    // Who asked, where the start says so; anything else of the asking chat's own start is
    // dropped, and a start that names no asker never takes the person's `off` (#2071).
    let made = match &start.made {
        asked @ crate::sandbox::Made::Asked { .. } => asked.clone(),
        _ => crate::sandbox::Made::Unnamed,
    };
    Ok(Start {
        made,
        persona: Some(with.persona),
        held: with.held,
        grants: with.grants,
        without_sandbox: with.without_sandbox,
        resume: None,
        resuming: None,
        ..start
    })
}

#[cfg(test)]
#[path = "dispatchunattended_tests.rs"]
mod tests;
