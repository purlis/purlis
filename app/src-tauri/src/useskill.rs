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
    /// Whether it is manual-only: run only when asked for by name, and free in the budget.
    pub manual: bool,
    /// The arguments it takes, by name, in order: one box each in the dialog.
    pub arguments: Vec<String>,
    /// What to pass it, in one line, where it says: the one box of a skill that names none.
    pub hint: Option<String>,
}

/// Which part of a chat's skill budget its set is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum BudgetOver {
    Skills,
    Chars,
    Both,
}

/// A chat's skill budget on its harness, and what its set spends of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillBudget {
    /// Skills its listing holds: every one but the manual-only ones.
    pub skills: u32,
    /// Characters of their descriptions.
    pub chars: u32,
    /// Manual-only skills, which cost nothing.
    pub manual: u32,
    /// The most skills the budget lists.
    pub most_skills: u32,
    /// The most description characters it holds.
    pub most_chars: u32,
    /// What the chat warns of: the part it is over, or null within it.
    pub over: Option<BudgetOver>,
}

/// What Use skill… offers a chat, and its budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ChatSkills {
    /// Whether the Skill studio is on: nothing is offered while it is off.
    pub on: bool,
    /// The chat's harness, as a person calls it, or null for a chat on none purlis knows: such a
    /// chat is offered nothing.
    pub harness: Option<String>,
    /// Every skill of its set, by name.
    pub skills: Vec<ChatSkill>,
    /// Its budget, where it has a harness.
    pub budget: Option<SkillBudget>,
}

/// What the clipboard is said to hold where the skill could not be typed.
const COPIED: crate::references::Copied = crate::references::Copied {
    on: "the skill is",
    it: "it is",
};

/// A count the window draws: a `u32`, as large as it gets.
fn small(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The skills of chat `session` of `held`, as the core works them out, or `None` for a chat that
/// is not open or runs no harness purlis knows.
fn of_the_chat(
    held: &Held,
    session: u32,
) -> Option<(
    purlis_core::harness::Harness,
    purlis_core::skills::ChatSkills,
)> {
    let chat = held.chats().skills_of(session)?;
    let harness = chat.harness?;
    let built_in =
        crate::bundled_plugin_here().and_then(|plugin| purlis_core::skills::in_bundle(&plugin));
    let skills = purlis_core::skills::of_a_chat(&purlis_core::skills::Running {
        plane: held.root(),
        workspace: chat.workspace.as_deref(),
        persona: chat.persona.as_deref(),
        harness,
        built_in: built_in.as_deref(),
        plane_skills: true,
    });
    Some((harness, skills))
}

/// `skills`, for the window.
pub(crate) fn shown(
    harness: Option<purlis_core::harness::Harness>,
    skills: Option<&purlis_core::skills::ChatSkills>,
) -> ChatSkills {
    let Some(skills) = skills else {
        return ChatSkills {
            on: true,
            harness: harness.map(|harness| harness.title().to_owned()),
            skills: Vec::new(),
            budget: None,
        };
    };
    let spent = skills.spent;
    ChatSkills {
        on: true,
        harness: harness.map(|harness| harness.title().to_owned()),
        skills: skills
            .offered
            .iter()
            .map(|one| ChatSkill {
                name: one.effective.skill.name.clone(),
                level: one.effective.origin.into(),
                label: purlis_core::skills::level_shown(one.effective.origin, one.effective.local),
                description: one.effective.skill.description.clone(),
                manual: one.declared.manual,
                arguments: one.declared.arguments.clone(),
                hint: one.declared.hint.clone(),
            })
            .collect(),
        budget: Some(SkillBudget {
            skills: small(spent.skills),
            chars: small(spent.chars),
            manual: small(spent.manual),
            most_skills: small(spent.budget.skills),
            most_chars: small(spent.budget.chars),
            over: spent.over().map(|over| match over {
                purlis_core::skills::Over::Skills => BudgetOver::Skills,
                purlis_core::skills::Over::Chars => BudgetOver::Chars,
                purlis_core::skills::Over::Both => BudgetOver::Both,
            }),
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
            return Ok(ChatSkills {
                on: false,
                harness: None,
                skills: Vec::new(),
                budget: None,
            });
        }
        let harness = held
            .chats()
            .skills_of(session)
            .and_then(|chat| chat.harness);
        let skills = of_the_chat(&held, session).map(|(_, skills)| skills);
        Ok(shown(harness, skills.as_ref()))
    })
    .await
}

/// **Types skill `skill` of chat `session`'s set into it, in its harness's syntax, with `values`
/// for the arguments it declares in their order, and never sends it** — or copies it, with the
/// reason, where it cannot be typed now. A skill that was typed has its run start, by Use
/// skill…, as the chat's next prompt does.
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
    let (harness, skills) = of_the_chat(&held, session)
        .ok_or_else(|| "purlis types a skill only into a chat on a harness it knows".to_owned())?;
    let typing = skills.typed(harness, &skill, &values)?;
    typed_and_held(&held, session, typing, &|text| clipboard.put_text(text))
}

/// `typing` typed into chat `session` of `held` unsent, or copied with the reason, with `copy`
/// as the clipboard; a skill that was typed has its run held for the chat's next prompt.
pub(crate) fn typed_and_held(
    held: &std::sync::Arc<Held>,
    session: u32,
    typing: purlis_core::skills::Typing,
    copy: &dyn Fn(&str) -> Result<(), String>,
) -> Result<Handed, String> {
    let handed = crate::references::typed_or_copied(held, session, typing.text, COPIED, copy)?;
    if matches!(handed, Handed::Typed { .. }) {
        held.hooks().skill_typed(
            held.root(),
            session,
            &typing.known_as,
            &purlis_core::reopen::mint(),
        );
    }
    Ok(handed)
}

#[cfg(test)]
mod tests;
