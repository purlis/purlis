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
}
