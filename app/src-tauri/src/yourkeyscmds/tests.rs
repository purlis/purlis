// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The key form's targets and W9's in-place replace (T1a-11; V7, U9, F-8, X2), over a config
//! home and the stub keyring, with synthetic values only.

use std::path::Path;

use purlis_core::seam::RealCredentials;
use purlis_core::secrets::keyring::{FileStore, STUB_FILE, Store as _};
use purlis_core::yourkeys::{self, SERVICE};

use super::*;

struct Here {
    config: tempfile::TempDir,
    credentials: RealCredentials,
}

impl Here {
    fn new() -> Self {
        let config = tempfile::tempdir().unwrap();
        let credentials = RealCredentials::new(Box::new(stub(config.path())));
        Self {
            config,
            credentials,
        }
    }
    fn root(&self) -> &Path {
        self.config.path()
    }
    fn save(&self, target: KeyTarget, value: &str) -> KeySaved {
        save(self.root(), &self.credentials, &NoneYet, &target, value).expect("the form saves")
    }
    fn ids(&self) -> Vec<Ulid> {
        yourkeys::list(self.root(), &self.credentials)
            .unwrap()
            .into_iter()
            .map(|l| l.entry.id)
            .collect()
    }
    fn value(&self, id: Ulid) -> Option<String> {
        stub(self.root())
            .get(SERVICE, &id.to_string())
            .unwrap()
            .map(purlis_core::secrets::keyring::Secret::into_inner)
    }
    fn marked(&self, id: Ulid) -> bool {
        mark::read_model_key(
            self.root(),
            &purlis_core::seam::RealFiles,
            &self.credentials,
            id,
        )
        .is_ok()
    }
    /// One keyless Claude Code profile on an Anthropic key, `name`.
    fn keyless(&self, name: &str) {
        let at = purlis_core::profilestore::path(self.root());
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(
            at,
            serde_json::json!({ "version": 1, "profiles": [{
                "name": name,
                "harness": "claude",
                "title": name,
                "folders": { "config": format!("/profiles/{name}/config") },
                "sign_in": {
                    "kind": "api-key",
                    "declaration": { "from": "built-in", "id": "anthropic" },
                },
            }]})
            .to_string(),
        )
        .unwrap();
    }
    fn key_of(&self, name: &str) -> Option<String> {
        purlis_core::profilestore::list(self.root())
            .unwrap()
            .into_iter()
            .find(|p| p.name == name)
            .and_then(|p| p.sign_in.and_then(|s| s.key().map(str::to_owned)))
    }
}

fn stub(config: &Path) -> FileStore {
    FileStore::at(config.join(STUB_FILE))
}

fn saved(said: KeySaved) -> Ulid {
    match said {
        KeySaved::Saved { id } => id.parse().unwrap(),
        other => panic!("the form did not save: {other:?}"),
    }
}

#[test]
fn the_key_form_takes_a_target() {
    // A new entry: one is created, holding the value, marked.
    let here = Here::new();
    let id = saved(here.save(KeyTarget::NewEntry, "synthetic-new"));
    assert_eq!(here.ids(), [id]);
    assert_eq!(here.value(id).as_deref(), Some("synthetic-new"));
    assert!(here.marked(id), "a saved key is marked");

    // This entry: the same id, the new value, still marked.
    let same = saved(here.save(
        KeyTarget::ThisEntry { id: id.to_string() },
        "synthetic-rotated",
    ));
    assert_eq!(same, id);
    assert_eq!(here.ids(), [id]);
    assert_eq!(here.value(id).as_deref(), Some("synthetic-rotated"));
    assert!(here.marked(id));

    // This entry, its item gone (Add a key… for a missing entry): written under the same id.
    stub(here.root()).delete(SERVICE, &id.to_string()).unwrap();
    let again = saved(here.save(
        KeyTarget::ThisEntry { id: id.to_string() },
        "synthetic-again",
    ));
    assert_eq!(again, id);
    assert_eq!(here.value(id).as_deref(), Some("synthetic-again"));
    assert!(here.marked(id), "written again and marked");

    // A key held: W9, with both its buttons, and nothing written.
    assert_eq!(
        here.save(KeyTarget::NewEntry, "synthetic-second"),
        KeySaved::Held {
            held: id.to_string(),
            may_replace: true
        }
    );
    assert_eq!(here.value(id).as_deref(), Some("synthetic-again"));

    // Aimed at a missing entry with another key held: only Use the key you already have (F-8).
    assert_eq!(
        here.save(
            KeyTarget::ThisEntry {
                id: Ulid::generate().to_string()
            },
            "synthetic-for-a-gone-entry",
        ),
        KeySaved::Held {
            held: id.to_string(),
            may_replace: false
        }
    );
    assert_eq!(here.ids(), [id]);

    // This profile: the saved entry's id is written into the keyless profile.
    let here = Here::new();
    here.keyless("claude-anthropic");
    let id = saved(here.save(
        KeyTarget::ThisProfile {
            name: "claude-anthropic".to_owned(),
        },
        "synthetic-for-a-profile",
    ));
    assert_eq!(here.key_of("claude-anthropic"), Some(id.to_string()));
    assert!(here.marked(id));

    // A profile that can't take a key: the save is undone, and nothing is left in your keys.
    let here = Here::new();
    assert!(
        save(
            here.root(),
            &here.credentials,
            &NoneYet,
            &KeyTarget::ThisProfile {
                name: "no-such-profile".to_owned()
            },
            "synthetic-undone",
        )
        .is_err()
    );
    assert!(
        here.ids().is_empty(),
        "a failed save takes out what it wrote"
    );
}

#[test]
fn w9_replace_replaces_in_place_with_the_value_in_hand() {
    let here = Here::new();
    let held = saved(here.save(KeyTarget::NewEntry, "synthetic-held"));
    here.keyless("claude-anthropic");
    let KeySaved::Held { held: offered, .. } = here.save(
        KeyTarget::ThisProfile {
            name: "claude-anthropic".to_owned(),
        },
        "synthetic-in-hand",
    ) else {
        panic!("W9 was not offered")
    };
    assert_eq!(offered, held.to_string());
    // W5's count of the chats on the old value comes first.
    assert_eq!(page::chats_using(here.root(), held, &[]), Ok(Vec::new()));

    // Pressing Replace the key… with the value still in hand: no second paste.
    assert_eq!(
        replace_held(
            here.root(),
            &here.credentials,
            &NoneYet,
            held,
            "synthetic-in-hand",
            Some("claude-anthropic"),
        ),
        Ok(KeySaved::Saved {
            id: held.to_string()
        })
    );
    assert_eq!(
        here.ids(),
        [held],
        "replaced in place, never a second entry"
    );
    assert_eq!(here.value(held).as_deref(), Some("synthetic-in-hand"));
    assert!(here.marked(held));
    assert_eq!(
        here.key_of("claude-anthropic"),
        Some(held.to_string()),
        "the save is finished: the profile points at the held entry"
    );
}

/// Pool key allowances holding one pool for every entry, recording each reseal (X1).
#[derive(Default)]
struct OnePool {
    resealed: std::sync::Mutex<Vec<Ulid>>,
}

impl pools::Allowances for OnePool {
    fn pools(&self, _entry: Ulid) -> Vec<String> {
        vec!["gpu".to_owned()]
    }
    fn revoke(&self, _entry: Ulid, _pool: &str) -> Result<(), String> {
        Ok(())
    }
    fn remove_all(&self, _entry: Ulid) -> Result<(), String> {
        Ok(())
    }
    fn reseal(&self, entry: Ulid) -> Result<(), String> {
        self.resealed.lock().unwrap().push(entry);
        Ok(())
    }
}

#[test]
fn a_replace_that_comes_back_unmarked_reseals_the_pools_after_marking_again() {
    // This entry, its item gone: Replace answers Unmarked, the form marks it again, and its
    // one pool is sealed to the new value too, so the rotated key keeps it (X1).
    let here = Here::new();
    let id = saved(here.save(KeyTarget::NewEntry, "synthetic-first"));
    stub(here.root()).delete(SERVICE, &id.to_string()).unwrap();
    let allowances = OnePool::default();
    assert_eq!(
        save(
            here.root(),
            &here.credentials,
            &allowances,
            &KeyTarget::ThisEntry { id: id.to_string() },
            "synthetic-written-again",
        ),
        Ok(KeySaved::Saved { id: id.to_string() })
    );
    assert!(here.marked(id));
    assert_eq!(*allowances.resealed.lock().unwrap(), [id], "the save's pool");

    // W9's Replace the key… on a held entry whose item is gone: the same.
    stub(here.root()).delete(SERVICE, &id.to_string()).unwrap();
    let allowances = OnePool::default();
    assert_eq!(
        replace_held(
            here.root(),
            &here.credentials,
            &allowances,
            id,
            "synthetic-in-hand",
            None,
        ),
        Ok(KeySaved::Saved { id: id.to_string() })
    );
    assert!(here.marked(id));
    assert_eq!(*allowances.resealed.lock().unwrap(), [id], "W9's pool");
}
