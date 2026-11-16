// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The start reads the key once, and a locked keychain is an Ask with *Unlock*** (T1a-61,
//! #6763; R-078, R-510, R-592).
//!
//! Every test runs against the fenced stub keyring behind the recording seam (T1a-69), with
//! synthetic values only: no test here reaches the operating system's keychain or a real key. A
//! locked keychain is the seam's store refusing as the Keychain does with its dialogs off
//! ([`VaultError::would_ask`]), and the OS's unlock is a [`Unlocks`] the test answers.

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use purlis_core::eventlog::stops::{self, AskGroup, AskRaised, Stop};
use purlis_core::seam::{Kind, Record, RecordingCredentials, RecordingFiles, Target};
use purlis_core::secrets::VaultError;
use purlis_core::secrets::keyring::{FileStore, Held, STUB_FILE, Secret, Store};
use purlis_core::window_copy::line;
use purlis_core::yourkeys::mark::{self, Binding};
use purlis_core::yourkeys::start::{Locked, Read, Unlock, Unlocking, Unlocks, read_once};
use purlis_core::yourkeys::{Checked, Product, SERVICE, Saved, add};
use ulid::Ulid;

/// The stand-in for an Anthropic key: never a real one.
const STAND_IN: &str = "stand-in-for-an-anthropic-key";

/// The stub keyring, refusing every read as a locked Keychain does while `locked` is set.
struct Lockable {
    store: FileStore,
    locked: Arc<AtomicBool>,
}

impl Store for Lockable {
    fn get(&self, service: &str, account: &str) -> Result<Option<Secret>, VaultError> {
        if self.locked.load(Ordering::SeqCst) {
            return Err(VaultError::would_ask(
                "purlis could not read the item: User interaction is not allowed.",
            ));
        }
        self.store.get(service, account)
    }
    fn set(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.store.set(service, account, value)
    }
    fn delete(&self, service: &str, account: &str) -> Result<bool, VaultError> {
        self.store.delete(service, account)
    }
}

/// A machine's config home with one marked Anthropic key in your keys, behind a recording seam.
struct Here {
    config: tempfile::TempDir,
    record: Record,
    credentials: RecordingCredentials,
    locked: Arc<AtomicBool>,
    entry: Ulid,
}

impl Here {
    fn new() -> Self {
        let config = tempfile::tempdir().unwrap();
        let record = Record::default();
        let locked = Arc::new(AtomicBool::new(false));
        let credentials = RecordingCredentials::new(
            Box::new(Lockable {
                store: FileStore::at(config.path().join(STUB_FILE)),
                locked: Arc::clone(&locked),
            }),
            record.clone(),
        );
        let entry = match add(
            config.path(),
            &credentials,
            Product::anthropic(),
            Checked::Passed,
            STAND_IN,
        ) {
            Ok(Saved::Added(id)) => id,
            other => panic!("the key was not added: {other:?}"),
        };
        mark::mark(config.path(), &credentials, entry, &anthropic())
            .unwrap_or_else(|why| panic!("the key was not marked: {why}"));
        record.clear();
        Self {
            config,
            record,
            credentials,
            locked,
            entry,
        }
    }

    fn root(&self) -> &Path {
        self.config.path()
    }

    fn files(&self) -> RecordingFiles {
        RecordingFiles::new(self.record.clone())
    }

    /// A start's one read of the entry.
    fn start(&self) -> Read {
        read_once(self.root(), &self.files(), &self.credentials, self.entry)
    }

    fn lock(&self) {
        self.locked.store(true, Ordering::SeqCst);
    }

    /// How many times the entry's own item was read since the last [`Record::clear`].
    fn key_reads(&self) -> usize {
        let item = Target::Entry {
            service: SERVICE.to_owned(),
            account: self.entry.to_string(),
        };
        self.record
            .ops()
            .iter()
            .filter(|op| op.kind == Kind::CredentialRead && op.target == item)
            .count()
    }

    /// What the start handed on to the host, or why it handed nothing.
    fn handed(read: Read) -> String {
        match read {
            Read::Key(value, _) => Secret::into_inner(value),
            Read::Locked(locked) => panic!("the keychain was locked for {}", locked.entry()),
            Read::Refused(why) => panic!("the start was refused: {why}"),
        }
    }

    fn locked(read: Read) -> Locked {
        match read {
            Read::Locked(locked) => locked,
            Read::Key(..) => panic!("the start read the key from a locked keychain"),
            Read::Refused(why) => panic!("the start was refused, not asked: {why}"),
        }
    }

    /// Answers `locked`'s Ask with *Unlock*, the OS's unlock answering `answer` and, where it
    /// unlocks, unlocking the seam's store. How many unlocks ran, and what the start did.
    fn unlock(&self, locked: Locked, answer: Unlock) -> (usize, Unlocking) {
        let os = OsUnlock {
            answer,
            locked: Arc::clone(&self.locked),
            ran: Rc::new(Cell::new(0)),
        };
        let ran = Rc::clone(&os.ran);
        let did = locked.unlock(&os, self.root(), &self.files(), &self.credentials);
        (ran.get(), did)
    }
}

/// The OS's unlock as the test answers it.
struct OsUnlock {
    answer: Unlock,
    locked: Arc<AtomicBool>,
    ran: Rc<Cell<usize>>,
}

impl Unlocks for OsUnlock {
    fn unlock(&self) -> Unlock {
        self.ran.set(self.ran.get() + 1);
        if self.answer == Unlock::Unlocked {
            self.locked.store(false, Ordering::SeqCst);
        }
        self.answer
    }
}

/// The Anthropic built-in's binding.
fn anthropic() -> Binding {
    let declaration = purlis_core::model_provider::built_ins()
        .iter()
        .find(|d| d.id == "anthropic")
        .expect("the Anthropic built-in ships");
    Binding::of(declaration)
}

/// The Ask a locked keychain raised: W10, in the model key's group, keyed by the entry.
fn assert_is_the_unlock_ask(locked: &Locked, entry: Ulid) {
    assert_eq!(locked.entry(), entry, "the Ask is keyed by the entry");
    assert_eq!(locked.says(), line::W10);
    assert_eq!(
        locked.raised(),
        Stop::AskRaised(AskRaised {
            ask: AskGroup::ModelKey
        })
    );
}

#[test]
fn one_entry_serves_two_chats_in_two_projects() {
    let here = Here::new();
    // Two chats in two projects under two personas: nothing of either reaches the read, which
    // is the entry's and the machine's alone, so no grant of either persona is asked.
    let projects = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
    for (project, _persona) in projects.iter().zip(["steward", "reviewer"]) {
        here.record.clear();
        assert_eq!(Here::handed(here.start()), STAND_IN);
        assert_eq!(here.key_reads(), 1, "one read of your keys per start");
        let reached: Vec<_> = here
            .record
            .ops()
            .into_iter()
            .filter_map(|op| match op.target {
                Target::Path(path) => Some(path),
                _ => None,
            })
            .filter(|path| !path.starts_with(here.root()))
            .collect();
        assert!(
            reached.is_empty(),
            "the start read past your keys (a project's vault or a grant): {reached:?}, in {}",
            project.path().display()
        );
    }
}

#[test]
fn a_locked_keychain_raises_unlock_then_the_start_goes_on() {
    let here = Here::new();
    here.lock();
    let locked = Here::locked(here.start());
    assert_is_the_unlock_ask(&locked, here.entry);

    let (ran, did) = here.unlock(locked, Unlock::Unlocked);
    assert_eq!(ran, 1, "one press of Unlock runs the OS's unlock once");
    match did {
        Unlocking::Read(value, _) => assert_eq!(Secret::into_inner(value), STAND_IN),
        Unlocking::Cancelled(_) | Unlocking::Failed(_) => {
            panic!("the start did not go on after the keychain unlocked")
        }
        Unlocking::Refused(why) => panic!("the start was refused after the unlock: {why}"),
    }
}

#[test]
fn a_cancelled_unlock_leaves_the_ask_and_makes_no_run() {
    let here = Here::new();
    here.lock();
    let locked = Here::locked(here.start());
    here.record.clear();

    let (ran, did) = here.unlock(locked, Unlock::Cancelled);
    assert_eq!(ran, 1);
    match did {
        Unlocking::Cancelled(locked) => assert_is_the_unlock_ask(&locked, here.entry),
        Unlocking::Read(..) => panic!("a cancelled unlock handed a key on"),
        Unlocking::Failed(_) => panic!("a cancelled unlock was said as a failure"),
        Unlocking::Refused(why) => panic!("a cancelled unlock refused the start: {why}"),
    }
    assert_eq!(here.key_reads(), 0, "a cancelled unlock reads nothing again");
}

#[test]
fn a_failed_unlock_shows_w84_and_makes_no_run() {
    let here = Here::new();
    here.lock();
    let locked = Here::locked(here.start());

    let (ran, did) = here.unlock(locked, Unlock::Failed);
    assert_eq!(ran, 1);
    match did {
        Unlocking::Failed(locked) => assert_is_the_unlock_ask(&locked, here.entry),
        Unlocking::Read(..) => panic!("a failed unlock handed a key on"),
        Unlocking::Cancelled(_) => panic!("a failed unlock was said as a cancel"),
        Unlocking::Refused(why) => panic!("a failed unlock refused the start: {why}"),
    }
}

/// **The spec's stops mapping table** (spec.md, "The stops in who-judges' events"), the rows a
/// ticket has raised so far: the stop, its kind, its group (or source) as the log writes it, and
/// what it is keyed by. Each ticket that adds a stop adds its row; T1a-49 audits it complete.
const TABLE: &[(&str, &str, &str, &str)] = &[(
    "Your keys locked at a start",
    stops::ASK_RAISED,
    "model_key",
    "entry",
)];

#[test]
fn the_event_kind_table_lists_every_stop_raised() {
    let here = Here::new();
    here.lock();
    let locked = Here::locked(here.start());
    // Every stop phase 1a raises, as core raises it: its row's name, the stop and its key.
    let raised: Vec<(&str, Stop, &str)> = vec![(
        "Your keys locked at a start",
        locked.raised(),
        locked.keyed_by(),
    )];
    for (name, stop, keyed) in raised {
        let group = match &stop {
            Stop::AskRaised(raised) => serde_json::to_value(raised.ask).unwrap(),
            Stop::AskAnswered(answered) => serde_json::to_value(answered.ask).unwrap(),
            Stop::Block(block) => serde_json::to_value(block.source).unwrap(),
            other => panic!("{name} is raised as a stop the table has no column for: {other:?}"),
        };
        let row = (name, stop.kind(), group.as_str().unwrap_or_default(), keyed);
        assert!(
            TABLE.contains(&row),
            "{row:?} is raised, and the stops mapping table lists no such row: {TABLE:?}"
        );
    }
}
