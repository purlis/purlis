// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A model key in your keys counts as a keyring vault** (decisions.md D6640-T3 item 7, T1a-72,
//! R-772): with one your-keys entry, and no keyring vault in the project, every chat's sandbox
//! denies the operating system's credential service, as a keyring vault makes it do. On Linux that
//! means no session bus a person allowed, since the Secret Service answers any process on it. With
//! an empty index the rule is as it was. And the test build's stub keyring is denied to every chat,
//! reading and writing, wherever a fenced build keeps it.

use std::path::{Path, PathBuf};

use purlis_core::sandbox::{Access, Class, Compiled, Denied, Machine, Os, Plane, Service, grant};
use purlis_core::seam::{Record, RecordingCredentials};
use purlis_core::secrets::keyring::{FileStore, STUB_FILE};
use purlis_core::yourkeys::{self, Checked, Product};

/// The sandbox on, without the package caches: a test machine has no data home to make them in.
const ON: &str = "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n";

/// A session bus socket a person allowed, under the runtime folder systemd makes for a user.
const RUNTIME_BUS: &str = "/run/user/4242/bus";

/// The session bus as the environment names it, under a name only the environment gives it.
const NAMED_BUS: &str = "/run/user/4343/purlis-test-session";

/// A project's folder, as the kernel names it, with a home beside it, and a committed registry
/// that keeps no vault in the keyring.
fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a folder");
    let held = dir.path().canonicalize().expect("as the kernel names it");
    let root = held.join("project");
    std::fs::create_dir_all(&root).expect("a project");
    std::fs::create_dir_all(held.join("home")).expect("a home");
    std::fs::write(
        root.join("vaults.json"),
        r#"{"vaults": {"dev": {"provider": "plain-file", "config": {"file": "d.json"}}}}"#,
    )
    .expect("the registry");
    (dir, root)
}

/// A machine whose home is the folder beside the project at `root`, with the session bus named.
fn machine(os: Os, root: &Path) -> Machine {
    let bus = format!("unix:path={NAMED_BUS},guid=0123456789abcdef");
    Machine {
        env: purlis_core::secrets::Env::of(&[(purlis_core::chatenv::SESSION_BUS, &bus)]),
        home: Some(root.with_file_name("home")),
        os,
    }
}

/// The config home of the machine beside the project at `root`.
fn config_home(root: &Path) -> PathBuf {
    root.with_file_name("home").join(".config")
}

/// One Anthropic key added to your keys on the machine beside `root`, through the window's write,
/// against the fenced stub keyring.
fn a_model_key(root: &Path) {
    let config = config_home(root);
    let credentials = RecordingCredentials::new(
        Box::new(FileStore::at(purlis_core::machine::dir(&config).join(STUB_FILE))),
        Record::default(),
    );
    yourkeys::add(
        &config,
        &credentials,
        Product::anthropic(),
        Checked::Passed,
        "synthetic-model-key-for-a-sandbox-test",
    )
    .expect("the key is added");
}

/// The local sockets a chat in the project at `root` on `os` is compiled with, where this chat was
/// allowed both session bus sockets.
fn granted_sockets(root: &Path, os: Os) -> Vec<PathBuf> {
    let plane = Plane::of(Some(ON));
    let policy = plane.said().policy.expect("the sandbox is on");
    let chat = grant::Grants {
        sockets: vec![PathBuf::from(RUNTIME_BUS), PathBuf::from(NAMED_BUS)],
        ..grant::Grants::default()
    };
    Compiled::granted(&policy, &plane, root, &machine(os, root), None, &chat)
        .widened
        .sockets
}

#[test]
fn with_one_your_keys_entry_and_no_keyring_vault_a_chats_sandbox_denies_the_credential_service() {
    let (_dir, root) = project();
    for os in [Os::MacOs, Os::Linux] {
        assert_eq!(
            Denied::of(&root, &machine(os, &root)).services,
            [],
            "{os:?}: with no model key the rule is as it was"
        );
    }
    a_model_key(&root);
    for os in [Os::MacOs, Os::Linux] {
        assert_eq!(
            Denied::of(&root, &machine(os, &root)).services,
            [Service::CredentialStore],
            "{os:?}"
        );
    }
}

#[test]
fn with_one_your_keys_entry_a_session_bus_a_person_allowed_is_not_given_to_a_chat() {
    let (_dir, root) = project();
    let both = [PathBuf::from(RUNTIME_BUS), PathBuf::from(NAMED_BUS)];
    assert_eq!(
        granted_sockets(&root, Os::Linux),
        both,
        "with no model key a person's grant stands as it did"
    );
    a_model_key(&root);
    assert_eq!(granted_sockets(&root, Os::Linux), Vec::<PathBuf>::new());
}

#[test]
fn an_index_purlis_cannot_read_counts_as_a_model_key_and_an_empty_one_does_not() {
    let (_dir, root) = project();
    let index = yourkeys::index_path(&config_home(&root));
    std::fs::create_dir_all(index.parent().expect("its folder")).expect("the folder");
    std::fs::write(&index, "{\"version\": 1, \"entries\": []}\n").expect("an empty index");
    assert_eq!(
        Denied::of(&root, &machine(Os::Linux, &root)).services,
        []
    );
    // A line this build can't read may still be a key.
    std::fs::write(
        &index,
        "{\"version\": 1, \"entries\": [{\"id\": \"from a newer purlis\"}]}\n",
    )
    .expect("a line this build can't read");
    assert_eq!(
        Denied::of(&root, &machine(Os::Linux, &root)).services,
        [Service::CredentialStore]
    );
    std::fs::write(&index, "not json").expect("an index that does not parse");
    assert_eq!(
        Denied::of(&root, &machine(Os::Linux, &root)).services,
        [Service::CredentialStore]
    );
}

#[test]
fn the_test_builds_stub_keyring_is_denied_to_every_chat_reading_and_writing() {
    let (_dir, root) = project();
    let config = config_home(&root);
    let mut stubs: Vec<PathBuf> = purlis_core::names::STATE_DIR
        .spellings()
        .map(|state| root.join(state).join(STUB_FILE))
        .collect();
    stubs.extend(
        purlis_core::machine::dirs_spelled(&config)
            .into_iter()
            .map(|home| home.join(STUB_FILE)),
    );
    for os in [Os::MacOs, Os::Linux] {
        let denied = Denied::of(&root, &machine(os, &root));
        for stub in &stubs {
            assert!(
                denied.paths.iter().any(|denial| denial.path == *stub
                    && denial.access == Access::ReadWrite
                    && denial.class == Class::Vaults),
                "{os:?}: {} is not denied, reading and writing",
                stub.display()
            );
        }
    }
}
