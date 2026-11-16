// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A running chat's sandbox goes stale when model keys arrive** (R-775, T1a-78, decisions.md
//! D6640-T3 item 9), core's half: the window's write that makes your keys hold a key tells whoever
//! waits on it, once, as it commits; and a chat's settled sandbox records whether it denies the
//! credential service, which is what a running chat is judged by. The app's half, which owes each
//! chat settled without the rule a restart at its pause, is `app/src-tauri/src/chats/tests.rs`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use purlis_core::harness::Harness;
use purlis_core::sandbox::{Machine, Os};
use purlis_core::seam::{Record, RecordingCredentials};
use purlis_core::secrets::keyring::{FileStore, STUB_FILE};
use purlis_core::yourkeys::{self, Checked, Product, Saved};

/// A sandboxed project's folder, as the kernel names it, with a home beside it.
fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a folder");
    let held = dir.path().canonicalize().expect("as the kernel names it");
    let root = held.join("project");
    std::fs::create_dir_all(&root).expect("a project");
    std::fs::create_dir_all(held.join("home")).expect("a home");
    std::fs::write(
        root.join(purlis_core::plane::MANIFEST),
        "[sandbox]\nmode = \"on\"\n",
    )
    .expect("charter.toml");
    (dir, root)
}

/// The config home of the machine beside the project at `root`.
fn config_home(root: &Path) -> PathBuf {
    root.with_file_name("home").join(".config")
}

/// A Mac whose home is the folder beside the project at `root`.
fn machine(root: &Path) -> Machine {
    Machine {
        env: purlis_core::secrets::Env::of(&[]),
        home: Some(root.with_file_name("home")),
        os: Os::MacOs,
    }
}

/// A key for `product` added to your keys under `config` through the window's write, against
/// the fenced stub keyring.
fn a_model_key(config: &Path, product: &str) {
    let credentials = RecordingCredentials::new(
        Box::new(FileStore::at(
            purlis_core::machine::dir(config).join(STUB_FILE),
        )),
        Record::default(),
    );
    let saved = yourkeys::add(
        config,
        &credentials,
        Product {
            from: "built-in".to_owned(),
            id: product.to_owned(),
        },
        Checked::Passed,
        "synthetic-model-key-for-a-stale-sandbox-test",
    )
    .expect("the key is added");
    assert!(matches!(saved, Saved::Added(_)), "{saved:?}");
}

#[test]
fn the_window_s_write_that_makes_your_keys_hold_a_key_tells_whoever_waits_once() {
    let (_dir, root) = project();
    let config = config_home(&root);
    let told = Arc::new(Mutex::new(0_u32));
    let arriving = yourkeys::when_keys_arrive({
        let (told, config) = (Arc::clone(&told), config.clone());
        // Only this test's config home: other tests add keys in this process too.
        move |at| {
            if at == config {
                *told.lock().unwrap() += 1;
            }
        }
    });

    a_model_key(&config, "anthropic");
    assert_eq!(*told.lock().unwrap(), 1, "the first key");
    a_model_key(&config, "another-provider");
    assert_eq!(
        *told.lock().unwrap(),
        1,
        "a second key finds the index holding one"
    );

    drop(arriving);
    let (_other, root) = project();
    a_model_key(&config_home(&root), "anthropic");
    assert_eq!(*told.lock().unwrap(), 1, "let go, it is told nothing");
}

#[test]
fn a_chats_settled_sandbox_records_whether_it_denies_the_credential_service() {
    let (_dir, root) = project();
    let settled = || {
        purlis_core::sandbox::for_start(Harness::ClaudeCode, &root, &machine(&root), &|_| true)
            .expect("compiles")
            .expect("sandboxed")
            .confines()
            .keyring
    };
    assert!(!settled(), "with an empty index, settled without the rule");
    a_model_key(&config_home(&root), "anthropic");
    assert!(settled(), "with a key in your keys, settled with it");
}
