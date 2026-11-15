// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Your keys holds a keychain value and its index, written by the window alone** (T1a-09,
//! #6753; R-169, R-172, R-173, R-619, R-030, R-490, R-621).
//!
//! Every test runs against the fenced stub keyring behind the recording seam (T1a-69), with
//! synthetic values only: no test here reaches the operating system's keychain or a real key.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use purlis_core::seam::{Kind, Record, RecordingCredentials, RecordingFiles, Target};
use purlis_core::secrets::VaultError;
use purlis_core::secrets::keyring::{FileStore, Held, STUB_FILE, Secret, Store};
use purlis_core::window_copy::{fill, line};
use purlis_core::yourkeys::{
    self, Approval, Checked, Product, ReadError, SERVICE, Saved, add, list, read_key, set_approval,
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

    fn index(&self) -> PathBuf {
        yourkeys::index_path(self.root())
    }

    /// Adds an Anthropic key through the window's write and returns its entry id.
    fn added(&self, value: &str) -> Ulid {
        match add(
            self.root(),
            &self.credentials,
            Product::anthropic(),
            Checked::Passed,
            value,
        ) {
            Ok(Saved::Added(id)) => id,
            other => panic!("the key was not added: {other:?}"),
        }
    }

    fn read(&self, id: Ulid) -> Result<String, ReadError> {
        read_key(
            self.root(),
            &RecordingFiles::new(self.record.clone()),
            &self.credentials,
            id,
        )
        .map(Secret::into_inner)
    }

    fn index_json(&self) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(self.index()).unwrap()).unwrap()
    }
}

/// The stub keyring every test build uses, straight, with no seam in front of it.
fn stub(config: &Path) -> FileStore {
    FileStore::at(config.join(STUB_FILE))
}

fn entry(account: &str) -> Target {
    Target::Entry {
        service: SERVICE.to_owned(),
        account: account.to_owned(),
    }
}

// ---------------------------------------------------------------------------------------------
// The one read function (R-169, R-172)

#[test]
fn the_read_function_reads_your_keys_only() {
    let here = Here::new();
    let id = here.added("synthetic-your-keys-value");
    // A project vault holding a key under the same name: what an old profile would have read.
    stub(here.root())
        .set(
            "purlis/ops/3f9a2c1b",
            "ANTHROPIC_API_KEY",
            "synthetic-project-vault-value",
        )
        .unwrap();
    stub(here.root())
        .set(
            "purlis/ops/3f9a2c1b",
            &id.to_string(),
            "synthetic-project-vault-value",
        )
        .unwrap();
    here.record.clear();

    assert_eq!(
        here.read(id).ok().as_deref(),
        Some("synthetic-your-keys-value")
    );
    let ops = here.record.ops();
    let credential_reads: Vec<&Target> = ops
        .iter()
        .filter(|op| op.kind == Kind::CredentialRead)
        .map(|op| &op.target)
        .collect();
    assert_eq!(credential_reads, [&entry(&id.to_string())], "{ops:?}");
    assert!(
        ops.iter().all(|op| match &op.target {
            Target::Path(path) => *path == here.index(),
            Target::Entry { service, .. } => service == SERVICE,
            Target::Program(_) => false,
        }),
        "the read reached past your keys: {ops:?}"
    );
}

#[test]
fn the_read_function_serves_one_entry_to_any_project_or_persona() {
    let here = Here::new();
    let id = here.added("synthetic-shared-value");
    // Two chats' contexts: two projects, each under a persona of its own, each with a vault the
    // persona gate would guard. The read takes no project and no persona, so neither can ask
    // for a grant; what each would have reached is planted, and must stay unread.
    let contexts = [("project-a", "steward"), ("project-b", "reviewer")];
    let projects: Vec<tempfile::TempDir> = contexts
        .iter()
        .map(|(_, persona)| {
            let project = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(project.path().join("personas").join(persona)).unwrap();
            std::fs::write(
                project.path().join("vaults.json"),
                r#"{"vaults":{"ops":{"provider":"keyring"}}}"#,
            )
            .unwrap();
            project
        })
        .collect();
    here.record.clear();

    let got: Vec<Option<String>> = projects.iter().map(|_| here.read(id).ok()).collect();

    assert_eq!(
        got,
        [
            Some("synthetic-shared-value".to_owned()),
            Some("synthetic-shared-value".to_owned()),
        ]
    );
    let ops = here.record.ops();
    for project in &projects {
        assert!(
            !ops.iter()
                .any(|op| matches!(&op.target, Target::Path(p) if p.starts_with(project.path()))),
            "a project's file was read for the key: {ops:?}"
        );
    }
    assert_eq!(here.record.count(Kind::CredentialRead), 2, "{ops:?}");
    assert_eq!(here.record.count(Kind::CredentialWrite), 0, "{ops:?}");
}

// ---------------------------------------------------------------------------------------------
// Who writes it (R-173)

/// The store's writers: each a function only the window calls.
const WRITERS: [&str; 2] = ["add", "set_approval"];

/// The surfaces a person or a chat reaches purlis through other than the window, each by the
/// source that serves it. A writer named in any of them is a write the window did not make.
const SURFACES: [(&str, &str); 4] = [
    ("crates/purlis-cli/src", "the `purlis` command's verbs"),
    (
        "crates/session-protocol/src",
        "the session protocol's methods",
    ),
    (
        "crates/host/src",
        "the session host, which serves the protocol",
    ),
    (
        "crates/purlis-core/src",
        "the verbs, protocol handlers and chat tools core implements",
    ),
];

/// The window: the one place a writer may be called from.
const THE_WINDOW: &str = "app/src-tauri/src";

fn rust_files(dir: &Path, into: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for item in read.flatten() {
        let path = item.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "tests") {
                continue;
            }
            rust_files(&path, into);
        } else if path.extension().is_some_and(|e| e == "rs")
            && !path.file_name().is_some_and(|n| n == "tests.rs")
        {
            into.push(path);
        }
    }
}

/// The writers `text` names through a path into the store: `yourkeys::add(`, or a `use` of
/// the store whose list holds one.
fn writers_named(text: &str) -> Vec<&'static str> {
    let mut named = Vec::new();
    for (at, _) in text.match_indices("yourkeys::") {
        let after = &text[at + "yourkeys::".len()..];
        let reach = if after.starts_with('{') {
            &after[..after.find('}').unwrap_or(after.len())]
        } else {
            let end = after
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(after.len());
            &after[..end]
        };
        for writer in WRITERS {
            let words = reach
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .any(|word| word == writer);
            if words && !named.contains(&writer) {
                named.push(writer);
            }
        }
    }
    named
}

#[test]
fn nothing_but_the_window_writes_your_keys() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // The writers are the store's: a rename would make the walk below look for nothing.
    let store = std::fs::read_to_string(root.join("crates/purlis-core/src/yourkeys.rs")).unwrap();
    for writer in WRITERS {
        assert!(
            store.contains(&format!("pub fn {writer}(")),
            "your keys has no writer `{writer}`"
        );
    }
    assert!(writers_named("crate::yourkeys::add(").contains(&"add"));
    assert!(writers_named("use crate::yourkeys::{list, set_approval};").contains(&"set_approval"));
    assert!(writers_named("crate::yourkeys::list(").is_empty());

    let mut offenders = Vec::new();
    for (surface, what) in SURFACES {
        let mut files = Vec::new();
        rust_files(&root.join(surface), &mut files);
        assert!(!files.is_empty(), "{surface} ({what}) holds no Rust file");
        for file in files {
            if file.ends_with("purlis-core/src/yourkeys.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&file).unwrap();
            let named = writers_named(&text);
            if !named.is_empty() {
                offenders.push(format!("{} ({what}): {named:?}", file.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "only the window ({THE_WINDOW}) writes your keys:\n{}",
        offenders.join("\n")
    );
}

// ---------------------------------------------------------------------------------------------
// The entry, its index line and its write order (R-619)

#[test]
fn an_entry_is_a_keychain_value_with_its_index_line() {
    let here = Here::new();
    let id = here.added("synthetic-entry-value");

    let writes: Vec<Target> = here
        .record
        .ops()
        .into_iter()
        .filter(|op| op.kind == Kind::CredentialWrite)
        .map(|op| op.target)
        .collect();
    assert_eq!(writes, [entry(&id.to_string())]);
    assert_eq!(
        stub(here.root())
            .get(SERVICE, &id.to_string())
            .unwrap()
            .map(Secret::into_inner)
            .as_deref(),
        Some("synthetic-entry-value")
    );

    let text = std::fs::read_to_string(here.index()).unwrap_or_default();
    assert!(!text.contains("synthetic-entry-value"), "{text}");
    let index: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    assert_eq!(
        index,
        serde_json::json!({
            "version": 1,
            "entries": [{
                "id": id.to_string(),
                "kind": "keychain",
                "product": { "from": "built-in", "id": "anthropic" },
                "approval": "off",
                "checked": "passed",
            }],
        })
    );
}

/// A keychain that refuses every write, noting whether the index held a line at that moment.
struct RefusingStore {
    index: PathBuf,
    saw_a_line: Arc<Mutex<bool>>,
}

impl Store for RefusingStore {
    fn get(&self, _: &str, _: &str) -> Result<Option<Secret>, VaultError> {
        Ok(None)
    }
    fn set(&self, _: &str, _: &str, _: &str) -> Result<Held, VaultError> {
        let lines = std::fs::read(&self.index)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|index| index["entries"].as_array().map(Vec::len))
            .unwrap_or(0);
        *self.saw_a_line.lock().unwrap() = lines == 1;
        Err(VaultError::new("the stub keychain refuses this write"))
    }
    fn delete(&self, _: &str, _: &str) -> Result<bool, VaultError> {
        Ok(false)
    }
}

#[test]
fn the_index_line_is_written_before_the_item_and_removed_on_failure() {
    let config = tempfile::tempdir().unwrap();
    let saw_a_line = Arc::new(Mutex::new(false));
    let credentials = RecordingCredentials::new(
        Box::new(RefusingStore {
            index: yourkeys::index_path(config.path()),
            saw_a_line: saw_a_line.clone(),
        }),
        Record::default(),
    );

    let saved = add(
        config.path(),
        &credentials,
        Product::anthropic(),
        Checked::Passed,
        "synthetic-refused-value",
    );

    assert!(saved.is_err(), "{saved:?}");
    assert!(
        *saw_a_line.lock().unwrap(),
        "the index held no line when the item was written"
    );
    assert_eq!(
        list(config.path(), &credentials).map(|l| l.len()),
        Ok(0),
        "a failed item write left its line behind"
    );
}

// ---------------------------------------------------------------------------------------------
// The four recovery rules (R-619)

#[test]
fn an_index_line_with_no_keychain_item_reads_unreadable() {
    let here = Here::new();
    let id = here.added("synthetic-lost-value");
    // The item gone, as a crash between the line and the item, or a person's delete, leaves it.
    assert!(stub(here.root()).delete(SERVICE, &id.to_string()).unwrap());

    let listed = list(here.root(), &here.credentials).unwrap();
    assert_eq!(
        listed
            .iter()
            .map(|l| (l.entry.id, l.readable))
            .collect::<Vec<_>>(),
        [(id, false)]
    );
    assert_eq!(here.read(id), Err(ReadError::Unreadable));
}

#[test]
fn a_keychain_item_with_no_line_is_ignored_and_listed_nowhere() {
    let here = Here::new();
    let kept = here.added("synthetic-kept-value");
    let stray = Ulid::generate();
    here.credentials
        .write_outside(SERVICE, &stray.to_string(), "synthetic-stray-value")
        .unwrap();
    here.record.clear();

    let listed: Vec<Ulid> = list(here.root(), &here.credentials)
        .unwrap()
        .iter()
        .map(|l| l.entry.id)
        .collect();
    assert_eq!(listed, [kept]);
    assert_eq!(here.read(stray), Err(ReadError::NotInYourKeys));
    assert!(
        !here
            .record
            .ops()
            .iter()
            .any(|op| op.target == entry(&stray.to_string())),
        "the stray item was asked of: {:?}",
        here.record.ops()
    );
}

#[test]
fn a_missing_index_reads_empty() {
    let here = Here::new();
    // A missing index lists nothing, and reading it makes no file.
    assert_eq!(list(here.root(), &here.credentials).map(|l| l.len()), Ok(0));
    assert!(!here.index().exists());
    assert_eq!(here.read(Ulid::generate()), Err(ReadError::NotInYourKeys));
    // And the first write starts it.
    let id = here.added("synthetic-first-value");
    assert_eq!(here.index_json()["entries"][0]["id"], id.to_string());
}

/// The index, written by hand as `text`.
fn plant(here: &Here, text: &str) {
    std::fs::create_dir_all(here.index().parent().unwrap()).unwrap();
    std::fs::write(here.index(), text).unwrap();
}

#[test]
fn an_unparseable_index_is_refused_and_never_overwritten() {
    let here = Here::new();
    plant(&here, "{ \"version\": 1, \"entries\": [ not json");

    let listed = list(here.root(), &here.credentials);
    assert!(
        listed
            .as_ref()
            .is_err_and(|why| why.contains(&here.index().display().to_string())),
        "{listed:?}"
    );
    let saved = add(
        here.root(),
        &here.credentials,
        Product::anthropic(),
        Checked::Passed,
        "synthetic-value",
    );
    assert!(saved.is_err(), "{saved:?}");
    assert_eq!(
        std::fs::read_to_string(here.index()).unwrap(),
        "{ \"version\": 1, \"entries\": [ not json"
    );
    assert_eq!(here.record.count(Kind::CredentialWrite), 0);
    assert!(matches!(
        here.read(Ulid::generate()),
        Err(ReadError::Index(_))
    ));
}

#[test]
fn an_index_version_above_one_refuses_the_write() {
    let here = Here::new();
    let newer = "{\"version\": 2, \"entries\": [], \"more\": true}\n";
    plant(&here, newer);

    let saved = add(
        here.root(),
        &here.credentials,
        Product::anthropic(),
        Checked::Passed,
        "synthetic-value",
    );

    assert!(
        saved.as_ref().is_err_and(|why| why.contains("version 2")),
        "{saved:?}"
    );
    assert_eq!(std::fs::read_to_string(here.index()).unwrap(), newer);
    assert_eq!(here.record.count(Kind::CredentialWrite), 0);
}

#[test]
fn an_unknown_field_is_kept_as_written() {
    let here = Here::new();
    let held = Ulid::generate();
    plant(
        &here,
        &serde_json::json!({
            "version": 1,
            "later": { "kept": true },
            "entries": [{
                "id": held.to_string(),
                "kind": "keychain",
                "product": { "from": "built-in", "id": "openai" },
                "approval": "off",
                "checked": "passed",
                "note": "from a later build",
            }],
        })
        .to_string(),
    );

    here.added("synthetic-value");

    let index = here.index_json();
    assert_eq!(index["later"], serde_json::json!({ "kept": true }));
    assert_eq!(index["entries"][0]["note"], "from a later build");
    assert_eq!(index["entries"].as_array().map(Vec::len), Some(2));
}

// ---------------------------------------------------------------------------------------------
// The index's own lock (R-619)

#[cfg(unix)]
#[test]
fn the_index_has_its_own_lock_file() {
    use std::sync::mpsc;
    use std::time::Duration;

    fn flock(path: &Path) -> std::fs::File {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = std::fs::File::create(path).unwrap();
        rustix::fs::flock(&file, rustix::fs::FlockOperation::LockExclusive).unwrap();
        file
    }
    fn add_in_a_thread(root: &Path, product: &str) -> mpsc::Receiver<Result<Saved, String>> {
        let (tx, rx) = mpsc::channel();
        let root = root.to_path_buf();
        let product = Product {
            from: "built-in".to_owned(),
            id: product.to_owned(),
        };
        std::thread::spawn(move || {
            let credentials = RecordingCredentials::new(Box::new(stub(&root)), Record::default());
            let _ = tx.send(add(
                &root,
                &credentials,
                product,
                Checked::Passed,
                "synthetic-value",
            ));
        });
        rx
    }

    let config = tempfile::tempdir().unwrap();
    let root = config.path();
    let index_lock = yourkeys::lock_path(root);
    let profiles_lock = purlis_core::machine::charterd(root).join("profiles.json.lock");
    assert_ne!(index_lock, profiles_lock);

    // The profiles store's lock held: a write of your keys goes ahead.
    let profiles = flock(&profiles_lock);
    let first = add_in_a_thread(root, "anthropic").recv_timeout(Duration::from_secs(20));
    assert!(matches!(first, Ok(Ok(Saved::Added(_)))), "{first:?}");
    drop(profiles);

    // The index's own lock held: a second writer waits for it, and writes once it is free.
    let index = flock(&index_lock);
    let second = add_in_a_thread(root, "another");
    assert!(
        second.recv_timeout(Duration::from_millis(500)).is_err(),
        "a writer went past the index's lock"
    );
    drop(index);
    let second = second.recv_timeout(Duration::from_secs(20));
    assert!(matches!(second, Ok(Ok(Saved::Added(_)))), "{second:?}");
}

// ---------------------------------------------------------------------------------------------
// One key per product in 1a (U9)

#[test]
fn a_second_key_for_one_product_offers_replace() {
    let here = Here::new();
    let first = here.added("synthetic-first-value");
    let before = std::fs::read(here.index()).unwrap_or_default();
    here.record.clear();

    let second = add(
        here.root(),
        &here.credentials,
        Product::anthropic(),
        Checked::Passed,
        "synthetic-second-value",
    );

    assert_eq!(second, Ok(Saved::ReplaceOffered { held: first }));
    assert_eq!(std::fs::read(here.index()).unwrap_or_default(), before);
    assert_eq!(here.record.count(Kind::CredentialWrite), 0);
    assert_eq!(
        here.read(first).ok().as_deref(),
        Some("synthetic-first-value")
    );
}

// ---------------------------------------------------------------------------------------------
// The approval setting (R-030, R-490, R-621)

#[test]
fn a_model_key_approval_other_than_off_is_refused_in_words() {
    let here = Here::new();
    let id = here.added("synthetic-value");
    let before = std::fs::read(here.index()).unwrap_or_default();

    for setting in ["every use", "once per chat", "on", ""] {
        assert_eq!(
            set_approval(here.root(), id, setting),
            Err(fill(line::W39, &[("setting", setting)])),
            "{setting:?}"
        );
    }
    assert_eq!(set_approval(here.root(), id, "off"), Ok(()));
    assert_eq!(std::fs::read(here.index()).unwrap_or_default(), before);

    // Adding the key in the window is its approval: it is read with nothing further asked.
    here.record.clear();
    assert_eq!(here.read(id).ok().as_deref(), Some("synthetic-value"));
    assert_eq!(here.record.count(Kind::CredentialRead), 1);
    assert_eq!(here.record.count(Kind::CredentialWrite), 0);
}

#[test]
fn the_approval_setting_lives_in_the_entry_and_reads_off() {
    let here = Here::new();
    let id = here.added("synthetic-value");

    let listed = list(here.root(), &here.credentials).unwrap_or_default();
    assert_eq!(
        listed
            .iter()
            .map(|l| (l.entry.id, l.entry.approval))
            .collect::<Vec<_>>(),
        [(id, Approval::Off)]
    );
    assert_eq!(
        std::fs::read(here.index())
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .map(|index| index["entries"][0]["approval"].clone()),
        Some(serde_json::json!("off"))
    );
}
