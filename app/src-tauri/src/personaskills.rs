// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A persona's Skills section** (SK-5, #2768): the blocks its view draws them in, in the panel
//! vocabulary every view answers in, behind the Skill studio's flag. Adding and removing a name
//! is `skillstudio::persona_skill`; the window lends the rows those controls
//! (`app/src/personaSkills.tsx`).

use std::path::Path;

use purlis_core::panel;

/// The key prefix of a persona skill row its `skills:` list names: the rest is the entry, which
/// the window's Remove sends back (`app/src/personaSkills.tsx`).
pub(crate) const NAMED_SKILL_ROW: &str = "named:";

/// **A persona's Skills section** (SK-5, #2768), as blocks: how many skills it brings into every
/// workspace, and one row each, its own first, then each its `skills:` list names. Every row
/// opens the Skill studio on the persona's chats; one its chats go without says why.
pub(crate) fn blocks(root: &Path, name: &str, built_in: Option<&Path>) -> Vec<panel::Block> {
    let listed = purlis_core::skills::persona::listed(root, name, built_in);
    let studio = format!("skills.studio:persona/{name}");
    let rows = listed
        .iter()
        .map(|one| panel::Row {
            key: match &one.named_as {
                Some(written) => format!("{NAMED_SKILL_ROW}{written}"),
                None => format!("own:{}", one.name),
            },
            text: match &one.left_out {
                Some(why) => format!("{} — {why}", one.name),
                None => one.name.clone(),
            },
            note: Some(one.level.clone()),
            mark: if one.left_out.is_some() {
                panel::Mark::Trouble
            } else {
                panel::Mark::Dot
            },
            tone: if one.left_out.is_some() {
                panel::Tone::Trouble
            } else {
                panel::Tone::Plain
            },
            detail: None,
            runs: Some(studio.clone()),
            actions: Vec::new(),
        })
        .collect();
    vec![
        panel::Block::Note {
            text: format!(
                "It brings {} into every workspace.",
                purlis_core::counts::counted(listed.len(), "skill", "skills")
            ),
            tone: panel::Tone::Plain,
        },
        panel::Block::List {
            rows,
            empty: panel::Empty {
                headline: "No skills go with it yet".into(),
                body: Some(
                    "Add one by its name, or open the Skill studio to see every skill its chats \
                     could use."
                        .to_owned(),
                ),
                offer: Some(studio),
            },
        },
    ]
}

#[cfg(test)]
mod tests;
