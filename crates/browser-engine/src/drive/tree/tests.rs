// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

use super::{FrameAx, NodeKey, Query, View, build};

/// `Accessibility.getFullAXTree` as the engine answered it for the fixture site's frames page.
const MAIN_FRAME: &str = include_str!("../../../tests/fixtures/devtools/ax-main-frame.json");
/// The same, for the page's cross-site payment frame, from that frame's own session.
const PAYMENT_FRAME: &str = include_str!("../../../tests/fixtures/devtools/ax-payment-frame.json");

fn frames() -> Vec<FrameAx> {
    vec![
        FrameAx {
            id: "MAIN-FRAME".into(),
            name: String::new(),
            url: "http://fixture.test/frames.html".into(),
            parent: None,
            owner: None,
            nodes: serde_json::from_str::<Value>(MAIN_FRAME).unwrap()["nodes"].clone(),
        },
        FrameAx {
            id: "PAYMENT-FRAME".into(),
            name: "payment".into(),
            url: "http://other.test/pay.html".into(),
            parent: Some("MAIN-FRAME".into()),
            owner: Some(50),
            nodes: serde_json::from_str::<Value>(PAYMENT_FRAME).unwrap()["nodes"].clone(),
        },
    ]
}

/// References as the browser deals them: one per node, in the order first asked for.
#[derive(Default)]
struct Refs(HashMap<NodeKey, String>);

impl Refs {
    fn of(&mut self) -> impl FnMut(&NodeKey) -> String + '_ {
        |key| {
            let next = format!("e{}", self.0.len() + 1);
            self.0.entry(key.clone()).or_insert(next).clone()
        }
    }
}

fn key(frame: &str, backend: i64) -> NodeKey {
    NodeKey {
        frame: frame.into(),
        backend,
    }
}

fn all() -> View {
    View {
        within: None,
        from: 1,
        lines: 1000,
    }
}

#[test]
fn a_snapshot_is_a_tree_of_elements_with_references_and_their_state() {
    let snapshot = build(&frames(), &HashSet::new(), &[]);
    assert_eq!(snapshot.title(), "Frames and fields");
    let mut refs = Refs::default();
    let text = snapshot.page(&all(), &mut refs.of()).unwrap();
    assert_eq!(
        text,
        "\
- heading \"Frames and fields\" [e1] level=1
- link \"Read more\" [e2] url=http://fixture.test/more.html
- textbox \"Name\" [e3] value=\"Ada\" focused required
- textbox \"Secret word\" [e4] value=\"open sesame\"
- paragraph \"You typed open sesame just now\" [e5]
- slider \"Volume\" [e6] value=\"40\" min=0 max=100
- checkbox \"Remember me\" [e7] unchecked
- frame \"payment\" [e8] http://other.test/pay.html
  - button \"Pay now\" [e9]
- button \"Inside the shadow\" [e10]
- text \"Below the fold\" [e11]
"
    );
}

#[test]
fn a_frame_is_named_by_its_name_or_else_its_title() {
    let mut frames = frames();
    frames[1].name = String::new();
    let snapshot = build(&frames, &HashSet::new(), &[]);
    let text = snapshot.page(&all(), &mut Refs::default().of()).unwrap();
    assert!(
        text.contains("- frame \"Payment\" [e8] http://other.test/pay.html\n"),
        "{text}"
    );
}

#[test]
fn a_reference_names_the_same_element_in_every_snapshot_of_its_document() {
    let snapshot = build(&frames(), &HashSet::new(), &[]);
    let mut refs = Refs::default();
    snapshot.page(&all(), &mut refs.of()).unwrap();
    let pay = refs.0[&key("PAYMENT-FRAME", 7)].clone();
    let again = build(&frames(), &HashSet::new(), &[])
        .page(&all(), &mut refs.of())
        .unwrap();
    assert!(
        again.contains(&format!("- button \"Pay now\" [{pay}]")),
        "{again}"
    );
}

#[test]
fn a_blanked_field_shows_no_name_value_or_content_and_its_value_is_gone_from_every_line() {
    let excluded = HashSet::from([key("MAIN-FRAME", 21)]);
    let snapshot = build(&frames(), &excluded, &[]);
    let text = snapshot.page(&all(), &mut Refs::default().of()).unwrap();
    assert!(text.contains("- textbox [e4] (blanked)\n"), "{text}");
    assert!(
        text.contains("- paragraph \"You typed (blanked) just now\" [e5]\n"),
        "{text}"
    );
    assert!(!text.contains("sesame"), "{text}");
    assert!(!text.contains("Secret word"), "{text}");
}

#[test]
fn a_blanked_field_is_not_found_by_its_value_and_its_value_is_not_seen_by_a_wait() {
    let excluded = HashSet::from([key("MAIN-FRAME", 21)]);
    let snapshot = build(&frames(), &excluded, &[]);
    let found = snapshot
        .find(
            &Query {
                text: Some("sesame".into()),
                role: None,
            },
            None,
            &mut Refs::default().of(),
        )
        .unwrap();
    assert_eq!(found, "Nothing on the page matches.\n");
    assert!(!snapshot.shows_text("open sesame"));
    assert!(snapshot.shows_text("You typed"));
}

/// A page with one list of `items` items.
fn long_page(items: usize) -> Vec<FrameAx> {
    let mut nodes = vec![
        json!({"nodeId":"1","ignored":false,"role":{"type":"internalRole","value":"RootWebArea"},"name":{"type":"computedString","value":"Long"},"childIds":["2"],"backendDOMNodeId":1}),
        json!({"nodeId":"2","ignored":false,"role":{"type":"role","value":"list"},"name":{"type":"computedString","value":""},"childIds":(0..items).map(|i| format!("i{i}")).collect::<Vec<_>>(),"backendDOMNodeId":2}),
    ];
    for i in 0..items {
        nodes.push(json!({"nodeId":format!("i{i}"),"ignored":false,"role":{"type":"role","value":"listitem"},"name":{"type":"computedString","value":""},"childIds":[format!("t{i}")],"backendDOMNodeId":100 + i}));
        nodes.push(json!({"nodeId":format!("t{i}"),"ignored":false,"role":{"type":"internalRole","value":"StaticText"},"name":{"type":"computedString","value":format!("Item {}", i + 1)},"childIds":[],"backendDOMNodeId":10_000 + i}));
    }
    vec![FrameAx {
        id: "MAIN".into(),
        name: String::new(),
        url: "http://fixture.test/long.html".into(),
        parent: None,
        owner: None,
        nodes: Value::Array(nodes),
    }]
}

#[test]
fn a_page_longer_than_one_snapshot_is_read_in_pages_that_say_where_the_next_starts() {
    let snapshot = build(&long_page(250), &HashSet::new(), &[]);
    let mut refs = Refs::default();
    let first = snapshot
        .page(
            &View {
                within: None,
                from: 1,
                lines: 100,
            },
            &mut refs.of(),
        )
        .unwrap();
    let lines: Vec<&str> = first.lines().collect();
    assert_eq!(lines[0], "- list [e1]");
    assert_eq!(lines[1], "  - listitem \"Item 1\" [e2]");
    assert_eq!(lines[99], "  - listitem \"Item 99\" [e100]");
    assert_eq!(
        lines[100],
        "(lines 1 to 100 of 251; the next page starts at line 101)"
    );
    let last = snapshot
        .page(
            &View {
                within: None,
                from: 201,
                lines: 100,
            },
            &mut refs.of(),
        )
        .unwrap();
    let lines: Vec<&str> = last.lines().collect();
    assert_eq!(lines[0], "  - listitem \"Item 200\" [e101]");
    assert_eq!(lines[50], "  - listitem \"Item 250\" [e151]");
    assert_eq!(lines[51], "(lines 201 to 251 of 251; the end of the page)");
}

#[test]
fn a_snapshot_scoped_to_a_reference_holds_only_that_element_and_what_is_inside_it() {
    let snapshot = build(&frames(), &HashSet::new(), &[]);
    let text = snapshot
        .page(
            &View {
                within: Some(key("MAIN-FRAME", 50)),
                from: 1,
                lines: 100,
            },
            &mut Refs::default().of(),
        )
        .unwrap();
    assert_eq!(
        text,
        "- frame \"payment\" [e1] http://other.test/pay.html\n  - button \"Pay now\" [e2]\n"
    );
}

#[test]
fn a_scope_that_is_not_on_the_page_is_refused() {
    let snapshot = build(&frames(), &HashSet::new(), &[]);
    let scoped = snapshot.page(
        &View {
            within: Some(key("MAIN-FRAME", 999)),
            from: 1,
            lines: 100,
        },
        &mut Refs::default().of(),
    );
    assert!(scoped.is_err());
}

#[test]
fn a_find_by_text_or_role_answers_references_with_the_line_to_read_from_and_the_frame() {
    let snapshot = build(&frames(), &HashSet::new(), &[]);
    let mut refs = Refs::default();
    let pay = snapshot
        .find(
            &Query {
                text: Some("pay NOW".into()),
                role: None,
            },
            None,
            &mut refs.of(),
        )
        .unwrap();
    assert_eq!(
        pay,
        "- button \"Pay now\" [e1] (line 9, in frame \"payment\")\n"
    );
    let boxes = snapshot
        .find(
            &Query {
                text: None,
                role: Some("textbox".into()),
            },
            None,
            &mut refs.of(),
        )
        .unwrap();
    assert_eq!(
        boxes,
        "- textbox \"Name\" [e2] value=\"Ada\" focused required (line 3)\n\
         - textbox \"Secret word\" [e3] value=\"open sesame\" (line 4)\n"
    );
}

#[test]
fn a_find_past_the_cut_reaches_an_element_no_first_page_shows() {
    let snapshot = build(&long_page(600), &HashSet::new(), &[]);
    let found = snapshot
        .find(
            &Query {
                text: Some("Item 543".into()),
                role: None,
            },
            None,
            &mut Refs::default().of(),
        )
        .unwrap();
    assert_eq!(found, "- listitem \"Item 543\" [e1] (line 544)\n");
}

#[test]
fn a_snapshot_is_built_by_value_not_by_the_engines_order_of_answering() {
    // A child listed before its parent, as an engine may: the tree is the same.
    let mut frames = frames();
    let Value::Array(nodes) = &mut frames[0].nodes else {
        panic!()
    };
    nodes.reverse();
    let reversed = build(&frames, &HashSet::new(), &[])
        .page(&all(), &mut Refs::default().of())
        .unwrap();
    let straight = build(&self::frames(), &HashSet::new(), &[])
        .page(&all(), &mut Refs::default().of())
        .unwrap();
    assert_eq!(reversed, straight);
}

#[test]
fn a_blanked_wrapper_with_no_name_is_blanked_and_what_it_holds_is_gone_everywhere() {
    // The page's generic wrapper (node 3) holds every field: blanking it blanks them all.
    let excluded = HashSet::from([key("MAIN-FRAME", 3)]);
    let text = build(&frames(), &excluded, &[])
        .page(&all(), &mut Refs::default().of())
        .unwrap();
    assert!(text.starts_with("- generic [e1] (blanked)\n"), "{text}");
    assert!(!text.contains("Ada"), "{text}");
    assert!(!text.contains("sesame"), "{text}");
}

#[test]
fn a_blanked_element_the_engine_ignores_is_still_blanked() {
    // Node 22 is ignored by the engine; its text would otherwise show through it.
    let excluded = HashSet::from([key("MAIN-FRAME", 70)]);
    let snapshot = build(&frames(), &excluded, &[]);
    let text = snapshot.page(&all(), &mut Refs::default().of()).unwrap();
    assert!(text.contains("- none ["), "{text}");
    assert!(text.contains("(blanked)"), "{text}");
    assert!(!text.contains("Below the fold"), "{text}");
    assert!(!snapshot.shows_text("below the fold"));
}

#[test]
fn text_blanked_before_is_taken_out_of_the_title_the_lines_and_the_addresses_whatever_its_case() {
    let mut frames = frames();
    frames[1].url = "http://other.test/pay.html?q=OPEN+SESAME".into();
    let blanked = [
        "open sesame".to_owned(),
        "frames and".to_owned(),
        "q=open+sesame".to_owned(),
    ];
    let snapshot = build(&frames, &HashSet::new(), &blanked);
    assert_eq!(snapshot.title(), "(blanked) fields");
    let text = snapshot.page(&all(), &mut Refs::default().of()).unwrap();
    assert!(!text.to_lowercase().contains("sesame"), "{text}");
    assert!(text.contains("pay.html?(blanked)"), "{text}");
    assert!(text.contains("- heading \"(blanked) fields\""), "{text}");
}

#[test]
fn an_echo_longer_than_a_line_shows_is_blanked_before_it_is_cut() {
    let mut frames = frames();
    let long = "x".repeat(195) + "open sesame";
    let Value::Array(nodes) = &mut frames[0].nodes else {
        panic!()
    };
    let echo = nodes.iter_mut().find(|n| n["nodeId"] == "16").unwrap();
    echo["name"]["value"] = Value::from(long);
    let excluded = HashSet::from([key("MAIN-FRAME", 21)]);
    let text = build(&frames, &excluded, &[])
        .page(&all(), &mut Refs::default().of())
        .unwrap();
    assert!(!text.contains("open"), "{text}");
}

#[test]
fn the_focused_element_is_known_so_the_field_typed_into_can_be_blanked() {
    let snapshot = build(&frames(), &HashSet::new(), &[]);
    assert_eq!(snapshot.focused(), Some(&key("MAIN-FRAME", 20)));
}
