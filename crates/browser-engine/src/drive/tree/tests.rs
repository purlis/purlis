// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

use super::{FrameAx, NodeKey, Query, View, build, scrubbed, scrubbed_address};

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
    let blanked = ["open sesame", "frames and"];
    let snapshot = build(&frames, &HashSet::new(), &blanked);
    assert_eq!(snapshot.title(), "(blanked) fields");
    let text = snapshot.page(&all(), &mut Refs::default().of()).unwrap();
    assert!(!text.to_lowercase().contains("sesame"), "{text}");
    assert!(text.contains("pay.html?q=(blanked)"), "{text}");
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
    assert_eq!(snapshot.focused(), &[key("MAIN-FRAME", 20)]);
}

/// A page whose document and one of its fields both say they have the focus, as a real engine
/// says once the agent has clicked the field (#2294): the field is what has the focus, and a
/// node the engine names twice is one.
#[test]
fn a_focused_field_is_what_has_the_focus_though_its_document_says_it_has_it_too() {
    let focused = |mut node: Value| {
        node["properties"] = json!([{ "name": "focused", "value": { "type": "booleanOrUndefined", "value": true } }]);
        node
    };
    let page = |nodes: Vec<Value>| {
        vec![FrameAx {
            id: "CODE-FRAME".into(),
            name: String::new(),
            url: "http://fixture.test/login.html".into(),
            parent: None,
            owner: None,
            nodes: Value::Array(nodes),
        }]
    };
    let field = page(vec![
        focused(ax(1, "RootWebArea", "Sign in", None, &[2, 3])),
        focused(ax(2, "textbox", "User name", None, &[])),
        ax(3, "textbox", "Password", None, &[]),
    ]);
    assert_eq!(
        build(&field, &HashSet::new(), &[]).focused(),
        &[key("CODE-FRAME", 2)]
    );
    // The document alone, named twice, is the one thing with the focus.
    let mut twice = ax(4, "RootWebArea", "Sign in", None, &[]);
    twice["backendDOMNodeId"] = json!(1);
    let document = page(vec![
        focused(ax(1, "RootWebArea", "Sign in", None, &[4])),
        focused(twice),
    ]);
    assert_eq!(
        build(&document, &HashSet::new(), &[]).focused(),
        &[key("CODE-FRAME", 1)]
    );
}

/// A page whose two frames each say something in them has the focus, the one its document,
/// the other a field: neither frame holds the other's, so which has the focus is unclear, and
/// both are said (#2294). A document focused with a field in a frame it holds is the field's.
#[test]
fn a_document_focused_beside_a_field_in_another_frame_reads_as_unclear() {
    let focused = |mut node: Value| {
        node["properties"] = json!([{ "name": "focused", "value": { "type": "booleanOrUndefined", "value": true } }]);
        node
    };
    let frame = |id: &str, parent: Option<&str>, owner: Option<i64>, nodes: Vec<Value>| FrameAx {
        id: id.into(),
        name: String::new(),
        url: format!("http://fixture.test/{id}.html"),
        parent: parent.map(str::to_owned),
        owner,
        nodes: Value::Array(nodes),
    };
    // The main document holds two frames, A's element 2 and B's element 3.
    let main = |focused_doc: bool| {
        let doc = ax(1, "RootWebArea", "Main", None, &[2, 3]);
        frame(
            "MAIN",
            None,
            None,
            vec![
                if focused_doc { focused(doc) } else { doc },
                ax(2, "Iframe", "", None, &[]),
                ax(3, "Iframe", "", None, &[]),
            ],
        )
    };
    let a = frame(
        "A",
        Some("MAIN"),
        Some(2),
        vec![focused(ax(10, "RootWebArea", "A", None, &[]))],
    );
    let b = frame(
        "B",
        Some("MAIN"),
        Some(3),
        vec![
            ax(20, "RootWebArea", "B", None, &[21]),
            focused(ax(21, "textbox", "Code", None, &[])),
        ],
    );
    assert_eq!(
        build(&[main(false), a, b.clone()], &HashSet::new(), &[]).focused(),
        &[key("B", 21), key("A", 10)],
        "two frames apart each say they have it"
    );
    assert_eq!(
        build(&[main(true), b], &HashSet::new(), &[]).focused(),
        &[key("B", 21)],
        "the main document holds the frame with the field"
    );
}

fn find_pay(frames: &[FrameAx], blanked: &[&str]) -> String {
    build(frames, &HashSet::new(), blanked)
        .find(
            &Query {
                text: Some("Pay now".into()),
                role: None,
            },
            None,
            &mut Refs::default().of(),
        )
        .unwrap()
}

#[test]
fn an_unnamed_frame_labelled_by_an_address_holding_blanked_text_shows_none_of_it() {
    let mut frames = frames();
    frames[1].name = String::new();
    frames[1].url = "http://other.test/pay.html?q=open%20sesame".into();
    let Value::Array(nodes) = &mut frames[0].nodes else {
        panic!()
    };
    let holder = nodes.iter_mut().find(|n| n["nodeId"] == "20").unwrap();
    holder["name"]["value"] = Value::from("");
    let found = find_pay(&frames, &["open sesame"]);
    assert_eq!(
        found,
        "- button \"Pay now\" [e1] (line 9, in frame \"http://other.test/pay.html?q=(blanked)\")\n"
    );
}

#[test]
fn a_frame_named_with_blanked_text_shows_none_of_it() {
    let mut frames = frames();
    frames[1].name = "Open Sesame".into();
    let found = find_pay(&frames, &["open sesame"]);
    assert!(found.contains("in frame \"(blanked)\""), "{found}");
    assert!(!found.to_lowercase().contains("sesame"), "{found}");
}

#[test]
fn blanked_text_is_found_in_an_address_however_the_address_encodes_it() {
    let cases = [
        ("hunter2~", "http://fixture.test/login?p=hunter2%7E"),
        ("hunter2~", "http://fixture.test/login?p=hunter2~"),
        ("pass word!", "http://fixture.test/login?p=pass%20word!"),
        ("pass word!", "http://fixture.test/login?p=pass+word%21"),
        ("a*b*c", "http://fixture.test/login?p=a%2Ab%2Ac"),
        ("a*b*c", "http://fixture.test/login?p=a*b*c"),
    ];
    for (value, address) in cases {
        assert_eq!(
            scrubbed_address(address, &[value]),
            "http://fixture.test/login?p=(blanked)",
            "{value} in {address}"
        );
    }
    // An address that holds nothing blanked is given back as written.
    assert_eq!(
        scrubbed_address("http://fixture.test/a%20b?c=d+e", &["zzz"]),
        "http://fixture.test/a%20b?c=d+e"
    );
}

#[test]
fn a_kept_text_and_its_beginnings_are_taken_out_whole_whatever_order_they_were_kept_in() {
    let kept = ["squ", "squi", "squir", "squirre", "squirrel9"];

    let said = scrubbed("Signed in as SQUIRREL9 today", &kept);

    assert_eq!(said, "Signed in as (blanked) today");
}

/// One node of an engine's tree: `id` is both its node id and its element's id.
fn ax(id: i64, role: &str, name: &str, value: Option<&str>, children: &[i64]) -> Value {
    let mut node = json!({
        "nodeId": id.to_string(),
        "ignored": false,
        "role": { "type": "role", "value": role },
        "name": { "type": "computedString", "value": name },
        "properties": [],
        "childIds": children.iter().map(i64::to_string).collect::<Vec<_>>(),
        "backendDOMNodeId": id,
        "frameId": "CODE-FRAME",
    });
    if let Some(value) = value {
        node["value"] = json!({ "type": "string", "value": value });
    }
    node
}

/// A page asking for a code in six boxes of one character each (a dash between the third and
/// the fourth), the code typed in, and the page then showing it whole: in its title, a line of
/// its own and the address of a frame.
fn code_page(boxes: [&str; 6], shows: &str) -> Vec<FrameAx> {
    let mut nodes = vec![
        ax(
            1,
            "RootWebArea",
            &format!("Code {shows}"),
            None,
            &[2, 30, 40],
        ),
        ax(
            2,
            "group",
            "Enter the code we sent",
            None,
            &[11, 12, 13, 20, 14, 15, 16],
        ),
        ax(20, "StaticText", "–", None, &[]),
        ax(
            30,
            "paragraph",
            &format!("You entered {shows}, thank you"),
            None,
            &[],
        ),
        ax(40, "Iframe", "", None, &[]),
    ];
    for (at, held) in boxes.iter().enumerate() {
        let id = 11 + i64::try_from(at).unwrap();
        nodes.push(ax(
            id,
            "textbox",
            &format!("Digit {}", at + 1),
            Some(held),
            &[],
        ));
    }
    vec![
        FrameAx {
            id: "CODE-FRAME".into(),
            name: String::new(),
            url: "http://fixture.test/code.html".into(),
            parent: None,
            owner: None,
            nodes: Value::Array(nodes),
        },
        FrameAx {
            id: "CHECK-FRAME".into(),
            name: String::new(),
            url: format!("http://fixture.test/check.html?code={shows}"),
            parent: Some("CODE-FRAME".into()),
            owner: Some(40),
            nodes: Value::Array(Vec::new()),
        },
    ]
}

fn boxes() -> HashSet<NodeKey> {
    (11..=16).map(|id| key("CODE-FRAME", id)).collect()
}

#[test]
fn a_code_typed_across_several_boxes_is_taken_out_whole_wherever_the_page_shows_it() {
    for shows in ["739214", "739–214"] {
        let snapshot = build(
            &code_page(["7", "3", "9", "2", "1", "4"], shows),
            &boxes(),
            &[],
        );
        let text = snapshot.page(&all(), &mut Refs::default().of()).unwrap();

        assert_eq!(snapshot.title(), "Code (blanked)", "{shows}");
        assert!(
            text.contains("- paragraph \"You entered (blanked), thank you\""),
            "{shows}:\n{text}"
        );
        assert!(
            text.contains("check.html?code=(blanked)"),
            "{shows}:\n{text}"
        );
        for part in ["739", "214"] {
            assert!(!text.contains(part), "{part} in\n{text}");
        }
        assert!(!snapshot.shows_text("739214"));
        assert!(snapshot.blanked_text().texts().contains(&"739214"));
    }
}

#[test]
fn a_code_is_taken_out_with_any_spaces_or_dashes_the_page_puts_between_its_digits() {
    let snapshot = build(
        &code_page(["7", "3", "9", "2", "1", "4"], "7 3 9-2 1 4"),
        &boxes(),
        &[],
    );
    let text = snapshot.page(&all(), &mut Refs::default().of()).unwrap();

    assert!(
        text.contains("- paragraph \"You entered (blanked), thank you\""),
        "{text}"
    );
}

#[test]
fn a_blanked_field_holding_a_value_of_its_own_next_to_the_boxes_is_no_part_of_their_code() {
    let mut frames = code_page(["7", "3", "9", "2", "1", "4"], "739214");
    let nodes = frames[0].nodes.as_array_mut().unwrap();
    nodes.push(ax(17, "textbox", "Email", Some("ada@fixture.test"), &[]));
    let group = nodes.iter_mut().find(|n| n["nodeId"] == "2").unwrap();
    group["childIds"].as_array_mut().unwrap().push(json!("17"));
    let mut excluded = boxes();
    excluded.insert(key("CODE-FRAME", 17));

    let snapshot = build(&frames, &excluded, &[]);
    let text = snapshot.page(&all(), &mut Refs::default().of()).unwrap();

    assert!(
        text.contains("- paragraph \"You entered (blanked), thank you\""),
        "{text}"
    );
}

#[test]
fn a_gone_line_holds_nothing_kept_out_since_it_was_read_in_an_address_either() {
    // #2137: a line read before a text was kept out is told without it, encoded or not.
    let snapshot = build(&frames(), &HashSet::new(), &[]);
    let before: Vec<super::Seen> = vec![
        (
            None,
            "- link \"Find\" url=http://fixture.test/?q=zq7%20kp9".into(),
        ),
        (None, "- text \"zq7 kp9 was typed\"".into()),
    ];
    let mut refer = |_: &NodeKey| "e1".to_owned();
    let mut kept = super::Kept::default();
    kept.run(&["zq7 kp9"]);
    let told = snapshot.changes(&before, 1000, &kept, &mut refer);
    assert!(told.contains("gone: - link \"Find\""), "{told}");
    assert!(told.contains("gone: - text \""), "{told}");
    assert!(!told.contains("zq7"), "{told}");
}
