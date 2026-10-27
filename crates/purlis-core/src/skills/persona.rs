// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A persona's skills** (SK-5, #2768): its own skills, in `personas/<p>/skills/` and
//! `skills.local/`, and the skills its `skills:` list names. Its page counts them; the Skill
//! studio, filtered to the persona, lists them and adds and removes a name, which edits that one
//! line of the persona's own definition. Making, editing and deleting a persona's own skill is
//! the studio's too (#2771).

use std::path::Path;

use super::{Reference, names_of};

/// The key a persona's definition names its skills by.
pub(crate) const KEY: &str = "skills";

/// The key a persona's definition pins skills by (SK-15, ADR 0102 §4): a comma-separated list of
/// names its chats always list.
pub(crate) const PINS_KEY: &str = "pinned-skills";

/// **How many skills `persona` brings into every workspace** (SK-5, #2768), in the plane at
/// `plane`: its own skills, and each other skill its `skills:` list names. What its page's
/// summary row counts; the Skill studio lists them.
pub fn brings(plane: &Path, persona: &str) -> usize {
    let mut names: Vec<String> = own(plane, persona);
    for reference in names_of(plane, persona) {
        let shown = match &reference {
            Reference::Named(name) => name.clone(),
            other => other.to_string(),
        };
        if !names.contains(&shown) {
            names.push(shown);
        }
    }
    names.len()
}

/// What a change to a persona's `skills:` list does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Names one more skill, as the last entry.
    Add,
    /// Takes every entry that names the skill out.
    Remove,
}

/// **The names of `persona`'s own skills**, shared and local, in the plane at `plane`: what its
/// chats load beside what its `skills:` list names. Read without asking git, so a local skill git
/// would commit is counted too.
pub(crate) fn own(plane: &Path, persona: &str) -> Vec<String> {
    [false, true]
        .into_iter()
        .filter_map(|local| super::persona_dir(plane, persona, local))
        .flat_map(|dir| super::read(&dir))
        .map(|skill| skill.name)
        .collect()
}

/// **Changes `persona`'s `skills:` list** (SK-5), in its own definition in the plane at
/// `plane`: [`Change::Add`] names `written` as the last entry, [`Change::Remove`] takes out every
/// entry that names the same skill (a built-in one under either prefix), and the line goes where
/// nothing is left on it. `Ok(false)` where the list already was so, and nothing is written. A
/// persona's own skill folder is never touched. Refused for an entry that names no skill, and for
/// a persona purlis cannot write.
pub fn change(plane: &Path, persona: &str, written: &str, change: Change) -> Result<bool, String> {
    let reference = Reference::parse(written).ok_or_else(|| {
        format!("\u{201c}{written}\u{201d} names no skill: write a skill's name, as the Skill studio lists it")
    })?;
    let mut entries = crate::personaverbs::declared_skills(plane, persona);
    let names = |one: &String| Reference::parse(one).as_ref() == Some(&reference);
    match change {
        Change::Add if entries.iter().any(names) => return Ok(false),
        Change::Add => entries.push(reference.to_string()),
        Change::Remove => {
            let before = entries.len();
            entries.retain(|one| !names(one));
            if entries.len() == before {
                return Ok(false);
            }
        }
    }
    write(plane, persona, KEY, &entries)?;
    Ok(true)
}

/// **Writes `persona`'s `pinned-skills:` line** as `pins`, or takes it out for none (SK-15).
pub(crate) fn write_pins(plane: &Path, persona: &str, pins: &[String]) -> Result<(), String> {
    write(plane, persona, PINS_KEY, pins)
}

/// `persona`'s own definition with its `key:` line saying `entries`, or without the line for
/// none.
fn write(plane: &Path, persona: &str, key: &str, entries: &[String]) -> Result<(), String> {
    if let Some(refused) = crate::personas::name_refusal(plane, persona) {
        return Err(refused);
    }
    crate::settings::writable(plane)?;
    let file = crate::personas::def_path(plane, persona);
    crate::contain::writable(plane, &file).map_err(|refused| refused.to_string())?;
    let text = crate::contain::read_text_no_link(plane, &file)
        .map_err(|why| format!("purlis could not read {}: {why}", file.display()))?;
    // A comma list reads back as these entries only when none holds a comma or a blank end.
    if let Some(odd) = entries
        .iter()
        .find(|entry| entry.is_empty() || entry.contains(',') || entry.trim() != entry.as_str())
    {
        return Err(format!(
            "{} cannot list '{}' on its `{key}:` line: a comma list's entry holds no comma \
             and no blank end, so nothing was written.",
            file.display(),
            crate::personas::one_line(odd)
        ));
    }
    let line = entries.join(", ");
    let next = crate::personas::with_key(&text, key, (!line.is_empty()).then_some(&line))
        .map_err(|why| format!("{} {why}, so nothing was written.", file.display()))?;
    if next == text {
        return Ok(());
    }
    crate::layer::write_whole(&file, &next)
}

#[cfg(test)]
mod tests;
