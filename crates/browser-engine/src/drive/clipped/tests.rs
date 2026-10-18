// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;

use serde_json::{Value, json};

use super::{Rect, clipped};

/// A document as the engine answers it: each node its name, its parent and its id; each laid
/// out one its box and its styles (`overflow-x`, `overflow-y`, `position`).
struct Doc {
    nodes: Vec<(&'static str, i64, i64)>,
    laid: Vec<(usize, [f64; 4], [&'static str; 3])>,
    scrolled: (f64, f64),
    /// The frame elements of this document, by node, and the document each one holds.
    contents: Vec<(usize, usize)>,
}

const SHOWN: [&str; 3] = ["visible", "visible", "static"];
const SCROLLS: [&str; 3] = ["auto", "auto", "static"];

fn answer(docs: &[Doc]) -> Value {
    let mut strings: Vec<String> = Vec::new();
    let mut index = |s: &str| -> i64 {
        let at = strings.iter().position(|t| t == s).unwrap_or_else(|| {
            strings.push(s.to_owned());
            strings.len() - 1
        });
        i64::try_from(at).unwrap()
    };
    let documents: Vec<Value> = docs
        .iter()
        .map(|doc| {
            let names: Vec<i64> = doc.nodes.iter().map(|(n, _, _)| index(n)).collect();
            let styles: Vec<Vec<i64>> = doc
                .laid
                .iter()
                .map(|(at, _, style)| {
                    if doc.nodes[*at].0 == "#text" {
                        Vec::new()
                    } else {
                        style.iter().map(|s| index(s)).collect()
                    }
                })
                .collect();
            json!({
                "nodes": {
                    "parentIndex": doc.nodes.iter().map(|(_, p, _)| *p).collect::<Vec<_>>(),
                    "nodeName": names,
                    "backendNodeId": doc.nodes.iter().map(|(_, _, b)| *b).collect::<Vec<_>>(),
                    "contentDocumentIndex": {
                        "index": doc.contents.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
                        "value": doc.contents.iter().map(|(_, d)| *d).collect::<Vec<_>>(),
                    },
                },
                "layout": {
                    "nodeIndex": doc.laid.iter().map(|(at, _, _)| *at).collect::<Vec<_>>(),
                    "bounds": doc.laid.iter().map(|(_, b, _)| b.to_vec()).collect::<Vec<_>>(),
                    "styles": styles,
                },
                "scrollOffsetX": doc.scrolled.0,
                "scrollOffsetY": doc.scrolled.1,
            })
        })
        .collect();
    json!({ "documents": documents, "strings": strings })
}

fn ids(of: &[i64]) -> HashSet<i64> {
    of.iter().copied().collect()
}

/// A page with a list 100 high that scrolls, holding three items 40 high: the first shown, the
/// second cut in half, the third wholly below the list's bottom.
fn list(list_style: [&'static str; 3], third_style: [&'static str; 3]) -> Doc {
    Doc {
        nodes: vec![
            ("#document", -1, 1),
            ("HTML", 0, 2),
            ("BODY", 1, 3),
            ("UL", 2, 4),
            ("LI", 3, 5),
            ("LI", 3, 6),
            ("LI", 3, 7),
        ],
        laid: vec![
            (1, [0.0, 0.0, 800.0, 2000.0], SHOWN),
            (2, [0.0, 0.0, 800.0, 2000.0], SHOWN),
            (3, [0.0, 50.0, 300.0, 100.0], list_style),
            (4, [0.0, 50.0, 300.0, 40.0], SHOWN),
            (5, [0.0, 130.0, 300.0, 40.0], SHOWN),
            (6, [0.0, 170.0, 300.0, 40.0], third_style),
        ],
        scrolled: (0.0, 0.0),
        contents: Vec::new(),
    }
}

#[test]
fn an_item_scrolled_out_of_its_list_is_cut_off_and_one_the_list_shows_is_not() {
    let read = answer(&[list(SCROLLS, SHOWN)]);
    assert_eq!(clipped(&read, &ids(&[5, 6, 7]), None), ids(&[7]));
}

#[test]
fn a_box_that_lets_what_overflows_it_show_cuts_nothing_off() {
    let read = answer(&[list(SHOWN, SHOWN)]);
    assert!(clipped(&read, &ids(&[5, 6, 7]), None).is_empty());
    // A box that cuts off only across leaves what is below it shown.
    let read = answer(&[list(["hidden", "visible", "static"], SHOWN)]);
    assert!(clipped(&read, &ids(&[7]), None).is_empty());
}

#[test]
fn an_element_taken_out_of_the_flow_is_not_said_to_be_cut_off_by_a_box_that_may_not_hold_it() {
    let fixed = answer(&[list(SCROLLS, ["visible", "visible", "fixed"])]);
    assert!(clipped(&fixed, &ids(&[7]), None).is_empty());
    // An absolute element is held by the nearest positioned box: a list that is not one does
    // not cut it off, and one that is does.
    let absolute = ["visible", "visible", "absolute"];
    let read = answer(&[list(SCROLLS, absolute)]);
    assert!(clipped(&read, &ids(&[7]), None).is_empty());
    let read = answer(&[list(["auto", "auto", "relative"], absolute)]);
    assert_eq!(clipped(&read, &ids(&[7]), None), ids(&[7]));
}

#[test]
fn the_page_s_root_and_body_are_left_to_the_viewport() {
    let mut doc = list(SHOWN, SHOWN);
    doc.laid[0].2 = ["hidden", "hidden", "static"];
    doc.laid[1] = (2, [0.0, 0.0, 800.0, 100.0], ["hidden", "hidden", "static"]);
    let read = answer(&[doc]);
    assert!(clipped(&read, &ids(&[7]), None).is_empty());
}

/// A page with a frame 200 by 100 at (10, 400), whose document is scrolled down 300 and holds
/// a heading at the top, a paragraph at 320 and a button at 900.
fn framed(page_box: [&'static str; 3]) -> Vec<Doc> {
    vec![
        Doc {
            nodes: vec![
                ("#document", -1, 1),
                ("HTML", 0, 2),
                ("BODY", 1, 3),
                ("DIV", 2, 4),
                ("IFRAME", 3, 5),
            ],
            laid: vec![
                (1, [0.0, 0.0, 800.0, 600.0], SHOWN),
                (2, [0.0, 0.0, 800.0, 600.0], SHOWN),
                (3, [0.0, 0.0, 800.0, 200.0], page_box),
                (4, [10.0, 400.0, 200.0, 100.0], SHOWN),
            ],
            scrolled: (0.0, 0.0),
            contents: vec![(4, 1)],
        },
        Doc {
            nodes: vec![
                ("#document", -1, 20),
                ("HTML", 0, 21),
                ("BODY", 1, 22),
                ("H1", 2, 23),
                ("P", 2, 24),
                ("BUTTON", 2, 25),
            ],
            laid: vec![
                (1, [0.0, 0.0, 200.0, 1000.0], SHOWN),
                (2, [0.0, 0.0, 200.0, 1000.0], SHOWN),
                (3, [0.0, 0.0, 200.0, 30.0], SHOWN),
                (4, [0.0, 320.0, 200.0, 20.0], SHOWN),
                (5, [0.0, 900.0, 80.0, 20.0], SHOWN),
            ],
            scrolled: (0.0, 300.0),
            contents: Vec::new(),
        },
    ]
}

#[test]
fn an_element_scrolled_out_of_the_frame_it_is_in_is_cut_off_and_one_the_frame_shows_is_not() {
    let read = answer(&framed(SHOWN));
    assert_eq!(clipped(&read, &ids(&[23, 24, 25]), None), ids(&[23, 25]));
}

#[test]
fn what_a_frame_cut_off_by_a_box_of_its_page_shows_is_cut_off_with_it() {
    let read = answer(&framed(SCROLLS));
    assert_eq!(clipped(&read, &ids(&[5, 24]), None), ids(&[5, 24]));
}

#[test]
fn a_frame_read_in_a_session_of_its_own_is_compared_with_what_it_shows() {
    let frame = framed(SHOWN).remove(1);
    let read = answer(&[frame]);
    let shows = Rect {
        x: 0.0,
        y: 300.0,
        width: 200.0,
        height: 100.0,
    };
    assert_eq!(
        clipped(&read, &ids(&[23, 24, 25]), Some(shows)),
        ids(&[23, 25])
    );
    assert!(clipped(&read, &ids(&[23, 24, 25]), None).is_empty());
}

#[test]
fn an_answer_that_cannot_be_read_cuts_nothing_off() {
    for read in [
        json!({}),
        json!({"documents": [], "strings": [1]}),
        json!({"documents": [{"nodes": {}, "layout": {}}], "strings": []}),
    ] {
        assert!(clipped(&read, &ids(&[1, 2]), None).is_empty(), "{read}");
    }
    // Styles read for another purpose do not line up with these.
    let mut read = answer(&[list(SCROLLS, SHOWN)]);
    read["documents"][0]["layout"]["styles"][2] = json!([0, 0, 0, 0, 0, 0, 0, 0]);
    assert!(clipped(&read, &ids(&[7]), None).is_empty());
}
