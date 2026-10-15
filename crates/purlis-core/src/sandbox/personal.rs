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
//! a person's sandbox down. A chat never writes the file: it is a manifest the sandbox denies
//! (ADR 0067 §5).
//!
//! What a level does to a chat's start is the start's ([`super::decide`]); this module only
//! reads, validates and writes the choice.

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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Personal {
    /// `[sandbox] level`, where set.
    pub level: Option<Choice>,
    /// `[sandbox.personas]`, each persona's level.
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

/// `text`, this machine's file, with the level of `persona` (or of the project, for `None`) set
/// to `choice`, or taken out for `None`, every other line kept as it was.
pub fn with(text: &str, persona: Option<&str>, choice: Option<Choice>) -> Result<String, String> {
    if let Some(name) = persona
        && !crate::contain::segment_ok(name)
    {
        return Err(format!(
            "'{}' cannot be a persona",
            crate::shown::short(name)
        ));
    }
    let mut doc = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|why| format!("{} is not TOML: {why}", crate::profiles::LOCAL_FILE))?;
    let sandbox = doc
        .entry(super::TABLE)
        .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .ok_or_else(|| {
            format!(
                "[{}] in {} is not a table",
                super::TABLE,
                crate::profiles::LOCAL_FILE
            )
        })?;
    match persona {
        None => match choice {
            Some(choice) => {
                sandbox.insert(LEVEL, toml_edit::value(choice.word()));
            }
            None => {
                sandbox.remove(LEVEL);
            }
        },
        Some(name) => {
            let personas = sandbox
                .entry(PERSONAS)
                .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()))
                .as_table_mut()
                .ok_or_else(|| {
                    format!(
                        "[{}.{PERSONAS}] in {} is not a table",
                        super::TABLE,
                        crate::profiles::LOCAL_FILE
                    )
                })?;
            match choice {
                Some(choice) => {
                    personas.insert(name, toml_edit::value(choice.word()));
                }
                None => {
                    personas.remove(name);
                }
            }
            if personas.is_empty() {
                sandbox.remove(PERSONAS);
            }
        }
    }
    Ok(doc.to_string())
}

#[cfg(test)]
mod tests;
