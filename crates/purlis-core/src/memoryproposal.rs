// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A memory or todo a chat proposes for a store that is not its own** (M74.1 T1.1-68, C1 and
//! C2; ADR 0067 §2 as amended 2026-11-25).
//!
//! A sandboxed chat writes only the memory and todos of the persona it runs as, of shared
//! memory and of its own workspace ([`crate::brokered`]). What it does to another persona's
//! memory, or another workspace's memory or todos, used to be refused with no one asked. For
//! the verbs below it is now a proposal ([`crate::hookwire::Ask::ProposeMemory`]): held for the
//! person, shown as exactly what it will do, written only on their Apply, and written exactly
//! as shown.
//!
//! **Which verbs** ([`proposable_kind`]): a memory remembered and a todo added, which only add;
//! and, by the person's ruling of 2026-11-25, a memory forgotten and a memory edited, each
//! shown with the entry as it is now: the whole entry removed, or the entry before and after.
//! Move, optimize, archive, unarchive, a todo's close and a workspace's vision stay refused,
//! with their words.
//!
//! **The target is named by the chat and judged by the app**: it must be a persona that loads,
//! or a workspace that is there, never `_shared`, which a chat reaches with `--shared`. Nothing
//! the line carries says who asks: that is the app's record of the chat.
//!
//! **What is shown is what is written, or nothing is.** A forget or an edit is pinned to the
//! entry's text as it was shown ([`Shown::state`]); the pin travels down to the write, which
//! checks it under the store's lock and acts only on the entry it was shown, by its exact name
//! ([`NotMade::Stale`] where it is not that text any more, or is gone). A remember or a todo
//! adds an entry and changes none, so it has nothing to be stale against.

use std::path::Path;

use crate::active::Place;
use crate::brokered::{Asker, Done, Store, Write};

/// The store a proposed write is aimed at, as the chat named it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "name", rename_all = "snake_case")]
pub enum Target {
    /// Another persona's own memory.
    Persona(String),
    /// Another workspace's memory and todos.
    Workspace(String),
}

impl Target {
    /// The target in words, as a row and a chat's note name it.
    pub fn said(&self) -> String {
        match self {
            Self::Persona(name) => format!("persona {}", crate::shown::short(name)),
            Self::Workspace(name) => format!("workspace {}", crate::shown::short(name)),
        }
    }

    /// The store a write of an entry names for this target.
    fn store(&self) -> Store {
        match self {
            Self::Persona(_) => Store::Persona,
            Self::Workspace(_) => Store::Workspace,
        }
    }
}

/// What a chat is told of a write that is not proposed: it stays refused, as it always was.
pub const NOT_PROPOSABLE: &str = "only a memory or todo added, or a memory forgotten or edited, \
     is proposed to the person for a store that is not this chat's own: nothing was written";

/// The exit code a memory verb says when the entry was not the text a proposal pinned it to,
/// so nothing was written ([`apply`] reads it as [`NotMade::Stale`]).
pub const STALE_CODE: u8 = 75;

/// What a proposal whose text draws as something other than it is is refused with.
pub const DRAWN_OTHERWISE: &str = "it carries a character that draws as nothing or as something \
     else, so what the person would read is not what would be written: nothing was written";

/// Whether `write` is one of the verbs a chat may propose for another store ([`Target`]):
/// remember, a todo added, forget, edit. The only list of them.
pub fn proposable_kind(write: &Write) -> bool {
    matches!(
        write,
        Write::PersonaRemember { .. }
            | Write::WorkspaceRemember { .. }
            | Write::Todo { .. }
            | Write::Forget { .. }
            | Write::EditMemory { .. }
    )
}

/// **A proposal as the person is shown it**: the write as it will be made, the entry it acts on
/// before and after, and what it is pinned to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// The write as held: its text trimmed as it is shown, and a forget or an edit naming the
    /// entry it was shown by its exact name, with the title and text it will have.
    pub write: Write,
    /// The entry's whole text as it is now: empty for what adds.
    pub before: String,
    /// What the proposal leaves: the sentence that says what is added; empty for a forget, whose
    /// entry is gone; the entry's whole new text for an edit.
    pub after: String,
    /// A digest of [`Self::before`] for a forget or an edit, which Apply checks the entry
    /// against; `None` for what adds, which nothing can make stale.
    pub state: Option<String>,
}

/// Why a proposal was not made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotMade {
    /// The entry is not the text the person was shown: nothing was written.
    Stale,
    /// Refused, for this sentence: nothing was written.
    Refused(String),
}

impl From<String> for NotMade {
    fn from(why: String) -> Self {
        Self::Refused(why)
    }
}

/// **`write` for `target`, as the person will be shown it** in the project at `root`, or why it
/// cannot be proposed: a verb [`proposable_kind`] names, of the kind the target takes, its store
/// there, the entry it names there, and a text the window shows as it is.
pub fn shown(root: &Path, write: Write, target: &Target) -> Result<Shown, String> {
    if !proposable_kind(&write) {
        return Err(NOT_PROPOSABLE.to_owned());
    }
    store_there(root, target)?;
    let shown = match crate::runnerwrite::held_as(write) {
        Write::Forget { of, slug } => {
            fits(of, target)?;
            let one = opened(root, target, &slug)?;
            Shown {
                write: Write::Forget {
                    of,
                    slug: one.exact,
                },
                state: crate::proposed::base_of(Some(one.entry.text.as_bytes())),
                before: one.entry.text,
                after: String::new(),
            }
        }
        Write::EditMemory {
            of,
            slug,
            title,
            text,
        } => {
            fits(of, target)?;
            let one = opened(root, target, &slug)?;
            let (title, text) = one.entry.revised(title.as_deref(), text.as_deref());
            let after = crate::memstore::edited(&one.entry.text, &title, &text)?;
            Shown {
                write: Write::EditMemory {
                    of,
                    slug: one.exact,
                    title: Some(title),
                    text: Some(text),
                },
                state: crate::proposed::base_of(Some(one.entry.text.as_bytes())),
                before: one.entry.text,
                after,
            }
        }
        added => {
            adds_to(&added, target)?;
            let (Write::PersonaRemember { text, .. }
            | Write::WorkspaceRemember { text, .. }
            | Write::Todo { text }) = &added
            else {
                return Err(NOT_PROPOSABLE.to_owned());
            };
            if text.is_empty() {
                return Err("there is no text to add: nothing was written".to_owned());
            }
            Shown {
                state: None,
                before: String::new(),
                after: said_whole(&added, target),
                write: added,
            }
        }
    };
    shown.write.check()?;
    // What looks like a credential is refused as that, not as a text that draws otherwise.
    if let Some(why) = crate::runnerwrite::refusal(&shown.write) {
        return Err(why.to_owned());
    }
    if !crate::runnerwrite::drawn_as_it_is(&shown.write) {
        return Err(DRAWN_OTHERWISE.to_owned());
    }
    Ok(shown)
}

/// That `of`, the store a forget or an edit names, is the one `target` is.
fn fits(of: Store, target: &Target) -> Result<(), String> {
    if of == target.store() {
        Ok(())
    } else {
        Err(NOT_FITTING.to_owned())
    }
}

/// What a write aimed at a store it does not fit is refused with.
const NOT_FITTING: &str = "that write does not fit the store it names: nothing was written";

/// That an adding write is of the kind `target` takes: a persona's memory for a persona, a
/// workspace's memory or a todo for a workspace. Shared memory is reached with `--shared`.
fn adds_to(write: &Write, target: &Target) -> Result<(), String> {
    match (write, target) {
        (Write::PersonaRemember { shared: false, .. }, Target::Persona(_))
        | (Write::WorkspaceRemember { .. } | Write::Todo { .. }, Target::Workspace(_)) => Ok(()),
        _ => Err(NOT_FITTING.to_owned()),
    }
}

/// That `target` is a store this project has: a persona that loads, or a workspace that is
/// there; never `_shared`.
fn store_there(root: &Path, target: &Target) -> Result<(), String> {
    match target {
        Target::Persona(name) => {
            if name == crate::contain::SHARED_PERSONA {
                return Err(
                    "shared memory is written with --shared, not proposed by name".to_owned(),
                );
            }
            match crate::personas::name_refusal(root, name) {
                Some(why) => Err(format!("{why}: nothing was written")),
                None => Ok(()),
            }
        }
        Target::Workspace(name) => {
            let there = crate::workspaces::Plane::open(root)
                .workspace(name)
                .is_ok_and(|ws| ws.dir().is_dir());
            if there {
                Ok(())
            } else {
                Err(format!(
                    "no workspace {} in this project: nothing was written",
                    crate::shown::short(name)
                ))
            }
        }
    }
}

/// One entry of the target's memory, as the window reads it to show it.
struct Opened {
    /// The entry's name without `.md`: what the write is held under.
    exact: String,
    entry: crate::workspaces::Opened,
}

/// The entry `slug` names in `target`'s memory, opened.
fn opened(root: &Path, target: &Target, slug: &str) -> Result<Opened, String> {
    use crate::memstore::Typed;
    let plane = crate::workspaces::Plane::open(root);
    let io = |e: std::io::Error| format!("{e}: nothing was written");
    let (exact, entry) = match target {
        Target::Workspace(name) => {
            let ws = plane.workspace(name).map_err(|e| e.to_string())?;
            let exact = ws.typed_memory(slug, Typed::Stored).map_err(io)?;
            let entry = ws.open_memory(&exact).map_err(io)?;
            (exact, entry)
        }
        Target::Persona(name) => {
            let persona = plane.persona(name).map_err(|e| e.to_string())?;
            let exact = persona.typed_memory(slug, Typed::Stored).map_err(io)?;
            let entry = persona.open_memory(&exact).map_err(io)?;
            (exact, entry)
        }
    };
    Ok(Opened {
        exact: exact.strip_suffix(".md").unwrap_or(&exact).to_owned(),
        entry,
    })
}

/// The most of a text a proposal's name carries: the whole text is in [`Shown::after`] alone.
const NAME_CUT_CHARS: usize = 60;

/// `text`'s first line, cut to [`NAME_CUT_CHARS`], as a proposal's name carries it.
fn cut(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    let kept: String = line.chars().take(NAME_CUT_CHARS).collect();
    if kept.len() < text.trim().len() {
        format!("{kept}…")
    } else {
        kept
    }
}

/// **What a proposal is called**, in one short line: where, and what. A text it adds is named by
/// its title, or a short cut of it, never whole: the whole is [`Shown::after`], and nothing long
/// goes into the audit, the chat's word or the list of waiting changes.
pub fn said(write: &Write, target: &Target) -> String {
    let what = match write {
        Write::Forget { slug, .. } => format!("Forget memory {slug}"),
        Write::EditMemory { slug, .. } => format!("Rewrite memory {slug}"),
        Write::Todo { text } => format!("Add a todo {:?}", cut(text)),
        Write::PersonaRemember { text, title, .. } | Write::WorkspaceRemember { text, title } => {
            format!(
                "Remember {:?}",
                title.as_deref().map_or_else(|| cut(text), cut)
            )
        }
        other => crate::runnerwrite::said(other),
    };
    format!("In {}: {what}", target.said())
}

/// **What an added memory or todo is, whole**, for the row's text: the TARGET store named
/// outright, the title on a line of its own, quoted so no text can pass as another line, then
/// the text, and what else the write does there.
fn said_whole(write: &Write, target: &Target) -> String {
    let (name, store, verb, title, text, also) = match (write, target) {
        (Write::PersonaRemember { text, title, .. }, Target::Persona(name)) => (
            name,
            "memory",
            "remember",
            title.as_deref(),
            text,
            "Its line in that memory's index is written too.",
        ),
        (Write::WorkspaceRemember { text, title }, Target::Workspace(name)) => (
            name,
            "memory",
            "remember",
            title.as_deref(),
            text,
            "Its line in that memory's index is written too.",
        ),
        (Write::Todo { text }, Target::Workspace(name)) => (
            name,
            "todos",
            "add the todo",
            None,
            text,
            "A todo already promoted to an issue, whose close a crash cut short, is closed \
             first, and the todo's index line is written too.",
        ),
        _ => return said(write, target),
    };
    let kind = match target {
        Target::Persona(_) => "persona",
        Target::Workspace(_) => "workspace",
    };
    let title = title.map_or_else(String::new, |title| format!("Title: {title:?}\n"));
    format!(
        "In {kind} {}'s {store}: {verb}.\n{title}{text}\n{also}",
        crate::shown::short(name)
    )
}

/// The lines of a memory verb's answer, as one sentence.
fn joined(lines: &[crate::repocmd::Say]) -> String {
    lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

/// **The write the person's Apply makes**, for chat `chat`, aimed at `target`: made as the app
/// makes a chat's own brokered write, with the target standing where the chat's own store
/// would. `held` is the write [`shown`] answered and `state` what it pinned: it is judged again
/// first, so a store gone since is refused, and an entry that is not the text the person was
/// shown is [`NotMade::Stale`] and nothing is written. The sentence to tell the person.
pub fn apply(
    root: &Path,
    chat: u32,
    held: &Write,
    target: &Target,
    state: Option<&str>,
    now: jiff::civil::DateTime,
) -> Result<String, NotMade> {
    // A first, cheap judgement, so a store gone since is refused and an entry already changed
    // costs no write; what makes the pin hold is the check at the write, under the store's lock.
    let again = shown(root, held.clone(), target)?;
    if again.state.as_deref() != state || &again.write != held {
        return Err(NotMade::Stale);
    }
    let asker = match target {
        Target::Persona(name) => Asker {
            chat,
            place: Place::PlaneRoot,
            persona: Some(name.clone()),
        },
        Target::Workspace(name) => Asker {
            chat,
            place: Place::Workspace(name.clone()),
            persona: None,
        },
    };
    match crate::brokered::answer_pinned(root, &asker, held, now, state)? {
        Done::Written(written) => Ok(format!("Written to {}.", written.path)),
        Done::Said { lines, code: 0 } => Ok(joined(&lines)),
        Done::Said {
            code: STALE_CODE, ..
        } => Err(NotMade::Stale),
        Done::Said { lines, .. } => Err(NotMade::Refused(joined(&lines))),
    }
}

#[cfg(test)]
mod tests;
