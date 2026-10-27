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
}

impl ChatSkills {
    /// Nothing offered: the studio is off (`on` false), or the chat runs no tool purlis knows.
    fn none(on: bool) -> ChatSkills {
        ChatSkills {
            on,
            known: false,
            skills: Vec::new(),
            budget: None,
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

/// The skills of chat `session` of `held`, as the core works them out, and the salt its prompts
/// are digested under, or `None` for a chat that is not open or runs no tool purlis knows the
/// skills of. One read of the chat.
fn of_the_chat(
    held: &Held,
    session: u32,
) -> Option<(
    purlis_core::skills::ChatSkills,
    Option<purlis_core::skillspan::PromptSalt>,
)> {
    let chat = held.chats().skills_of(session)?;
    let built_in =
        crate::bundled_plugin_here().and_then(|plugin| purlis_core::skills::in_bundle(&plugin));
    let skills = purlis_core::skills::for_use_skill(&purlis_core::skills::Running {
        plane: held.root(),
        workspace: chat.workspace.as_deref(),
        persona: chat.persona.as_deref(),
        harness: chat.harness?,
        built_in: built_in.as_deref(),
        plane_skills: chat.plane_skills,
    });
    Some((skills, chat.salt))
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
        Ok(shown(
            of_the_chat(&held, session)
                .map(|(skills, _)| skills)
                .as_ref(),
        ))
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
    let (skills, salt) = of_the_chat(&held, session).ok_or_else(|| NO_TOOL.to_owned())?;
    let typing = typing_of(&skills, salt.as_ref(), &skill, &values)?;
    typed_and_held(&held, session, typing, &|text| clipboard.put_text(text))
}

/// What Use skill… types to start `skill` of `skills` with `values`, its text digested under
/// the chat's `salt`; refused for a chat that has no id yet, whose prompts nothing could match.
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
