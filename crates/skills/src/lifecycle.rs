// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A skill's lifecycle state and its owners** (SK-19, #2782; spec #2762 decisions 62 and 63),
//! kept beside its folder in `<folder>.toml` with its grant (ADR 0099 §1), never inside
//! `SKILL.md`.
//!
//! - **Deprecated** (`lifecycle = "deprecated"`, and optionally `replacement = "<name>"`): the
//!   skill still works and reaches every chat it reached. The set marks it
//!   ([`crate::Effective::deprecated`]) so a router lists it after the rest and names its
//!   replacement (ADR 0102 §2); the studio shows it after the others.
//! - **Archived** (`lifecycle = "archived"`): the skill never loads. The set leaves it out,
//!   "archived" ([`crate::Reason::Archived`]), and it takes no name from a wider copy. Its folder
//!   stays where it is and in the project's history, so it is restored by making it active again.
//! - **Owners** (`owners = ["…"]`): who looks after the skill. Informational: on an organisation's
//!   project a change by someone else will go through a review once the policy layer lands
//!   (SD-14, #697).
//!
//! The file is read as TOML and only the keys here are taken from it, each checked against an
//! allow-list of values; anything else in it is kept as it is by a write.

use std::path::Path;

/// The key that holds a skill's lifecycle state.
pub const LIFECYCLE: &str = "lifecycle";

/// The key that names a deprecated skill's replacement.
pub const REPLACEMENT: &str = "replacement";

/// The key that holds a skill's owners.
pub const OWNERS: &str = "owners";

/// The most owners a skill names.
pub const MOST_OWNERS: usize = 16;

/// The longest owner's name, in characters.
pub const MOST_OWNER: usize = 64;

/// The most bytes of a `<folder>.toml` purlis reads: it holds a few short keys.
pub const MOST_BESIDE: u64 = 64 * 1024;

/// What a deprecated skill says: the skill to use instead, where it names one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deprecated {
    pub replacement: Option<String>,
}

/// A skill's lifecycle state (decision 62).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Lifecycle {
    /// In use: the default, and what a missing file or key says.
    #[default]
    Active,
    /// Still works, points to a replacement, and is shown after the rest.
    Deprecated(Deprecated),
    /// Never loads, and is kept to be restored.
    Archived,
}

impl Lifecycle {
    /// The word the file holds: `active`, `deprecated` or `archived`.
    pub fn word(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Deprecated(_) => "deprecated",
            Self::Archived => "archived",
        }
    }
}

/// What purlis knows about a skill beside its grant: its lifecycle state and its owners.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct About {
    pub lifecycle: Lifecycle,
    pub owners: Vec<String>,
}

/// The table in the file beside `folder`, read without following a link, or `None` where there
/// is no such file; why not, as the end of a sentence, where it cannot be read.
pub(crate) fn beside_table(folder: &Path) -> Result<Option<toml::Table>, String> {
    let file = crate::beside(folder);
    if std::fs::symlink_metadata(&file).is_err_and(|why| why.kind() == std::io::ErrorKind::NotFound)
    {
        return Ok(None);
    }
    let text = crate::handed::read_plain(&file, MOST_BESIDE).map_err(|why| match why {
        crate::handed::NotRead::Unreadable(why) => format!("it cannot be read ({why})"),
        crate::handed::NotRead::NotPlain => "it is not a plain file".to_owned(),
        crate::handed::NotRead::TooLarge => {
            format!("it is larger than {} KiB", MOST_BESIDE / 1024)
        }
    })?;
    text.parse::<toml::Table>().map(Some).map_err(|why| {
        format!(
            "it is not TOML: {}",
            crate::grant::first_line(&why.to_string())
        )
    })
}

/// **What purlis knows about the skill in `folder`**: active with no owners where
/// `<folder>.toml` is not there or names neither, or why it cannot be read. A reader treats an
/// error as active with no owners: the skill keeps working, and the studio says why.
pub fn about_of(folder: &Path) -> Result<About, String> {
    let Some(table) = beside_table(folder)? else {
        return Ok(About::default());
    };
    Ok(About {
        lifecycle: lifecycle_in(&table)?,
        owners: owners_in(&table)?,
    })
}

/// Whether the skill in `folder` is archived. A file that cannot be read archives nothing.
pub fn archived(folder: &Path) -> bool {
    about_of(folder).is_ok_and(|about| about.lifecycle == Lifecycle::Archived)
}

/// The lifecycle state `table` holds, or why it holds none purlis knows.
fn lifecycle_in(table: &toml::Table) -> Result<Lifecycle, String> {
    let replacement = match table.get(REPLACEMENT) {
        None => None,
        Some(toml::Value::String(name)) if crate::change::name_problem(name).is_none() => {
            Some(name.clone())
        }
        Some(other) => {
            return Err(format!(
                "{REPLACEMENT} is {other}, which is no skill's name"
            ));
        }
    };
    match table.get(LIFECYCLE) {
        None => Ok(Lifecycle::Active),
        Some(toml::Value::String(word)) => match word.as_str() {
            "active" => Ok(Lifecycle::Active),
            "deprecated" => Ok(Lifecycle::Deprecated(Deprecated { replacement })),
            "archived" => Ok(Lifecycle::Archived),
            _ => Err(format!(
                "{LIFECYCLE} is {word:?}, which is none of \"active\", \"deprecated\" or \
                 \"archived\""
            )),
        },
        Some(other) => Err(format!("{LIFECYCLE} is {other}, which is no word")),
    }
}

/// The owners `table` names, or why they are not a list of names.
fn owners_in(table: &toml::Table) -> Result<Vec<String>, String> {
    let Some(value) = table.get(OWNERS) else {
        return Ok(Vec::new());
    };
    let toml::Value::Array(items) = value else {
        return Err(format!("{OWNERS} is {value}, which is not a list of names"));
    };
    if items.len() > MOST_OWNERS {
        return Err(format!("{OWNERS} names more than {MOST_OWNERS}"));
    }
    items
        .iter()
        .map(|item| match item.as_str() {
            Some(name) if owner_problem(name).is_none() => Ok(name.to_owned()),
            _ => Err(format!("{OWNERS} holds {item}, which is no owner's name")),
        })
        .collect()
}

/// What is wrong with `owner` as an owner's name, as the end of a sentence, or `None` where it is
/// one: 1 to 64 letters, digits and `.`, `_`, `-`, `+` or `@`, as a forge login or an email is.
pub fn owner_problem(owner: &str) -> Option<String> {
    if owner.is_empty() {
        return Some("it is empty".to_owned());
    }
    if owner.chars().count() > MOST_OWNER {
        return Some(format!("it is longer than {MOST_OWNER} characters"));
    }
    owner
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || "._-+@".contains(*c)))
        .map(|odd| {
            format!(
                "it holds {odd:?}, and an owner is a login or an email: letters, digits and . _ \
                 - + @"
            )
        })
}

/// **Writes `lifecycle` as the lifecycle state of the skill in `folder`**, keeping every other
/// key of `<folder>.toml`. Active, the default, removes both keys; a deprecated skill with no
/// replacement keeps none. Refused, and nothing written, where the file is not TOML.
pub fn set_lifecycle(folder: &Path, lifecycle: &Lifecycle) -> Result<(), String> {
    crate::grant::edit_beside(folder, |doc| {
        doc.remove(REPLACEMENT);
        match lifecycle {
            Lifecycle::Active => {
                doc.remove(LIFECYCLE);
            }
            Lifecycle::Deprecated(Deprecated { replacement }) => {
                doc[LIFECYCLE] = toml_edit::value("deprecated");
                if let Some(name) = replacement {
                    doc[REPLACEMENT] = toml_edit::value(name.as_str());
                }
            }
            Lifecycle::Archived => doc[LIFECYCLE] = toml_edit::value("archived"),
        }
    })
}

/// **Writes `owners` as the owners of the skill in `folder`**, keeping every other key. None
/// removes the key. Refused, and nothing written, for a name that is no owner's or more than
/// [`MOST_OWNERS`].
pub fn set_owners(folder: &Path, owners: &[String]) -> Result<(), String> {
    if owners.len() > MOST_OWNERS {
        return Err(format!(
            "a skill names at most {MOST_OWNERS} owners, so nothing changed"
        ));
    }
    if let Some((owner, why)) = owners
        .iter()
        .find_map(|owner| owner_problem(owner).map(|why| (owner, why)))
    {
        return Err(format!(
            "{owner:?} cannot be an owner: {why}, so nothing changed"
        ));
    }
    crate::grant::edit_beside(folder, |doc| {
        if owners.is_empty() {
            doc.remove(OWNERS);
        } else {
            let list: toml_edit::Array = owners.iter().map(String::as_str).collect();
            doc[OWNERS] = toml_edit::value(list);
        }
    })
}

#[cfg(test)]
mod tests;
