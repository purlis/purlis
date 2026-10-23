// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which skills a skill names, and which name it** (SK-19, #2782; spec #2762 decision 57): the
//! uses / used-by graph, read from each skill's text by the one reader of a skill reference, which
//! the lint of broken references (SK-8) reads by too. No composition engine: a reference is a
//! skill's text naming another (`skills:<name>`, `purlis:<name>`), and nothing runs one from
//! another but the chat.
//!
//! Pure: purlis-core reads the skills' texts and hands them in.

use std::collections::{HashMap, HashSet};

/// The most skills one graph is drawn from: a project of more is drawn from the first this many,
/// and says so.
pub const MOST_SKILLS: usize = 2000;

/// The most references one skill's text is read for: a text naming more is read for the first
/// this many.
pub const MOST_REFERENCES: usize = 200;

/// One skill a skill's text names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Use {
    /// As the text writes it: `skills:deploy`, `purlis:secrets`.
    pub said: String,
    /// The skill's name.
    pub name: String,
    /// Whether it names one of the project's skills (`skills:`), not a built-in one.
    pub project: bool,
    /// Whether no skill of that name is there: a broken reference, which the lint says too.
    pub broken: bool,
}

/// **The names `text` gives after `namespace:`**, in the order it gives them: where the
/// namespace is not the end of a longer word and a skill's name follows. The one reader of a
/// skill reference.
fn named_in(text: &str, namespace: &str) -> Vec<String> {
    let needle = format!("{namespace}:");
    let word = |c: char| c.is_ascii_alphanumeric() || "_.-/".contains(c);
    text.match_indices(&needle)
        .filter(|(at, _)| !text[..*at].chars().next_back().is_some_and(word))
        .filter_map(|(at, _)| {
            let rest = &text[at + needle.len()..];
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
                .collect();
            let name = name.trim_end_matches('-');
            (!name.is_empty() && crate::change::name_problem(name).is_none())
                .then(|| name.to_owned())
        })
        .collect()
}

/// **Every skill `text` names, once each**, in the order it names them: the project's by
/// `namespace` (`skills`), checked against `plane`, and the built-in ones by their prefixes,
/// checked against `built_in`. At most [`MOST_REFERENCES`].
pub fn uses_in(text: &str, namespace: &str, plane: &[String], built_in: &[String]) -> Vec<Use> {
    let mut out: Vec<Use> = Vec::new();
    let mut look = |prefix: &str, known: &[String], project: bool| {
        for name in named_in(text, prefix) {
            let said = format!("{prefix}:{name}");
            if out.len() >= MOST_REFERENCES || out.iter().any(|one| one.said == said) {
                continue;
            }
            out.push(Use {
                broken: !known.contains(&name),
                said,
                name,
                project,
            });
        }
    };
    look(namespace, plane, true);
    for prefix in crate::BUILT_IN_PREFIXES {
        look(prefix, built_in, false);
    }
    out
}

/// One skill of the graph: what it names, and the skills whose text names it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Node {
    pub uses: Vec<Use>,
    /// The names of the project's skills that name this one, each once, by name.
    pub used_by: Vec<String>,
}

/// **The uses / used-by graph of `skills`**, each a skill's name and its `SKILL.md`'s text, read
/// against the built-in skills' names `built_in`: by name. Of two copies of a name, the uses of
/// each are joined. At most [`MOST_SKILLS`] are read; the second answer says whether some were
/// left out. O(skills + references).
pub fn graph(
    skills: &[(String, String)],
    namespace: &str,
    built_in: &[String],
) -> (HashMap<String, Node>, bool) {
    let cut = skills.len() > MOST_SKILLS;
    let read = &skills[..skills.len().min(MOST_SKILLS)];
    let plane: Vec<String> = read
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let mut nodes: HashMap<String, Node> = HashMap::new();
    for (name, text) in read {
        let uses = uses_in(text, namespace, &plane, built_in);
        let node = nodes.entry(name.clone()).or_default();
        for one in uses {
            if !node.uses.iter().any(|had| had.said == one.said) {
                node.uses.push(one);
            }
        }
    }
    let edges: Vec<(String, String)> = nodes
        .iter()
        .flat_map(|(from, node)| {
            node.uses
                .iter()
                .filter(|one| one.project && !one.broken && one.name != *from)
                .map(|one| (one.name.clone(), from.clone()))
        })
        .collect();
    for (to, from) in edges {
        // Each pair comes once: a node's uses hold each name once.
        nodes.entry(to).or_default().used_by.push(from);
    }
    for node in nodes.values_mut() {
        node.used_by.sort();
    }
    (nodes, cut)
}

#[cfg(test)]
mod tests;
