// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The person's own sandbox level for a project, on their machine** (#2071, the operator's
//! ruling of 2026-10-14): `on` (the full sandbox), `floor` (the default) or `off` (no sandbox),
//! for the whole project and, optionally, per persona.
//!
//! Kept only in this machine's `purlis.local.toml`, never in a committed file, as `[sandbox]`
//! keys beside the person's own hosts:
//!
//! ```toml
//! [sandbox]
//! level = "floor"
//!
//! [sandbox.personas]
//! devops = "off"
//! ```
//!
//! **A persona's level wins over the project's** for a chat started as that persona; a chat
//! with neither takes the default.
//!
//! **Read only where the file is this machine's** ([`crate::settings::LayerText::Text`]): a
//! `purlis.local.toml` git would carry is left out whole, so no clone of a repository can turn
//! a person's sandbox down.
//!
//! **A chat CAN write the file.** `purlis.local.toml` sits in the project, where a sandboxed
//! chat writes, so what it says is never taken on its word alone (ADR 0096 §7). What a start
//! honours is [`in_force`]: the file's level only where it is the one you set or confirmed in
//! Settings on this machine, or a stricter one. The record of those is in this machine's
//! config home ([`super::local`] `levels_mine`), which no sandboxed chat writes. A lower level
//! the file alone says waits for your Confirm ([`waiting`]), and meanwhile the start takes what
//! you confirmed, or the floor. **Never read [`of`] to decide a start.**
//!
//! What a level does to a chat's start is the start's ([`super::decide`]); this module reads,
//! validates and judges the choice, and keeps the record of what you confirmed.

use std::collections::BTreeMap;
use std::path::Path;

/// The key of the project's level, in `[sandbox]`.
pub const LEVEL: &str = "level";

/// The table of each persona's level, in `[sandbox]`.
pub const PERSONAS: &str = "personas";

/// A level a person chooses.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    /// The full sandbox: the hosts, folders and Allow asks of the project's `[sandbox]`.
    On,
    /// The floor: no network or folder limits, and no vault read and no later-code write.
    Floor,
    /// No sandbox at all.
    Off,
}

impl Choice {
    /// Every choice, strictest first.
    pub const ALL: [Self; 3] = [Self::On, Self::Floor, Self::Off];

    /// The word the file holds.
    pub fn word(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Floor => "floor",
            Self::Off => "off",
        }
    }

    /// The choice `word` names, or `None`.
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.word() == word)
    }

    /// What it is called where it is shown: on a tab, in the title bar, in Settings.
    pub fn said(self) -> &'static str {
        match self {
            Self::On => "full sandbox",
            Self::Floor => "floor",
            Self::Off => "no sandbox",
        }
    }
}

/// What the person chose for one project: the project's level and each persona's.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Personal {
    /// `[sandbox] level`, where set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<Choice>,
    /// `[sandbox.personas]`, each persona's level.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub personas: BTreeMap<String, Choice>,
}

impl Personal {
    /// The level a chat started as `persona` (or as none) runs at by the person's choice: the
    /// persona's, else the project's, else `None` (the default).
    pub fn for_persona(&self, persona: Option<&str>) -> Option<Choice> {
        persona
            .and_then(|name| self.personas.get(name).copied())
            .or(self.level)
    }

    /// Whether nothing is chosen.
    pub fn is_empty(&self) -> bool {
        self.level.is_none() && self.personas.is_empty()
    }

    /// The level of one slot: the project's (`None`) or a persona's.
    pub fn at(&self, persona: Option<&str>) -> Option<Choice> {
        match persona {
            None => self.level,
            Some(name) => self.personas.get(name).copied(),
        }
    }

    /// The level of one slot set, or taken out for `None`.
    pub fn set(&mut self, persona: Option<&str>, choice: Option<Choice>) {
        match (persona, choice) {
            (None, choice) => self.level = choice,
            (Some(name), Some(choice)) => {
                self.personas.insert(name.to_owned(), choice);
            }
            (Some(name), None) => {
                self.personas.remove(name);
            }
        }
    }

    /// Whether any level the person chose is `off`.
    pub fn any_off(&self) -> bool {
        self.level == Some(Choice::Off) || self.personas.values().any(|one| *one == Choice::Off)
    }
}

/// One thing the file says that is not taken, with the key it is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub key: Vec<String>,
    pub why: String,
}

/// The choices in `table`, this machine's `[sandbox]`, and what is refused in it. A value that
/// is not a level, and a persona name that cannot be one, are refused and not taken.
pub fn read(table: &toml::Table, file: &str) -> (Personal, Vec<Refused>) {
    let mut personal = Personal::default();
    let mut refused = Vec::new();
    let words = Choice::ALL.map(Choice::word).join(", ");
    if let Some(value) = table.get(LEVEL) {
        match value.as_str().and_then(Choice::of_word) {
            Some(choice) => personal.level = Some(choice),
            None => refused.push(Refused {
                key: vec![super::TABLE.to_owned(), LEVEL.to_owned()],
                why: format!(
                    "{}.{LEVEL} in {file} is {value}, and a level is one of {words}",
                    super::TABLE
                ),
            }),
        }
    }
    match table.get(PERSONAS) {
        None => {}
        Some(toml::Value::Table(personas)) => {
            for (name, value) in personas {
                let key = vec![
                    super::TABLE.to_owned(),
                    PERSONAS.to_owned(),
                    name.to_owned(),
                ];
                if !crate::contain::segment_ok(name) {
                    refused.push(Refused {
                        key,
                        why: format!(
                            "{}.{PERSONAS} in {file} names '{}', which cannot be a persona",
                            super::TABLE,
                            crate::shown::short(name)
                        ),
                    });
                    continue;
                }
                match value.as_str().and_then(Choice::of_word) {
                    Some(choice) => {
                        personal.personas.insert(name.clone(), choice);
                    }
                    None => refused.push(Refused {
                        key,
                        why: format!(
                            "{}.{PERSONAS}.{name} in {file} is {value}, and a level is one of \
                             {words}",
                            super::TABLE
                        ),
                    }),
                }
            }
        }
        Some(_) => refused.push(Refused {
            key: vec![super::TABLE.to_owned(), PERSONAS.to_owned()],
            why: format!(
                "{}.{PERSONAS} in {file} is not a table — it holds one level per persona",
                super::TABLE
            ),
        }),
    }
    (personal, refused)
}

/// What the person chose for the project at `root`: read from this machine's own
/// `purlis.local.toml`, and nothing from one git would carry ([`crate::settings::LayerText`]).
pub fn of(root: &Path) -> Personal {
    of_layer(&crate::settings::layer_text(
        root,
        crate::settings::Which::Local,
    ))
}

/// [`of`], from the local file as its layer was read: only a file that is this machine's own
/// says anything.
pub fn of_layer(layer: &crate::settings::LayerText) -> Personal {
    match layer {
        crate::settings::LayerText::Text(text) => of_text(text),
        crate::settings::LayerText::LeftOut { .. } | crate::settings::LayerText::Nothing => {
            Personal::default()
        }
    }
}

/// [`of`], from the file's text.
pub fn of_text(text: &str) -> Personal {
    let Ok(top) = text.parse::<toml::Table>() else {
        return Personal::default();
    };
    match top.get(super::TABLE).and_then(toml::Value::as_table) {
        Some(table) => read(table, crate::profiles::LOCAL_FILE).0,
        None => Personal::default(),
    }
}

/// The level one slot is honoured at: the file's (`file`) where it is what you confirmed
/// (`confirmed`) or stricter, the floor standing for what you confirmed where you confirmed
/// nothing; else what you confirmed, or not set (the floor). A slot the file does not set is not set, whatever the record says.
fn judged(file: Option<Choice>, confirmed: Option<Choice>) -> Option<Choice> {
    let file = file?;
    // `Choice` is ordered strictest first.
    if file <= confirmed.unwrap_or(Choice::Floor) {
        Some(file)
    } else {
        confirmed
    }
}

/// What `file`, the levels the local file says, is honoured as, given `confirmed`, the record
/// of what you set or confirmed in Settings on this machine ([`judged`], slot by slot).
pub fn honoured(file: &Personal, confirmed: &Personal) -> Personal {
    let mut out = Personal {
        level: judged(file.level, confirmed.level),
        personas: BTreeMap::new(),
    };
    for (name, choice) in &file.personas {
        if let Some(level) = judged(Some(*choice), confirmed.personas.get(name).copied()) {
            out.personas.insert(name.clone(), level);
        }
    }
    out
}

/// A level the file says and that is not honoured until you confirm it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Awaiting {
    /// The persona's slot, or the project's for `None`.
    pub persona: Option<String>,
    /// What the file says.
    pub level: Choice,
}

/// Every slot of `file` that [`honoured`] does not take as the file says it.
pub fn awaiting(file: &Personal, confirmed: &Personal) -> Vec<Awaiting> {
    let slots = std::iter::once((None, file.level)).chain(
        file.personas
            .iter()
            .map(|(name, choice)| (Some(name.as_str()), Some(*choice))),
    );
    slots
        .filter_map(|(persona, level)| {
            let level = level?;
            (judged(Some(level), confirmed.at(persona)) != Some(level)).then(|| Awaiting {
                persona: persona.map(str::to_owned),
                level,
            })
        })
        .collect()
}

/// **What a chat's start honours** for the project at `root`: this machine's file as
/// [`honoured`] judges it against what you confirmed.
pub fn in_force(root: &Path) -> Personal {
    honoured(&of(root), &super::local::confirmed_levels(root))
}

/// The levels of the project at `root` that wait for your Confirm.
pub fn waiting(root: &Path) -> Vec<Awaiting> {
    awaiting(&of(root), &super::local::confirmed_levels(root))
}

/// Settings' Confirm of `shown`, the level the row showed for the slot: recorded as yours only
/// while the file still says it, so nothing written between the showing and the click is
/// confirmed in its place. Answers the level confirmed.
pub fn confirm(root: &Path, persona: Option<&str>, shown: Choice) -> Result<Choice, String> {
    match of(root).at(persona) {
        Some(now) if now == shown => {
            set_by_you(root, persona, Some(now))?;
            Ok(now)
        }
        Some(now) => Err(format!(
            "the file now says {} here, not the {} you were shown. Nothing was confirmed: \
             look at it again",
            now.said(),
            shown.said()
        )),
        None => Err(format!(
            "the file no longer sets a level here, so there is no {} to confirm",
            shown.said()
        )),
    }
}

/// Records `choice` for the slot as yours: Settings' save of a level, and its Confirm, never a
/// chat (a sandboxed chat cannot write the record).
pub fn set_by_you(
    root: &Path,
    persona: Option<&str>,
    choice: Option<Choice>,
) -> Result<(), String> {
    if let Some(name) = persona
        && !crate::contain::segment_ok(name)
    {
        return Err(format!(
            "'{}' cannot be a persona",
            crate::shown::short(name)
        ));
    }
    super::local::change_levels(root, |mine| mine.set(persona, choice))
        .map_err(|why| format!("purlis could not keep your level on this machine: {why}"))
}

/// Takes off the record each slot `local`, this machine's file as Settings read or wrote it
/// (`None`: there is none), no longer sets, as [`super::local::forget_hosts_not_in`] does for
/// hosts: a level taken out by hand and written back later is not confirmed again. A file that
/// is not TOML takes nothing off. Best effort.
pub fn forget_not_in(root: &Path, local: Option<&str>) {
    let file = match local {
        None => Personal::default(),
        Some(text) if text.parse::<toml::Table>().is_err() => return,
        Some(text) => of_text(text),
    };
    let mine = super::local::confirmed_levels(root);
    let kept = Personal {
        level: mine.level.filter(|_| file.level.is_some()),
        personas: mine
            .personas
            .iter()
            .filter(|(name, _)| file.personas.contains_key(*name))
            .map(|(name, choice)| (name.clone(), *choice))
            .collect(),
    };
    if kept != mine {
        let _ = super::local::change_levels(root, |held| *held = kept);
    }
}

#[cfg(test)]
mod tests;
