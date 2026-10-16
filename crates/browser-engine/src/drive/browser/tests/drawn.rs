// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A screenshot paints over what was kept out wherever the page draws it (#2303): text the
//! tree does not show, a canvas, an image, an element drawn with an image, and a frame the read
//! does not reach; and it is withheld when what the page draws cannot be read.

use serde_json::{Value, json};

use super::{browser, id, pixel, ref_of, snapshot};
use crate::cdp::CallError;
use crate::drive::drawn::STYLES;
use crate::drive::{Call, Content, Refusal};

/// A document of a stand-in page as the engine's layout read lists it: its nodes, and which of
/// them are laid out.
struct Document {
    frame: String,
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
    fn new(frame: &str) -> Self {
        let mut document = Self {
            frame: frame.to_owned(),
            nodes: Vec::new(),
        };
        document.push(Node {
            backend: 1,
            kind: 9,
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
            kind: 1,
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
            kind: 3,
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
                    .filter(|_| node.kind == 1)
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
            "documents": [{"frameId": frame, "nodes": nodes, "layout": layout,
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
        kind: 1,
        name: "::after".into(),
        pseudo: Some("after".into()),
        laid_out: Some((None, Vec::new())),
        ..Node::default()
    });
    page.push(Node {
        backend: 86,
        parent: Some(after),
        kind: 3,
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
        kind: 1,
        name: "OPTION".into(),
        ..Node::default()
    });
    page.push(Node {
        backend: 93,
        parent: Some(option),
        kind: 3,
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
    draws(&engine, "S-PAGE-1", &page, &[(99, [8.0, 8.0, 9.0, 9.0])]);
    keep_out_the_secret(&browser);

    let after = shot(&browser, 1);

    assert_eq!(pixel(&after, 9, 9), GREY);
}
