// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **No block is ever the source of an ask** (R-171, R-240; T1-26, #6417), at the core seam: a
//! block written into the event log, of whatever source and qualifier, leaves the asks registry's
//! list and the ✋'s count as they were. Asks holds asks only.
//!
//! Against the app's own records on a pretend session host, with a real event log: each block is
//! written the way the window's stop writer writes one (`planes.rs`, R-290): into the log, then
//! the needs-you mark of the ask it was is taken back.
//!
//! **A pin** (Q208): it holds what the code already does, so it was green when written. It turns
//! red on either mutation it is here for:
//!
//! - `Chat::block_withdrawn` (`purlis-core`, `state.rs:1298`) clearing `needs_you` without its
//!   guards (an ended chat, its own work back, no ask that ended its wait): a block would take
//!   away the ask of a chat whose turn is the person's;
//! - `Hooks::write_stop` (`hooks.rs:3647`) not recording the block: the log would not hold one
//!   block of each source and qualifier.

use purlis_core::eventlog::stops::{BLOCK, Block, Qualifier, Source, Stop, Target};

use super::*;

/// Every source a block may have.
const SOURCES: [Source; 5] = [
    Source::Guard,
    Source::HarnessCheck,
    Source::Sandbox,
    Source::Policy,
    Source::ProjectSetting,
];

/// Every qualifier a block may have, and none.
const QUALIFIERS: [Option<Qualifier>; 3] =
    [None, Some(Qualifier::Unverified), Some(Qualifier::CantTell)];

/// A target of the kind `source` refuses.
fn a_target_of(source: Source) -> Target {
    match source {
        Source::Guard => Target::guard_arm("force-push"),
        Source::HarnessCheck => Target::folder("secrets"),
        Source::Sandbox => Target::host("api.example.com"),
        Source::Policy => Target::service("com.example.helper"),
        Source::ProjectSetting => Target::folder("vendor"),
    }
    .expect("a target")
}

#[test]
fn no_block_is_ever_the_source_of_an_ask() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let log = plane.root.join("events");
    let host = Pretend::default();
    let planes = planes_recording_events(&log).running_sessions_on({
        let host = host.clone();
        Arc::new(move |_, _: &std::path::Path| Box::new(host.clone()))
    });
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let chat = a_chat_on_work(&held, &plane.root);
    // A turn ended with the next move the person's: one ask the ✋ counts, so "unchanged" is
    // more than an empty list staying empty.
    the_board_hears(&held, chat, Event::UserPromptSubmit);
    the_board_hears(&held, chat, Event::Stop);
    let before = crate::asking::every_ask(&held).asks;
    assert!(!before.is_empty(), "a chat waiting on the person is an ask");

    for source in SOURCES {
        for qualifier in QUALIFIERS {
            held.hooks()
                .write_stop(
                    chat,
                    Stop::Block(Block {
                        tool: "Bash".to_owned(),
                        source,
                        qualifier,
                        target: Some(a_target_of(source)),
                        args: None,
                    }),
                )
                .expect("the block is written");
            held.block_withdrawn(chat);
        }
    }

    let written = purlis_core::eventlog::read(&log)
        .expect("the log")
        .into_iter()
        .filter(|event| event.kind == BLOCK)
        .count();
    assert_eq!(
        written,
        SOURCES.len() * QUALIFIERS.len(),
        "every block was written"
    );
    let after = crate::asking::every_ask(&held).asks;
    assert_eq!(after, before, "a block added, took away or moved an ask");
}
