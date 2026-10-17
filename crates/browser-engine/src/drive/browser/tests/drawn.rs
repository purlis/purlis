// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A screenshot paints over what was kept out wherever the page draws it (#2303): text the
//! tree does not show, a canvas, an image, an element drawn with an image, and a frame the read
//! does not reach; and it is withheld when what the page draws cannot be read.

use serde_json::{Value, json};

use super::takeover::{aim_now, key, the_person_types_the_code};
use super::{browser, focus_on, id, pixel, ref_of, snapshot, wait_for};
use crate::cdp::CallError;
use crate::drive::drawn::STYLES;
use crate::drive::{Call, Content, PageId, Refusal};

/// The engine's numbers for the kinds of node.
const ELEMENT_NODE: i64 = 1;
const TEXT_NODE: i64 = 3;
const DOCUMENT_NODE: i64 = 9;

/// A document of a stand-in page as the engine's layout read lists it: its nodes, and which of
/// them are laid out.
struct Document {
    frame: String,
    /// The address of the document, as the engine read it.
    url: String,
    nodes: Vec<Node>,
}

/// The text a laid-out node draws, and its styles by name.
type Layout = (Option<String>, Vec<(String, String)>);

#[derive(Default, Clone)]
struct Node {
    backend: i64,
    parent: Option<usize>,
    kind: i64,
    name: String,
    value: String,
    attributes: Vec<(String, String)>,
    input: Option<String>,
    pseudo: Option<String>,
    /// Laid out, with the text it draws (for a text) and its styles.
    laid_out: Option<Layout>,
}

impl Document {
    /// The document `frame` shows in the stand-in's frames page.
    fn new(frame: &str) -> Self {
        let url = match frame {
            "PAGE-1" => "http://fixture.test/frames.html",
            "PAYMENT-FRAME" | "PAYMENT" => "http://other.test/pay.html",
            _ => "http://fixture.test/new.html",
        };
        Self::at(frame, url)
    }

    /// A document of `frame` at `url`.
    fn at(frame: &str, url: &str) -> Self {
        let mut document = Self {
            frame: frame.to_owned(),
            url: url.to_owned(),
            nodes: Vec::new(),
        };
        document.push(Node {
            backend: 1,
            kind: DOCUMENT_NODE,
            name: "#document".into(),
            laid_out: Some((None, Vec::new())),
            ..Node::default()
        });
        document
    }

    fn push(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// A laid-out element under `parent` (the document when `None`).
    fn element(&mut self, backend: i64, parent: Option<usize>, name: &str) -> usize {
        self.push(Node {
            backend,
            parent: Some(parent.unwrap_or(0)),
            kind: ELEMENT_NODE,
            name: name.into(),
            laid_out: Some((None, Vec::new())),
            ..Node::default()
        })
    }

    /// A laid-out text under `parent`, drawing `text`.
    fn text(&mut self, backend: i64, parent: usize, text: &str) -> usize {
        self.push(Node {
            backend,
            parent: Some(parent),
            kind: TEXT_NODE,
            name: "#text".into(),
            value: text.into(),
            laid_out: Some((Some(text.into()), Vec::new())),
            ..Node::default()
        })
    }

    fn node(&mut self, at: usize) -> &mut Node {
        &mut self.nodes[at]
    }

    /// The answer of `DOMSnapshot.captureSnapshot`, as the engine gives it.
    fn answer(&self) -> Value {
        let mut strings: Vec<String> = Vec::new();
        let mut index = |text: &str| -> i64 {
            if text.is_empty() {
                return -1;
            }
            let at = strings.iter().position(|s| s == text).unwrap_or_else(|| {
                strings.push(text.to_owned());
                strings.len() - 1
            });
            i64::try_from(at).unwrap()
        };
        let frame = index(&self.frame);
        let url = index(&self.url);
        let mut nodes = json!({
            "parentIndex": [], "nodeType": [], "nodeName": [], "nodeValue": [],
            "backendNodeId": [], "attributes": [],
            "inputValue": {"index": [], "value": []},
            "textValue": {"index": [], "value": []},
            "pseudoType": {"index": [], "value": []},
            "contentDocumentIndex": {"index": [], "value": []},
        });
        let mut layout = json!({"nodeIndex": [], "styles": [], "text": [], "bounds": []});
        let push = |v: &mut Value, at: &str, item: Value| {
            v.pointer_mut(at)
                .unwrap()
                .as_array_mut()
                .unwrap()
                .push(item);
        };
        for (at, node) in self.nodes.iter().enumerate() {
            push(
                &mut nodes,
                "/parentIndex",
                json!(node.parent.map_or(-1, |p| i64::try_from(p).unwrap())),
            );
            push(&mut nodes, "/nodeType", json!(node.kind));
            push(&mut nodes, "/nodeName", json!(index(&node.name)));
            push(&mut nodes, "/nodeValue", json!(index(&node.value)));
            push(&mut nodes, "/backendNodeId", json!(node.backend));
            let attributes: Vec<i64> = node
                .attributes
                .iter()
                .flat_map(|(k, v)| [index(k), index(v)])
                .collect();
            push(&mut nodes, "/attributes", json!(attributes));
            if let Some(input) = &node.input {
                push(&mut nodes, "/inputValue/index", json!(at));
                push(&mut nodes, "/inputValue/value", json!(index(input)));
            }
            if let Some(pseudo) = &node.pseudo {
                push(&mut nodes, "/pseudoType/index", json!(at));
                push(&mut nodes, "/pseudoType/value", json!(index(pseudo)));
            }
            if let Some((text, styles)) = &node.laid_out {
                push(&mut layout, "/nodeIndex", json!(at));
                push(
                    &mut layout,
                    "/text",
                    json!(text.as_deref().map_or(-1, &mut index)),
                );
                // As the engine does, only an element has styles.
                let styles: Vec<i64> = STYLES
                    .iter()
                    .filter(|_| node.kind == ELEMENT_NODE)
                    .map(|name| {
                        let value = styles
                            .iter()
                            .find(|(n, _)| n == name)
                            .map_or("none", |(_, v)| v.as_str());
                        index(value)
                    })
                    .collect();
                push(&mut layout, "/styles", json!(styles));
                push(&mut layout, "/bounds", json!([0, 0, 1, 1]));
            }
        }
        json!({
            "documents": [{"frameId": frame, "documentURL": url, "nodes": nodes, "layout": layout,
                "textBoxes": {"layoutIndex": [], "bounds": [], "start": [], "length": []}}],
            "strings": strings,
        })
    }
}

/// What a session's documents draw when a test says nothing: a document with nothing in it.
pub(super) fn nothing(frame: &str) -> Value {
    Document::new(frame).answer()
}

/// The stand-in's page draws `document` in session `session`, each of `quads` where the engine
/// places that node.
fn draws(engine: &super::Engine, session: &str, document: &Document, quads: &[(i64, [f64; 4])]) {
    let mut model = super::super::lock(&engine.model);
    model.drawn.insert(session.to_owned(), document.answer());
    for (backend, [left, top, right, bottom]) in quads {
        model.quads.insert(
            *backend,
            vec![*left, *top, *right, *top, *right, *bottom, *left, *bottom],
        );
    }
}

/// The page keeps "open sesame" out: its Secret word field is blanked (its box, painted over,
/// is (0, 1) to (8, 7) on the 10 by 10 picture).
fn keep_out_the_secret(browser: &super::Browser) {
    let secret = ref_of(&snapshot(browser, 100), "- textbox \"Secret word\"");
    browser.blank(&secret).unwrap();
}

fn shot(browser: &super::Browser, n: u32) -> Vec<u8> {
    match browser
        .call(&id(n), Call::Screenshot { page: None })
        .unwrap()
        .content
    {
        Content::Image(png) => png,
        other => panic!("a screenshot is an image: {other:?}"),
    }
}

const GREY: [u8; 3] = [128, 128, 128];
const WHITE: [u8; 3] = [255, 255, 255];

#[test]
fn a_screenshot_paints_over_every_canvas_and_image_once_the_page_keeps_something_out() {
    let (browser, engine) = browser();
    let mut page = Document::new("PAGE-1");
    page.element(80, None, "CANVAS");
    page.element(81, None, "IMG");
    draws(
        &engine,
        "S-PAGE-1",
        &page,
        &[(80, [8.0, 8.0, 9.0, 9.0]), (81, [0.0, 9.0, 1.0, 9.5])],
    );
    let before = shot(&browser, 1);
    assert_eq!(pixel(&before, 9, 9), WHITE, "nothing is kept out yet");

    keep_out_the_secret(&browser);
    let after = shot(&browser, 2);

    assert_eq!(pixel(&after, 9, 9), GREY, "the canvas");
    assert_eq!(pixel(&after, 0, 9), GREY, "the image");
    assert_eq!(pixel(&after, 9, 0), WHITE);
}

#[test]
fn a_screenshot_paints_over_text_the_tree_does_not_show_that_holds_what_was_kept_out() {
    let (browser, engine) = browser();
    let mut page = Document::new("PAGE-1");
    // A copy the page hides from its tree (`aria-hidden`): the text has no box of its own the
    // engine gives, so the span it is in is painted.
    let hidden = page.element(82, None, "SPAN");
    page.node(hidden).attributes = vec![("aria-hidden".into(), "true".into())];
    page.text(83, hidden, "You said Open Sesame");
    // Text the page's style draws (`content:`), on a pseudo-element of a paragraph.
    let paragraph = page.element(84, None, "P");
    let after = page.push(Node {
        backend: 85,
        parent: Some(paragraph),
        kind: ELEMENT_NODE,
        name: "::after".into(),
        pseudo: Some("after".into()),
        laid_out: Some((None, Vec::new())),
        ..Node::default()
    });
    page.push(Node {
        backend: 86,
        parent: Some(after),
        kind: TEXT_NODE,
        name: "#text".into(),
        laid_out: Some((Some("open sesame".into()), Vec::new())),
        ..Node::default()
    });
    // Text that holds nothing kept out stays.
    let other = page.element(87, None, "SPAN");
    page.text(88, other, "Welcome back");
    draws(
        &engine,
        "S-PAGE-1",
        &page,
        &[
            (82, [8.0, 8.0, 9.0, 9.0]),
            (84, [0.0, 9.0, 1.0, 9.5]),
            (87, [8.0, 0.0, 9.0, 0.5]),
        ],
    );
    keep_out_the_secret(&browser);

    let after = shot(&browser, 1);

    assert_eq!(pixel(&after, 9, 9), GREY, "the hidden copy");
    assert_eq!(pixel(&after, 0, 9), GREY, "the text the style draws");
    assert_eq!(
        pixel(&after, 9, 0),
        WHITE,
        "text that holds nothing kept out"
    );
}

#[test]
fn a_screenshot_paints_over_a_field_or_list_that_shows_what_was_kept_out() {
    let (browser, engine) = browser();
    let mut page = Document::new("PAGE-1");
    let mirror = page.element(90, None, "INPUT");
    page.node(mirror).input = Some("open sesame".into());
    let list = page.element(91, None, "SELECT");
    let option = page.push(Node {
        backend: 92,
        parent: Some(list),
        kind: ELEMENT_NODE,
        name: "OPTION".into(),
        ..Node::default()
    });
    page.push(Node {
        backend: 93,
        parent: Some(option),
        kind: TEXT_NODE,
        name: "#text".into(),
        value: "Recent: open sesame".into(),
        ..Node::default()
    });
    draws(
        &engine,
        "S-PAGE-1",
        &page,
        &[(90, [8.0, 8.0, 9.0, 9.0]), (91, [0.0, 9.0, 1.0, 9.5])],
    );
    keep_out_the_secret(&browser);

    let after = shot(&browser, 1);

    assert_eq!(pixel(&after, 9, 9), GREY, "the field");
    assert_eq!(pixel(&after, 0, 9), GREY, "the drop-down");
}

#[test]
fn a_screenshot_paints_over_an_element_drawn_with_an_image_or_a_paint_script() {
    let (browser, engine) = browser();
    let mut page = Document::new("PAGE-1");
    let pictured = page.element(94, None, "DIV");
    page.node(pictured).laid_out = Some((
        None,
        vec![(
            "background-image".into(),
            "url(\"data:image/png;base64,AAAA\")".into(),
        )],
    ));
    let painted = page.element(95, None, "DIV");
    page.node(painted).laid_out = Some((None, vec![("mask-image".into(), "paint(echo)".into())]));
    let plain = page.element(96, None, "DIV");
    page.node(plain).laid_out = Some((
        None,
        vec![(
            "background-image".into(),
            "linear-gradient(red, blue)".into(),
        )],
    ));
    draws(
        &engine,
        "S-PAGE-1",
        &page,
        &[
            (94, [8.0, 8.0, 9.0, 9.0]),
            (95, [0.0, 9.0, 1.0, 9.5]),
            (96, [8.0, 0.0, 9.0, 0.5]),
        ],
    );
    keep_out_the_secret(&browser);

    let after = shot(&browser, 1);

    assert_eq!(pixel(&after, 9, 9), GREY, "an image");
    assert_eq!(pixel(&after, 0, 9), GREY, "a paint script");
    assert_eq!(pixel(&after, 9, 0), WHITE, "a gradient");
}

#[test]
fn a_screenshot_paints_over_a_frame_whose_document_is_read_nowhere() {
    let (browser, engine) = browser();
    let mut page = Document::new("PAGE-1");
    // The payment frame's element: its document is read in its own session.
    page.element(50, None, "IFRAME");
    page.element(97, None, "IFRAME");
    draws(
        &engine,
        "S-PAGE-1",
        &page,
        &[(50, [8.0, 0.0, 9.0, 0.5]), (97, [8.0, 8.0, 9.0, 9.0])],
    );
    keep_out_the_secret(&browser);

    let after = shot(&browser, 1);

    assert_eq!(pixel(&after, 9, 9), GREY, "a frame read nowhere");
    assert_eq!(
        pixel(&after, 9, 0),
        WHITE,
        "a frame read in its own session"
    );
}

#[test]
fn a_screenshot_is_withheld_when_what_the_page_draws_cannot_be_read() {
    let (browser, engine) = browser();
    keep_out_the_secret(&browser);
    engine.on(|sent, _| {
        (sent.method == "DOMSnapshot.captureSnapshot").then_some(Err(CallError::Engine {
            code: -32000,
            message: "Failed".into(),
        }))
    });
    let shot = browser.call(&id(1), Call::Screenshot { page: None });
    assert!(matches!(shot, Err(Refusal::Withheld(_))), "{shot:?}");
}

#[test]
fn a_screenshot_is_withheld_when_a_frame_says_what_it_draws_in_words_purlis_cannot_read() {
    let (browser, engine) = browser();
    keep_out_the_secret(&browser);
    super::super::lock(&engine.model)
        .drawn
        .insert("S-PAYMENT".into(), json!({"documents": [{"nodes": {}}]}));
    let shot = browser.call(&id(1), Call::Screenshot { page: None });
    assert!(matches!(shot, Err(Refusal::Withheld(_))), "{shot:?}");
}

#[test]
fn a_screenshot_is_withheld_when_the_page_draws_an_element_again_outside_its_box() {
    let (browser, engine) = browser();
    let mut page = Document::new("PAGE-1");
    let canvas = page.element(98, None, "CANVAS");
    page.node(canvas).laid_out = Some((
        None,
        vec![("-webkit-box-reflect".into(), "below 0px".into())],
    ));
    draws(&engine, "S-PAGE-1", &page, &[(98, [8.0, 8.0, 9.0, 9.0])]);
    keep_out_the_secret(&browser);
    let shot = browser.call(&id(1), Call::Screenshot { page: None });
    assert!(matches!(shot, Err(Refusal::Withheld(_))), "{shot:?}");
}

#[test]
fn a_screenshot_paints_over_what_was_kept_out_shown_split_over_elements() {
    let (browser, engine) = browser();
    let mut page = Document::new("PAGE-1");
    let line = page.element(99, None, "P");
    page.text(100, line, "Hello OPEN ");
    let bold = page.element(101, Some(line), "B");
    page.text(102, bold, "sesame");
    // The bold part is drawn outside the line's box.
    draws(
        &engine,
        "S-PAGE-1",
        &page,
        &[(99, [8.0, 8.0, 9.0, 9.0]), (102, [0.0, 9.0, 1.0, 9.5])],
    );
    keep_out_the_secret(&browser);

    let after = shot(&browser, 1);

    assert_eq!(pixel(&after, 9, 9), GREY, "the line");
    assert_eq!(pixel(&after, 0, 9), GREY, "the part drawn outside it");
}

#[test]
fn a_screenshot_paints_over_what_was_kept_out_spaced_otherwise_than_it_was_typed() {
    let (browser, engine) = browser();
    let mut page = Document::new("PAGE-1");
    let line = page.element(103, None, "P");
    page.text(104, line, "You said open\u{a0}\n   sesame");
    draws(&engine, "S-PAGE-1", &page, &[(103, [8.0, 8.0, 9.0, 9.0])]);
    keep_out_the_secret(&browser);

    let after = shot(&browser, 1);

    assert_eq!(pixel(&after, 9, 9), GREY);
}

#[test]
fn a_screenshot_paints_over_a_drop_down_whose_option_is_labelled_with_what_was_kept_out() {
    let (browser, engine) = browser();
    let mut page = Document::new("PAGE-1");
    let list = page.element(105, None, "SELECT");
    for (backend, name) in [(106, "OPTGROUP"), (107, "OPTION")] {
        page.push(Node {
            backend,
            parent: Some(list),
            kind: ELEMENT_NODE,
            name: name.into(),
            attributes: vec![("label".into(), "Saved: open sesame".into())],
            ..Node::default()
        });
    }
    let other = page.element(108, None, "SELECT");
    page.push(Node {
        backend: 109,
        parent: Some(other),
        kind: ELEMENT_NODE,
        name: "OPTION".into(),
        attributes: vec![("label".into(), "Nothing kept".into())],
        ..Node::default()
    });
    draws(
        &engine,
        "S-PAGE-1",
        &page,
        &[(105, [8.0, 8.0, 9.0, 9.0]), (108, [8.0, 0.0, 9.0, 0.5])],
    );
    keep_out_the_secret(&browser);

    let after = shot(&browser, 1);

    assert_eq!(pixel(&after, 9, 9), GREY, "the labelled drop-down");
    assert_eq!(
        pixel(&after, 9, 0),
        WHITE,
        "a drop-down that holds nothing kept out"
    );
}

#[test]
fn a_screenshot_paints_over_what_was_kept_out_inside_a_frame_where_the_frame_puts_it() {
    let (browser, engine) = browser();
    let mut frame = Document::new("PAYMENT-FRAME");
    let line = frame.element(110, None, "P");
    frame.text(111, line, "Card for open sesame");
    // Within the frame, the line is at (3, 3); the frame's box starts at (5, 5) on the page.
    draws(&engine, "S-PAYMENT", &frame, &[(110, [3.0, 3.0, 4.0, 4.0])]);
    frame_drawn_as(&engine, json!([5, 5, 9, 5, 9, 9, 5, 9]), 4.0, 4.0);
    keep_out_the_secret(&browser);

    let after = shot(&browser, 1);

    assert_eq!(pixel(&after, 9, 9), GREY);
    assert_eq!(
        pixel(&after, 3, 9),
        WHITE,
        "a frame drawn at its own size is painted over only where it draws the text"
    );
}

/// The payment frame's element is drawn at `quad` on the page (its border and content boxes
/// both), laid out `width` by `height` CSS pixels before any transform.
fn frame_drawn_as(engine: &super::Engine, quad: Value, width: f64, height: f64) {
    engine.on(move |sent, _| {
        (sent.method == "DOM.getBoxModel").then(|| {
            Ok(json!({"model":{"content":quad,"padding":quad,"border":quad,
                "margin":quad,"width":width,"height":height}}))
        })
    });
}

/// The payment frame shows "Card for open sesame" at (`x`, `y`) to (`x` + 1, `y` + 1) of its
/// own document.
fn the_frame_shows_the_secret_at(engine: &super::Engine, x: f64, y: f64) {
    let mut frame = Document::new("PAYMENT-FRAME");
    let line = frame.element(110, None, "P");
    frame.text(111, line, "Card for open sesame");
    draws(
        engine,
        "S-PAYMENT",
        &frame,
        &[(110, [x, y, x + 1.0, y + 1.0])],
    );
}

#[test]
fn a_screenshot_paints_over_the_whole_of_a_frame_the_page_draws_smaller_than_it_lays_it_out() {
    let (browser, engine) = browser();
    // The frame is laid out 10 by 10 and drawn half that size at (4, 4): the line at (9, 9) of
    // its document is drawn at (8.5, 8.5) on the page.
    the_frame_shows_the_secret_at(&engine, 9.0, 9.0);
    frame_drawn_as(&engine, json!([4, 4, 9, 4, 9, 9, 4, 9]), 10.0, 10.0);
    keep_out_the_secret(&browser);

    let after = shot(&browser, 1);

    assert_eq!(pixel(&after, 9, 9), GREY);
}

#[test]
fn a_screenshot_paints_over_the_whole_of_a_frame_the_page_turns() {
    let (browser, engine) = browser();
    // The frame is turned an eighth of a turn about (6, 6): its corners are the points of a
    // diamond.
    the_frame_shows_the_secret_at(&engine, 0.0, 0.0);
    frame_drawn_as(&engine, json!([6, 2, 10, 6, 6, 10, 2, 6]), 5.66, 5.66);
    keep_out_the_secret(&browser);

    let after = shot(&browser, 1);

    assert_eq!(pixel(&after, 9, 9), GREY);
}

#[test]
fn a_screenshot_is_withheld_when_where_a_frame_is_drawn_is_not_known() {
    let (browser, engine) = browser();
    the_frame_shows_the_secret_at(&engine, 0.0, 0.0);
    engine.on(|sent, _| {
        (sent.method == "DOM.getBoxModel").then(|| Ok(json!({"model":{"width":4,"height":4}})))
    });
    keep_out_the_secret(&browser);

    let shot = browser.call(&id(1), Call::Screenshot { page: None });

    assert!(matches!(shot, Err(Refusal::Withheld(_))), "{shot:?}");
}

#[test]
fn a_screenshot_paints_over_what_moved_while_the_picture_was_taken_the_whole_way() {
    let (browser, engine) = browser();
    let mut page = Document::new("PAGE-1");
    let toast = page.element(112, None, "DIV");
    page.text(113, toast, "Signed in as open sesame");
    draws(&engine, "S-PAGE-1", &page, &[]);
    keep_out_the_secret(&browser);
    // The toast slides down while the picture is taken: at the top before it, at the bottom
    // after.
    let asked = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let seen = asked.clone();
    engine.on(move |sent, _| {
        (sent.method == "DOM.getContentQuads" && sent.params["backendNodeId"] == 112).then(|| {
            let top = seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0;
            let y = if top { 0.0 } else { 8.0 };
            Ok(json!({"quads": [[9.0, y, 9.5, y, 9.5, y + 1.0, 9.0, y + 1.0]]}))
        })
    });

    let after = shot(&browser, 1);

    assert_eq!(pixel(&after, 9, 0), GREY, "where it was");
    assert_eq!(pixel(&after, 9, 9), GREY, "where it went");
    assert_eq!(pixel(&after, 9, 5), GREY, "the way between");
}

#[test]
fn a_screenshot_paints_over_what_the_person_typed_where_the_tree_does_not_show_it() {
    let (browser, engine) = browser();
    let mut page = Document::new("PAGE-1");
    let hidden = page.element(114, None, "SPAN");
    page.node(hidden).attributes = vec![("aria-hidden".into(), "true".into())];
    page.text(115, hidden, "Signed in as QZWV");
    draws(&engine, "S-PAGE-1", &page, &[(114, [8.0, 8.0, 9.0, 9.0])]);
    focus_on(&engine, &[]);
    browser.take_over();
    let aim = aim_now(&browser);
    for letter in ["q", "z", "w", "v"] {
        browser.person_input(&key(&aim, letter, true)).unwrap();
    }
    browser.hand_back();

    let after = shot(&browser, 1);

    assert_eq!(pixel(&after, 9, 9), GREY, "the hidden copy");
    assert_eq!(pixel(&after, 1, 1), WHITE, "nothing else");
}

/// A page `target` opens at `url`, opened by `opener` if any, and is set up: it is `p2`.
fn a_page_opens(engine: &super::Engine, target: &str, url: &str, opener: Option<&str>) -> PageId {
    let mut info = json!({"targetId": target, "type": "page", "title": "", "url": url,
        "attached": false});
    if let Some(opener) = opener {
        info["openerId"] = json!(opener);
    }
    engine.emit("Target.targetCreated", json!({ "targetInfo": info }), None);
    let session = format!("S-{target}");
    wait_for("the page is set up", || {
        engine
            .sent("Page.getFrameTree")
            .iter()
            .any(|s| s.session.as_deref() == Some(session.as_str()))
    });
    std::thread::sleep(std::time::Duration::from_millis(50));
    PageId::new("p2")
}

/// A canvas at (8, 8) on `target`'s page, its document at `url`, and nothing else.
fn a_canvas_on(engine: &super::Engine, target: &str, url: &str) {
    let mut page = Document::at(target, url);
    page.element(120, None, "CANVAS");
    draws(
        engine,
        &format!("S-{target}"),
        &page,
        &[(120, [8.0, 8.0, 9.0, 9.0])],
    );
}

fn shot_of(browser: &super::Browser, page: &PageId, n: u32) -> Vec<u8> {
    match browser
        .call(
            &id(n),
            Call::Screenshot {
                page: Some(page.clone()),
            },
        )
        .unwrap()
        .content
    {
        Content::Image(png) => png,
        other => panic!("a screenshot is an image: {other:?}"),
    }
}

#[test]
fn another_sites_canvas_and_images_are_not_painted_over() {
    let (browser, engine) = browser();
    keep_out_the_secret(&browser);
    let other = a_page_opens(&engine, "PAGE-8", "https://shop.example.com/", None);
    a_canvas_on(&engine, "PAGE-8", "https://shop.example.com/");

    let after = shot_of(&browser, &other, 1);

    assert_eq!(pixel(&after, 9, 9), WHITE);
}

#[test]
fn a_canvas_on_another_part_of_the_kept_site_is_painted_over() {
    let (browser, engine) = browser();
    keep_out_the_secret(&browser);
    let sub = a_page_opens(&engine, "PAGE-8", "http://www.fixture.test/next.html", None);
    a_canvas_on(&engine, "PAGE-8", "http://www.fixture.test/next.html");

    let after = shot_of(&browser, &sub, 1);

    assert_eq!(pixel(&after, 9, 9), GREY);
}

#[test]
fn a_canvas_in_a_page_the_kept_page_opened_is_painted_over() {
    let (browser, engine) = browser();
    keep_out_the_secret(&browser);
    let popup = a_page_opens(&engine, "PAGE-8", "about:blank", Some("PAGE-1"));
    a_canvas_on(&engine, "PAGE-8", "about:blank");

    let after = shot_of(&browser, &popup, 1);

    assert_eq!(pixel(&after, 9, 9), GREY);
}

#[test]
fn what_the_person_typed_is_painted_over_on_another_site_but_its_canvas_is_not() {
    let (browser, engine) = browser();
    focus_on(&engine, &[]);
    browser.take_over();
    let aim = aim_now(&browser);
    for letter in ["q", "z", "w", "v"] {
        browser.person_input(&key(&aim, letter, true)).unwrap();
    }
    browser.hand_back();
    let other = a_page_opens(&engine, "PAGE-8", "https://shop.example.com/", None);
    let mut page = Document::at("PAGE-8", "https://shop.example.com/");
    page.element(120, None, "CANVAS");
    let echo = page.element(121, None, "SPAN");
    page.text(122, echo, "Searching for qzwv");
    draws(
        &engine,
        "S-PAGE-8",
        &page,
        &[(120, [8.0, 8.0, 9.0, 9.0]), (121, [0.0, 9.0, 1.0, 9.5])],
    );

    let after = shot_of(&browser, &other, 1);

    assert_eq!(pixel(&after, 0, 9), GREY, "what the person typed");
    assert_eq!(pixel(&after, 9, 9), WHITE, "another site's canvas");
}

#[test]
fn a_page_opened_by_the_kept_page_on_another_site_is_painted_over() {
    let (browser, engine) = browser();
    keep_out_the_secret(&browser);
    let popup = a_page_opens(
        &engine,
        "PAGE-8",
        "https://pay.example.com/",
        Some("PAGE-1"),
    );
    a_canvas_on(&engine, "PAGE-8", "https://pay.example.com/");

    let after = shot_of(&browser, &popup, 1);

    assert_eq!(pixel(&after, 9, 9), GREY);
}

#[test]
fn a_page_still_showing_the_kept_sites_document_is_painted_over_whatever_its_address_says() {
    let (browser, engine) = browser();
    keep_out_the_secret(&browser);
    // The page was told it is on another site, but the document it shows is still the kept
    // site's.
    let page = a_page_opens(&engine, "PAGE-8", "https://shop.example.com/", None);
    a_canvas_on(&engine, "PAGE-8", "http://fixture.test/frames.html");

    let after = shot_of(&browser, &page, 1);

    assert_eq!(pixel(&after, 9, 9), GREY);
}

#[test]
fn a_page_on_no_site_opened_by_none_is_painted_over_while_anything_is_kept() {
    let (browser, engine) = browser();
    keep_out_the_secret(&browser);
    let blank = a_page_opens(&engine, "PAGE-8", "about:blank", None);
    a_canvas_on(&engine, "PAGE-8", "about:blank");

    let after = shot_of(&browser, &blank, 1);

    assert_eq!(pixel(&after, 9, 9), GREY);
}

#[test]
fn the_site_a_text_is_kept_on_is_the_one_of_the_document_its_page_shows() {
    let (browser, engine) = browser();
    // The page is told of an address on another site, while it still shows the fixture's
    // document: a load that has not yet replaced it.
    engine.emit(
        "Target.targetInfoChanged",
        json!({"targetInfo":{"targetId":"PAGE-1","type":"page","title":"","url":"https://shop.example.com/"}}),
        None,
    );
    std::thread::sleep(std::time::Duration::from_millis(30));
    keep_out_the_secret(&browser);
    let sub = a_page_opens(&engine, "PAGE-8", "http://www.fixture.test/next.html", None);
    a_canvas_on(&engine, "PAGE-8", "http://www.fixture.test/next.html");

    let after = shot_of(&browser, &sub, 1);

    assert_eq!(pixel(&after, 9, 9), GREY);
}

#[test]
fn a_screenshot_paints_over_a_code_typed_across_several_boxes_wherever_the_page_draws_it() {
    let (browser, engine) = browser();
    the_person_types_the_code(&browser, &engine, &[1, 2, 3, 4, 5, 6], false);
    let mut page = Document::new("PAGE-1");
    let said = page.element(116, None, "P");
    page.text(117, said, "Your code 739-214 is right");
    draws(&engine, "S-PAGE-1", &page, &[(116, [8.0, 8.0, 9.0, 9.0])]);

    let after = shot(&browser, 1);

    assert_eq!(pixel(&after, 9, 9), GREY);
}

#[test]
fn a_screenshot_is_withheld_when_the_frames_element_is_not_known() {
    let (browser, engine) = browser();
    the_frame_shows_the_secret_at(&engine, 0.0, 0.0);
    engine.on(|sent, _| (sent.method == "DOM.getFrameOwner").then(|| Ok(json!({"nodeId": 0}))));
    keep_out_the_secret(&browser);

    let shot = browser.call(&id(1), Call::Screenshot { page: None });

    assert!(matches!(shot, Err(Refusal::Withheld(_))), "{shot:?}");
}

/// The page draws its payment frame's element (node 50) inside a box drawn through `filter`,
/// and the Secret word field (node 21) as it is.
fn the_frame_is_drawn_through(engine: &super::Engine, filter: &str) {
    let mut page = Document::new("PAGE-1");
    let around = page.element(97, None, "DIV");
    page.node(around).laid_out = Some((None, vec![("filter".into(), filter.into())]));
    page.element(50, Some(around), "IFRAME");
    page.element(21, None, "INPUT");
    draws(engine, "S-PAGE-1", &page, &[]);
}

#[test]
fn a_screenshot_is_withheld_when_a_frame_is_drawn_through_a_filter_that_may_draw_it_elsewhere() {
    for filter in ["drop-shadow(rgb(0, 0, 0) 40px 40px 0px)", "url(\"#moved\")"] {
        let (browser, engine) = browser();
        the_frame_is_drawn_through(&engine, filter);
        keep_out_the_secret(&browser);

        let shot = browser.call(&id(1), Call::Screenshot { page: None });

        assert!(
            matches!(shot, Err(Refusal::Withheld(_))),
            "{filter}: {shot:?}"
        );
    }
}

#[test]
fn a_screenshot_is_withheld_when_a_blanked_field_is_drawn_through_such_a_filter() {
    let (browser, engine) = browser();
    let mut page = Document::new("PAGE-1");
    let field = page.element(21, None, "INPUT");
    page.node(field).laid_out = Some((
        None,
        vec![(
            "filter".into(),
            "drop-shadow(rgb(0, 0, 0) 40px 40px 0px)".into(),
        )],
    ));
    draws(&engine, "S-PAGE-1", &page, &[]);
    keep_out_the_secret(&browser);

    let shot = browser.call(&id(1), Call::Screenshot { page: None });

    assert!(matches!(shot, Err(Refusal::Withheld(_))), "{shot:?}");
}

#[test]
fn a_frame_drawn_through_a_filter_that_draws_it_in_its_box_does_not_withhold_the_screenshot() {
    let (browser, engine) = browser();
    the_frame_is_drawn_through(&engine, "blur(2px) grayscale(1)");
    keep_out_the_secret(&browser);

    let shot = browser.call(&id(1), Call::Screenshot { page: None });

    assert!(shot.is_ok(), "{shot:?}");
}
