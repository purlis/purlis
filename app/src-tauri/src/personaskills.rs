// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A persona's Skills summary** (SK-5, #2768): the block its view draws it in, in the panel
//! vocabulary every view answers in, behind the Skill studio's flag. The skills themselves are
//! listed, added and removed in the Skill studio filtered to the persona (`skillstudio`).

use std::path::Path;

use purlis_core::panel;

/// **A persona's Skills summary** (SK-5, #2768; spec decision 35): one row, how many skills it
/// brings into every workspace, which opens the Skill studio on a chat as the persona. Every
/// skill choice is the studio's, so the page only counts.
pub(crate) fn blocks(root: &Path, name: &str) -> Vec<panel::Block> {
    let brings = purlis_core::skills::persona::brings(root, name);
    vec![panel::Block::List {
        rows: vec![panel::Row {
            key: "skills".to_owned(),
            text: format!(
                "Brings {} into every workspace",
                purlis_core::counts::counted(brings, "skill", "skills")
            ),
            note: Some("Open in Skill studio".to_owned()),
            mark: panel::Mark::Dot,
            tone: panel::Tone::Plain,
            detail: None,
            runs: Some(format!("skills.studio:persona/{name}")),
            actions: Vec::new(),
        }],
        empty: panel::Empty {
            headline: "Skills".into(),
            body: None,
            offer: None,
        },
    }]
}

#[cfg(test)]
mod tests;
