// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use super::*;
use crate::runnerinstall::BINARY;

fn versions(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn only_versions_older_than_the_one_before_the_running_one_may_go() {
    assert_eq!(
        may_go("3.0.0", &versions(&["1.0.0", "2.0.0", "3.0.0"])),
        ["1.0.0"]
    );
    // Ordered as versions, not as text: 1.10.0 is after 1.9.0.
    assert_eq!(
        may_go("1.10.0", &versions(&["1.8.0", "1.9.0", "1.10.0", "1.2.0"])),
        ["1.2.0", "1.8.0"]
    );
    // A dev build is before its release.
    assert_eq!(
        may_go(
            "1.4.0",
            &versions(&["1.4.0-dev.7", "1.4.0-dev.8", "1.4.0", "1.3.0"])
        ),
        ["1.3.0", "1.4.0-dev.7"]
    );
}

#[test]
fn the_running_version_the_one_before_it_a_newer_one_and_one_not_read_as_a_version_stay() {
    // Nothing before the running one but the one before it.
    assert!(may_go("2.0.0", &versions(&["1.0.0", "2.0.0"])).is_empty());
    // A version newer than the running one is not this desktop's to remove.
    assert!(may_go("1.0.0", &versions(&["1.0.0", "2.0.0", "3.0.0"])).is_empty());
    // A folder not read as a version is never chosen, and does not count as the one before.
    assert_eq!(
        may_go("3.0.0", &versions(&["1.0.0", "2.0", "2.0.0", "3.0.0"])),
        ["1.0.0"]
    );
    // A running version not read as one removes nothing.
    assert!(may_go("3.0", &versions(&["1.0.0", "2.0.0"])).is_empty());
}

/// A runner's config home with `installed` versions, each with its binary and its host's folder.
struct Runner {
    _home: tempfile::TempDir,
    config_root: PathBuf,
    socket: PathBuf,
}

impl Runner {
    fn with(installed: &[&str]) -> Runner {
        // Short: a socket's path must fit in 104 bytes on macOS.
        let home = tempfile::Builder::new().prefix("pr").tempdir().unwrap();
        let root = std::fs::canonicalize(home.path()).unwrap();
        let config_root = root.join("c");
        let run = root.join("r");
        std::fs::create_dir_all(&run).unwrap();
        let runner = Runner {
            _home: home,
            config_root,
            socket: run.join("purlisd.sock"),
        };
        for version in installed {
            std::fs::create_dir_all(runner.binary(version).parent().unwrap()).unwrap();
            std::fs::write(runner.binary(version), b"x").unwrap();
            std::fs::create_dir_all(runner.host_folder(version)).unwrap();
            std::fs::write(runner.host_folder(version).join("credentials"), b"x").unwrap();
        }
        runner
    }

    fn binary(&self, version: &str) -> PathBuf {
        crate::machine::dir(&self.config_root)
            .join(SERVER_DIR)
            .join(version)
            .join(BINARY)
    }

    fn host_folder(&self, version: &str) -> PathBuf {
        host_folder_of(&crate::machine::charterd(&self.config_root), version)
    }

    fn socket_of(&self, version: &str) -> PathBuf {
        crate::hostsocket::of_version(&self.socket, version)
    }

    fn prune(&self, running: &str) -> Pruned {
        prune_in(&self.config_root, running, &self.socket)
    }
}

fn gone(runner: &Runner, version: &str) -> bool {
    !runner.binary(version).parent().unwrap().exists() && !runner.host_folder(version).exists()
}

#[test]
fn a_drained_versions_folders_are_removed_and_the_running_one_and_the_one_before_stay() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let pruned = runner.prune("3.0.0");
    assert_eq!(pruned.removed, ["1.0.0"]);
    assert!(pruned.kept.is_empty(), "{:?}", pruned.kept);
    assert!(gone(&runner, "1.0.0"));
    for version in ["2.0.0", "3.0.0"] {
        assert!(runner.binary(version).exists(), "{version}'s binary stays");
        assert!(
            runner.host_folder(version).exists(),
            "{version}'s host stays"
        );
    }
    // Pruned again, nothing more goes.
    assert!(runner.prune("3.0.0").removed.is_empty());
}

#[test]
fn a_version_whose_host_holds_its_lock_is_never_removed() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let lock = std::fs::File::create(runner.socket_of("1.0.0").with_extension("lock")).unwrap();
    lock.try_lock().unwrap();
    let pruned = runner.prune("3.0.0");
    assert!(pruned.removed.is_empty());
    assert_eq!(pruned.kept, [("1.0.0".to_owned(), Live::Serving)]);
    assert!(runner.binary("1.0.0").exists() && runner.host_folder("1.0.0").exists());
    drop(lock);
    assert_eq!(runner.prune("3.0.0").removed, ["1.0.0"]);
}

#[test]
fn a_version_whose_host_answers_on_its_socket_is_never_removed() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    // A host whose lock file the system cleaned away still answers.
    let listening = std::os::unix::net::UnixListener::bind(runner.socket_of("1.0.0")).unwrap();
    let pruned = runner.prune("3.0.0");
    assert!(pruned.removed.is_empty());
    assert_eq!(pruned.kept, [("1.0.0".to_owned(), Live::Serving)]);
    assert!(runner.binary("1.0.0").exists() && runner.host_folder("1.0.0").exists());
    drop(listening);
    // A socket file a host that ended left behind answers nothing, and goes with the version.
    assert_eq!(runner.prune("3.0.0").removed, ["1.0.0"]);
    assert!(!runner.socket_of("1.0.0").exists());
}

#[test]
fn a_version_whose_host_left_chats_in_its_record_is_never_removed() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let record = runner.host_folder("1.0.0").join(HOST_RECORD);
    std::fs::write(&record, b"{\"started\":{\"program\":\"p1\"}}\n").unwrap();
    let pruned = runner.prune("3.0.0");
    assert!(pruned.removed.is_empty());
    assert_eq!(pruned.kept, [("1.0.0".to_owned(), Live::Chats)]);
    assert!(runner.binary("1.0.0").exists() && record.exists());
    // A host that ended as it should empties its record.
    std::fs::write(&record, b"").unwrap();
    assert_eq!(runner.prune("3.0.0").removed, ["1.0.0"]);
}

#[test]
fn the_host_record_named_is_the_one_the_host_keeps() {
    assert_eq!(HOST_RECORD, purlis_host::HOST_RECORD);
}

#[test]
fn a_version_folder_that_is_a_link_is_removed_as_a_link_and_what_it_names_stays() {
    let runner = Runner::with(&["2.0.0", "3.0.0"]);
    let elsewhere = runner.config_root.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::write(elsewhere.join(BINARY), b"x").unwrap();
    let server = runner
        .binary("2.0.0")
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    std::os::unix::fs::symlink(&elsewhere, server.join("1.0.0")).unwrap();
    assert_eq!(runner.prune("3.0.0").removed, ["1.0.0"]);
    assert!(std::fs::symlink_metadata(server.join("1.0.0")).is_err());
    assert!(elsewhere.join(BINARY).exists(), "what the link named stays");
}

#[test]
fn what_the_runner_printed_names_the_versions_it_removed() {
    let printed = "removed=1.0.0\nkept=1.1.0 serving\nremoved=; rm -rf /\nremoved=1.2.0\n";
    assert_eq!(removed_from(printed), ["1.0.0", "1.2.0"]);
    assert_eq!(
        Pruned {
            removed: versions(&["1.0.0"]),
            kept: vec![("1.1.0".to_owned(), Live::Chats)],
            failed: Vec::new(),
        }
        .printed(),
        "removed=1.0.0\nkept=1.1.0 chats\n"
    );
}

#[test]
fn the_prune_command_runs_the_running_versions_binary_for_sh() {
    assert_eq!(
        prune_command("/home/r/.config/purlis", "3.0.0"),
        "exec sh -c '/home/r/.config/purlis/server/3.0.0/purlis bridge --prune'"
    );
}
