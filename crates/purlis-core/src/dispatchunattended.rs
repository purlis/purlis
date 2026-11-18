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
    /// The project's policy denies the pair (`[dispatch.denies]`): the target, and why.
    Denied(String, dispatchgrant::Deny),
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
            Self::Denied(target, deny) => dispatchgrant::denied_said(target, *deny),
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
        Covers::Denied(deny) => Answer::Refused(Refusal::Denied(target.to_owned(), deny)),
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
            | Refusal::Denied(..)
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
        Answer::Refused(Refusal::Denied(..)) => dispatchgrant::DENY_RULE,
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
const BYPASS_FLAGS: [&str; 4] = [
    "--dangerously-skip-permissions",
    "--dangerously-bypass-approvals-and-sandbox",
    "--yolo",
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
    use crate::sandbox::codex::{is_flag, split};
    command.iter().enumerate().find_map(|(at, word)| {
        if BYPASS_FLAGS.contains(&word.as_str()) {
            return Some(word.clone());
        }
        if !is_flag(word) {
            return None;
        }
        let (flag, attached) = split(word);
        let next = || command.get(at + 1).map(String::as_str);
        let value = attached.or_else(next)?;
        // A value may be written quoted and spaced (`approval_policy = "never"`).
        let plain = value.replace(['"', '\'', ' '], "");
        let off = if flag == CLAUDE_CODE_MODE_FLAG {
            ClaudeCodeMode::of(&plain) == ClaudeCodeMode::BypassPermissions
        } else {
            MODE_FLAGS
                .iter()
                .any(|(known, never)| flag == *known && plain == *never)
        };
        off.then(|| match attached {
            Some(_) => word.clone(),
            None => format!("{flag} {value}"),
        })
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
    let flag = crate::shown::short(&bypass_in(command)?);
    let profile = crate::shown::short(profile);
    Some(match by {
        NamedBy::TheAskingChat => format!(
            "profile '{profile}' starts its harness with the permission prompts off ({flag}), \
             and a task never takes that from the chat that dispatched it. Dispatch \
             from a chat on a profile that asks."
        ),
        NamedBy::TheDispatch => format!(
            "the dispatch names profile '{profile}', which starts its harness with the \
             permission prompts off ({flag}), and purlis starts no chat for another chat on \
             such a profile. Name a profile that asks, or none."
        ),
        NamedBy::ThePersona(persona) => format!(
            "persona '{}' names profile '{profile}', which starts its harness with the \
             permission prompts off ({flag}), and purlis starts no chat for another chat on \
             such a profile. Give the persona a profile that asks, from its view, or name one \
             with --profile.",
            crate::shown::short(persona)
        ),
    })
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
