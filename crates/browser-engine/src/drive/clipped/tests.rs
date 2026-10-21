// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;

use serde_json::{Value, json};

use super::{Rect, clipped, empty, through};

/// A document as the engine answers it: each node its name, its parent and its id; each laid
/// out one its box and its styles (`overflow-x`, `overflow-y`, `position`, `display`,
/// `overflow-clip-margin`), and its client box the size of its box unless `clients` says other.
struct Doc {
    nodes: Vec<(&'static str, i64, i64)>,
    laid: Vec<(usize, [f64; 4], Style)>,
    clients: Vec<(usize, [f64; 4])>,
    /// Each laid out element's border and padding (left, top, right, bottom, in pixels) and
    /// transform, where not none of each (#2840).
    boxes: Vec<(usize, Boxes)>,
    scrolled: (f64, f64),
    /// The frame elements of this document, by node, and the document each one holds.
    contents: Vec<(usize, usize)>,
}

type Style = [&'static str; 5];

/// An element's border and padding, each side its own, and its transform.
#[derive(Clone, Copy)]
struct Boxes {
    border: [f64; 4],
    padding: [f64; 4],
    transform: &'static str,
    rotate: &'static str,
    scale: &'static str,
    offset_path: &'static str,
}

const NO_BOXES: Boxes = Boxes {
    border: [0.0; 4],
    padding: [0.0; 4],
    transform: "none",
    rotate: "none",
    scale: "none",
    offset_path: "none",
};

const SHOWN: Style = ["visible", "visible", "static", "block", "0px"];
const SCROLLS: Style = ["auto", "auto", "static", "block", "0px"];

/// `style` with `position`.
fn at(style: Style, position: &'static str) -> Style {
    [style[0], style[1], position, style[3], style[4]]
}

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
                .map(|(node, _, style)| {
                    if doc.nodes[*node].0 == "#text" {
                        Vec::new()
                    } else {
                        let boxes = doc
                            .boxes
                            .iter()
                            .find(|(n, _)| n == node)
                            .map_or(NO_BOXES, |(_, b)| *b);
                        let sides: Vec<String> = boxes
                            .border
                            .iter()
                            .chain(&boxes.padding)
                            .map(|n| format!("{n}px"))
                            .collect();
                        let mut out: Vec<i64> = style.iter().map(|s| index(s)).collect();
                        out.extend(sides.iter().map(|s| index(s)));
                        out.push(index(boxes.transform));
                        out.push(index(boxes.rotate));
                        out.push(index(boxes.scale));
                        out.push(index(boxes.offset_path));
                        out
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
                    "clientRects": doc.laid.iter().map(|(node, b, _)| {
                        doc.clients
                            .iter()
                            .find(|(n, _)| n == node)
                            .map_or_else(|| vec![0.0, 0.0, b[2], b[3]], |(_, c)| c.to_vec())
                    }).collect::<Vec<_>>(),
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
fn list(list_style: Style, third_style: Style) -> Doc {
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
        clients: Vec::new(),
        boxes: Vec::new(),
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
    let read = answer(&[list(["hidden", "visible", "static", "block", "0px"], SHOWN)]);
    assert!(clipped(&read, &ids(&[7]), None).is_empty());
}

#[test]
fn an_element_taken_out_of_the_flow_is_not_said_to_be_cut_off_by_a_box_that_may_not_hold_it() {
    let fixed = answer(&[list(SCROLLS, at(SHOWN, "fixed"))]);
    assert!(clipped(&fixed, &ids(&[7]), None).is_empty());
    // An absolute element is held by the nearest positioned box: a list that is not one does
    // not cut it off, and one that is does.
    let absolute = at(SHOWN, "absolute");
    let read = answer(&[list(SCROLLS, absolute)]);
    assert!(clipped(&read, &ids(&[7]), None).is_empty());
    let read = answer(&[list(at(SCROLLS, "relative"), absolute)]);
    assert_eq!(clipped(&read, &ids(&[7]), None), ids(&[7]));
}

#[test]
fn the_page_s_root_and_body_are_left_to_the_viewport() {
    let mut doc = list(SHOWN, SHOWN);
    let hidden = ["hidden", "hidden", "static", "block", "0px"];
    doc.laid[0].2 = hidden;
    doc.laid[1] = (2, [0.0, 0.0, 800.0, 100.0], hidden);
    let read = answer(&[doc]);
    assert!(clipped(&read, &ids(&[7]), None).is_empty());
}

/// A page with a frame 200 by 100 at (10, 400), whose document is scrolled down 300 and holds
/// a heading at the top, a paragraph at 320 and a button at 900.
fn framed(page_box: Style) -> Vec<Doc> {
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
            clients: Vec::new(),
            boxes: Vec::new(),
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
            clients: Vec::new(),
            boxes: Vec::new(),
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
fn an_inline_box_cuts_nothing_off_whatever_its_overflow() {
    // CSS applies overflow to no inline box: a positioned span with `overflow: hidden` holds an
    // absolutely placed child far below it, and the page shows that child.
    let doc = Doc {
        nodes: vec![
            ("#document", -1, 1),
            ("HTML", 0, 2),
            ("BODY", 1, 3),
            ("SPAN", 2, 4),
            ("B", 3, 5),
        ],
        laid: vec![
            (1, [0.0, 0.0, 800.0, 2000.0], SHOWN),
            (2, [0.0, 0.0, 800.0, 2000.0], SHOWN),
            (
                3,
                [0.0, 0.0, 40.0, 16.0],
                ["hidden", "hidden", "relative", "inline", "0px"],
            ),
            (4, [0.0, 400.0, 40.0, 16.0], at(SHOWN, "absolute")),
        ],
        scrolled: (0.0, 0.0),
        contents: Vec::new(),
        clients: Vec::new(),
        boxes: Vec::new(),
    };
    assert!(clipped(&answer(&[doc]), &ids(&[5]), None).is_empty());
    // The same box as an inline block cuts it off.
    let mut doc = list(SHOWN, SHOWN);
    doc.laid[2].2 = ["hidden", "hidden", "static", "inline-block", "0px"];
    assert_eq!(clipped(&answer(&[doc]), &ids(&[7]), None), ids(&[7]));
}

#[test]
fn overflow_clip_with_a_margin_to_paint_into_cuts_nothing_off() {
    let clip = |margin| answer(&[list(["clip", "clip", "static", "block", margin], SHOWN)]);
    assert!(clipped(&clip("20px"), &ids(&[7]), None).is_empty());
    assert_eq!(clipped(&clip("0px"), &ids(&[7]), None), ids(&[7]));
}

#[test]
fn a_frame_drawn_smaller_than_it_shows_its_document_is_compared_at_the_size_it_shows_it() {
    // The frame element is drawn at a quarter of its size (a transform), 50 by 25, but shows
    // its document 200 by 100: the paragraph at 320, within that, is shown.
    let mut docs = framed(SHOWN);
    docs[0].laid[3].1 = [10.0, 400.0, 50.0, 25.0];
    docs[0].clients = vec![(4, [0.0, 0.0, 200.0, 100.0])];
    let read = answer(&docs);
    assert_eq!(clipped(&read, &ids(&[23, 24, 25]), None), ids(&[23, 25]));
    // A frame whose client box is not given is not compared.
    let mut read = answer(&framed(SHOWN));
    read["documents"][0]["layout"]
        .as_object_mut()
        .unwrap()
        .remove("clientRects");
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
    read["documents"][0]["layout"]["styles"][2] = json!([0, 0, 0]);
    assert!(clipped(&read, &ids(&[7]), None).is_empty());
}

const HIDDEN: Style = ["hidden", "hidden", "static", "block", "0px"];

/// A page whose box cuts off what overflows it below `box_bottom`, holding a frame element laid
/// out at `frame` with the client box `client` and `boxes`, whose document is scrolled to its
/// top and holds a paragraph 20 high at `child_at` (node 30) (#2840).
fn in_a_box(
    box_bottom: f64,
    frame: [f64; 4],
    client: [f64; 4],
    boxes: Boxes,
    child_at: f64,
) -> Vec<Doc> {
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
                (3, [0.0, 0.0, 800.0, box_bottom], HIDDEN),
                (4, frame, SHOWN),
            ],
            scrolled: (0.0, 0.0),
            contents: vec![(4, 1)],
            clients: vec![(4, client)],
            boxes: vec![(4, boxes)],
        },
        Doc {
            nodes: vec![
                ("#document", -1, 20),
                ("HTML", 0, 21),
                ("BODY", 1, 22),
                ("P", 2, 30),
            ],
            laid: vec![
                (1, [0.0, 0.0, 200.0, 1000.0], SHOWN),
                (2, [0.0, 0.0, 200.0, 1000.0], SHOWN),
                (3, [0.0, child_at, 200.0, 20.0], SHOWN),
            ],
            scrolled: (0.0, 0.0),
            contents: Vec::new(),
            clients: Vec::new(),
            boxes: Vec::new(),
        },
    ]
}

fn cut_in_a_box(
    box_bottom: f64,
    frame: [f64; 4],
    client: [f64; 4],
    boxes: Boxes,
    child_at: f64,
) -> bool {
    let read = answer(&in_a_box(box_bottom, frame, client, boxes, child_at));
    clipped(&read, &ids(&[30]), None).contains(&30)
}

const FRAME: [f64; 4] = [10.0, 400.0, 200.0, 100.0];
const CLIENT: [f64; 4] = [0.0, 0.0, 200.0, 100.0];

#[test]
fn a_box_on_the_page_that_cuts_off_part_of_a_frame_marks_what_it_hides_of_the_frame() {
    // The box ends 50 into the frame.
    assert!(!cut_in_a_box(450.0, FRAME, CLIENT, NO_BOXES, 20.0));
    assert!(cut_in_a_box(450.0, FRAME, CLIENT, NO_BOXES, 60.0));
}

#[test]
fn a_frame_with_padding_shows_its_document_inside_it() {
    // 20 of padding on every side: the document starts at (30, 420), and a paragraph 32 down
    // it starts at 452, below the box's bottom at 450.
    let padded = Boxes {
        padding: [20.0; 4],
        ..NO_BOXES
    };
    let (frame, client) = ([10.0, 400.0, 240.0, 140.0], [0.0, 0.0, 240.0, 140.0]);
    assert!(cut_in_a_box(450.0, frame, client, padded, 32.0));
    assert!(!cut_in_a_box(450.0, frame, client, padded, 25.0));
}

#[test]
fn a_frame_whose_border_is_wider_on_one_side_shows_its_document_inside_that_border() {
    // A border of 30 on top alone: the document starts at 430, and a paragraph 25 down it at
    // 455, below the box's bottom at 450.
    let topped = Boxes {
        border: [0.0, 30.0, 0.0, 0.0],
        ..NO_BOXES
    };
    let frame = [10.0, 400.0, 200.0, 130.0];
    assert!(cut_in_a_box(450.0, frame, CLIENT, topped, 25.0));
    assert!(!cut_in_a_box(450.0, frame, CLIENT, topped, 15.0));
}

#[test]
fn a_frame_the_page_scales_is_compared_where_the_page_draws_it() {
    // Drawn at half its size, 100 by 50: a paragraph 40 down its document is drawn 20 down the
    // frame, at 420 to 430, inside the box that ends at 430; one 60 down it, at 430, is not.
    let halved = Boxes {
        transform: "matrix(0.5, 0, 0, 0.5, 0, 0)",
        ..NO_BOXES
    };
    let frame = [10.0, 400.0, 100.0, 50.0];
    assert!(!cut_in_a_box(430.0, frame, CLIENT, halved, 40.0));
    assert!(cut_in_a_box(430.0, frame, CLIENT, halved, 60.0));
}

#[test]
fn a_frame_the_page_turns_is_not_said_to_be_cut_off() {
    let turned = Boxes {
        transform: "matrix(0, 1, -1, 0, 0, 0)",
        ..NO_BOXES
    };
    assert!(!cut_in_a_box(410.0, FRAME, CLIENT, turned, 90.0));
    // Nor a frame in a box the page turns.
    let mut docs = in_a_box(410.0, FRAME, CLIENT, NO_BOXES, 90.0);
    docs[0].boxes.push((2, turned));
    assert!(!clipped(&answer(&docs), &ids(&[30]), None).contains(&30));
}

#[test]
fn a_box_in_a_frame_further_out_cuts_off_what_a_nested_frame_holds() {
    // The page's box ends at 450; a frame at 400 holds, at its top, a frame whose paragraph 60
    // down it is drawn at 460, and one 20 down it at 420.
    let nested = |child_at: f64| {
        let mut docs = in_a_box(
            450.0,
            [0.0, 400.0, 300.0, 200.0],
            [0.0, 0.0, 300.0, 200.0],
            NO_BOXES,
            0.0,
        );
        docs[1] = Doc {
            nodes: vec![
                ("#document", -1, 20),
                ("HTML", 0, 21),
                ("BODY", 1, 22),
                ("IFRAME", 2, 23),
            ],
            laid: vec![
                (1, [0.0, 0.0, 300.0, 200.0], SHOWN),
                (2, [0.0, 0.0, 300.0, 200.0], SHOWN),
                (3, [0.0, 0.0, 200.0, 100.0], SHOWN),
            ],
            scrolled: (0.0, 0.0),
            contents: vec![(3, 2)],
            clients: vec![(3, CLIENT)],
            boxes: Vec::new(),
        };
        docs.push(Doc {
            nodes: vec![
                ("#document", -1, 40),
                ("HTML", 0, 41),
                ("BODY", 1, 42),
                ("P", 2, 30),
            ],
            laid: vec![
                (1, [0.0, 0.0, 200.0, 1000.0], SHOWN),
                (2, [0.0, 0.0, 200.0, 1000.0], SHOWN),
                (3, [0.0, child_at, 200.0, 20.0], SHOWN),
            ],
            scrolled: (0.0, 0.0),
            contents: Vec::new(),
            clients: Vec::new(),
            boxes: Vec::new(),
        });
        clipped(&answer(&docs), &ids(&[30]), None).contains(&30)
    };
    assert!(nested(60.0));
    assert!(!nested(20.0));
}

/// The page around a frame from another site, read in a session of its own (#2689): its frame
/// element is node 5, at (10, 400), 200 by 100, in a box that ends at `box_bottom`.
fn around_a_frame(box_bottom: f64) -> Value {
    let mut page = in_a_box(box_bottom, FRAME, CLIENT, NO_BOXES, 0.0).remove(0);
    page.contents.clear();
    answer(&[page])
}

/// The whole of what a frame scrolled to 300 shows: its document from 300 to 400.
const WHOLE_VIEW: Rect = Rect {
    x: 0.0,
    y: 300.0,
    width: 200.0,
    height: 100.0,
};

#[test]
fn a_box_on_the_page_that_cuts_off_a_whole_frame_from_another_site_lets_none_of_it_show() {
    let shown = through(&around_a_frame(200.0), 5, WHOLE_VIEW, (0.0, 300.0), None)
        .expect("the frame's element is in the read");
    assert!(empty(&shown), "{shown:?}");
}

#[test]
fn a_box_on_the_page_that_cuts_off_part_of_a_frame_from_another_site_lets_the_rest_show() {
    let shown = through(&around_a_frame(450.0), 5, WHOLE_VIEW, (0.0, 300.0), None);
    assert_eq!(
        shown,
        Some(Rect {
            x: 10.0,
            y: 400.0,
            width: 200.0,
            height: 50.0
        })
    );
}

#[test]
fn a_frame_from_another_site_no_box_cuts_off_shows_its_whole_size() {
    let shown = through(&around_a_frame(600.0), 5, WHOLE_VIEW, (0.0, 300.0), None);
    assert_eq!(
        shown,
        Some(Rect {
            x: 10.0,
            y: 400.0,
            width: 200.0,
            height: 100.0
        })
    );
}

#[test]
fn a_frame_whose_element_is_not_in_the_read_shows_what_is_not_known() {
    assert_eq!(
        through(&around_a_frame(600.0), 99, WHOLE_VIEW, (0.0, 300.0), None),
        None
    );
}

/// A frame laid out at (10, 400), 200 by 100, whose document holds a list 40 high that scrolls,
/// with an item at its top (node 31) and one below its bottom (node 32), in a page with no box
/// around the frame (#2840).
fn a_list_in_a_frame(boxes: Boxes) -> Vec<Doc> {
    let mut docs = in_a_box(600.0, FRAME, CLIENT, boxes, 0.0);
    docs[1] = Doc {
        nodes: vec![
            ("#document", -1, 20),
            ("HTML", 0, 21),
            ("BODY", 1, 22),
            ("UL", 2, 23),
            ("LI", 3, 31),
            ("LI", 3, 32),
        ],
        laid: vec![
            (1, [0.0, 0.0, 200.0, 100.0], SHOWN),
            (2, [0.0, 0.0, 200.0, 100.0], SHOWN),
            (3, [0.0, 0.0, 200.0, 40.0], SCROLLS),
            (4, [0.0, 0.0, 200.0, 20.0], SHOWN),
            (5, [0.0, 60.0, 200.0, 20.0], SHOWN),
        ],
        scrolled: (0.0, 0.0),
        contents: Vec::new(),
        clients: Vec::new(),
        boxes: Vec::new(),
    };
    docs
}

#[test]
fn an_item_its_own_list_hides_is_marked_in_a_frame_that_cannot_be_placed() {
    let turned = Boxes {
        transform: "matrix(0, 1, -1, 0, 0, 0)",
        ..NO_BOXES
    };
    assert_eq!(
        clipped(&answer(&a_list_in_a_frame(turned)), &ids(&[31, 32]), None),
        ids(&[32])
    );
    // Nor a frame whose client box is not given.
    let mut read = answer(&a_list_in_a_frame(NO_BOXES));
    read["documents"][0]["layout"]
        .as_object_mut()
        .unwrap()
        .remove("clientRects");
    assert_eq!(clipped(&read, &ids(&[31, 32]), None), ids(&[32]));
}

/// Whether the paragraph 90 down a frame in a box ending at 410 is said to be cut off, the frame
/// given `boxes`.
fn cut_through(boxes: Boxes) -> bool {
    cut_in_a_box(410.0, FRAME, CLIENT, boxes, 90.0)
}

#[test]
fn a_frame_the_rotate_property_turns_is_not_said_to_be_cut_off() {
    assert!(cut_through(Boxes {
        rotate: "0deg",
        ..NO_BOXES
    }));
    assert!(!cut_through(Boxes {
        rotate: "45deg",
        ..NO_BOXES
    }));
}

#[test]
fn a_frame_the_scale_property_flips_or_flattens_is_not_said_to_be_cut_off() {
    assert!(cut_through(Boxes {
        scale: "0.5 0.5",
        ..NO_BOXES
    }));
    assert!(!cut_through(Boxes {
        scale: "-1 1",
        ..NO_BOXES
    }));
    assert!(!cut_through(Boxes {
        scale: "0",
        ..NO_BOXES
    }));
}

#[test]
fn a_frame_on_an_offset_path_is_not_said_to_be_cut_off() {
    assert!(!cut_through(Boxes {
        offset_path: "path(\"M 0 0 L 100 100\")",
        ..NO_BOXES
    }));
}

#[test]
fn an_item_scrolled_out_of_a_list_in_a_fixed_box_is_cut_off() {
    // A fixed panel holds a list 100 high that scrolls; its third item is below the list's
    // bottom. The list cuts it off before the panel says nothing more is known (#2840).
    let doc = Doc {
        nodes: vec![
            ("#document", -1, 1),
            ("HTML", 0, 2),
            ("BODY", 1, 3),
            ("DIV", 2, 4),
            ("UL", 3, 5),
            ("LI", 4, 6),
            ("LI", 4, 7),
        ],
        laid: vec![
            (1, [0.0, 0.0, 800.0, 2000.0], SHOWN),
            (2, [0.0, 0.0, 800.0, 2000.0], SHOWN),
            (3, [0.0, 0.0, 300.0, 400.0], at(SHOWN, "fixed")),
            (4, [0.0, 50.0, 300.0, 100.0], SCROLLS),
            (5, [0.0, 50.0, 300.0, 40.0], SHOWN),
            (6, [0.0, 170.0, 300.0, 40.0], SHOWN),
        ],
        scrolled: (0.0, 0.0),
        contents: Vec::new(),
        clients: Vec::new(),
        boxes: Vec::new(),
    };
    assert_eq!(clipped(&answer(&[doc]), &ids(&[6, 7]), None), ids(&[7]));
}

#[test]
fn every_way_the_rotate_property_names_a_turn_is_a_turn() {
    for rotate in ["x 45deg", "1 0 0 45deg", "0.5rad", "0.25turn", "50grad"] {
        assert!(!cut_through(Boxes { rotate, ..NO_BOXES }), "{rotate}");
    }
    for rotate in ["none", "0deg", "x 0deg", "0turn"] {
        assert!(cut_through(Boxes { rotate, ..NO_BOXES }), "{rotate}");
    }
}
