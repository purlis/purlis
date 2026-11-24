// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One `ask.raised` per ask (T1.1-27, Q262, #6874): an ask a chat's proxy holds is reported on
//! two roads, `register_live` and the hook road's block closure, and writes its raise once. Driven
//! at both writers with a recording stop writer, so no chat is started: the hook road's part is
//! [`crate::taskblocks::first_report`], the one question its closure asks before it writes.

use super::*;
use crate::hooks::{HeardBlock, blocked};
use crate::taskblocks::{Blocks, first_report};
use purlis_core::eventlog::stops::{AskGroup, AskRaised, Stop};
use purlis_core::hookwire::SandboxBlocked;
use purlis_core::sandboxblock::{Block, Kind, Operation};

const CHAT: u32 = 7;
const HOST: &str = "one-ask.example.test";

/// What a pretend stop writer was asked to write: each raise, in order.
type Written = Arc<Mutex<Vec<(u32, Stop)>>>;

fn writer() -> (Arc<Mutex<Option<StopWriter>>>, Written) {
    let written: Written = Arc::default();
    let seen = Arc::clone(&written);
    let write: StopWriter = Arc::new(move |chat, stop, how| {
        assert!(matches!(how, BlockWritten::Raised), "only raises here");
        seen.lock().unwrap().push((chat, stop));
        Ok(())
    });
    (Arc::new(Mutex::new(Some(write))), written)
}

fn plane() -> (tempfile::TempDir, crate::planes::PlaneId) {
    let dir = tempfile::tempdir().expect("a project");
    let root = dir.path().canonicalize().expect("real");
    let id =
        serde_json::from_value(serde_json::json!(root.display().to_string())).expect("a plane id");
    (dir, id)
}

/// What the window is told of a host refusal chat [`CHAT`]'s hook, or its proxy's raise down the
/// hooks road, sent: naming `target`, or nothing.
fn told(plane: &crate::planes::PlaneId, target: Option<&str>) -> crate::hooks::ChatBlocked {
    blocked(
        plane,
        &HeardBlock::from(SandboxBlocked {
            chat: CHAT,
            sandbox_blocked: Block {
                operation: Operation::Connect,
                kind: Kind::Host,
                ours: false,
            },
            harness: Some("claude".to_owned()),
            target: target.map(str::to_owned),
            read_back: false,
            suggested: None,
            purlis_command: None,
        }),
    )
}

/// The hook road's closure, as far as the ask's raise: what `Planes::open` does once the block
/// is held (`planes.rs`, "R-229: an ask raised, in the chat's Chat log").
fn hook_road(blocks: &Blocks, stops: &Mutex<Option<StopWriter>>, told: &crate::hooks::ChatBlocked) {
    // The block is held as the road hears it, as `taskblocks::heard` does.
    if let Some(held) = crate::taskblocks::held_of(told) {
        blocks.heard(told.session, held);
    }
    if first_report(blocks, told) {
        let write = lock(stops).clone().expect("a writer");
        write(
            told.session,
            Stop::AskRaised(AskRaised {
                ask: AskGroup::Sandbox,
            }),
            BlockWritten::Raised,
        )
        .expect("written");
    }
}

fn raises(written: &Written) -> usize {
    written
        .lock()
        .unwrap()
        .iter()
        .filter(|(chat, stop)| *chat == CHAT && matches!(stop, Stop::AskRaised(_)))
        .count()
}

/// A held ask, however many times its proxy and its hook say so, writes one raise.
#[test]
fn a_held_ask_writes_exactly_one_ask_raised() {
    let (_dir, plane) = plane();
    let blocks = Blocks::default();
    let (stops, written) = writer();
    let target = format!("{HOST}:443");

    assert!(register_live(&blocks, &stops, CHAT, &target));
    assert!(register_live(&blocks, &stops, CHAT, &target), "told again");
    hook_road(&blocks, &stops, &told(&plane, Some(&target)));
    hook_road(&blocks, &stops, &told(&plane, Some(&target)));
    assert_eq!(raises(&written), 1, "one ask, one raise");

    // Its answer ends it: the same host asked about later is a new ask, and raises again.
    let block = crate::taskblocks::held_of(&told(&plane, Some(&target))).expect("a block");
    blocks.answered(CHAT, &block);
    assert!(register_live(&blocks, &stops, CHAT, &target));
    assert_eq!(raises(&written), 2, "a new ask raises its own");
}

/// The proxy's raise reaches the app twice, once as `register_live` and once down the hooks
/// road. Whichever lands first owns the raise, and the other writes nothing.
#[test]
fn the_same_ask_heard_on_both_roads_is_one_ask() {
    let target = format!("{HOST}:443");
    let (_dir, plane) = plane();

    // The live road first.
    let blocks = Blocks::default();
    let (stops, written) = writer();
    assert!(register_live(&blocks, &stops, CHAT, &target));
    hook_road(&blocks, &stops, &told(&plane, Some(&target)));
    assert_eq!(raises(&written), 1, "live first");

    // The hooks road first.
    let blocks = Blocks::default();
    let (stops, written) = writer();
    hook_road(&blocks, &stops, &told(&plane, Some(&target)));
    assert!(register_live(&blocks, &stops, CHAT, &target));
    assert_eq!(raises(&written), 1, "hooks first");

    // Another host is another ask.
    assert!(register_live(
        &blocks,
        &stops,
        CHAT,
        "other.example.test:443"
    ));
    assert_eq!(raises(&written), 2, "a second ask raises its own");
}

/// M-BRK-01 (chat 680's shape): one call's refusal is told named by the proxy and unnamed by the
/// hook's detector. It is one ask, in either order.
#[test]
fn a_brokered_call_raises_one_ask_not_a_named_and_an_unnamed() {
    let target = format!("{HOST}:443");
    let (_dir, plane) = plane();

    // Named first, then unnamed.
    let blocks = Blocks::default();
    let (stops, written) = writer();
    assert!(register_live(&blocks, &stops, CHAT, &target));
    hook_road(&blocks, &stops, &told(&plane, None));
    assert_eq!(raises(&written), 1, "named, then unnamed");

    // Unnamed first, then named by the live road.
    let blocks = Blocks::default();
    let (stops, written) = writer();
    hook_road(&blocks, &stops, &told(&plane, None));
    assert!(register_live(&blocks, &stops, CHAT, &target));
    assert_eq!(raises(&written), 1, "unnamed, then named by the live road");

    // Unnamed first, then named down the hooks road.
    let blocks = Blocks::default();
    let (stops, written) = writer();
    hook_road(&blocks, &stops, &told(&plane, None));
    hook_road(&blocks, &stops, &told(&plane, Some(&target)));
    assert_eq!(raises(&written), 1, "unnamed, then named by the hook");
}
