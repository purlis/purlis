// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A drag whose source is a native drag-and-drop element (#2136), through a real engine: to a
//! drop target, and by an offset. A drag the person cuts short by taking over leaves no drag of
//! the engine's own under way: the next one drops (#2839).
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use std::sync::Arc;

use common::drive::{call, open, ref_on, sees, snapshot};
use common::site::{started, started_judged};
use purlis_browser_engine::drive::{Call, DragTo, Gate, Judge, Outbound, Settled, Why};

#[test]
fn a_native_drag_source_dragged_to_a_drop_target_drops_there() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "native-drag.html");
    let tree = snapshot(&browser);
    let card = ref_on(&tree, "\"Card\"");
    let bin = ref_on(&tree, "\"Bin\"");
    let dragged = call(
        &browser,
        Call::Drag {
            from: card,
            to: DragTo::Element(bin),
        },
    );
    assert_eq!(dragged.settled, Settled::Yes, "{dragged:?}");
    assert!(
        sees(&browser, "Dropped card in the bin"),
        "{}",
        snapshot(&browser)
    );
}

#[test]
fn a_native_drag_source_dragged_by_an_offset_drops_that_far_from_where_it_started() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "native-drag.html");
    let card = ref_on(&snapshot(&browser), "\"Card\"");
    call(
        &browser,
        Call::Drag {
            from: card,
            to: DragTo::Offset { x: 120.0, y: 60.0 },
        },
    );
    assert!(
        sees(&browser, "Dropped card moved by 120,60"),
        "{}",
        snapshot(&browser)
    );
}

#[test]
fn a_native_drop_under_a_frame_of_another_site_is_judged_as_sent_there() {
    // The bin has a frame of another site drawn over it: the drop would land in that frame.
    let judge: Judge = Arc::new(|out: &Outbound| {
        if out.address.starts_with("http://other.test/") && out.why != Why::Reads {
            Gate::Ask {
                site: "other.test:80".to_owned(),
            }
        } else {
            Gate::Go
        }
    });
    let Some((browser, _data, _site)) = started_judged(judge) else {
        return;
    };
    open(&browser, "native-drag-overlay.html");
    let tree = snapshot(&browser);
    let card = ref_on(&tree, "\"Card\"");
    let bin = ref_on(&tree, "\"Bin\"");
    let dragged = call(
        &browser,
        Call::Drag {
            from: card,
            to: DragTo::Element(bin),
        },
    );
    assert!(
        dragged
            .note
            .as_deref()
            .is_some_and(|n| n.contains("nothing was dropped")),
        "{dragged:?}"
    );
    assert!(
        dragged.asked.contains(&"other.test:80".to_owned()),
        "{dragged:?}"
    );
    assert!(snapshot(&browser).contains("Nothing dropped"));
}

#[test]
fn a_native_drag_cut_by_the_person_taking_over_leaves_the_next_drag_to_drop() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let browser = Arc::new(browser);
    open(&browser, "native-drag.html");
    let tree = snapshot(&browser);
    let (card, bin) = (ref_on(&tree, "\"Card\""), ref_on(&tree, "\"Bin\""));
    let dragging = Arc::clone(&browser);
    let cut = std::thread::spawn(move || {
        dragging.call(
            &common::drive::next_id(),
            Call::Drag {
                from: card,
                to: DragTo::Element(bin),
            },
        )
    });
    // Whenever it lands in the drag, the take-over cuts it.
    std::thread::sleep(std::time::Duration::from_millis(20));
    browser.take_over();
    let _ = cut.join().unwrap();
    // The engine's word of its own drag may come after the cut drag ended: it is cancelled then.
    std::thread::sleep(std::time::Duration::from_millis(500));
    browser.hand_back();
    // The page again, so only the next drag can say it dropped.
    open(&browser, "native-drag.html");
    let tree = snapshot(&browser);
    let (card, bin) = (ref_on(&tree, "\"Card\""), ref_on(&tree, "\"Bin\""));
    call(
        &browser,
        Call::Drag {
            from: card,
            to: DragTo::Element(bin),
        },
    );
    assert!(
        sees(&browser, "Dropped card in the bin"),
        "{}",
        snapshot(&browser)
    );
}
