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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Use {
    /// As the text writes it: `skills:deploy`, `purlis:purlis-secrets`.
    pub said: String,
    /// The skill's name.
    pub name: String,
    /// Whether it names one of the project's skills (`skills:`), not a built-in one.
    pub project: bool,
    /// Whether no skill of that name is there: a broken reference, which the lint says too.
    pub broken: bool,
    /// Whether the text names it bare, not as `skills:<name>`: a hint ([`hints_in`], #4510),
    /// never broken, as only a skill there is can be one.
    pub hint: bool,
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
/// `namespace` (`skills`), each asked of `plane` whether there is one of that name, and the
/// built-in ones by their prefixes, asked of `built_in`. At most [`MOST_REFERENCES`], so the
/// check that each is named once is over a bounded list.
pub fn uses_in(
    text: &str,
    namespace: &str,
    plane: &dyn Fn(&str) -> bool,
    built_in: &dyn Fn(&str) -> bool,
) -> Vec<Use> {
    let mut out: Vec<Use> = Vec::new();
    let mut look = |prefix: &str, known: &dyn Fn(&str) -> bool, project: bool| {
        for name in named_in(text, prefix) {
            let said = format!("{prefix}:{name}");
            if out.len() >= MOST_REFERENCES || out.iter().any(|one| one.said == said) {
                continue;
            }
            // A built-in skill by the name it had before it took the prefix reads as it now.
            let name = if project {
                name
            } else {
                match crate::Reference::parse(&said) {
                    Some(crate::Reference::BuiltIn(own)) => own,
                    _ => name,
                }
            };
            out.push(Use {
                broken: !known(&name),
                said,
                name,
                project,
                hint: false,
            });
        }
    };
    look(namespace, plane, true);
    for prefix in crate::BUILT_IN_PREFIXES {
        look(prefix, built_in, false);
    }
    out
}

/// The quotes a bare name is read inside ([`hints_in`]), each opening mark with its closing one:
/// straight and curly double quotes, and a code span's backtick. Not a single quote, which is an
/// apostrophe as often.
const QUOTES: [(char, char); 3] = [('"', '"'), ('\u{201c}', '\u{201d}'), ('`', '`')];

/// The word a line must hold, beside a quoted name, for the name to be read as a hint: on its
/// own, in any case, or as "skills".
const SKILL_WORD: &str = "skill";

/// Whether `text` holds [`SKILL_WORD`] as a word of its own, or its plural: not inside a longer
/// word such as "skillful".
fn says_skill(text: &str) -> bool {
    let text = text.to_lowercase();
    let letter = |c: char| c.is_alphanumeric();
    text.match_indices(SKILL_WORD).any(|(at, _)| {
        let rest = &text[at + SKILL_WORD.len()..];
        let rest = rest.strip_prefix('s').unwrap_or(rest);
        !text[..at].chars().next_back().is_some_and(letter)
            && !rest.chars().next().is_some_and(letter)
    })
}

/// **Every skill `text` names bare, as a hint, once each** (#4510), in the order it names them:
/// a name that is the whole of a quote or a code span ([`QUOTES`]) closed on its line, where the
/// rest of that line holds the word "skill" in any case, and that `known` says is a skill there
/// is (one of the same project, or of the same source), not the skill `own` itself. So
/// ``Use the Skill tool with "grilling"`` hints at `grilling` where there is one, and a quoted
/// word that names no skill is never read. Each is a project [`Use`] marked [`Use::hint`], never
/// broken. At most [`MOST_REFERENCES`].
///
/// Nothing decides from a hint what a chat does: it is what `purlis skill uses`, the lint and an
/// install's review tell the person, and what an install brings along.
pub fn hints_in(text: &str, own: &str, known: &dyn Fn(&str) -> bool) -> Vec<Use> {
    let mut out: Vec<Use> = Vec::new();
    for line in text.lines() {
        for (open, close, name) in hint_quotes(line, own, known) {
            if out.len() >= MOST_REFERENCES {
                return out;
            }
            if out.iter().any(|one| one.name == name) {
                continue;
            }
            out.push(Use {
                said: line[open..close].to_owned(),
                name: name.to_owned(),
                project: true,
                broken: false,
                hint: true,
            });
        }
    }
    out
}

/// **`text` with every quote that hints at `name` ([`hints_in`]) written as a reference**
/// (#5874): `"grilling"` becomes `"skills:grilling"` (with `namespace` before the colon), inside
/// its own marks, so the line reads the same and the reference is checked. Every other line,
/// quote and name is left as it was, line endings too.
pub fn written_as_reference(text: &str, own: &str, name: &str, namespace: &str) -> String {
    let only = |one: &str| one == name;
    let mut out = String::with_capacity(text.len() + namespace.len() + 1);
    for line in text.split_inclusive('\n') {
        let mut at = 0;
        for (open, close, _) in hint_quotes(line, own, &only) {
            let inner = open + line[open..].chars().next().map_or(1, char::len_utf8);
            let end = close - line[..close].chars().next_back().map_or(1, char::len_utf8);
            out.push_str(&line[at..inner]);
            out.push_str(namespace);
            out.push(':');
            out.push_str(&line[inner..end]);
            at = end;
        }
        out.push_str(&line[at..]);
    }
    out
}

/// **The first line of `text` that hints at `name`** ([`hints_in`]), counted from 1 as an
/// editor numbers them, or `None` where none does.
pub fn hint_line(text: &str, own: &str, name: &str) -> Option<usize> {
    let only = |one: &str| one == name;
    text.lines()
        .position(|line| !hint_quotes(line, own, &only).is_empty())
        .map(|at| at + 1)
}

/// Each quote of `line` that hints at a skill: its opening mark's and past its closing mark's
/// byte offsets, and the name it holds, which `known` says is a skill there is, not `own`, on a
/// line that says "skill" beside it.
fn hint_quotes<'a>(
    line: &'a str,
    own: &str,
    known: &dyn Fn(&str) -> bool,
) -> Vec<(usize, usize, &'a str)> {
    quoted(line)
        .into_iter()
        .filter_map(|(open, close)| {
            let inner_start = open + line[open..].chars().next().map_or(1, char::len_utf8);
            let inner_end = close - line[..close].chars().next_back().map_or(1, char::len_utf8);
            let name = &line[inner_start..inner_end];
            let rest = format!("{}{}", &line[..open], &line[close..]);
            (name != own
                && crate::change::name_problem(name).is_none()
                && says_skill(&rest)
                && known(name))
            .then_some((open, close, name))
        })
        .collect()
}

/// Each quote of `line` ([`QUOTES`]) that closes on it, as byte offsets from its opening mark to
/// past its closing one, in order: one quote's closing mark never opens the next.
fn quoted(line: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut chars = line.char_indices();
    while let Some((at, c)) = chars.next() {
        let Some((_, close)) = QUOTES.iter().find(|(open, _)| *open == c) else {
            continue;
        };
        for (end, inner) in chars.by_ref() {
            if inner == *close {
                out.push((at, end + inner.len_utf8()));
                break;
            }
        }
    }
    out
}

/// One skill of the graph: what it names, and the skills whose text names it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Node {
    pub uses: Vec<Use>,
    /// The names of the project's skills that name this one, each once, by name.
    pub used_by: Vec<String>,
    /// The names of the project's skills whose text names this one only bare, as a hint
    /// ([`hints_in`]), each once, by name.
    pub hinted_by: Vec<String>,
}

/// **The uses / used-by graph of `skills`**, each a skill's name and its `SKILL.md`'s text, read
/// against the built-in skills' names `built_in`: by name. Of two copies of a name, the uses of
/// each are joined. At most [`MOST_SKILLS`] are read; the second answer says whether some were
/// left out. Each name is looked up in a set, so the graph costs the skills' texts and their
/// references, at most [`MOST_REFERENCES`] each.
pub fn graph(
    skills: &[(String, String)],
    namespace: &str,
    built_in: &[String],
) -> (HashMap<String, Node>, bool) {
    let cut = skills.len() > MOST_SKILLS;
    let read = &skills[..skills.len().min(MOST_SKILLS)];
    let plane: HashSet<&str> = read.iter().map(|(name, _)| name.as_str()).collect();
    let built_in: HashSet<&str> = built_in.iter().map(String::as_str).collect();
    let mut nodes: HashMap<String, Node> = HashMap::new();
    for (name, text) in read {
        let mut uses = uses_in(text, namespace, &|one| plane.contains(one), &|one| {
            built_in.contains(one)
        });
        uses.extend(hints_in(text, name, &|one| plane.contains(one)));
        let node = nodes.entry(name.clone()).or_default();
        for one in uses {
            // A hint is the same use as any of the project's of its name, however quoted; of the
            // two, a reference wins, in this text or in another copy's.
            let had = node.uses.iter().position(|had| {
                if one.hint || had.hint {
                    had.project && one.project && had.name == one.name
                } else {
                    had.said == one.said
                }
            });
            match had {
                None => node.uses.push(one),
                Some(at) if node.uses[at].hint && !one.hint => node.uses[at] = one,
                Some(_) => {}
            }
        }
    }
    let edges: Vec<(String, String, bool)> = nodes
        .iter()
        .flat_map(|(from, node)| {
            node.uses
                .iter()
                .filter(|one| one.project && !one.broken && one.name != *from)
                .map(|one| (one.name.clone(), from.clone(), one.hint))
        })
        .collect();
    for (to, from, hint) in edges {
        // Each pair comes once: a node's uses hold each name once, as a reference or a hint.
        let node = nodes.entry(to).or_default();
        if hint {
            node.hinted_by.push(from);
        } else {
            node.used_by.push(from);
        }
    }
    for node in nodes.values_mut() {
        node.used_by.sort();
        node.hinted_by.sort();
    }
    (nodes, cut)
}

#[cfg(test)]
mod tests;
