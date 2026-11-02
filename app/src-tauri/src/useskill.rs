// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Use skill…** (SK-12, #2775; decisions 19, 40 and 61): a chat's own skills, picked from the
//! palette or its pane's Skills chip, typed into the chat in its harness's syntax and never
//! sent, with the arguments each declares; and the chat's skill budget.
//!
//! **The window names a skill and its arguments' values; the core builds the text.** The chat's
//! set is worked out as its start worked it out (`purlis_core::skills::of_a_chat`), a skill not in
//! it is refused, and the chat's harness adapter renders it. Nothing the window sends is typed
//! as it was sent.
//!
//! **Typed the way a reference is** (`crate::references::typed_or_copied`): one bracketed paste
//! with nothing after it, only while the chat is at its prompt, and copied with the reason where
//! it cannot be typed now (opencode, and a chat mid-turn). A skill that was typed has its run
//! held for the chat's next prompt, started by Use skill… (`crate::hooks::Hooks::skill_typed`).
//!
//! Behind the `skill-studio` flag until its finish line (#2786).

use serde::Serialize;
use specta::Type;

use crate::planes::{Held, PlaneId, Planes};
use crate::references::Handed;
use crate::skillstudio::Level;

/// One skill Use skill… offers a chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ChatSkill {
    /// Its name, which [`use_skill`] is handed back.
    pub name: String,
    pub level: Level,
    /// Its level as a person reads it: `project`, `local workspace`.
    pub label: String,
    /// What it does and when to use it.
    pub description: String,
    /// Whether it is manual-only: run only when asked for by name.
    pub manual: bool,
    /// Whether it asks for anything before it is typed: the dialog's boxes, or none.
    pub takes_arguments: bool,
    /// The arguments it takes, by name, in order: one box each in the dialog.
    pub arguments: Vec<String>,
    /// What to pass it, in one line, where it says: the one box of a skill that names none.
    pub hint: Option<String>,
    /// Why the router hid it from the chat's list, where it did (SK-15): Use skill… then asks
    /// the chat to load it.
    pub hidden: Option<crate::skillstudio::SkillHidden>,
}

/// Which part of a chat's skill budget its set warns of: at four fifths of it or past it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum BudgetWarns {
    Skills,
    Chars,
    Both,
}

impl From<purlis_core::skills::Over> for BudgetWarns {
    fn from(over: purlis_core::skills::Over) -> BudgetWarns {
        match over {
            purlis_core::skills::Over::Skills => BudgetWarns::Skills,
            purlis_core::skills::Over::Chars => BudgetWarns::Chars,
            purlis_core::skills::Over::Both => BudgetWarns::Both,
        }
    }
}

/// **What is still past a chat's budget** once its router hid all it may (SK-15): the core's
/// `StillOver`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum BudgetStillOver {
    /// purlis's own skills and the ones the chat's program and repos bring, which no router hides.
    NotHideable,
    /// Those fit, and the pinned skills on top of them do not.
    Pins,
}

impl From<purlis_core::skills::StillOver> for BudgetStillOver {
    fn from(over: purlis_core::skills::StillOver) -> BudgetStillOver {
        match over {
            purlis_core::skills::StillOver::NotHideable => BudgetStillOver::NotHideable,
            purlis_core::skills::StillOver::Pins => BudgetStillOver::Pins,
        }
    }
}

/// A chat's skill budget on its harness, and what its set spends of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillBudget {
    /// Skills its listing holds.
    pub skills: u32,
    /// Characters of what it lists of them.
    pub chars: u32,
    /// Manual-only skills in the set, which cost nothing: every harness keeps them out of its
    /// listing (SK-8).
    pub manual: u32,
    /// The most skills the budget lists.
    pub most_skills: u32,
    /// The most characters it holds.
    pub most_chars: u32,
    /// What the chat warns of, or null below four fifths of both.
    pub warns: Option<BudgetWarns>,
    /// Skills the router hid behind the index skill, its set being over the budget (SK-15).
    pub hidden: u32,
    /// What is still past the budget with all the router may hide hidden, where something is:
    /// all of it is then listed (ADR 0102 §4).
    pub still_over: Option<BudgetStillOver>,
    /// Which of the chat's workspace and persona pin a skill (SK-15d): where the pins are
    /// undone in the Skill studio when they alone are past the budget.
    pub pinned_by: Vec<crate::skillchange::SkillPinner>,
    /// Where the chat reads none of its pins because its workspace's and its persona's together
    /// pass the most it reads, though each alone is accepted (#3391): both, and how many each
    /// pins. Null where its pins are read.
    pub pins_together: Option<crate::skillchange::SkillPinsTogether>,
}

/// What Use skill… offers a chat, and its budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ChatSkills {
    /// Whether the Skill studio is on: nothing is offered while it is off.
    pub on: bool,
    /// Whether the chat runs a tool purlis knows the skills of: one that does not is offered
    /// nothing.
    pub known: bool,
    /// Every skill of its set, by name.
    pub skills: Vec<ChatSkill>,
    /// Its budget, where it is known.
    pub budget: Option<SkillBudget>,
    /// The clones of its workspace it works in, by name, whose repo skills its harness reads
    /// (#3462): what the Skills view narrows a chat's repo skills to. None for a chat in no clone.
    pub repos: Vec<String>,
    /// Why the chat gets no skills, where its harness is a declared one skills are not supported
    /// on (SK-30): said in place of the words for a tool purlis does not know.
    pub not_supported: Option<String>,
    /// How its router extension did at its start (SK-15b): null for a chat routed by the
    /// built-in router, which is asked nothing.
    pub routing: Option<SkillRouting>,
    /// The skills its persona names that it started without, as its tab's Notice says them
    /// (SK-5), until that is put away: null for a chat that has every one. Held from its start,
    /// so the chip on its pane says every need of the chat's own with no read of its own (#3953).
    pub missing: Option<crate::chats::SkillsMissing>,
}

/// **How a chat's router extension did at its start** (SK-15b, ADR 0102 §6): the core's routing
/// record, as the chat's tab and the studio say it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillRouting {
    /// The router asked, by its extension's id.
    pub router: String,
    /// How long its question took, in milliseconds.
    pub ms: u32,
    /// Whether the chat lists what the built-in router shows instead.
    pub fell_back: bool,
    /// What a person is told of it, in the core's words.
    pub said: String,
    /// How it did on the chat's latest prompt (#4297), where it reads prompts and was asked on
    /// one. Optional in the window's type, as most routers never are.
    #[specta(optional)]
    pub last_prompt: Option<SkillPromptRouting>,
}

/// The event the window is sent as a chat's latest prompt changes whether it fell back (#4445).
/// Not `plane-changed`: this routing is kept in memory, and that event says what moved on disk.
pub const PROMPT_ROUTING: &str = "chat-prompt-routing";

/// What `chat-prompt-routing` carries: which chat to read again, and nothing of how it did, so
/// tellings that land out of order leave the window nothing to get wrong.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ChatPromptRouting {
    pub plane: PlaneId,
    pub session: u32,
}

/// Told each chat whose latest prompt changes whether it fell back.
pub type PromptRoutingTeller = std::sync::Arc<dyn Fn(ChatPromptRouting) + Send + Sync + 'static>;

/// **How a chat's router did on its latest prompt** (#4297, ADR 0102 §6): the core's routing
/// record of it, as the chat's tab and the studio say it. No word of the prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillPromptRouting {
    /// Whether the prompt went on with no pointer because of how it did.
    pub fell_back: bool,
    /// What a person is told of it, in the core's words.
    pub said: String,
}

impl From<&purlis_core::skills::router::Record> for SkillPromptRouting {
    fn from(record: &purlis_core::skills::router::Record) -> SkillPromptRouting {
        SkillPromptRouting {
            fell_back: record.fell_back.is_some(),
            said: record.prompt_said(),
        }
    }
}

impl SkillRouting {
    /// How it did at its chat's start and on the chat's latest prompt.
    pub(crate) fn of(
        routed: &purlis_core::skills::router::Routed,
        last_prompt: Option<&purlis_core::skills::router::Record>,
    ) -> SkillRouting {
        SkillRouting {
            last_prompt: last_prompt.map(Into::into),
            ..routed.into()
        }
    }
}

impl From<&purlis_core::skills::router::Routed> for SkillRouting {
    fn from(routed: &purlis_core::skills::router::Routed) -> SkillRouting {
        SkillRouting {
            router: routed.record.router.clone(),
            ms: u32::try_from(routed.record.ms).unwrap_or(u32::MAX),
            fell_back: routed.record.fell_back.is_some(),
            said: routed.said(),
            last_prompt: None,
        }
    }
}

impl ChatSkills {
    /// Nothing offered: the studio is off (`on` false), or the chat runs no tool purlis knows.
    fn none(on: bool) -> ChatSkills {
        ChatSkills {
            on,
            known: false,
            skills: Vec::new(),
            budget: None,
            repos: Vec::new(),
            not_supported: None,
            routing: None,
            missing: None,
        }
    }
}

/// What the clipboard is said to hold where the skill could not be typed, and the rule it is
/// typed by: at the chat's prompt, on every harness ([`purlis_core::reference::at_its_prompt`]).
const COPIED: crate::references::Copied = crate::references::Copied {
    on: "the skill is",
    it: "it is",
    may: |_, now| purlis_core::reference::at_its_prompt(now),
};

/// Why a skill is typed into no chat that runs no tool purlis knows: the words Use skill… says.
const NO_TOOL: &str = "This chat runs no tool purlis knows, so it has no skills to use here.";

/// Why a skill is typed into no chat whose start has not given it its id yet.
const NOT_STARTED: &str =
    "This chat has not started yet, so nothing was typed. Try again once it has.";

/// A count the window draws: a `u32`, as large as it gets.
pub(crate) fn small(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// **The skills of `chat` of `held`, as the core works them out**, and the salt its prompts are
/// digested under, or `None` for a chat that runs no tool purlis knows the skills of.
fn of_this_chat(
    held: &Held,
    chat: &crate::chats::ChatForSkills,
) -> Option<(
    purlis_core::skills::ChatSkills,
    Option<purlis_core::skillspan::PromptSalt>,
)> {
    let built_in =
        crate::bundled_plugin_here().and_then(|plugin| purlis_core::skills::in_bundle(&plugin));
    let skills = purlis_core::skills::for_use_skill(&purlis_core::skills::Running {
        plane: held.root(),
        workspace: chat.workspace.as_deref(),
        persona: chat.persona.as_deref(),
        harness: chat.harness?,
        built_in: built_in.as_deref(),
        plane_skills: chat.plane_skills,
        start: Some(&chat.start),
    });
    Some((skills, chat.salt.clone()))
}

/// **The clones `chat` of `held` works in**, by name (#3462): none for a chat at the project's
/// root or in no clone.
fn repos_of_the_chat(held: &Held, chat: &crate::chats::ChatForSkills) -> Vec<String> {
    let (Some(workspace), Some(cwd)) = (chat.workspace.as_deref(), chat.cwd.as_deref()) else {
        return Vec::new();
    };
    purlis_core::skills::clones_of_chat(held.root(), workspace, cwd)
        .into_iter()
        .map(|repo| repo.name)
        .collect()
}

/// `skills`, for the window: none for a chat that runs no tool purlis knows.
pub(crate) fn shown(skills: Option<&purlis_core::skills::ChatSkills>) -> ChatSkills {
    let Some(skills) = skills else {
        return ChatSkills::none(true);
    };
    let spent = skills.spent;
    ChatSkills {
        on: true,
        known: true,
        skills: skills
            .offered
            .iter()
            .map(|one| ChatSkill {
                name: one.effective.skill.name.clone(),
                level: one.effective.origin.into(),
                label: purlis_core::skills::level_shown(one.effective.origin, one.effective.local),
                description: one.effective.skill.description.clone(),
                manual: one.declared.manual,
                takes_arguments: one.takes_arguments,
                arguments: one.declared.arguments.clone(),
                hint: one.declared.hint.clone(),
                hidden: one.hidden.as_ref().map(Into::into),
            })
            .collect(),
        budget: Some(SkillBudget {
            skills: small(spent.skills),
            chars: small(spent.chars),
            manual: small(spent.manual),
            most_skills: small(spent.budget.skills),
            most_chars: small(spent.budget.chars),
            warns: spent.warns().map(BudgetWarns::from),
            hidden: small(
                skills
                    .offered
                    .iter()
                    .filter(|one| one.hidden.is_some())
                    .count(),
            ),
            still_over: skills.still_over.map(Into::into),
            pinned_by: skills.pinned_by.iter().cloned().map(Into::into).collect(),
            pins_together: skills.pins_together.as_ref().map(Into::into),
        }),
        repos: Vec::new(),
        not_supported: None,
        routing: None,
        missing: None,
    }
}

/// **The skills chat `session` has, as Use skill… offers them, and its skill budget**: nothing
/// while the Skill studio is off. On a blocking thread, as it reads the project's skill folders.
#[tauri::command]
#[specta::specta]
pub(crate) async fn chat_skills(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<ChatSkills, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the chat's skills", move || {
        if !purlis_core::skills::studio_on_here() {
            return Ok(ChatSkills::none(false));
        }
        let Some(chat) = held.chats().skills_of(session) else {
            return Ok(shown(None));
        };
        let mut shown = shown(
            of_this_chat(&held, &chat)
                .map(|(skills, _)| skills)
                .as_ref(),
        );
        if shown.known {
            shown.repos = repos_of_the_chat(&held, &chat);
            shown.routing = chat
                .start
                .routed
                .as_deref()
                .map(|routed| SkillRouting::of(routed, chat.last_prompt.as_ref()));
            shown.missing = held.chats().skills_missing(session);
        } else {
            shown.not_supported = chat.not_supported.clone();
        }
        Ok(shown)
    })
    .await
}

/// **Types skill `skill` of chat `session`'s set into it, in its harness's syntax, with `values`
/// for the arguments it declares in their order, and never sends it** — or copies it, with the
/// reason, where it cannot be typed now. A skill that was typed has its run start, by Use
/// skill…, once a prompt shows it was sent.
// On the window's thread, as every command that types into a chat is (`INPUT_ORDER`): what is
// typed lands in the order it was sent, and the chat's session is reached as a reference's is.
// The set is read from the skill folders here, as a reference reads its branch. Refused, in the
// core's sentence, for a skill that is not the chat's. Not a doc comment past here, because the
// generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub(crate) fn use_skill(
    planes: tauri::State<'_, Planes>,
    clipboard: tauri::State<'_, crate::vaults::SystemClipboard>,
    plane: PlaneId,
    session: u32,
    skill: String,
    values: Vec<String>,
) -> Result<Handed, String> {
    let held = planes.held(&plane)?;
    if !purlis_core::skills::studio_on_here() {
        return Err("The Skill studio is not turned on on this machine.".to_owned());
    }
    let chat = held
        .chats()
        .skills_of(session)
        .ok_or_else(|| NO_TOOL.to_owned())?;
    let (skills, salt) = of_this_chat(&held, &chat).ok_or_else(|| {
        chat.not_supported
            .clone()
            .unwrap_or_else(|| NO_TOOL.to_owned())
    })?;
    let typing = typing_of(&skills, salt.as_ref(), &skill, &values)?;
    typed_and_held(&held, session, typing, &|text| clipboard.put_text(text))
}

/// What Use skill… types to start `skill` of `skills` with `values`, its text digested under
/// the chat's `salt`; refused for a chat that has no prompt key, whose prompts nothing could
/// match.
fn typing_of(
    skills: &purlis_core::skills::ChatSkills,
    salt: Option<&purlis_core::skillspan::PromptSalt>,
    skill: &str,
    values: &[String],
) -> Result<purlis_core::skills::Typing, String> {
    let salt = salt.ok_or_else(|| NOT_STARTED.to_owned())?;
    skills.typed(skill, values, salt)
}

/// `typing` typed into chat `session` of `held` unsent, or copied with the reason, with `copy`
/// as the clipboard; a skill that was typed has its run held for the prompt it is sent in.
pub(crate) fn typed_and_held(
    held: &std::sync::Arc<Held>,
    session: u32,
    typing: purlis_core::skills::Typing,
    copy: &dyn Fn(&str) -> Result<(), String>,
) -> Result<Handed, String> {
    let handed = crate::references::typed_or_copied(held, session, typing.text, COPIED, copy)?;
    // A skill the router hid is loaded with `skill_load`, whose load opens its run.
    if let (Handed::Typed { .. }, purlis_core::skills::Start::Held(run)) = (&handed, typing.start) {
        held.hooks().skill_typed(held.root(), session, run);
    }
    Ok(handed)
}

#[cfg(test)]
mod tests;
