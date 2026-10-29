// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A persona's Skills summary** (SK-5, #2768): the block its view draws it in, in the panel
//! vocabulary every view answers in, behind the Skill studio's flag. The skills themselves are
//! listed, added and removed in the Skill studio filtered to the persona (`skillstudio`).

use std::path::Path;

use purlis_core::panel;

/// **A persona's Skills summary** (SK-5, #2768; spec decision 35; #3695, the amendment's S10):
/// one line of counts, how many skills it brings into every workspace, which opens the Skill
/// studio's Library on a chat as the persona; and, where `needing` items of its Needs you wait,
/// "K need you", which opens that page. A row runs one catalogue row, so the line's two ways in
/// are two rows of one list. Every skill choice is the studio's, so the page only counts.
pub(crate) fn blocks(root: &Path, name: &str, needing: usize) -> Vec<panel::Block> {
    let brings = purlis_core::skills::persona::brings(root, name);
    let mut rows = vec![panel::Row {
        key: "skills".to_owned(),
        text: format!(
            "Brings {} into every workspace",
            purlis_core::counts::counted(brings, "skill", "skills")
        ),
        note: Some("Open Skill studio".to_owned()),
        mark: panel::Mark::Dot,
        tone: panel::Tone::Plain,
        detail: None,
        runs: Some(crate::skillstudio::studio_as(name)),
        actions: Vec::new(),
    }];
    if needing > 0 {
        rows.push(panel::Row {
            key: "skills-need-you".to_owned(),
            text: format!(
                "{needing} {}",
                purlis_core::counts::plural(needing, "needs you", "need you")
            ),
            note: Some("Needs you".to_owned()),
            // Something that wants reading: nothing is wrong, so not the trouble tone.
            mark: panel::Mark::Note,
            tone: panel::Tone::Plain,
            detail: None,
            runs: Some(crate::skillstudio::needs_as(name)),
            actions: Vec::new(),
        });
    }
    vec![panel::Block::List {
        rows,
        empty: panel::Empty {
            headline: "Skills".into(),
            body: None,
            offer: None,
        },
    }]
}

#[cfg(test)]
mod tests;
