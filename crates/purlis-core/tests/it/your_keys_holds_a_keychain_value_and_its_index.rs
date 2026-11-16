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
const WRITERS: [&str; 3] = ["add", "replace", "set_approval"];

/// Where the code a person or a chat reaches purlis through lives: every crate of the
/// workspace (the `purlis` command's verbs, the session protocol and the host serving it, the
/// chat tools, the browser engine, the stand-in) and the app's own Rust, whose files also serve
/// chats (`secretasks.rs`, `secretfills.rs`, `hooks.rs`, `vaultroute.rs`).
const SURFACES: [&str; 2] = ["crates", "app/src-tauri/src"];

/// The only files that may write your keys or name its keychain service: the store itself and
/// the window's commands for it (T1a-11 and T1a-13 add the second; it is allowed before then).
const ALLOWED: [&str; 2] = [
    "crates/purlis-core/src/yourkeys.rs",
    "app/src-tauri/src/yourkeyscmds.rs",
];

/// Every Rust file under `dir` that ships: no `tests` folder or `tests.rs`, no build output.
fn rust_files(dir: &Path, into: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for item in read.flatten() {
        let path = item.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| n == "tests" || n == "target" || n == "node_modules")
            {
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

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// What `text` reaches of the store's writing side: a writer or [`SERVICE`] named through a path
/// into the store (`yourkeys::add(`, `use …yourkeys::{SERVICE, set_approval}`), a glob of it
/// (`yourkeys::*`), an alias of it (`yourkeys as yk`, `yourkeys::{self as yk}`), or the
/// service's name written out (`"purlis/@your-keys"`).
fn writes_named(text: &str) -> Vec<String> {
    let mut named: Vec<String> = Vec::new();
    let mut note = |what: String| {
        if !named.contains(&what) {
            named.push(what);
        }
    };
    if text.contains("@your-keys") {
        note("the service's name, @your-keys".to_owned());
    }
    for (at, _) in text.match_indices("yourkeys") {
        if text[..at].chars().next_back().is_some_and(is_ident) {
            continue;
        }
        let after = &text[at + "yourkeys".len()..];
        if after.starts_with(|c: char| is_ident(c)) {
            continue;
        }
        if after.trim_start().starts_with("as ") {
            note("an alias of the store".to_owned());
            continue;
        }
        let Some(path) = after.strip_prefix("::") else {
            continue;
        };
        let reach = if path.starts_with('{') {
            // The whole group, nested groups and all.
            let mut depth = 0usize;
            let end = path
                .char_indices()
                .find(|&(_, c)| {
                    match c {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                    depth == 0
                })
                .map_or(path.len(), |(i, _)| i + 1);
            &path[..end]
        } else if path.starts_with('*') {
            "*"
        } else {
            &path[..path.find(|c: char| !is_ident(c)).unwrap_or(path.len())]
        };
        if reach.contains('*') {
            note("a glob of the store".to_owned());
        }
        let words: Vec<&str> = reach.split(|c: char| !is_ident(c)).collect();
        if reach.contains("self as ") {
            note("an alias of the store".to_owned());
        }
        for word in WRITERS.iter().copied().chain(["SERVICE"]) {
            if words.contains(&word) {
                note(word.to_owned());
            }
        }
    }
    named
}

#[test]
fn nothing_but_the_window_writes_your_keys() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // The writers are the store's: a rename would make the walk below look for nothing.
    let store = std::fs::read_to_string(root.join(ALLOWED[0])).unwrap();
    for writer in WRITERS {
        assert!(
            store.contains(&format!("pub fn {writer}(")),
            "your keys has no writer `{writer}`"
        );
    }
    assert!(store.contains("pub const SERVICE: &str = \"purlis/@your-keys\";"));
    // Each way of reaching a writer is seen, and a reader is not a write.
    for (text, sees) in [
        ("crate::yourkeys::add(", "add"),
        ("use crate::yourkeys::{list, set_approval};", "set_approval"),
        (
            "use purlis_core::yourkeys::{\n    self,\n    add,\n};",
            "add",
        ),
        (
            "use purlis_core::{yourkeys::{Product, set_approval}};",
            "set_approval",
        ),
        ("use purlis_core::yourkeys::add as put;", "add"),
        ("use purlis_core::yourkeys::*;", "a glob of the store"),
        ("use purlis_core::yourkeys as yk;", "an alias of the store"),
        (
            "use purlis_core::{yourkeys as yk};",
            "an alias of the store",
        ),
        (
            "use purlis_core::yourkeys::{self as yk};",
            "an alias of the store",
        ),
        ("credentials.set(yourkeys::SERVICE, &id, value)", "SERVICE"),
        ("use crate::yourkeys::{SERVICE as S};", "SERVICE"),
        (
            "store.set(\"purlis/@your-keys\", &id, value)",
            "the service's name, @your-keys",
        ),
    ] {
        assert!(
            writes_named(text).iter().any(|n| n == sees),
            "{text:?} was not seen to name {sees}: {:?}",
            writes_named(text)
        );
    }
    for reader in [
        "crate::yourkeys::list(",
        "home.join(crate::yourkeys::DIR)",
        "use purlis_core::yourkeys::{self, Product, read_key};",
        "notyourkeys::add(",
        "SERVICE.to_owned()",
    ] {
        assert!(
            writes_named(reader).is_empty(),
            "{reader:?}: {:?}",
            writes_named(reader)
        );
    }

    let mut offenders = Vec::new();
    let mut walked = 0;
    for surface in SURFACES {
        let mut files = Vec::new();
        rust_files(&root.join(surface), &mut files);
        assert!(!files.is_empty(), "{surface} holds no Rust file");
        for file in files {
            let at = file.strip_prefix(&root).unwrap_or(&file).to_path_buf();
            if ALLOWED.iter().any(|allowed| at == Path::new(allowed)) {
                continue;
            }
            walked += 1;
            let text = std::fs::read_to_string(&file).unwrap();
            let named = writes_named(&text);
            if !named.is_empty() {
                offenders.push(format!("{}: {named:?}", at.display()));
            }
        }
    }
    // The walk reached the chat-reachable files the window's crate and the other crates hold.
    for must in [
        "app/src-tauri/src/secretasks.rs",
        "app/src-tauri/src/secretfills.rs",
        "app/src-tauri/src/hooks.rs",
        "app/src-tauri/src/vaultroute.rs",
        "crates/browser-engine",
        "crates/stand-in",
        "crates/purlis-cli/src",
        "crates/host/src",
        "crates/session-protocol/src",
    ] {
        assert!(
            root.join(must).exists(),
            "{must} is gone: name what replaced it"
        );
    }
    assert!(walked > 100, "the walk read only {walked} files");
    assert!(
        offenders.is_empty(),
        "only your keys' store and the window's commands for it ({}) write your keys or name \
         its service:\n{}",
        ALLOWED[1],
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

/// The stub keyring with a write that answers as told: held as `held`, or landing and then
/// failing; and a delete that fails when told to.
struct Answering {
    inner: FileStore,
    held: Held,
    lands_then_fails: bool,
    delete_fails: bool,
}

impl Answering {
    fn at(config: &Path) -> Self {
        Self {
            inner: stub(config),
            held: Held::NoRule,
            lands_then_fails: false,
            delete_fails: false,
        }
    }
}

impl Store for Answering {
    fn get(&self, service: &str, account: &str) -> Result<Option<Secret>, VaultError> {
        self.inner.get(service, account)
    }
    fn set(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.inner.set(service, account, value)?;
        if self.lands_then_fails {
            return Err(VaultError::new(
                "the stub keychain wrote the item and then failed",
            ));
        }
        Ok(self.held)
    }
    fn delete(&self, service: &str, account: &str) -> Result<bool, VaultError> {
        if self.delete_fails {
            return Err(VaultError::new("the stub keychain refuses this delete"));
        }
        self.inner.delete(service, account)
    }
}

/// Adds an Anthropic key through `store` behind a recording seam: what `add` answered, the
/// account its one write went to, and the record.
fn add_through(config: &Path, store: Answering) -> (Result<Saved, String>, String, Record) {
    let record = Record::default();
    let credentials = RecordingCredentials::new(Box::new(store), record.clone());
    let saved = add(
        config,
        &credentials,
        Product::anthropic(),
        Checked::Passed,
        "synthetic-value",
    );
    let written: Vec<String> = record
        .ops()
        .into_iter()
        .filter(|op| op.kind == Kind::CredentialWrite)
        .filter_map(|op| match op.target {
            Target::Entry { service, account } if service == SERVICE => Some(account),
            _ => None,
        })
        .collect();
    assert_eq!(written.len(), 1, "{:?}", record.ops());
    (saved, written[0].clone(), record)
}

#[test]
fn a_key_the_keychain_does_not_hold_to_the_app_is_refused_and_taken_out() {
    let config = tempfile::tempdir().unwrap();
    // Outside the app on macOS the write falls back to the calling program's own item.
    let (saved, account, record) = add_through(
        config.path(),
        Answering {
            held: Held::NotYet,
            ..Answering::at(config.path())
        },
    );

    assert!(saved.is_err(), "{saved:?}");
    assert!(
        stub(config.path())
            .get(SERVICE, &account)
            .unwrap()
            .is_none()
    );
    assert!(
        record
            .ops()
            .iter()
            .any(|op| op.kind == Kind::CredentialDelete && op.target == entry(&account)),
        "{:?}",
        record.ops()
    );
    let credentials = RecordingCredentials::new(Box::new(stub(config.path())), Record::default());
    assert_eq!(list(config.path(), &credentials).map(|l| l.len()), Ok(0));
    // Held to the app, or by a store that keeps no rule, the key is added.
    for held in [Held::ToTheApp, Held::NoRule] {
        let config = tempfile::tempdir().unwrap();
        let (saved, _, _) = add_through(
            config.path(),
            Answering {
                held,
                ..Answering::at(config.path())
            },
        );
        assert!(matches!(saved, Ok(Saved::Added(_))), "{held:?}: {saved:?}");
    }
}

#[test]
fn an_item_whose_write_failed_is_taken_out_again() {
    let config = tempfile::tempdir().unwrap();
    let (saved, account, record) = add_through(
        config.path(),
        Answering {
            lands_then_fails: true,
            ..Answering::at(config.path())
        },
    );

    assert!(saved.is_err(), "{saved:?}");
    assert!(
        stub(config.path())
            .get(SERVICE, &account)
            .unwrap()
            .is_none(),
        "the item the failed write left is still in the keychain"
    );
    assert!(
        record
            .ops()
            .iter()
            .any(|op| op.kind == Kind::CredentialDelete && op.target == entry(&account)),
        "{:?}",
        record.ops()
    );

    // Where the keychain refuses the delete too, the error says the item may be left.
    let config = tempfile::tempdir().unwrap();
    let (saved, _, _) = add_through(
        config.path(),
        Answering {
            lands_then_fails: true,
            delete_fails: true,
            ..Answering::at(config.path())
        },
    );
    assert!(
        saved
            .as_ref()
            .is_err_and(|why| why.contains("may be left in your keychain")),
        "{saved:?}"
    );
}

#[test]
fn an_unread_line_naming_the_product_blocks_a_second_key() {
    let unread = |product: serde_json::Value| {
        serde_json::json!({
            "version": 1,
            "entries": [{
                "id": Ulid::generate().to_string(),
                "kind": "reference",
                "product": product,
                "approval": "off",
                "checked": "passed",
            }],
        })
        .to_string()
    };
    // A line this build can't read, for the product or for a product it can't tell: no add.
    for planted in [
        unread(serde_json::json!({ "from": "built-in", "id": "anthropic" })),
        unread(serde_json::json!("not a product")),
        "{\"version\": 1, \"entries\": [{\"id\": 7}]}".to_owned(),
    ] {
        let here = Here::new();
        plant(&here, &planted);
        let saved = add(
            here.root(),
            &here.credentials,
            Product::anthropic(),
            Checked::Passed,
            "synthetic-value",
        );
        assert!(saved.is_err(), "{planted}: {saved:?}");
        assert_eq!(std::fs::read_to_string(here.index()).unwrap(), planted);
        assert_eq!(here.record.count(Kind::CredentialWrite), 0, "{planted}");
    }
    // One for another product is kept, and does not block.
    let here = Here::new();
    plant(
        &here,
        &unread(serde_json::json!({ "from": "built-in", "id": "openai" })),
    );
    here.added("synthetic-value");
    assert_eq!(
        here.index_json()["entries"].as_array().map(Vec::len),
        Some(2)
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
