// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A call the Linux wrap's supervisor refused becomes the block macOS's kernel line of the same
//! refusal becomes: the same operation, kind, target and owner, so the window offers the same.

use super::*;
use crate::sandboxblock::denial::Denial;
use std::path::Path;

const ROOT: &str = "/home/dev/plane";
const CHAT: &str = "/home/dev/plane/workspaces/alpha/repo";
const HOME: &str = "/home/dev";

fn place() -> Place<'static> {
    Place {
        root: Path::new(ROOT),
        chat: Path::new(CHAT),
        cwd: Path::new(CHAT),
        home: Some(Path::new(HOME)),
    }
}

fn refused(operation: &'static str, target: &str, process: &str) -> Refusal {
    Refusal {
        operation,
        target: target.to_owned(),
        process: Some(process.to_owned()),
    }
}

/// What macOS's kernel line of the same refusal sorts to.
fn as_seatbelt_says_it(line: &str) -> (Block, Option<String>) {
    Denial::of_line(line)
        .expect("a kernel line")
        .block(&place())
}

#[test]
fn a_refused_write_outside_the_project_is_the_block_seatbelt_s_line_of_it_is() {
    let told = block_of(
        &refused("file-write-create", "/home/dev/outside", "touch"),
        &place(),
    );
    assert_eq!(
        told,
        Some((
            Block {
                operation: Operation::Write,
                kind: Kind::Home,
                ours: false,
            },
            Some("/home/dev/outside".to_owned()),
        ))
    );
    assert_eq!(
        told,
        Some(as_seatbelt_says_it(
            "Sandbox: touch(4120) deny(1) file-write-create /home/dev/outside"
        ))
    );
}

#[test]
fn a_refused_write_of_the_project_s_own_files_names_the_path_a_grant_would() {
    let told = block_of(
        &refused(
            "file-write-data",
            "/home/dev/plane/workspaces/beta/notes.md",
            "sh",
        ),
        &place(),
    );
    assert_eq!(
        told,
        Some((
            Block {
                operation: Operation::Write,
                kind: Kind::ProjectFiles,
                ours: false,
            },
            Some("/home/dev/plane/workspaces/beta/notes.md".to_owned()),
        ))
    );
}

#[test]
fn a_refused_file_no_longer_linked_is_named_by_the_name_it_had() {
    // The kernel names such a file with " (deleted)" after its last name.
    for (operation, named, kind, had) in [
        (
            "file-write-data",
            "/home/dev/plane/workspaces/beta/notes.md (deleted)",
            Kind::ProjectFiles,
            "/home/dev/plane/workspaces/beta/notes.md",
        ),
        (
            "file-write-data",
            "/home/dev/outside.log (deleted)",
            Kind::Home,
            "/home/dev/outside.log",
        ),
    ] {
        let told = block_of(&refused(operation, named, "sh"), &place());
        assert_eq!(
            told,
            Some((
                Block {
                    operation: Operation::Write,
                    kind,
                    ours: false,
                },
                Some(had.to_owned()),
            )),
            "{named}"
        );
    }
}

#[test]
fn a_refused_socket_no_longer_linked_is_named_by_the_path_it_had() {
    let told = block_of(
        &refused(
            "network-outbound",
            "/home/dev/run/agent.sock (deleted)",
            "sh",
        ),
        &place(),
    );
    assert_eq!(
        told.and_then(|(block, target)| (block.kind == Kind::LocalSocket).then_some(target)),
        Some(Some("/home/dev/run/agent.sock".to_owned()))
    );
}

#[test]
fn a_refused_read_of_purlis_s_state_names_nothing_to_allow() {
    assert_eq!(
        block_of(
            &refused(
                "file-read-data",
                "/home/dev/plane/.purlis/vaults/ops.json",
                "cat"
            ),
            &place(),
        ),
        Some((
            Block {
                operation: Operation::Read,
                kind: Kind::ProjectState,
                ours: false,
            },
            None,
        ))
    );
}

#[test]
fn a_refusal_of_purlis_s_own_process_is_purlis_s_own_and_names_nothing() {
    assert_eq!(
        block_of(
            &refused("file-write-create", "/home/dev/outside", "purlis"),
            &place(),
        ),
        Some((
            Block {
                operation: Operation::Write,
                kind: Kind::Home,
                ours: true,
            },
            None,
        ))
    );
}

#[test]
fn a_refusal_whose_process_could_not_be_read_is_the_chat_s() {
    let mut refusal = refused("file-write-create", "/home/dev/outside", "x");
    refusal.process = None;
    let (block, _) = block_of(&refusal, &place()).expect("a block");
    assert!(!block.ours);
}

#[test]
fn a_refused_socket_and_a_refused_service_on_this_machine_name_what_a_grant_would() {
    assert_eq!(
        block_of(
            &refused("network-outbound", "/var/run/docker.sock", "docker"),
            &place()
        ),
        Some((
            Block {
                operation: Operation::Connect,
                kind: Kind::LocalSocket,
                ours: false,
            },
            Some("/var/run/docker.sock".to_owned()),
        ))
    );
    for at in ["127.0.0.1:5432", "[::1]:5432"] {
        assert_eq!(
            block_of(&refused("network-outbound", at, "psql"), &place()),
            Some((
                Block {
                    operation: Operation::Connect,
                    kind: Kind::LocalService,
                    ours: false,
                },
                Some("5432".to_owned()),
            )),
            "{at}"
        );
    }
}

#[test]
fn a_refused_connection_to_an_internet_address_is_one_past_the_proxy_that_names_its_address() {
    // The chat reaches the internet through purlis's proxy alone, which tells the hosts it
    // refuses by name; an address a program dialled past it is no host to allow, and is told
    // as what it is (#2423).
    for (named, address) in [
        ("93.184.216.34:443", "93.184.216.34:443"),
        ("[2606:2800:220:1::1]:80", "[2606:2800:220:1::1]:80"),
    ] {
        let told = block_of(&refused("network-outbound", named, "curl"), &place());
        assert_eq!(
            told,
            Some((
                Block {
                    operation: Operation::Connect,
                    kind: Kind::Address,
                    ours: false,
                },
                Some(address.to_owned()),
            )),
            "{named}"
        );
        let (block, _) = told.expect("told");
        assert!(tells(&block));
        assert!(!crate::sandboxblock::is_the_proxys(&block));
        assert_eq!(
            block.said(),
            "a connection to an internet address by a program that does not go through the \
             sandbox's proxy"
        );
    }
}

#[test]
fn nothing_is_told_from_a_chat_the_app_did_not_start_sandboxed_or_that_names_no_socket() {
    let full = |name: &str| -> Option<String> {
        match name {
            crate::hookwire::SANDBOXED_ENV => Some("1".to_owned()),
            crate::hookwire::SOCKET_ENV => Some("/run/purlis/hooks.sock".to_owned()),
            crate::hookwire::CHAT_ENV => Some("7".to_owned()),
            crate::hookwire::HARNESS_ENV => Some("codex".to_owned()),
            CHAT_DIR_ENV => Some(CHAT.to_owned()),
            "HOME" => Some(HOME.to_owned()),
            _ => None,
        }
    };
    let here = Path::new(CHAT);
    assert!(teller_in(&full, here).is_some());
    for missing in [
        crate::hookwire::SANDBOXED_ENV,
        crate::hookwire::SOCKET_ENV,
        crate::hookwire::CHAT_ENV,
        CHAT_DIR_ENV,
    ] {
        let without = |name: &str| (name != missing).then(|| full(name)).flatten();
        assert!(teller_in(&without, here).is_none(), "without {missing}");
    }
    let relative = |name: &str| {
        if name == CHAT_DIR_ENV {
            Some("workspaces/alpha/repo".to_owned())
        } else {
            full(name)
        }
    };
    assert!(
        teller_in(&relative, here).is_none(),
        "a chat folder that is not absolute"
    );
}
