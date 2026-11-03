// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A persona's Secret needs** (ADR 0109, spec #5054; #5072, #5076): the credentials its
//! committed definition says its work needs, each a key and one line saying what for.
//!
//! One frontmatter line a need, `secret-need.<KEY>: <purpose>` (`docs/plane-format.md`,
//! `personas/<name>/persona.md`): the line format holds no lists of pairs, and a line a key keeps
//! the change that adds one to a single line a person reads and approves. Inherited along
//! `extends`, a child's line for a key answering instead of its parent's.
//!
//! **The need is shared; the fill never is.** Nothing here names a value, a vault entry it
//! links to, or whether anyone filled it: only the key and why, which a teammate's first ask
//! shows them.

use std::path::Path;

/// What a need's frontmatter key starts with: the key follows it.
pub const PREFIX: &str = "secret-need.";

/// The longest key a Secret ask takes, in bytes.
pub const MOST_KEY_BYTES: usize = 128;

/// One Secret need: the key, and the definition's one line saying what it is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Need {
    pub key: String,
    pub purpose: String,
}

/// **Whether `key` is a name a Secret ask takes**: what an environment variable is named by, so
/// the brokered run can hand it over under its own name. ASCII letters, digits, `_`, `-` and
/// `.`, not starting with a digit, `-` or `.`, at most [`MOST_KEY_BYTES`].
pub fn is_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= MOST_KEY_BYTES
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        && key
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
}

/// **The Secret needs persona `persona` declares** in the project at `root`, down its `extends`
/// chain, in the order a definition first names them. A line whose key is no key a Secret ask
/// takes ([`is_key`]) is no need. None for a persona that does not load.
pub fn of(root: &Path, persona: &str) -> Vec<Need> {
    let mut needs: Vec<Need> = Vec::new();
    for ancestor in crate::personas::lineage(root, persona).iter().rev() {
        let Some(pairs) = crate::personas::load(root, ancestor) else {
            continue;
        };
        for (name, purpose) in pairs {
            let Some(key) = name.strip_prefix(PREFIX).filter(|key| is_key(key)) else {
                continue;
            };
            match needs.iter_mut().find(|need| need.key == key) {
                Some(need) => need.purpose = purpose,
                None => needs.push(Need {
                    key: key.to_owned(),
                    purpose,
                }),
            }
        }
    }
    needs
}

/// The need `persona` declares for `key`, where it declares one.
pub fn declared(root: &Path, persona: &str, key: &str) -> Option<Need> {
    of(root, persona).into_iter().find(|need| need.key == key)
}

/// **What a persona's definition reads with a need added**, as purlis proposes it after a
/// first fill of a key the persona does not declare (#5076): `text` with a
/// `secret-need.<key>: <purpose>` line, and a `vault: <vault>` line where the persona named
/// none. Every other line is left as written ([`purlis_skills::fence::with_keys`]). Refused
/// for a key no need takes, or a definition with no frontmatter to hold the lines.
pub fn with_need(
    text: &str,
    need: Option<(&str, &str)>,
    vault: Option<&str>,
) -> Result<String, String> {
    let mut keys: Vec<(String, Option<String>)> = Vec::new();
    if let Some((key, purpose)) = need {
        if !is_key(key) {
            return Err(format!("{key} is no key a Secret need takes"));
        }
        let purpose = crate::shown::one_line(purpose.trim(), super::ask::MOST_PURPOSE_CHARS);
        let purpose = if purpose.is_empty() {
            NO_PURPOSE_SAID.to_owned()
        } else {
            purpose
        };
        keys.push((format!("{PREFIX}{key}"), Some(purpose)));
    }
    if let Some(vault) = vault {
        keys.push(("vault".to_owned(), Some(vault.to_owned())));
    }
    let keys: Vec<(&str, Option<&str>)> = keys
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_deref()))
        .collect();
    purlis_skills::fence::with_keys(text, &keys).map_err(|why| format!("the definition {why}"))
}

/// The purpose a need is proposed with where the chat said none: the person edits it before
/// applying, or after.
pub const NO_PURPOSE_SAID: &str = "asked for by a chat; say what for";

#[cfg(test)]
mod tests {
    use super::*;

    fn plane(personas: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("charter.toml"), "").unwrap();
        for (name, text) in personas {
            let at = dir.path().join("personas").join(name);
            std::fs::create_dir_all(&at).unwrap();
            std::fs::write(at.join("persona.md"), text).unwrap();
        }
        dir
    }

    #[test]
    fn a_persona_declares_each_need_on_a_line_of_its_own_and_inherits_its_parents() {
        let plane = plane(&[
            (
                "base",
                "---\nrole: Base\nsecret-need.GITHUB_TOKEN: open pull requests\n\
                 secret-need.STRIPE_KEY: old words\n---\n",
            ),
            (
                "shop",
                "---\nrole: Shop\nextends: base\nsecret-need.STRIPE_KEY: charge test cards \
                 in checkout e2e\nsecret-need.9BAD: no key\nsecret-need.: nothing\n---\n",
            ),
        ]);
        assert_eq!(
            of(plane.path(), "shop"),
            [
                Need {
                    key: "GITHUB_TOKEN".into(),
                    purpose: "open pull requests".into()
                },
                Need {
                    key: "STRIPE_KEY".into(),
                    purpose: "charge test cards in checkout e2e".into()
                },
            ]
        );
        assert_eq!(
            declared(plane.path(), "shop", "GITHUB_TOKEN").map(|need| need.purpose),
            Some("open pull requests".into())
        );
        assert_eq!(declared(plane.path(), "shop", "OTHER"), None);
        assert!(of(plane.path(), "nobody").is_empty());
    }

    #[test]
    fn a_key_is_what_an_environment_variable_is_named_by() {
        for key in ["TOKEN", "_X", "stripe.key", "A-1"] {
            assert!(is_key(key), "{key}");
        }
        for key in [
            "",
            "1A",
            "-A",
            ".A",
            "A B",
            "A=B",
            "A/B",
            "Ä",
            &"A".repeat(129),
        ] {
            assert!(!is_key(key), "{key}");
        }
    }

    #[test]
    fn a_need_is_proposed_as_one_line_and_a_vault_where_the_persona_named_none() {
        let text = "---\nrole: Shop\nvault: none\n---\n\nThe charter.\n";
        let changed = with_need(
            text,
            Some(("STRIPE_KEY", "charge\ntest cards")),
            Some("shop"),
        )
        .expect("made");
        assert_eq!(
            changed,
            "---\nrole: Shop\nvault: shop\nsecret-need.STRIPE_KEY: charge⟦U+000A⟧test cards\n---\n\nThe charter.\n"
        );
        assert_eq!(
            with_need("---\nrole: Shop\n---\n", Some(("K", " ")), None).unwrap(),
            format!("---\nrole: Shop\nsecret-need.K: {NO_PURPOSE_SAID}\n---\n")
        );
        assert!(with_need("no frontmatter\n", Some(("K", "p")), None).is_err());
        assert!(with_need("---\n---\n", Some(("1K", "p")), None).is_err());
    }
}
