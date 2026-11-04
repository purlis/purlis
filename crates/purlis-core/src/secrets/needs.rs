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

/// **The Secret needs persona `persona` declares** in the project at `root`, **as committed**:
/// each definition down its `extends` chain read from the project's `HEAD`, in the order a
/// definition first names them. **Never the file on disk**, which a chat can write: a line a
/// chat added would lift policy's lock on undeclared keys and show the chat's own words as the
/// persona's (#5078). A line whose key is no key a Secret ask takes ([`is_key`]) is no need.
/// None for a persona with no committed definition, or a project that is no git repository.
pub fn of(root: &Path, persona: &str) -> Vec<Need> {
    // The chain as committed, child first, each name once.
    let mut chain: Vec<Vec<(String, String)>> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let mut next = Some(persona.to_owned());
    while let Some(name) = next.take() {
        if seen.contains(&name)
            || seen.len() >= MOST_ANCESTORS
            || !crate::personas::valid_name(&name)
        {
            break;
        }
        let Some(text) = committed_definition(root, &name) else {
            break;
        };
        let pairs = crate::personas::frontmatter(&text);
        next = crate::personas::meta(&pairs, "extends")
            .map(str::to_owned)
            .filter(|parent| !parent.is_empty());
        seen.push(name);
        chain.push(pairs);
    }
    let mut needs: Vec<Need> = Vec::new();
    for pairs in chain.into_iter().rev() {
        for (name, purpose) in pairs {
            let Some(key) = key_of(&name) else {
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

/// The longest `extends` chain read.
const MOST_ANCESTORS: usize = 16;

/// The most of a definition read from `HEAD`, in bytes.
const MOST_DEFINITION_BYTES: usize = 256 * 1024;

/// Persona `name`'s definition as the project's `HEAD` holds it: `personas/<name>/persona.md`,
/// else the legacy flat `personas/<name>.md`; none where neither is committed.
fn committed_definition(root: &Path, name: &str) -> Option<String> {
    [
        format!("personas/{name}/persona.md"),
        format!("personas/{name}.md"),
    ]
    .into_iter()
    .find_map(|rel| {
        crate::worktree::git::run(
            root,
            &["show", &format!("HEAD:{rel}")],
            crate::worktree::git::READ,
        )
        .ok()
        .filter(crate::worktree::git::Run::ok)
        .map(|run| run.out)
        .filter(|text| text.len() <= MOST_DEFINITION_BYTES)
    })
}

/// The need `persona` declares for `key`, where it declares one.
pub fn declared(root: &Path, persona: &str, key: &str) -> Option<Need> {
    of(root, persona).into_iter().find(|need| need.key == key)
}

/// **What a persona's definition reads with a need added**, as the person remembers it on a
/// Fill or from a vault's row (#5240, ADR 0110): `text` with a
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

/// The key a frontmatter line named `name` declares a need for, where it is a need's line and
/// a key a Secret ask takes.
fn key_of(name: &str) -> Option<&str> {
    name.strip_prefix(PREFIX).filter(|key| is_key(key))
}

/// The purpose a need is written with where none was said: the person edits it after.
pub const NO_PURPOSE_SAID: &str = "asked for by a chat; say what for";

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Commits everything in the project at `root`, a new git repository: what a need is read
    /// from.
    pub(crate) fn committed(root: &Path) {
        use crate::testgit::run;
        run(root, &["init", "-q", "-b", "main", "."]);
        run(root, &["config", "user.name", "Fixture"]);
        run(root, &["config", "user.email", "fixture@example.invalid"]);
        run(root, &["add", "-A"]);
        run(root, &["commit", "-q", "-m", "a project"]);
    }

    fn plane(personas: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("charter.toml"), "").unwrap();
        for (name, text) in personas {
            let at = dir.path().join("personas").join(name);
            std::fs::create_dir_all(&at).unwrap();
            std::fs::write(at.join("persona.md"), text).unwrap();
        }
        committed(dir.path());
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
    fn only_a_committed_line_declares_a_need() {
        let plane = plane(&[("ops", "---\nrole: Ops\nsecret-need.KEPT: kept\n---\n")]);
        std::fs::write(
            plane.path().join("personas/ops/persona.md"),
            "---\nrole: Ops\nsecret-need.KEPT: changed\nsecret-need.NEW: written by a chat\n---\n",
        )
        .unwrap();
        assert_eq!(
            of(plane.path(), "ops"),
            [Need {
                key: "KEPT".into(),
                purpose: "kept".into()
            }]
        );
        let bare = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(bare.path().join("personas/ops")).unwrap();
        std::fs::write(
            bare.path().join("personas/ops/persona.md"),
            "---\nsecret-need.K: p\n---\n",
        )
        .unwrap();
        assert!(of(bare.path(), "ops").is_empty(), "no repository, no need");
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
