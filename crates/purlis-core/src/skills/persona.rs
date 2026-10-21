// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A persona's skills, as its page lists and edits them** (SK-5, #2768): its own skills, in
//! `personas/<p>/skills/` and `skills.local/`, and the skills its `skills:` list names, each with
//! the level it comes from or why its chats go without it. Adding and removing a name edits that
//! one line of the persona's own definition. Making, editing and deleting a persona's own skill
//! is the Skill studio's (#2771).

use std::path::Path;

use super::{Origin, Reason, Reference, level_shown, names_of};

/// The key a persona's definition names its skills by.
pub const KEY: &str = "skills";

/// One skill a persona's page lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// What its row says: the skill's name, or what the `skills:` list writes for one the
    /// project has no folder of.
    pub name: String,
    /// The entry of the `skills:` list that names it, or `None` for one of the persona's own
    /// that its list does not name. Only an entry can be removed from the page.
    pub named_as: Option<String>,
    /// Where it comes from, as a person reads it: `persona`, `project`, `built-in`,
    /// `installed`, with `local` before a local one's.
    pub level: String,
    /// Why its chats at the project's root go without it, in one line, where they do.
    pub left_out: Option<String>,
}

/// **The skills of `persona`'s page**, in the plane at `plane`, with the built-in skills in
/// `built_in`: its own skills by name, then each skill its `skills:` list names, in the list's
/// order. Read as a chat at the project's root, where every project skill reaches, and with no
/// harness looked at: a harness skill is the harness's to have, and a chat's tab says when it
/// does not.
pub fn listed(plane: &Path, persona: &str, built_in: Option<&Path>) -> Vec<Listed> {
    let set = super::in_the_studio(built_in, plane, None, Some(persona));
    let names = names_of(plane, persona);
    let named = |name: &str| {
        names
            .iter()
            .find(|one| matches!(one, Reference::Named(n) if n == name))
            .map(ToString::to_string)
    };
    let mut out: Vec<Listed> = set
        .effective
        .iter()
        .filter(|e| e.origin == Origin::Persona)
        .map(|e| Listed {
            name: e.skill.name.clone(),
            named_as: named(&e.skill.name),
            level: level_shown(e.origin, e.local),
            left_out: None,
        })
        .chain(
            set.left_out
                .iter()
                .filter(|l| l.origin == Origin::Persona)
                .map(|l| Listed {
                    name: l.name.clone(),
                    named_as: named(&l.name),
                    level: level_shown(l.origin, l.local),
                    left_out: Some(l.reason.to_string()),
                }),
        )
        .collect();
    for reference in &names {
        let written = reference.to_string();
        if out
            .iter()
            .any(|one| one.named_as.as_deref() == Some(written.as_str()))
        {
            continue;
        }
        let built_in = matches!(reference, Reference::BuiltIn(_));
        let gets = set.effective.iter().find(|e| match reference {
            Reference::BuiltIn(name) => e.origin == Origin::BuiltIn && e.skill.name == *name,
            Reference::Named(name) => e.skill.name == *name,
            Reference::Harness { .. } => false,
        });
        let hidden = || {
            set.left_out
                .iter()
                .find(|l| l.name == written || (!built_in && l.name == reference.name()))
        };
        let (level, left_out) = match gets {
            Some(e) => (level_shown(e.origin, e.local), None),
            None => match hidden() {
                Some(l) => (level_shown(l.origin, l.local), Some(l.reason.to_string())),
                None => (level_shown(Origin::Harness, false), None),
            },
        };
        out.push(Listed {
            name: written.clone(),
            named_as: Some(written),
            level,
            left_out,
        });
    }
    out
}

/// **The names of `persona`'s own skills**, shared and local, in the plane at `plane`: what its
/// chats load beside what its `skills:` list names. Read without asking git, so a local skill git
/// would commit is counted too.
pub fn own(plane: &Path, persona: &str) -> Vec<String> {
    [false, true]
        .into_iter()
        .filter_map(|local| super::persona_dir(plane, persona, local))
        .flat_map(|dir| super::read(&dir))
        .map(|skill| skill.name)
        .collect()
}

/// **Adds `written` to `persona`'s `skills:` list** (SK-5), in its own definition in the plane
/// at `plane`, as the last entry. `Ok(false)` where the list already names that skill (a built-in
/// one under either prefix), and nothing is written. Refused for an entry that names no skill,
/// and for a persona purlis cannot write.
pub fn add(plane: &Path, persona: &str, written: &str) -> Result<bool, String> {
    let reference = Reference::parse(written).ok_or_else(|| {
        format!("{written:?} names no skill: write a skill's name, as the Skill studio lists it")
    })?;
    let mut entries = crate::personaverbs::declared_skills(plane, persona);
    if entries
        .iter()
        .any(|one| Reference::parse(one).as_ref() == Some(&reference))
    {
        return Ok(false);
    }
    entries.push(reference.to_string());
    write(plane, persona, &entries)?;
    Ok(true)
}

/// **Takes `written` out of `persona`'s `skills:` list** (SK-5): every entry that names the same
/// skill. The line goes where nothing is left on it. `Ok(false)` where the list does not name it.
/// A persona's own skill folder is never touched.
pub fn remove(plane: &Path, persona: &str, written: &str) -> Result<bool, String> {
    let reference = Reference::parse(written);
    let mut entries = crate::personaverbs::declared_skills(plane, persona);
    let before = entries.len();
    entries.retain(|one| Reference::parse(one) != reference);
    if entries.len() == before {
        return Ok(false);
    }
    write(plane, persona, &entries)?;
    Ok(true)
}

/// `persona`'s own definition with its `skills:` line saying `entries`, or without the line for
/// none.
fn write(plane: &Path, persona: &str, entries: &[String]) -> Result<(), String> {
    if let Some(refused) = crate::personas::name_refusal(plane, persona) {
        return Err(refused);
    }
    crate::settings::writable(plane)?;
    let file = crate::personas::def_path(plane, persona);
    crate::contain::writable(plane, &file).map_err(|refused| refused.to_string())?;
    let text = crate::contain::read_text_no_link(plane, &file)
        .map_err(|why| format!("purlis could not read {}: {why}", file.display()))?;
    let line = entries.join(", ");
    let Some(next) = crate::personas::with_key(&text, KEY, (!line.is_empty()).then_some(&line))
    else {
        return Err(format!(
            "{} has no frontmatter to hold a `{KEY}:` line, so nothing was written.",
            file.display()
        ));
    };
    if next == text {
        return Ok(());
    }
    crate::layer::write_whole(&file, &next)
}

/// Whether `reason` is one a chat's tab says (the chat starts without the skill), rather than one
/// the studio alone shows.
pub fn said_on_the_tab(reason: &Reason) -> bool {
    matches!(reason, Reason::NotOnThisMachine | Reason::NoSuchBuiltIn)
}

#[cfg(test)]
mod tests;
