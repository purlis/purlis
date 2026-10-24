// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The uses / used-by graph of the project's skills** (SK-19, #2782; spec #2762 decision 57):
//! each skill's text read by the one reader of skill references ([`purlis_skills::uses`]), which
//! the lint of broken references reads by too, so a reference the graph draws broken is one the
//! editor warns of. The skill's tab draws one skill's node.

use std::path::Path;

use purlis_skills::uses;
pub use purlis_skills::uses::{MOST_SKILLS, Use};

use super::verbs::Level;
use super::{LINKED_PLUGIN, built_in_names};

/// One skill's place in the graph, as its tab draws it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsesOf {
    /// The skills its text names, each marked broken where there is none of that name.
    pub uses: Vec<Use>,
    /// The project's skills whose text names it, by name.
    pub used_by: Vec<String>,
    /// Whether the project holds more skills than the graph reads, so some were left out.
    pub cut: bool,
}

/// **What the skill `name` names, and which of the project's skills name it**, read from every
/// skill of the project (its own and each workspace's and persona's, shared and local) by
/// following no link, at most [`uses::MOST_SKILLS`], against the built-in skills in `built_in`.
pub fn uses_of(plane: &Path, built_in: Option<&Path>, name: &str) -> UsesOf {
    // Every level, each persona's among them (SK-32).
    let dirs: Vec<std::path::PathBuf> = Level::every(plane)
        .iter()
        .filter_map(|level| level.dir(plane))
        .collect();
    let mut skills: Vec<(String, String)> = Vec::new();
    'dirs: for dir in dirs {
        for skill in super::read(&dir) {
            // One past the most, so the graph can say it was cut.
            if skills.len() > uses::MOST_SKILLS {
                break 'dirs;
            }
            let text = purlis_skills::read_skill_md(&skill.path).unwrap_or_default();
            skills.push((skill.name, text));
        }
    }
    let (mut graph, cut) = uses::graph(&skills, LINKED_PLUGIN, &built_in_names(built_in));
    let node = graph.remove(name).unwrap_or_default();
    UsesOf {
        uses: node.uses,
        used_by: node.used_by,
        cut,
    }
}

#[cfg(test)]
mod tests;
