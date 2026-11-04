// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Secret need offers the person dismissed** (#5159, ADR 0109): `app/secret-needs-dismissed.json`
//! in the project's state folder, on this machine only.
//!
//! After a first fill of a key a persona does not declare, the app offers to add it as a Secret
//! need ([`super::needs`]). Dismissing that offer is kept here as the persona's name and the
//! key's, so the same pair is not offered again on the next fill, **until the persona's Secret
//! needs change**: each dismissal keeps the keys the persona declared as committed when it was
//! made, and holds only while they are the same. The vault view's *Offer as a Secret need ▸*
//! lifts it. **Names only**: no value, no purpose, no vault, nothing about the fill.
//!
//! # It fails open
//!
//! A dismissal only quiets an offer, and an offer writes nothing until the person applies it.
//! So a file that does not read is read as no dismissals, and the worst that does is ask again.
//! **A write never replaces what it could not read**: [`dismiss`] and [`lift`] refuse, and
//! the bytes stay as they are.
//!
//! ```json
//! {"dismissed": [{"persona": "devops", "key": "STRIPE_KEY", "needs": ["GITHUB_TOKEN"]}]}
//! ```
//!
//! An entry with no `needs` holds whatever the persona declares.

use std::io;
use std::path::{Path, PathBuf};

/// The file, relative to the project's state folder ([`path`]).
pub const IN_STATE: &str = "app/secret-needs-dismissed.json";

/// The key that holds the pairs.
const KEY: &str = "dismissed";

/// The file in the project at `root`.
pub fn path(root: &Path) -> PathBuf {
    crate::names::state(root).join(IN_STATE)
}

/// One dismissal: the persona, the key, and the keys it declared then, sorted (`None` where the
/// entry does not say).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dismissal {
    pub persona: String,
    pub key: String,
    needs: Option<Vec<String>>,
}

impl Dismissal {
    fn is(&self, persona: &str, key: &str) -> bool {
        self.persona == persona && self.key == key
    }

    /// Whether it still holds, the persona now declaring `declared`.
    fn holds(&self, declared: &[String]) -> bool {
        self.needs
            .as_deref()
            .is_none_or(|then| then == sorted(declared))
    }

    fn json(&self) -> serde_json::Value {
        let mut entry = serde_json::json!({"persona": self.persona, "key": self.key});
        if let Some(needs) = &self.needs {
            entry["needs"] = needs.clone().into();
        }
        entry
    }
}

fn sorted(keys: &[String]) -> Vec<String> {
    let mut keys = keys.to_vec();
    keys.sort();
    keys.dedup();
    keys
}

/// The dismissals `text` holds, or `None` where it is not the shape above.
fn parsed(text: &str) -> Option<Vec<Dismissal>> {
    let serde_json::Value::Object(whole) = serde_json::from_str(text).ok()? else {
        return None;
    };
    let Some(list) = whole.get(KEY) else {
        return Some(Vec::new());
    };
    list.as_array()?
        .iter()
        .map(|one| {
            let needs = match one.get("needs") {
                None => None,
                Some(list) => Some(
                    list.as_array()?
                        .iter()
                        .map(|key| key.as_str().map(str::to_owned))
                        .collect::<Option<Vec<_>>>()?,
                ),
            };
            Some(Dismissal {
                persona: one.get("persona")?.as_str()?.to_owned(),
                key: one.get("key")?.as_str()?.to_owned(),
                needs: needs.map(|keys| sorted(&keys)),
            })
        })
        .collect()
}

/// What the file holds, or `None` where it is there and does not read.
fn on_disk(root: &Path) -> Option<Vec<Dismissal>> {
    match std::fs::read_to_string(path(root)) {
        Ok(text) => parsed(&text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Some(Vec::new()),
        Err(_) => None,
    }
}

/// **The dismissals the person made** in the project at `root`, in the order they were made,
/// whether or not they still hold. Empty where there is no file, or one that does not read.
pub fn read(root: &Path) -> Vec<Dismissal> {
    on_disk(root).unwrap_or_default()
}

/// Whether the offer of `key` for `persona` was dismissed, and still holds while the persona
/// declares `declared` (the keys of its committed Secret needs).
pub fn is_dismissed(root: &Path, persona: &str, key: &str, declared: &[String]) -> bool {
    read(root)
        .iter()
        .any(|one| one.is(persona, key) && one.holds(declared))
}

/// Rewrites the file with `how` done to its pairs; refused where it does not read.
fn change(root: &Path, how: impl FnOnce(&mut Vec<Dismissal>)) -> io::Result<()> {
    let unread = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} does not read; fix it or delete it, which dismisses nothing",
                path(root).display()
            ),
        )
    };
    let path = path(root);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::rewrite::update(root, &path, |now| {
        let mut kept = match now {
            None => Vec::new(),
            Some(text) => parsed(text).ok_or_else(unread)?,
        };
        let was = kept.clone();
        how(&mut kept);
        if kept == was {
            return Ok(None);
        }
        let list: Vec<serde_json::Value> = kept.iter().map(Dismissal::json).collect();
        serde_json::to_string_pretty(&serde_json::json!({ KEY: list }))
            .map(|text| Some(format!("{text}\n")))
            .map_err(io::Error::other)
    })
    .map(|_| ())
}

/// Records that the person dismissed the offer of each of `keys` for `persona`, while it
/// declares `declared`. A pair kept already is kept once, as of now.
pub fn dismiss(root: &Path, persona: &str, keys: &[String], declared: &[String]) -> io::Result<()> {
    if keys.is_empty() {
        return Ok(());
    }
    change(root, |kept| {
        kept.retain(|one| !(one.persona == persona && keys.contains(&one.key)));
        kept.extend(sorted(keys).into_iter().map(|key| Dismissal {
            persona: persona.to_owned(),
            key,
            needs: Some(sorted(declared)),
        }));
    })
}

/// Lifts the dismissal of `key` for `persona`: the person asked for the offer again. Nothing is
/// written where there is none.
pub fn lift(root: &Path, persona: &str, key: &str) -> io::Result<()> {
    change(root, |kept| kept.retain(|one| !one.is(persona, key)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn a_dismissal_holds_while_the_persona_declares_what_it_did_then() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        dismiss(root, "ops", &keys(&["K", "K"]), &keys(&["B", "A"])).unwrap();
        assert_eq!(read(root).len(), 1);
        assert!(is_dismissed(root, "ops", "K", &keys(&["A", "B"])));
        assert!(!is_dismissed(root, "dev", "K", &keys(&["A", "B"])));
        assert!(
            !is_dismissed(root, "ops", "K", &keys(&["A"])),
            "the persona's needs changed"
        );
        lift(root, "ops", "K").unwrap();
        assert!(!is_dismissed(root, "ops", "K", &keys(&["A", "B"])));
    }

    #[test]
    fn a_file_that_does_not_read_dismisses_nothing_and_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(path(root).parent().unwrap()).unwrap();
        let unread = "{\"dismissed\": [{\"persona\": 1}]}";
        std::fs::write(path(root), unread).unwrap();
        assert!(read(root).is_empty(), "read as no dismissals");
        assert!(!is_dismissed(root, "ops", "K", &[]));
        assert!(dismiss(root, "ops", &keys(&["L"]), &[]).is_err());
        assert_eq!(std::fs::read_to_string(path(root)).unwrap(), unread);
    }
}
