// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis policy check`** (T1a-64, R-432, H-11; design §3.16): what this build reads in a
//! policy file, before an administrator rolls it out. One line per fact, plain text a script can
//! read:
//!
//! 1. `file: <path> (…)`: the file read, and whether its owner and permissions were judged. With
//!    no path it is [`MACHINE_FILE`], judged as the reader judges it ([`installed`]); a draft at
//!    a path is read without that judgement, so a file can be checked before it is installed
//!    ([`draft`]).
//! 2. `not read: <why>`, and nothing more, where the file can't be read or parsed (exit 2).
//! 3. `lowest purlis version that reads this file: <version>`: the highest first release
//!    ([`FIRST_READ`]) among the file's keys this build knows, or *not in a stable release yet*,
//!    naming the keys no stable release reads.
//! 4. `not enforced by this version: <key>`, for each key this build parses and doesn't enforce:
//!    in 1a, every key inside `models`.
//! 5. `not known to this version: <key>`, for each key this build doesn't know.
//! 6. `chats on a key: …` and `chats on a sign-in: …`: whether this build refuses a chat that
//!    reaches a model under the file, in the words the window refuses it with (W36, W64, W66).
//!
//! **The exit status** ([`Checked::status`]): 0 when this build reads the file and enforces every
//! key in it; 1 when a key is unenforced or unknown, or `min-version` is above this build; 2 when
//! the file can't be read or parsed. who-judges R-272 adds the harness matrix to this output
//! later.

use std::path::Path;

use super::{
    DISPATCH_LOCKS, FIRST_READ, FirstRead, Judging, KEYS, Locks, MACHINE_FILE, TOP_KEYS, json,
    min_version, parsed,
};
use crate::keyroute::Route;
use crate::model_provider::Format;

/// What the check says, one line per fact, and the status it exits with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    /// The facts, in the order the module says.
    pub lines: Vec<String>,
    /// 0, 1 or 2, as the module says.
    pub status: u8,
}

/// **This machine's policy file**, [`MACHINE_FILE`], judged as the reader judges it: its trust
/// check, then the parse. In a test build, the policy fixture stands in for it (R-699).
pub fn installed() -> Checked {
    #[cfg(any(test, feature = "test-support"))]
    if let Some(fixture) = super::TEST_FIXTURE.with(|fixture| fixture.borrow().clone()) {
        let file = fixture.file.clone();
        return judged(
            &file,
            &Judging {
                fixture: Some(fixture),
            },
        );
    }
    judged(Path::new(MACHINE_FILE), &Judging::default())
}

fn judged(file: &Path, judging: &Judging) -> Checked {
    let first = format!("file: {} (judged as purlis reads it)", file.display());
    let text = match super::read_trusted(file, judging) {
        Ok(Some(text)) => Ok(text),
        Ok(None) if cfg!(unix) => Err("there is no file there".to_owned()),
        Ok(None) => Err("purlis reads no policy file on this system yet".to_owned()),
        Err(why) => Err(why),
    };
    checked(first, file, text)
}

/// **A draft at `file`**, read before it is installed: its owner and permissions, and its
/// folder's, are not judged, and the first line says so. The rest is the reader's: the 64 KiB
/// cap, UTF-8, and the parse.
pub fn draft(file: &Path) -> Checked {
    let first = format!(
        "file: {} (a draft: its owner and permissions are not judged)",
        file.display()
    );
    let text = match std::fs::File::open(file) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Err("there is no file there".to_owned())
        }
        Err(err) => Err(format!("it cannot be opened: {err}")),
        Ok(opened) => super::capped(opened),
    };
    checked(first, file, text)
}

/// The check of `text`, read from `file` (or why it wasn't), after its `first` line.
fn checked(first: String, file: &Path, text: Result<String, String>) -> Checked {
    let not_read = |why: String| Checked {
        lines: vec![first.clone(), format!("not read: {why}")],
        status: 2,
    };
    let text = match text {
        Ok(text) => text,
        Err(why) => return not_read(why),
    };
    // The reader's order: a JSON parse, then `min-version`, then the strict parse.
    let top = match json(&text) {
        Ok(top) => top,
        Err(why) => return not_read(why),
    };
    let min_version_above = match min_version(&top) {
        Ok(above) => above.is_some(),
        Err(why) => return not_read(why),
    };
    let keys = Keys::of(&top);
    // The strict parse of what this build knows: a key it doesn't know is said on a line of its
    // own, and what else refuses the file is said as the reader says it.
    if let Err(why) = parsed(&keys.known_only(&top), file) {
        return not_read(why);
    }

    let mut lines = vec![first, lowest(&keys.known)];
    lines.extend(
        keys.unenforced
            .iter()
            .map(|key| format!("not enforced by this version: {key}")),
    );
    lines.extend(
        keys.unknown
            .iter()
            .map(|key| format!("not known to this version: {key}")),
    );
    let locks = Locks::parse(&text, file);
    for (chats, route) in [
        (
            "chats on a key",
            Route::Proxied {
                format: Format::AnthropicMessages,
            },
        ),
        ("chats on a sign-in", Route::OwnSignIn),
    ] {
        lines.push(match locks.refuses_model_start(route) {
            Some(refusal) => format!("{chats}: refused: {refusal}"),
            None => format!("{chats}: not refused"),
        });
    }
    let status = if keys.unenforced.is_empty() && keys.unknown.is_empty() && !min_version_above {
        0
    } else {
        1
    };
    Checked { lines, status }
}

/// The keys a policy file says, as the check names them.
struct Keys {
    /// Each key this build knows, as [`FIRST_READ`] names it.
    known: Vec<String>,
    /// Each key inside `models`: parsed, and not enforced by this build.
    unenforced: Vec<String>,
    /// Each key this build doesn't know, as the check names it.
    unknown: Vec<String>,
    /// Where each of those is: the table it is in (`sandbox` or `dispatch`), or none at the top.
    unknown_at: Vec<(Option<String>, String)>,
}

impl Keys {
    fn of(top: &serde_json::Value) -> Self {
        let mut keys = Self {
            known: Vec::new(),
            unenforced: Vec::new(),
            unknown: Vec::new(),
            unknown_at: Vec::new(),
        };
        let Some(top) = top.as_object() else {
            return keys;
        };
        for (key, value) in top {
            if !TOP_KEYS.contains(&key.as_str()) {
                keys.unknown.push(key.clone());
                keys.unknown_at.push((None, key.clone()));
                continue;
            }
            keys.known.push(key.clone());
            let Some(inner) = value.as_object() else {
                continue;
            };
            for inner_key in inner.keys() {
                let named = format!("{key}.{inner_key}");
                match key.as_str() {
                    "models" => keys.unenforced.push(named),
                    "sandbox" | "dispatch" if known(&named) => keys.known.push(named),
                    "sandbox" | "dispatch" => {
                        keys.unknown.push(named);
                        keys.unknown_at.push((Some(key.clone()), inner_key.clone()));
                    }
                    _ => {}
                }
            }
        }
        keys
    }

    /// `top` without the keys this build doesn't know, for the strict parse.
    fn known_only(&self, top: &serde_json::Value) -> serde_json::Value {
        let mut top = top.clone();
        for (table, key) in &self.unknown_at {
            let holding = match table {
                Some(table) => top.get_mut(table.as_str()),
                None => Some(&mut top),
            };
            if let Some(holding) = holding.and_then(serde_json::Value::as_object_mut) {
                holding.remove(key.as_str());
            }
        }
        top
    }
}

/// Whether `key`, as the check names it, is one the parser takes.
fn known(key: &str) -> bool {
    match key.split_once('.') {
        Some(("sandbox", inner)) => KEYS.contains(&inner),
        Some(("dispatch", inner)) => {
            DISPATCH_LOCKS.contains(&inner)
                || crate::dispatchlimits::Limit::of_word(inner).is_some()
        }
        _ => TOP_KEYS.contains(&key),
    }
}

/// **The lowest purlis version that reads a file saying `known`**: the highest first release
/// among them, or, where any is in no stable release yet, that, naming those keys. A file that
/// says nothing is read by the first release that reads a policy file at all.
fn lowest(known: &[String]) -> String {
    let first = |key: &str| {
        FIRST_READ
            .iter()
            .find(|(listed, _)| *listed == key)
            .map(|(_, first)| *first)
    };
    let unreleased: Vec<&str> = FIRST_READ
        .iter()
        .filter(|(key, first)| *first == FirstRead::NotYet && known.iter().any(|said| said == key))
        .map(|(key, _)| *key)
        .collect();
    let lead = "lowest purlis version that reads this file";
    if !unreleased.is_empty() {
        return format!(
            "{lead}: not in a stable release yet ({})",
            unreleased.join(", ")
        );
    }
    let release = |first: Option<FirstRead>| match first {
        Some(FirstRead::Release(version)) => semver::Version::parse(version).ok(),
        _ => None,
    };
    let earliest = FIRST_READ
        .iter()
        .filter_map(|(_, first)| release(Some(*first)))
        .min();
    let version = known
        .iter()
        .filter_map(|key| release(first(key)))
        .max()
        .or(earliest)
        .map_or_else(String::new, |version| version.to_string());
    format!("{lead}: {version}")
}
