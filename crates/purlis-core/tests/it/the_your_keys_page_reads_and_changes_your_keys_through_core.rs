// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The *Your keys* page reads and changes your keys through core** (T1a-11, #6756; R-171,
//! R-578).
//!
//! The page lists what core answers: each entry with the origins its mark binds it to, its
//! approval setting, the profiles and chats that use it and the pools it is sent to; *Replace*
//! re-seals the mark and the entry's pool key allowances in one command; *Remove* deletes the
//! item, the mark, the allowances, then the index line, so a failure part-way leaves a line the
//! page shows (W55) and never an item nothing lists (H-14). The allowance store and the runner
//! link are T1a-53's and T1a-74's, reached here through recording seams.
//!
//! Every test runs against the fenced stub keyring behind the recording seam (T1a-69), with
//! synthetic values only.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use purlis_core::seam::{Kind, RealCredentials, Record, RecordingCredentials};
use purlis_core::secrets::VaultError;
use purlis_core::secrets::keyring::{FileStore, Held, Presence, STUB_FILE, Secret, Store};
use purlis_core::yourkeys::mark::{self, Binding};
use purlis_core::yourkeys::page::{self, ChatUse, Row, State};
use purlis_core::yourkeys::pools::{self, Allowances, Link, NoneYet};
use purlis_core::yourkeys::{
    self, Approval, Checked, Product, Replaced, SERVICE, Saved, add, remove, replace,
};
use ulid::Ulid;

/// A config home and the stub keyring beside it, behind a recording seam.
struct Here {
    config: tempfile::TempDir,
    record: Record,
    credentials: RecordingCredentials,
}

impl Here {
    fn new() -> Self {
        let config = tempfile::tempdir().unwrap();
        let record = Record::default();
        let credentials = RecordingCredentials::new(Box::new(stub(config.path())), record.clone());
        Self {
            config,
            record,
            credentials,
        }
    }

    fn root(&self) -> &Path {
        self.config.path()
    }

    /// Adds an Anthropic key through the window's write, marks it, and returns its entry id.
    fn marked(&self, value: &str) -> Ulid {
        let id = match add(
            self.root(),
            &self.credentials,
            Product::anthropic(),
            Checked::Passed,
            value,
        ) {
            Ok(Saved::Added(id)) => id,
            other => panic!("the key was not added: {other:?}"),
        };
        mark::mark(self.root(), &self.credentials, id, &anthropic())
            .unwrap_or_else(|why| panic!("the key was not marked: {why}"));
        id
    }

    fn rows(&self, chats: &[ChatUse], allowances: &dyn Allowances) -> Vec<Row> {
        page::rows(self.root(), &self.credentials, chats, allowances).expect("the page reads")
    }

    fn row(&self, id: Ulid) -> Option<Row> {
        self.rows(&[], &NoneYet).into_iter().find(|row| row.id == id)
    }

    fn item(&self, id: Ulid) -> Option<String> {
        stub(self.root())
            .get(SERVICE, &id.to_string())
            .unwrap()
            .map(Secret::into_inner)
    }

    fn line(&self, id: Ulid) -> bool {
        yourkeys::list(self.root(), &RealCredentials::new(Box::new(stub(self.root()))))
            .unwrap()
            .iter()
            .any(|listed| listed.entry.id == id)
    }

    fn seal(&self, id: Ulid) -> PathBuf {
        mark::path(self.root(), id)
    }

    /// Writes `profiles.json` with one Claude Code profile per `(name, entry)`.
    fn profiles(&self, named: &[(&str, Option<Ulid>)]) {
        let records: Vec<serde_json::Value> = named
            .iter()
            .map(|(name, entry)| {
                let mut sign_in = serde_json::json!({
                    "kind": "api-key",
                    "declaration": { "from": "built-in", "id": "anthropic" },
                });
                if let Some(entry) = entry {
                    sign_in["key"] = serde_json::json!(entry.to_string());
                }
                serde_json::json!({
                    "name": name,
                    "harness": "claude",
                    "title": name,
                    "folders": { "config": format!("/profiles/{name}/config") },
                    "sign_in": sign_in,
                })
            })
            .collect();
        let at = purlis_core::profilestore::path(self.root());
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(
            at,
            serde_json::to_string(&serde_json::json!({ "version": 1, "profiles": records }))
                .unwrap(),
        )
        .unwrap();
    }
}

/// The stub keyring every test build uses, straight, with no seam in front of it.
fn stub(config: &Path) -> FileStore {
    FileStore::at(config.join(STUB_FILE))
}

/// The Anthropic built-in's binding: its reference and its origins.
fn anthropic() -> Binding {
    let declaration = purlis_core::model_provider::built_ins()
        .iter()
        .find(|d| d.id == "anthropic")
        .expect("the Anthropic built-in ships");
    Binding::of(declaration)
}

fn chat(name: &str, profile: &str, pool: Option<&str>) -> ChatUse {
    ChatUse {
        name: name.to_owned(),
        profile: profile.to_owned(),
        pool: pool.map(str::to_owned),
    }
}

/// One call made of a recording seam.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Revoke(Ulid, String),
    RemoveAll(Ulid),
    Reseal(Ulid),
    Removal(String, Ulid),
}

/// A pool key allowance store holding rows in memory, recording each call, and failing where
/// told; with what the store looked like when its rows were all removed.
#[derive(Default, Clone)]
struct Recording {
    rows: Arc<Mutex<Vec<(Ulid, String)>>>,
    calls: Arc<Mutex<Vec<Call>>>,
    fails: Arc<Mutex<BTreeSet<&'static str>>>,
    seen_at_remove_all: Arc<Mutex<Option<Box<dyn Fn() -> String + Send + Sync>>>>,
    said_at_remove_all: Arc<Mutex<Option<String>>>,
}

impl Recording {
    fn holding(rows: &[(Ulid, &str)]) -> Self {
        let held = Self::default();
        *held.rows.lock().unwrap() = rows.iter().map(|(e, p)| (*e, (*p).to_owned())).collect();
        held
    }
    fn failing(self, what: &'static str) -> Self {
        self.fails.lock().unwrap().insert(what);
        self
    }
    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
    fn fails(&self, what: &str) -> Result<(), String> {
        if self.fails.lock().unwrap().contains(what) {
            Err(format!("the stand-in store refused {what}"))
        } else {
            Ok(())
        }
    }
}

impl Allowances for Recording {
    fn pools(&self, entry: Ulid) -> Vec<String> {
        self.rows
            .lock()
            .unwrap()
            .iter()
            .filter(|(e, _)| *e == entry)
            .map(|(_, p)| p.clone())
            .collect()
    }
    fn revoke(&self, entry: Ulid, pool: &str) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(Call::Revoke(entry, pool.to_owned()));
        self.fails("revoke")?;
        self.rows
            .lock()
            .unwrap()
            .retain(|(e, p)| !(*e == entry && p == pool));
        Ok(())
    }
    fn remove_all(&self, entry: Ulid) -> Result<(), String> {
        self.calls.lock().unwrap().push(Call::RemoveAll(entry));
        if let Some(look) = self.seen_at_remove_all.lock().unwrap().as_ref() {
            *self.said_at_remove_all.lock().unwrap() = Some(look());
        }
        self.fails("remove_all")?;
        self.rows.lock().unwrap().retain(|(e, _)| *e != entry);
        Ok(())
    }
    fn reseal(&self, entry: Ulid) -> Result<(), String> {
        self.calls.lock().unwrap().push(Call::Reseal(entry));
        self.fails("reseal")
    }
}

impl Link for Recording {
    fn removal(&self, pool: &str, entry: Ulid) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(Call::Removal(pool.to_owned(), entry));
        self.fails("removal")
    }
}

// ---------------------------------------------------------------------------------------------
// The list (R-171)

#[test]
fn the_keys_page_lists_each_entry_with_its_users() {
    let here = Here::new();
    let id = here.marked("synthetic-listed-value");
    here.profiles(&[
        ("claude-anthropic", Some(id)),
        ("claude-work", Some(id)),
        ("claude-keyless", None),
    ]);
    let chats = [
        chat("Fix the parser", "claude-anthropic", None),
        chat("Write the docs", "claude-work", Some("gpu")),
        chat("Plan the week", "claude-keyless", None),
        chat("Shell", "claude", None),
    ];

    let rows = here.rows(&chats, &NoneYet);

    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!(row.id, id);
    assert_eq!(row.product, Product::anthropic());
    assert_eq!(
        row.origins,
        anthropic().origins.into_iter().collect::<Vec<_>>(),
        "the origins its mark binds it to"
    );
    assert!(!row.origins.is_empty());
    assert_eq!(row.approval, Approval::Off);
    assert_eq!(row.checked, Checked::Passed);
    assert_eq!(row.state, State::Marked);
    assert_eq!(row.profiles, ["claude-anthropic", "claude-work"]);
    assert_eq!(row.chats, ["Fix the parser", "Write the docs"]);
    assert!(row.pools.is_empty(), "no allowance store yet lists no pool");
}

#[test]
fn the_keys_page_says_each_rows_state() {
    let here = Here::new();
    let marked = here.marked("synthetic-marked");
    // An orphaned line: the item is gone, the line stays (W55).
    let missing = {
        let other = add(
            here.root(),
            &here.credentials,
            Product {
                from: "yours".to_owned(),
                id: "other".to_owned(),
            },
            Checked::NotCheckedYet,
            "synthetic-orphan",
        );
        let Ok(Saved::Added(id)) = other else {
            panic!("{other:?}")
        };
        stub(here.root()).delete(SERVICE, &id.to_string()).unwrap();
        id
    };
    // Changed outside purlis (W60).
    here.credentials
        .write_outside(SERVICE, &marked.to_string(), "synthetic-outside")
        .unwrap();

    let rows = here.rows(&[], &NoneYet);
    let state = |id: Ulid| rows.iter().find(|r| r.id == id).map(|r| r.state.clone());
    assert_eq!(state(marked), Some(State::Unmarked));
    assert_eq!(state(missing), Some(State::Missing));
    assert_eq!(
        rows.iter()
            .find(|r| r.id == missing)
            .map(|r| r.checked),
        Some(Checked::NotCheckedYet)
    );
}

/// A keychain that answers every read with an error, as a locked one does.
struct Locked(FileStore);

impl Store for Locked {
    fn get(&self, _: &str, _: &str) -> Result<Option<Secret>, VaultError> {
        Err(VaultError::new("the keychain is locked"))
    }
    fn set(&self, s: &str, a: &str, v: &str) -> Result<Held, VaultError> {
        self.0.set(s, a, v)
    }
    fn delete(&self, s: &str, a: &str) -> Result<bool, VaultError> {
        self.0.delete(s, a)
    }
    fn exists(&self, _: &str, _: &str) -> Presence {
        Presence::Unknown
    }
}

#[test]
fn a_locked_keychain_lists_its_rows_as_unreadable_and_unlock_says_so() {
    let here = Here::new();
    let id = here.marked("synthetic-locked");
    let locked = RecordingCredentials::new(Box::new(Locked(stub(here.root()))), Record::default());

    let rows = page::rows(here.root(), &locked, &[], &NoneYet).unwrap();
    assert!(
        matches!(&rows[0].state, State::Locked { why } if why.contains("locked")),
        "{rows:?}"
    );
    assert!(page::unlock(here.root(), &locked, id).is_err(), "W84");
    assert_eq!(page::unlock(here.root(), &here.credentials, id), Ok(true));
}

// ---------------------------------------------------------------------------------------------
// Replace and Remove (R-171)

#[test]
fn replace_and_remove_update_the_store() {
    let here = Here::new();
    let id = here.marked("synthetic-before");
    let allowances = Recording::holding(&[(id, "gpu")]);

    // Replace: the value, the mark and the allowances, in one command, and the check's answer.
    assert_eq!(
        replace(
            here.root(),
            &here.credentials,
            &allowances,
            id,
            "synthetic-after",
            Checked::NotCheckedYet,
        ),
        Ok(Replaced::Marked)
    );
    assert_eq!(here.item(id).as_deref(), Some("synthetic-after"));
    assert_eq!(here.row(id).map(|r| r.state), Some(State::Marked));
    assert_eq!(allowances.calls(), [Call::Reseal(id)]);
    assert_eq!(
        here.row(id).map(|r| r.checked),
        Some(Checked::NotCheckedYet),
        "Replace updates what the check found"
    );

    // Remove: the item, the mark, the allowances, then the line, in that order.
    let root = here.root().to_path_buf();
    *allowances.seen_at_remove_all.lock().unwrap() = Some(Box::new(move || {
        format!(
            "item {}, mark {}, line {}",
            stub(&root).get(SERVICE, &id.to_string()).unwrap().is_some(),
            mark::path(&root, id).exists(),
            yourkeys::list(&root, &RealCredentials::new(Box::new(stub(&root))))
                .unwrap()
                .iter()
                .any(|l| l.entry.id == id)
        )
    }));
    here.record.clear();
    assert_eq!(
        remove(here.root(), &here.credentials, &allowances, id),
        Ok(())
    );
    assert_eq!(
        allowances.said_at_remove_all.lock().unwrap().as_deref(),
        Some("item false, mark false, line true"),
        "the allowances go after the item and the mark, and before the line"
    );
    assert_eq!(here.record.count(Kind::CredentialDelete), 1);
    assert_eq!(here.item(id), None);
    assert!(!here.seal(id).exists());
    assert!(!here.line(id));
    assert!(allowances.pools(id).is_empty());
    assert_eq!(here.row(id), None);
}

/// A keychain whose deletes fail.
struct Undeleting(FileStore);

impl Store for Undeleting {
    fn get(&self, s: &str, a: &str) -> Result<Option<Secret>, VaultError> {
        self.0.get(s, a)
    }
    fn set(&self, s: &str, a: &str, v: &str) -> Result<Held, VaultError> {
        self.0.set(s, a, v)
    }
    fn delete(&self, _: &str, _: &str) -> Result<bool, VaultError> {
        Err(VaultError::new("the keychain refused the delete"))
    }
    fn exists(&self, s: &str, a: &str) -> Presence {
        self.0.exists(s, a)
    }
}

#[test]
fn remove_fails_safely_after_each_step() {
    use std::os::unix::fs::PermissionsExt as _;

    // 1. The item's delete fails: the whole entry stays.
    let here = Here::new();
    let id = here.marked("synthetic-step-one");
    let undeleting =
        RecordingCredentials::new(Box::new(Undeleting(stub(here.root()))), Record::default());
    let said = remove(here.root(), &undeleting, &NoneYet, id);
    assert!(said.as_ref().is_err_and(|e| e.contains("refused the delete")), "{said:?}");
    assert_eq!(here.row(id).map(|r| r.state), Some(State::Marked));

    // 2. The mark's delete fails: the item is gone, the line stays, shown as W55.
    let here = Here::new();
    let id = here.marked("synthetic-step-two");
    std::fs::remove_file(here.seal(id)).unwrap();
    std::fs::create_dir_all(here.seal(id).join("held")).unwrap();
    assert!(remove(here.root(), &here.credentials, &NoneYet, id).is_err());
    assert_eq!(here.item(id), None);
    assert_eq!(here.row(id).map(|r| r.state), Some(State::Missing));

    // 3. The allowances' delete fails: the same.
    let here = Here::new();
    let id = here.marked("synthetic-step-three");
    let allowances = Recording::holding(&[(id, "gpu")]).failing("remove_all");
    let said = remove(here.root(), &here.credentials, &allowances, id);
    assert!(said.as_ref().is_err_and(|e| e.contains("refused remove_all")), "{said:?}");
    assert_eq!(here.item(id), None);
    assert!(!here.seal(id).exists());
    assert_eq!(here.row(id).map(|r| r.state), Some(State::Missing));

    // 4. The line's write fails: the same.
    let here = Here::new();
    let id = here.marked("synthetic-step-four");
    let folder = yourkeys::index_path(here.root())
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o500)).unwrap();
    let said = remove(here.root(), &here.credentials, &NoneYet, id);
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(said.is_err(), "{said:?}");
    assert_eq!(here.item(id), None);
    assert_eq!(here.row(id).map(|r| r.state), Some(State::Missing));
}

#[test]
fn remove_counts_the_chats_that_use_the_entry() {
    let here = Here::new();
    let id = here.marked("synthetic-counted");
    here.profiles(&[("claude-anthropic", Some(id)), ("claude-keyless", None)]);
    let chats = [
        chat("Fix the parser", "claude-anthropic", None),
        chat("Run the tests", "claude-anthropic", Some("gpu")),
        chat("Plan the week", "claude-keyless", None),
    ];

    assert_eq!(
        page::chats_using(here.root(), id, &chats),
        ["Fix the parser", "Run the tests"]
    );
    assert!(page::chats_using(here.root(), id, &chats[2..]).is_empty());
    assert_eq!(
        page::chats_using(here.root(), Ulid::generate(), &chats),
        Vec::<String>::new()
    );
}

// ---------------------------------------------------------------------------------------------
// The pools (R-171, X1)

#[test]
fn the_keys_page_lists_the_pools_a_key_is_sent_to() {
    let here = Here::new();
    let id = here.marked("synthetic-pooled");
    let other = Ulid::generate();
    let allowances = Recording::holding(&[(id, "gpu"), (other, "cpu"), (id, "lab")]);

    let rows = here.rows(&[], &allowances);

    assert_eq!(rows[0].pools, ["gpu", "lab"]);
}

#[test]
fn stop_sending_deletes_that_pools_row() {
    let here = Here::new();
    let id = here.marked("synthetic-stopped");
    here.profiles(&[("claude-anthropic", Some(id))]);
    let allowances = Recording::holding(&[(id, "gpu"), (id, "lab")]);
    let chats = [
        chat("On gpu", "claude-anthropic", Some("gpu")),
        chat("On lab", "claude-anthropic", Some("lab")),
        chat("Here", "claude-anthropic", None),
    ];
    assert_eq!(page::chats_on_pool(here.root(), id, "gpu", &chats), ["On gpu"]);

    assert_eq!(pools::stop_sending(id, "gpu", &allowances, &allowances), Ok(()));

    assert_eq!(allowances.pools(id), ["lab"]);
    assert_eq!(
        allowances.calls(),
        [
            Call::Revoke(id, "gpu".to_owned()),
            Call::Removal("gpu".to_owned(), id)
        ],
        "the row goes, then the pool's runs are handed the removal"
    );

    // A revoke that fails keeps the row, hands nothing to the link, and says why (W129).
    let failing = Recording::holding(&[(id, "lab")]).failing("revoke");
    let said = pools::stop_sending(id, "lab", &failing, &failing);
    assert!(said.as_ref().is_err_and(|e| e.contains("refused revoke")), "{said:?}");
    assert_eq!(failing.pools(id), ["lab"]);
    assert_eq!(failing.calls(), [Call::Revoke(id, "lab".to_owned())]);
}

#[test]
fn the_rows_try_again_records_what_the_check_found() {
    let here = Here::new();
    let id = here.marked("synthetic-checked");
    assert_eq!(
        yourkeys::set_checked(here.root(), id, Checked::RefusedAtSave),
        Ok(())
    );
    assert_eq!(here.row(id).map(|r| r.checked), Some(Checked::RefusedAtSave));
    assert!(yourkeys::set_checked(here.root(), Ulid::generate(), Checked::Passed).is_err());
}
