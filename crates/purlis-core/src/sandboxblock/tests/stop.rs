// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every kind of sandbox refusal is one block event with its target** (R-222's core step,
//! T1-10; Q206): a refused write, a service on this machine, a host kept blocked and each of the
//! floor's denial classes, written through the event log's recorder and read back; and each
//! Block the network record's Blocked lately lists has its block event.

use std::path::{Path, PathBuf};

use super::record::Entry;
use super::stop::{Reported, sandbox_block};
use super::*;
use crate::eventlog::stops::{BLOCK, Qualifier, Source, Stop, Target};
use crate::eventlog::{ArgsKey, Log, Recorder, read};
use crate::sandbox::Class;

const DEVICE: &str = "01J9ZZDEVICE0000000000000";

/// One refusal: what it was of, what the app offered to allow, who reported it, and the target
/// its block must name.
struct Case {
    what: &'static str,
    block: Block,
    named: Option<String>,
    reported: Reported,
    target: Option<Target>,
}

fn refused(operation: Operation, kind: Kind) -> Block {
    Block {
        operation,
        kind,
        ours: false,
    }
}

/// The cases, for a chat of the project at `root` on a machine whose home is `home`.
fn cases(root: &Path, home: &Path) -> Vec<Case> {
    let at = |path: &str| Some(root.join(path).display().to_string());
    let mut cases = vec![
        // A write refused outside the chat's folders, in the project's own files: the folder,
        // by its path inside the project, never the absolute path the app offered.
        Case {
            what: "a refused write",
            block: refused(Operation::Write, Kind::ProjectFiles),
            named: at("records/2026"),
            reported: Reported::ByItsHook,
            target: Some(Target::folder("records/2026").unwrap()),
        },
        // `sandbox-local`: a service on this machine by its port, and a local socket by its
        // file's name.
        Case {
            what: "a service on this machine",
            block: refused(Operation::Connect, Kind::LocalService),
            named: Some("5432".to_owned()),
            reported: Reported::ByItsHook,
            target: Some(Target::service("5432").unwrap()),
        },
        Case {
            what: "a local socket",
            block: refused(Operation::Connect, Kind::LocalSocket),
            named: Some("/var/run/docker.sock".to_owned()),
            reported: Reported::ByItsHook,
            target: Some(Target::service("docker.sock").unwrap()),
        },
        // A host kept blocked: what its held connection was to, by purlis's own proxy.
        Case {
            what: "a host kept blocked",
            block: refused(Operation::Connect, Kind::Host),
            named: Some("api.example.com".to_owned()),
            reported: Reported::ByTheProxy,
            target: Some(Target::host("api.example.com").unwrap()),
        },
    ];
    // Each of the floor's denial classes, by the path a refusal of it names (as
    // `every_denial_class_is_held_by_every_harness_charter_compiles_for` names each).
    for class in Class::ALL {
        let (block, named, target) = match class {
            Class::Vaults => (
                refused(Operation::Read, Kind::ProjectState),
                at(".charter/vaults"),
                Some(Target::folder(".charter/vaults").unwrap()),
            ),
            Class::Integrity => (
                refused(Operation::Write, Kind::ProjectState),
                at(".charter/app"),
                Some(Target::folder(".charter/app").unwrap()),
            ),
            Class::LaterCode => (
                refused(Operation::Write, Kind::ProtectedFile),
                at(".git"),
                Some(Target::folder(".git").unwrap()),
            ),
            // In the home, outside the project: no path a folder target admits (ADR 0075 §4),
            // so its block names none.
            Class::HumanPowers => (
                refused(Operation::Write, Kind::Home),
                Some(home.join(".config/charter").display().to_string()),
                None,
            ),
            // On a runner, the git internals the runner operates on, in the project.
            Class::RunnerInternals => (
                refused(Operation::Write, Kind::System),
                at(".git/objects"),
                Some(Target::folder(".git/objects").unwrap()),
            ),
        };
        cases.push(Case {
            what: class.word(),
            block,
            named,
            reported: Reported::ByItsHook,
            target,
        });
    }
    cases
}

/// The block events in the log at `dir`, each as its body reads back.
fn blocks_in(dir: &Path) -> Vec<crate::eventlog::stops::Block> {
    read(dir)
        .unwrap()
        .into_iter()
        .filter(|event| event.kind == BLOCK)
        .map(|event| serde_json::from_value(event.body).expect("a block's body"))
        .collect()
}

#[test]
fn every_sandbox_refusal_kind_writes_one_block_event_with_its_target() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    let home = PathBuf::from("/Users/op");
    let log = tempfile::tempdir().unwrap();
    let mut recorder = Recorder::new(
        Log::open(log.path(), DEVICE).unwrap(),
        ArgsKey::open(log.path()).unwrap(),
    );

    // One refusal of each kind, one block event each.
    let cases = cases(&root, &home);
    for case in &cases {
        let before = blocks_in(log.path()).len();
        let stop = sandbox_block(
            case.block.kind,
            case.named.as_deref(),
            &root,
            Some("Bash"),
            case.reported,
        );
        recorder.stop(&root, Some(3), &stop).unwrap();
        let after = blocks_in(log.path());
        assert_eq!(after.len(), before + 1, "{}: one block event", case.what);
        let written = after.last().unwrap();
        assert_eq!(written.source, Source::Sandbox, "{}", case.what);
        assert_eq!(written.target, case.target, "{}: its target", case.what);
        assert_eq!(written.tool, "Bash", "{}", case.what);
        assert_eq!(written.args, None, "{}: no call's arguments", case.what);
        // Q235: a refusal the chat's own hook reported is unverified from a run whose hooks
        // its reach holds (this run recorded none); purlis's proxy's own word is plain.
        let qualifier = match case.reported {
            Reported::ByItsHook => Some(Qualifier::Unverified),
            Reported::ByTheProxy => None,
        };
        assert_eq!(written.qualifier, qualifier, "{}", case.what);
        // Never a command, its arguments or output: the absolute path stays off the log.
        if let Some(named) = case.named.as_deref().filter(|named| named.starts_with('/')) {
            let body = serde_json::to_string(written).unwrap();
            assert!(!body.contains(named), "{}: {body}", case.what);
        }
    }

    // For a fixture network record: every Block its Blocked lately lists has its block event.
    let heard = [
        (
            refused(Operation::Connect, Kind::Host),
            Some("api.example.com"),
        ),
        (
            refused(Operation::Connect, Kind::Host),
            Some("cdn.example.net:8443"),
        ),
        (
            refused(Operation::Lookup, Kind::Host),
            Some("registry.example.org"),
        ),
        (
            refused(Operation::Connect, Kind::LocalSocket),
            Some("/var/run/docker.sock"),
        ),
        // A host its report did not name, which the person types: listed with none.
        (refused(Operation::Connect, Kind::Host), None),
        // Not a host's: kept in the record, and not what Blocked lately lists.
        (
            refused(Operation::Write, Kind::ProjectFiles),
            Some("/elsewhere/records"),
        ),
    ];
    let fixture = tempfile::tempdir().unwrap();
    let mut written = Recorder::new(
        Log::open(fixture.path(), DEVICE).unwrap(),
        ArgsKey::open(fixture.path()).unwrap(),
    );
    let mut entries = Vec::new();
    for (block, named) in heard {
        entries.push(Entry::blocked(
            &block,
            named,
            record::Chat::default(),
            None,
            1_000,
        ));
        let stop = sandbox_block(block.kind, named, &root, None, Reported::ByItsHook);
        assert!(matches!(stop, Stop::Block(_)));
        written.stop(&root, Some(3), &stop).unwrap();
    }
    let events = blocks_in(fixture.path());
    let lately: Vec<&Entry> = entries
        .iter()
        .filter(|entry| entry.is_host_block())
        .collect();
    assert_eq!(lately.len(), 5, "the fixture lists five");
    for entry in &lately {
        let block = entry.block.expect("a Block line");
        let host = entry.target.as_deref().or(entry.looked_up.as_deref());
        let wanted = match (block.kind, host) {
            (Kind::Host, Some(host)) => Some(Target::host(host).unwrap()),
            (Kind::LocalSocket, _) => Some(Target::service("docker.sock").unwrap()),
            _ => None,
        };
        assert!(
            events
                .iter()
                .any(|event| event.source == Source::Sandbox && event.target == wanted),
            "Blocked lately's {entry:?} has no block event in {events:?}"
        );
    }
    assert_eq!(events.len(), entries.len(), "one block event per refusal");
}
