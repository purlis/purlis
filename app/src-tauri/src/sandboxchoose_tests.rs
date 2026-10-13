// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The person names what a sandbox block did not (#1884): what its Notice offers to choose,
//! that a choice is bound to the block held and judged as a named target is, that nothing a
//! chat sent reaches the list, and that a block a chat only read back asks nothing until it is
//! taken up. Driven on `Chats` with no chat started; a real socket is never bound here, since
//! every folder a test can make one in is one a chat may write, where no socket is offered.

use super::*;
use crate::hooks::{HeardBlock, READ_BACK_SAID, blocked, blocked_on};
use crate::sandboxing::GrantLevel;
use purlis_core::hookwire::SandboxBlocked;
use purlis_core::sandbox::services::TRUSTD;
use purlis_core::sandboxblock::{Block, Kind, Operation};

fn plane() -> (tempfile::TempDir, PlaneId) {
    let dir = tempfile::tempdir().expect("a project");
    let root = dir.path().canonicalize().expect("real");
    let id =
        serde_json::from_value(serde_json::json!(root.display().to_string())).expect("a plane id");
    (dir, id)
}

const CERTIFICATE: Block = Block {
    operation: Operation::Lookup,
    kind: Kind::CertificateCheck,
    ours: false,
};

const SOCKET: Block = Block {
    operation: Operation::Connect,
    kind: Kind::LocalSocket,
    ours: false,
};

/// What the window is told of `block`, sent by chat 4's hook naming nothing.
fn told(plane: &PlaneId, block: Block, read_back: bool) -> ChatBlocked {
    blocked(
        plane,
        &HeardBlock::from(SandboxBlocked {
            chat: 4,
            sandbox_blocked: block,
            harness: Some("claude".to_owned()),
            target: None,
            read_back,
        }),
    )
}

fn shown(block: Block, what: GrantWhat) -> BlockShown {
    BlockShown {
        operation: block.operation.word().to_owned(),
        kind: block.kind.word().to_owned(),
        what,
        target: String::new(),
    }
}

#[test]
fn a_service_the_sandbox_did_not_name_is_the_persons_to_choose() {
    let (_dir, plane) = plane();
    let told = told(&plane, CERTIFICATE, false);
    // Start without the sandbox stays; the choice stands beside it.
    assert_eq!(told.offer, BlockOffer::Unsandboxed);
    assert_eq!(told.choose, Some(GrantWhat::Service));
    assert_eq!(told.target, None);
    let route = told.route.clone().expect("said");
    assert!(route.contains("you can choose it"), "{route}");
    assert!(!route.contains("there is no service to allow"), "{route}");
    // Held, so a choice is bound to it; listed by no ask until something is named.
    assert_eq!(
        crate::taskblocks::held_of(&told),
        Some(HeldBlock::unnamed(
            "lookup",
            "certificate-check",
            GrantWhat::Service
        ))
    );
    // purlis's own operation is a bug to report: nothing to choose.
    let ours = blocked_on(
        &plane,
        4,
        &Block {
            ours: true,
            ..CERTIFICATE
        },
        None,
        None,
    );
    assert_eq!((ours.offer, ours.choose), (BlockOffer::None, None));
}

#[test]
#[cfg(target_os = "macos")]
fn a_socket_the_sandbox_did_not_name_is_the_persons_to_choose() {
    let (_dir, plane) = plane();
    let told = told(&plane, SOCKET, false);
    assert_eq!(told.offer, BlockOffer::Unsandboxed);
    assert_eq!(told.choose, Some(GrantWhat::Socket));
    let route = told.route.clone().expect("said");
    assert!(route.contains("you can choose it"), "{route}");
    assert!(!route.contains("there is no socket to allow"), "{route}");
    assert_eq!(
        crate::taskblocks::held_of(&told),
        Some(HeldBlock::unnamed(
            "connect",
            "local-socket",
            GrantWhat::Socket
        ))
    );
    // One the sandbox named that no grant may name keeps its own sentence, and the person may
    // choose the socket the service keeps elsewhere.
    let refused = blocked_on(&plane, 4, &SOCKET, None, Some("docker.sock"));
    assert_eq!(refused.offer, BlockOffer::Unsandboxed);
    assert_eq!(refused.choose, Some(GrantWhat::Socket));
    assert!(
        refused
            .route
            .as_deref()
            .is_some_and(|why| why.contains("whole path")),
        "{:?}",
        refused.route
    );
}

#[test]
fn policy_that_forbids_every_socket_grant_leaves_nothing_to_choose() {
    use purlis_core::sandbox::policy::{Locks, set_for_this_test};
    let (_dir, plane) = plane();
    set_for_this_test(Locks::parse(
        r#"{"owner": "IT", "sandbox": {"socket-grants": false}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    ));
    for block in [CERTIFICATE, SOCKET] {
        let told = told(&plane, block, false);
        assert_eq!(told.choose, None, "{block:?}");
        assert_eq!(crate::taskblocks::held_of(&told), None, "{block:?}");
    }
    set_for_this_test(Locks::none());
}

#[test]
fn the_services_listed_are_the_ones_purlis_can_allow_and_none_is_typed() {
    let (_dir, plane) = plane();
    let listed = choices_from(
        plane.root(),
        &sandbox::Machine::this(),
        &[],
        GrantWhat::Service,
    );
    assert!(!listed.typed, "a service has no path to check");
    assert_eq!(listed.choices.len(), 1);
    let check = &listed.choices[0];
    assert_eq!(check.target, TRUSTD);
    assert_eq!(check.named, "Certificate check");
    assert!(check.says.contains("check certificates"), "{}", check.says);
}

#[test]
fn the_sockets_listed_come_from_the_apps_own_look_and_only_a_real_one_is_listed() {
    let (_dir, plane) = plane();
    let home = tempfile::tempdir().expect("a home");
    let machine = sandbox::Machine {
        home: Some(home.path().to_path_buf()),
        ..sandbox::Machine::this()
    };
    // A file where Docker's socket would be, and an environment that names a file: neither is
    // a socket, so neither is listed. A path may still be typed.
    let docker = home.path().join(".docker/run");
    std::fs::create_dir_all(&docker).expect("made");
    std::fs::write(docker.join("docker.sock"), "not a socket").expect("written");
    let named = home.path().join("agent.sock");
    std::fs::write(&named, "not a socket").expect("written");
    let env = move |key: &str| (key == "SSH_AUTH_SOCK").then(|| named.clone().into_os_string());
    // Only the places under the fixture home: this machine's own are not looked at here.
    let places: Vec<std::path::PathBuf> = services::candidates(machine.home.as_deref(), &env)
        .into_iter()
        .filter(|place| place.starts_with(home.path()))
        .collect();
    assert!(places.contains(&docker.join("docker.sock")), "{places:?}");
    assert!(
        places.contains(&home.path().join("agent.sock")),
        "{places:?}"
    );
    let listed = choices_from(plane.root(), &machine, &places, GrantWhat::Socket);
    assert!(listed.typed);
    assert_eq!(listed.choices, Vec::new());
    // A host or a folder is not chosen this way.
    for what in [GrantWhat::Host, GrantWhat::Write] {
        let none = choices_from(plane.root(), &machine, &places, what);
        assert!(none.choices.is_empty() && !none.typed, "{what:?}");
    }
}

#[test]
fn a_chosen_service_is_the_block_a_named_one_is_and_the_chat_is_held_on_it() {
    let (_dir, plane) = plane();
    let chats = crate::chats::Chats::new();
    let unnamed_shown = shown(CERTIFICATE, GrantWhat::Service);
    // Not held on it: refused whole.
    let why = chosen(&plane, &chats, 4, &unnamed_shown, TRUSTD).expect_err("not held");
    assert!(
        why.contains("not blocked on what this Notice showed"),
        "{why}"
    );
    crate::taskblocks::heard(&chats, &told(&plane, CERTIFICATE, false));
    let unnamed = HeldBlock::unnamed("lookup", "certificate-check", GrantWhat::Service);
    assert!(chats.blocks().holds(4, &unnamed));
    assert!(
        !asks_a_block(&chats),
        "nothing is asked until something is named"
    );

    // A service purlis does not list is not chosen, whatever is sent; nothing changes.
    for other in ["com.example.helper", "com.apple.SecurityServer", "  "] {
        assert!(
            chosen(&plane, &chats, 4, &unnamed_shown, other).is_err(),
            "{other}"
        );
        assert!(chats.blocks().holds(4, &unnamed), "{other}");
    }
    // Another chat's block, or another kind's, is not this one.
    assert!(chosen(&plane, &chats, 5, &unnamed_shown, TRUSTD).is_err());
    assert!(
        chosen(
            &plane,
            &chats,
            4,
            &shown(SOCKET, GrantWhat::Socket),
            "/x.sock"
        )
        .is_err()
    );
    assert!(
        chosen(
            &plane,
            &chats,
            4,
            &shown(CERTIFICATE, GrantWhat::Host),
            TRUSTD
        )
        .is_err()
    );

    let named = chosen(&plane, &chats, 4, &unnamed_shown, TRUSTD).expect("chosen");
    // Exactly what a block the sandbox named says: the same function made it.
    assert_eq!(
        named,
        blocked_on(&plane, 4, &CERTIFICATE, None, Some(TRUSTD))
    );
    assert_eq!(named.offer, BlockOffer::Service);
    assert_eq!(named.target.as_deref(), Some(TRUSTD));
    assert_eq!(
        named.levels,
        [GrantLevel::Once, GrantLevel::Chat, GrantLevel::You]
    );
    assert!(
        named
            .route
            .as_deref()
            .is_some_and(|said| said.contains("check certificates")),
        "{:?}",
        named.route
    );
    // Held on the named block now, which an Allow answers; the unnamed one is gone, so a
    // second choice finds nothing to choose for.
    let held = HeldBlock {
        operation: "lookup".to_owned(),
        kind: "certificate-check".to_owned(),
        what: GrantWhat::Service,
        target: TRUSTD.to_owned(),
    };
    assert!(chats.blocks().holds(4, &held));
    assert!(!chats.blocks().holds(4, &unnamed));
    assert!(asks_a_block(&chats), "a named block is an ask");
    assert!(chosen(&plane, &chats, 4, &unnamed_shown, TRUSTD).is_err());
    let answer = BlockShown {
        target: TRUSTD.to_owned(),
        ..unnamed_shown
    };
    assert_eq!(
        crate::taskblocks::shown_one(chats.blocks(), 4, &answer),
        Ok(held)
    );
}

/// Whether the asks registry would list a block of any chat.
fn asks_a_block(chats: &crate::chats::Chats) -> bool {
    let locks = purlis_core::sandbox::policy::Locks::none();
    chats
        .blocks()
        .every_since()
        .iter()
        .any(|(session, block, _)| crate::asking::sandbox_block(*session, block, &locks).is_some())
}

#[test]
fn a_chosen_socket_that_no_grant_may_name_is_refused_with_why_and_may_be_chosen_again() {
    let (dir, plane) = plane();
    let chats = crate::chats::Chats::new();
    // Held as the window would be told of it on macOS, whatever this build runs on.
    let unnamed = HeldBlock::unnamed("connect", "local-socket", GrantWhat::Socket);
    chats.blocks().heard(4, unnamed.clone());
    let of = shown(SOCKET, GrantWhat::Socket);
    let file = dir.path().join("docker.sock");
    std::fs::write(&file, "not a socket").expect("written");
    for typed in [
        "docker.sock".to_owned(),
        "~/.docker/run/docker.sock".to_owned(),
        file.display().to_string(),
        dir.path().join("gone.sock").display().to_string(),
        String::new(),
    ] {
        let why = chosen(&plane, &chats, 4, &of, &typed).expect_err(&typed);
        assert!(!why.is_empty(), "{typed}");
        assert!(
            chats.blocks().holds(4, &unnamed),
            "{typed}: still to choose"
        );
    }
    #[cfg(target_os = "macos")]
    {
        let why = chosen(&plane, &chats, 4, &of, &file.display().to_string()).expect_err("a file");
        assert!(why.contains("not a local socket on this machine"), "{why}");
        let why = chosen(&plane, &chats, 4, &of, "docker.sock").expect_err("relative");
        assert!(why.contains("whole path"), "{why}");
    }
}

#[test]
fn a_block_a_chat_only_read_back_is_quiet_until_the_person_takes_it_up() {
    let (_dir, plane) = plane();
    let chats = crate::chats::Chats::new();
    let quiet = told(&plane, CERTIFICATE, true);
    assert_eq!(quiet.quiet.as_deref(), Some(READ_BACK_SAID));
    // Everything else it says is what the block of a command says.
    assert_eq!(
        ChatBlocked {
            quiet: None,
            ..quiet.clone()
        },
        told(&plane, CERTIFICATE, false)
    );
    let host = ChatBlocked {
        offer: BlockOffer::Host,
        operation: "connect".to_owned(),
        kind: "host".to_owned(),
        choose: None,
        ..quiet.clone()
    };
    crate::taskblocks::heard(&chats, &host);
    let unnamed_host = HeldBlock::unnamed_host("connect", "host");
    // Held, so an answer can be checked against it, and listed by no ask.
    assert!(chats.blocks().holds(4, &unnamed_host));
    assert!(chats.blocks().is_quiet(4, &unnamed_host));
    assert!(chats.blocks().every_since().is_empty());

    let of = BlockShown {
        operation: "connect".to_owned(),
        kind: "host".to_owned(),
        what: GrantWhat::Host,
        target: String::new(),
    };
    // Another chat's, or a block never heard: nothing to take up.
    assert!(taken_up(chats.blocks(), 5, &of).is_err());
    assert!(taken_up(chats.blocks(), 4, &shown(SOCKET, GrantWhat::Socket)).is_err());
    assert!(chats.blocks().every_since().is_empty());
    // Taken up: an ask as any block is. A second press changes nothing.
    taken_up(chats.blocks(), 4, &of).expect("taken up");
    assert!(!chats.blocks().is_quiet(4, &unnamed_host));
    assert_eq!(chats.blocks().every_since().len(), 1);
    taken_up(chats.blocks(), 4, &of).expect("still held");
    assert_eq!(chats.blocks().every_since().len(), 1);
}

#[test]
fn a_command_refused_the_same_makes_a_quiet_block_ask_and_a_read_never_quiets_one() {
    let chats = crate::chats::Chats::new();
    let blocks = chats.blocks();
    let block = HeldBlock::unnamed_host("connect", "host");
    blocks.heard_quietly(4, block.clone());
    assert!(blocks.is_quiet(4, &block));
    // The chat ran the command and it was refused: asked about as ever.
    blocks.heard(4, block.clone());
    assert!(!blocks.is_quiet(4, &block));
    assert_eq!(blocks.every_since().len(), 1);
    // Reading the refusal back afterwards does not take the question away.
    blocks.heard_quietly(4, block.clone());
    assert!(!blocks.is_quiet(4, &block));
    assert_eq!(blocks.every_since().len(), 1);
    // Answered or ended: nothing is kept of it.
    blocks.heard_quietly(5, block.clone());
    blocks.answered(5, &block);
    assert!(!blocks.is_quiet(5, &block) && !blocks.holds(5, &block));
    blocks.heard_quietly(6, block.clone());
    blocks.ended(6);
    assert!(!blocks.is_quiet(6, &block) && !blocks.holds(6, &block));
}
