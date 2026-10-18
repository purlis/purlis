// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The memory in front of the forge's keyring (#2656), over a fake store that counts what it is
//! asked and can hold a delete half-way.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::*;

const SERVICE: &str = "purlis/@forge/github.com/01J9MEMORYTEST";
const ACCOUNT: &str = "token";

/// A store in memory that counts its reads and searches, and whose delete, once armed, says it
/// started and then waits to be let go.
#[derive(Default)]
struct Fake {
    items: Mutex<BTreeMap<(String, String), String>>,
    reads: AtomicUsize,
    searches: AtomicUsize,
    held_delete: Mutex<Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>>,
}

impl Fake {
    fn holding(value: &str) -> Arc<Fake> {
        let fake = Arc::new(Fake::default());
        fake.items
            .lock()
            .unwrap()
            .insert(key(SERVICE, ACCOUNT), value.to_owned());
        fake
    }

    /// Replace the value behind the memory's back, as another process would.
    fn written_elsewhere(&self, value: &str) {
        self.items
            .lock()
            .unwrap()
            .insert(key(SERVICE, ACCOUNT), value.to_owned());
    }
}

impl Store for Fake {
    fn get(&self, service: &str, account: &str) -> Result<Option<Secret>, VaultError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .items
            .lock()
            .unwrap()
            .get(&key(service, account))
            .cloned()
            .map(Secret::new))
    }

    fn set(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.items
            .lock()
            .unwrap()
            .insert(key(service, account), value.to_owned());
        Ok(Held::NoRule)
    }

    fn delete(&self, service: &str, account: &str) -> Result<bool, VaultError> {
        if let Some((started, go)) = self.held_delete.lock().unwrap().take() {
            started.send(()).unwrap();
            go.recv().unwrap();
        }
        Ok(self
            .items
            .lock()
            .unwrap()
            .remove(&key(service, account))
            .is_some())
    }

    fn exists(&self, service: &str, account: &str) -> Presence {
        self.searches.fetch_add(1, Ordering::SeqCst);
        if self
            .items
            .lock()
            .unwrap()
            .contains_key(&key(service, account))
        {
            Presence::Present
        } else {
            Presence::Absent
        }
    }
}

fn value(store: &dyn Store) -> Option<String> {
    store.get(SERVICE, ACCOUNT).unwrap().map(Secret::into_inner)
}

#[test]
fn whether_it_is_there_and_its_value_are_one_read_between_them() {
    let fake = Fake::holding("first");
    let memory = Remembering::over(fake.clone(), true);
    assert_eq!(memory.exists(SERVICE, ACCOUNT), Presence::Present);
    assert_eq!(value(&memory).as_deref(), Some("first"));
    assert_eq!(memory.exists(SERVICE, ACCOUNT), Presence::Present);
    assert_eq!(
        (
            fake.reads.load(Ordering::SeqCst),
            fake.searches.load(Ordering::SeqCst)
        ),
        (1, 0)
    );
}

#[test]
fn where_the_store_cannot_be_asked_without_a_question_of_its_own_nothing_is_read_for_presence() {
    let fake = Fake::holding("first");
    let memory = Remembering::over(fake.clone(), false);
    assert_eq!(memory.exists(SERVICE, ACCOUNT), Presence::Present);
    assert_eq!(fake.reads.load(Ordering::SeqCst), 0);
}

#[test]
fn an_item_that_is_gone_is_asked_again_and_never_remembered_as_gone() {
    let fake = Arc::new(Fake::default());
    let memory = Remembering::over(fake.clone(), true);
    assert_eq!(memory.exists(SERVICE, ACCOUNT), Presence::Absent);
    fake.written_elsewhere("made-later");
    assert_eq!(value(&memory).as_deref(), Some("made-later"));
}

#[test]
fn a_forgotten_item_is_read_afresh() {
    let fake = Fake::holding("first");
    let memory = Remembering::over(fake.clone(), true);
    assert_eq!(value(&memory).as_deref(), Some("first"));
    fake.written_elsewhere("second");
    assert_eq!(
        value(&memory).as_deref(),
        Some("first"),
        "answered from memory"
    );
    memory.forget(SERVICE, ACCOUNT);
    assert_eq!(value(&memory).as_deref(), Some("second"));
}

#[test]
fn a_read_racing_a_delete_never_puts_the_deleted_value_back() {
    let fake = Fake::holding("first");
    let memory = Remembering::over(fake.clone(), true);
    let (started_tx, started) = mpsc::channel();
    let (go, go_rx) = mpsc::channel();
    *fake.held_delete.lock().unwrap() = Some((started_tx, go_rx));

    let deleting = {
        let memory = memory.clone();
        std::thread::spawn(move || memory.delete(SERVICE, ACCOUNT).unwrap())
    };
    started.recv().unwrap();
    let reading = {
        let memory = memory.clone();
        std::thread::spawn(move || value(&memory))
    };
    // Give the read every chance to reach the store while the delete is half-way: it does
    // unless the memory holds it until the delete is done.
    let until = Instant::now() + Duration::from_millis(300);
    while fake.reads.load(Ordering::SeqCst) == 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    go.send(()).unwrap();
    assert!(deleting.join().unwrap());
    let _ = reading.join().unwrap();

    assert_eq!(
        value(&memory),
        None,
        "a deleted item is never handed out again from memory"
    );
}

#[test]
fn a_failed_write_leaves_nothing_remembered() {
    struct Refusing(Arc<Fake>);
    impl Store for Refusing {
        fn get(&self, service: &str, account: &str) -> Result<Option<Secret>, VaultError> {
            self.0.get(service, account)
        }
        fn set(&self, _: &str, _: &str, _: &str) -> Result<Held, VaultError> {
            Err(VaultError::new("refused"))
        }
        fn delete(&self, service: &str, account: &str) -> Result<bool, VaultError> {
            self.0.delete(service, account)
        }
    }
    let fake = Fake::holding("first");
    let memory = Remembering::over(Arc::new(Refusing(fake.clone())), true);
    assert_eq!(value(&memory).as_deref(), Some("first"));
    assert!(memory.set(SERVICE, ACCOUNT, "second").is_err());
    fake.written_elsewhere("third");
    assert_eq!(value(&memory).as_deref(), Some("third"));
}
