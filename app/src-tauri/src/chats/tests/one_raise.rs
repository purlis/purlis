// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One `ask.raised` per ask (T1.1-27, Q262, #6874): an ask a chat's proxy holds is reported on
//! two roads, `register_live` and the hook road's block closure, and writes its raise once. Driven
//! at both writers with a recording stop writer, so no chat is started: the hook road's part is
//! [`crate::taskblocks::raise_once`], the one function its closure calls to decide and write.

use super::*;
use crate::hooks::{HeardBlock, blocked};
use crate::taskblocks::{Blocks, HeldBlock, raise_once};
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
/// is told (`taskblocks::heard`, then `raise_once`): an echo of a held connection's block is
/// dropped, the rest is held as the road hears it, and the first report of it writes its raise.
fn hook_road(blocks: &Blocks, stops: &Mutex<Option<StopWriter>>, told: &crate::hooks::ChatBlocked) {
    if let Some(held) = crate::taskblocks::held_of(told) {
        if blocks.echoes(told.session, &held) {
            return;
        }
        blocks.heard(told.session, held);
    }
    raise_once(blocks, told, || {
        let write = lock(stops).clone().expect("a writer");
        write(
            told.session,
            Stop::AskRaised(AskRaised {
                ask: AskGroup::Sandbox,
            }),
            BlockWritten::Raised,
        )
        .expect("written");
    });
}

fn raises(written: &Written) -> usize {
    written
        .lock()
        .unwrap()
        .iter()
        .filter(|(chat, stop)| *chat == CHAT && matches!(stop, Stop::AskRaised(_)))
        .count()
}

/// The asks the registry lists for [`CHAT`]: what the raises must equal, one for each.
fn listed(blocks: &Blocks) -> usize {
    blocks
        .every_since()
        .iter()
        .filter(|(chat, ..)| *chat == CHAT)
        .count()
}

/// One held block, one raise: the raises written are the asks the registry holds.
#[track_caller]
fn assert_the_raises_follow_the_registry(blocks: &Blocks, written: &Written, held: usize) {
    assert_eq!(listed(blocks), held, "the asks the registry holds");
    assert_eq!(raises(written), held, "one raise for each");
}

fn host_block(host: &str) -> HeldBlock {
    live_block(&format!("{host}:443")).expect("a host block")
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
    assert_the_raises_follow_the_registry(&blocks, &written, 1);

    // Its answer ends it: the same host asked about in a later turn is a new ask, and raises again.
    let block = crate::taskblocks::held_of(&told(&plane, Some(&target))).expect("a block");
    blocks.answered(CHAT, &block);
    blocks.turn_began(CHAT);
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
    assert_the_raises_follow_the_registry(&blocks, &written, 1);

    // The hooks road first.
    let blocks = Blocks::default();
    let (stops, written) = writer();
    hook_road(&blocks, &stops, &told(&plane, Some(&target)));
    assert!(register_live(&blocks, &stops, CHAT, &target));
    assert_the_raises_follow_the_registry(&blocks, &written, 1);

    // Another host is another ask.
    assert!(register_live(
        &blocks,
        &stops,
        CHAT,
        "other.example.test:443"
    ));
    assert_the_raises_follow_the_registry(&blocks, &written, 2);
}

/// M-BRK-01 (chat 680's shape): one call's refusal is told named by the proxy and unnamed by the
/// hook's detector. Where the registry ends up holding one block for it, it is one raise.
#[test]
fn a_brokered_call_raises_one_ask_not_a_named_and_an_unnamed() {
    let target = format!("{HOST}:443");
    let (_dir, plane) = plane();

    // Named by the proxy first, then unnamed by the hook: the unnamed one is the named one's
    // echo (`Blocks::echoes`), dropped.
    let blocks = Blocks::default();
    let (stops, written) = writer();
    assert!(register_live(&blocks, &stops, CHAT, &target));
    hook_road(&blocks, &stops, &told(&plane, None));
    assert_the_raises_follow_the_registry(&blocks, &written, 1);

    // Unnamed by the hook first, then named by the proxy: the named, held as waiting, takes the
    // unnamed one's place and its raise.
    let blocks = Blocks::default();
    let (stops, written) = writer();
    hook_road(&blocks, &stops, &told(&plane, None));
    assert!(register_live(&blocks, &stops, CHAT, &target));
    assert_the_raises_follow_the_registry(&blocks, &written, 1);
}

/// F2: named down the hooks road after the same refusal was heard unnamed there, the registry
/// holds both blocks (`Blocks::heard` retires neither), so each is an ask and raises its own.
#[test]
fn a_named_and_an_unnamed_block_the_registry_holds_both_raise_one_each() {
    let target = format!("{HOST}:443");
    let (_dir, plane) = plane();
    let blocks = Blocks::default();
    let (stops, written) = writer();
    hook_road(&blocks, &stops, &told(&plane, None));
    hook_road(&blocks, &stops, &told(&plane, Some(&target)));
    assert_the_raises_follow_the_registry(&blocks, &written, 2);
}

/// F1: host A named on the hook road is raised; unnamed host B, refused after it, is not A's
/// echo (A does not wait), is held as its own block and listed, so it raises too.
#[test]
fn an_unnamed_host_held_beside_a_named_one_gets_its_own_raise() {
    let target = format!("{HOST}:443");
    let (_dir, plane) = plane();
    let blocks = Blocks::default();
    let (stops, written) = writer();
    hook_road(&blocks, &stops, &told(&plane, Some(&target)));
    hook_road(&blocks, &stops, &told(&plane, None));
    assert_the_raises_follow_the_registry(&blocks, &written, 2);
}

/// F3: no writer yet, or a write that fails, leaves no claim: the next report of the ask raises.
#[test]
fn an_ask_whose_raise_was_not_written_raises_when_next_told() {
    let target = format!("{HOST}:443");
    let blocks = Blocks::default();

    let none: Mutex<Option<StopWriter>> = Mutex::new(None);
    assert!(register_live(&blocks, &none, CHAT, &target));
    let (stops, written) = writer();
    assert!(register_live(&blocks, &stops, CHAT, &target));
    assert_the_raises_follow_the_registry(&blocks, &written, 1);

    let blocks = Blocks::default();
    let failing: StopWriter = Arc::new(|_, _, _| Err(std::io::Error::other("disk full")));
    assert!(register_live(
        &blocks,
        &Mutex::new(Some(failing)),
        CHAT,
        &target
    ));
    let (stops, written) = writer();
    assert!(register_live(&blocks, &stops, CHAT, &target));
    assert_the_raises_follow_the_registry(&blocks, &written, 1);
}

/// F4: only a held block is claimed, so an ask that left (answered, or pushed out at once)
/// leaves no claim to swallow the next one's raise.
#[test]
fn a_claim_is_made_only_on_a_held_block_and_goes_with_it() {
    let blocks = Blocks::default();
    let block = host_block(HOST);
    assert!(!blocks.claim_raise(CHAT, &block), "not held: no claim");
    blocks.heard(CHAT, block.clone());
    assert!(blocks.claim_raise(CHAT, &block));
    assert!(!blocks.claim_raise(CHAT, &block), "the same ask joins");
    blocks.answered(CHAT, &block);
    assert!(!blocks.claim_raise(CHAT, &block), "answered: no claim");
    blocks.heard(CHAT, block.clone());
    assert!(!blocks.claim_raise(CHAT, &block), "its echo, this turn");
    blocks.turn_began(CHAT);
    blocks.heard(CHAT, block.clone());
    assert!(
        blocks.claim_raise(CHAT, &block),
        "a new turn's ask raises again"
    );
}

/// An ask pushed out by newer ones, and an ended chat's, give their claims up.
#[test]
fn a_claim_goes_when_its_ask_is_pushed_out_or_the_chat_ends() {
    let blocks = Blocks::default();
    let first = host_block("host-0.example.test");
    blocks.heard(CHAT, first.clone());
    assert!(blocks.claim_raise(CHAT, &first));
    for at in 1..=10 {
        blocks.heard(CHAT, host_block(&format!("host-{at}.example.test")));
    }
    assert!(!blocks.holds(CHAT, &first), "pushed out");
    blocks.heard(CHAT, first.clone());
    assert!(blocks.claim_raise(CHAT, &first), "pushed out: claim gone");

    blocks.ended(CHAT);
    assert!(!blocks.claim_raise(CHAT, &first), "ended: nothing held");
    blocks.heard(CHAT, first.clone());
    assert!(blocks.claim_raise(CHAT, &first), "ended: claim gone");
}

/// Two reports landing together: one wins.
#[test]
fn two_threads_claiming_one_ask_at_once_raise_it_once() {
    for _ in 0..50 {
        let blocks = Arc::new(Blocks::default());
        let block = host_block(HOST);
        blocks.heard(CHAT, block.clone());
        let gate = Arc::new(std::sync::Barrier::new(8));
        let wins: usize = (0..8)
            .map(|_| {
                let (blocks, block, gate) = (blocks.clone(), block.clone(), gate.clone());
                std::thread::spawn(move || {
                    gate.wait();
                    blocks.claim_raise(CHAT, &block)
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|one| usize::from(one.join().expect("a thread")))
            .sum();
        assert_eq!(wins, 1);
    }
}

/// A runner's service by its port is keyed as a host is: one ask, one raise, a new one after its
/// answer.
#[test]
fn a_runner_service_ask_raises_once_by_its_port() {
    let blocks = Blocks::default();
    let (stops, written) = writer();
    assert!(register_live(&blocks, &stops, CHAT, "127.0.0.1:5432"));
    assert!(register_live(&blocks, &stops, CHAT, "127.0.0.1:5432"));
    assert_the_raises_follow_the_registry(&blocks, &written, 1);
    let block = live_block("127.0.0.1:5432").expect("a port block");
    assert_eq!(block.what, crate::sandboxing::GrantWhat::Port);
    // A newer service ask takes the place of the one before in the registry (one block per
    // operation and kind, a host aside), and raises its own.
    assert!(register_live(&blocks, &stops, CHAT, "127.0.0.1:5433"));
    assert_eq!(listed(&blocks), 1);
    assert_eq!(raises(&written), 2);
    blocks.answered(CHAT, &block);
    assert!(register_live(&blocks, &stops, CHAT, "127.0.0.1:5432"));
    assert_eq!(raises(&written), 3);
}

/// The proxy's refusal of a host:port is keyed the same on both roads, whatever the port is
/// written as: `live_block` (the live road) and `held_of` (the hook road, from a line shaped as
/// the real hook's) are one block, so the second report joins the first.
#[test]
fn both_roads_key_one_proxy_refusal_alike() {
    let (_dir, plane) = plane();
    for target in [
        format!("{HOST}:443"),
        format!("{HOST}:80"),
        format!("{HOST}:8443"),
        HOST.to_owned(),
        format!("{}:443", HOST.to_uppercase()),
    ] {
        let live = live_block(&target).expect("a live block");
        let hook = crate::taskblocks::held_of(&told(&plane, Some(&target))).expect("a hook block");
        assert_eq!(live, hook, "one key for {target}");
    }
}

/// A late report of an answered ask (its proxy line or hook line landing after the answer) is
/// that ask's echo: no second `ask.raised` in the turn. The next turn's ask raises its own.
#[test]
fn a_late_report_of_an_answered_ask_raises_nothing() {
    let target = format!("{HOST}:443");
    let (_dir, plane) = plane();
    let blocks = Blocks::default();
    let (stops, written) = writer();
    assert!(register_live(&blocks, &stops, CHAT, &target));
    assert_eq!(raises(&written), 1);
    let block = live_block(&target).expect("a block");
    blocks.answered(CHAT, &block);

    // Late, on either road, in the same turn.
    assert!(register_live(&blocks, &stops, CHAT, &target));
    hook_road(&blocks, &stops, &told(&plane, Some(&target)));
    assert_eq!(raises(&written), 1, "an echo of the answered ask");

    // The chat's next turn forgets it.
    blocks.answered(CHAT, &block);
    blocks.turn_began(CHAT);
    assert!(register_live(&blocks, &stops, CHAT, &target));
    assert_eq!(raises(&written), 2, "a new turn's ask is new");
}
