// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A person's SVG as an icon (#2922): shapes and checked values come out, nothing else.

use super::*;

fn icon(text: &str) -> SafeSvg {
    read(text.as_bytes()).unwrap_or_else(|why| panic!("{why}: {text}"))
}

/// Every tag and every attribute name and value the icon holds, as one string, so a test can
/// say a word is nowhere in it.
fn everything(svg: &SafeSvg) -> String {
    let mut all = svg.view_box.clone();
    for shape in &svg.shapes {
        all.push_str(shape.tag);
        for (key, value) in &shape.attributes {
            all.push_str(key);
            all.push_str(value);
        }
    }
    all
}

#[test]
fn a_plain_icon_is_its_view_box_and_its_shapes() {
    let svg = icon(
        r##"<?xml version="1.0"?>
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#123">
          <path d="M1 1h22"/><circle cx="12" cy="12" r="4"/>
        </svg>"##,
    );
    assert_eq!(svg.view_box, "0 0 24 24");
    assert_eq!(svg.shapes.len(), 2);
    assert_eq!(svg.shapes[0].tag, "path");
    assert_eq!(svg.shapes[0].attributes["d"], "M1 1h22");
    // The root's paint is carried onto each shape.
    assert_eq!(svg.shapes[1].attributes["fill"], "none");
    assert_eq!(svg.shapes[1].attributes["stroke"], "#123");
}

#[test]
fn a_script_a_foreign_object_and_what_they_hold_are_left_out() {
    let svg = icon(
        r#"<svg viewBox="0 0 10 10">
          <script>alert(1)</script>
          <foreignObject><div xmlns="http://www.w3.org/1999/xhtml"><path d="M0 0"/></div></foreignObject>
          <style>path { fill: url(https://example.invalid/x) }</style>
          <g><a href="https://example.invalid"><path d="M5 5"/></a></g>
          <path d="M1 1" onclick="alert(1)" onload="x()"/>
        </svg>"#,
    );
    assert_eq!(svg.shapes.len(), 1);
    assert_eq!(
        svg.shapes[0].attributes.keys().copied().collect::<Vec<_>>(),
        ["d"]
    );
    let all = everything(&svg);
    for word in [
        "script",
        "alert",
        "foreignObject",
        "style",
        "href",
        "example",
        "onclick",
    ] {
        assert!(!all.contains(word), "{word} in {all}");
    }
}

#[test]
fn nothing_that_names_another_resource_is_kept() {
    let svg = icon(
        r##"<svg viewBox="0 0 10 10" xmlns:xlink="http://www.w3.org/1999/xlink">
          <defs><linearGradient id="g"/></defs>
          <use href="#p" xlink:href="https://example.invalid/i.svg#p"/>
          <image href="https://example.invalid/i.png" width="10" height="10"/>
          <path d="M1 1" fill="url(#g)" stroke="url(https://example.invalid/p)"
                style="fill: url(#g); stroke-width: 2; mask: url(#m)" filter="url(#f)"
                clip-path="url(#c)" mask="url(#m)"/>
        </svg>"##,
    );
    assert_eq!(svg.shapes.len(), 1);
    let shape = &svg.shapes[0];
    assert_eq!(shape.attributes["fill"], "currentColor");
    assert_eq!(shape.attributes["stroke"], "currentColor");
    assert_eq!(shape.attributes["stroke-width"], "2");
    let all = everything(&svg);
    for word in [
        "url",
        "example",
        "href",
        "filter",
        "mask",
        "clip-path",
        "#g",
    ] {
        assert!(!all.contains(word), "{word} in {all}");
    }
}

#[test]
fn a_value_outside_its_grammar_is_dropped() {
    let svg = icon(
        r#"<svg viewBox="0 0 10 10">
          <path d="M1 1 expression(x)" />
          <rect x="1" y="calc(1px)" width="2" height="2" transform="translate(1 2) evil(3)"
                fill-rule="weird" stroke-linecap="round"/>
        </svg>"#,
    );
    // A path whose d is not path data draws nothing, but it is still a shape the file has.
    assert!(!svg.shapes[0].attributes.contains_key("d"));
    let rect = &svg.shapes[1].attributes;
    assert_eq!(rect["x"], "1");
    assert!(!rect.contains_key("y"));
    assert!(!rect.contains_key("transform"));
    assert!(!rect.contains_key("fill-rule"));
    assert_eq!(rect["stroke-linecap"], "round");
}

#[test]
fn a_group_passes_its_paint_transform_and_opacity_on() {
    let svg = icon(
        r#"<svg width="32" height="16">
          <g fill="red" transform="translate(1,2)" opacity="0.5">
            <g transform="scale(2)" opacity="0.5"><path d="M0 0h1" fill="blue"/></g>
            <line x1="0" y1="0" x2="1" y2="1"/>
          </g>
        </svg>"#,
    );
    assert_eq!(svg.view_box, "0 0 32 16");
    let path = &svg.shapes[0].attributes;
    assert_eq!(path["fill"], "blue");
    assert_eq!(path["transform"], "translate(1,2) scale(2)");
    assert_eq!(path["opacity"], "0.25");
    let line = &svg.shapes[1].attributes;
    assert_eq!(line["fill"], "red");
    assert_eq!(line["transform"], "translate(1,2)");
}

#[test]
fn a_file_that_is_no_icon_is_refused_without_repeating_it() {
    for (text, why) in [
        ("not xml at all", "the file is not an SVG"),
        ("not xml at all <", "the file is not well-formed SVG"),
        ("<html><path d=\"M0 0\"/></html>", "the file is not an SVG"),
        ("<svg><path d=\"M0 0\"/></svg>", "the SVG has no view box"),
        (
            "<svg viewBox=\"0 0 0 10\"><path d=\"M0 0\"/></svg>",
            "the SVG has no view box",
        ),
        (
            "<svg viewBox=\"0 0 10 10\"><text>hi</text></svg>",
            "the SVG draws no shape an icon may",
        ),
        (
            "<svg viewBox=\"0 0 10 10\"><path d=\"M0 0\"></svg>",
            "the file is not well-formed SVG",
        ),
    ] {
        let refused = read(text.as_bytes()).expect_err(text);
        assert_eq!(refused.0, why, "{text}");
    }
    assert_eq!(
        read(&[0xff, 0xfe, 0x00]).expect_err("not text").0,
        "the file is not UTF-8 text"
    );
}

#[test]
fn an_entity_is_never_expanded() {
    let text = r#"<?xml version="1.0"?>
        <!DOCTYPE svg [<!ENTITY big "M0 0h1h1h1h1"><!ENTITY ext SYSTEM "file:///etc/passwd">]>
        <svg viewBox="0 0 10 10"><path d="&big;"/><path d="M1 1">&ext;</path></svg>"#;
    let svg = icon(text);
    assert!(!svg.shapes[0].attributes.contains_key("d"));
    assert_eq!(svg.shapes[1].attributes["d"], "M1 1");
}

#[test]
fn an_icon_is_bounded() {
    let big = format!(
        "<svg viewBox=\"0 0 1 1\"><path d=\"M0 0\"/>{}</svg>",
        " ".repeat(MOST_BYTES as usize)
    );
    assert_eq!(
        read(big.as_bytes()).expect_err("too big").0,
        "the file is larger than an icon may be"
    );

    let many = format!(
        "<svg viewBox=\"0 0 1 1\">{}</svg>",
        "<path d=\"M0 0\"/>".repeat(MOST_SHAPES + 1)
    );
    assert_eq!(
        read(many.as_bytes()).expect_err("too many shapes").0,
        "the file draws more shapes than an icon may"
    );

    let deep = format!(
        "<svg viewBox=\"0 0 1 1\">{}<path d=\"M0 0\"/>{}</svg>",
        "<g>".repeat(MOST_DEPTH),
        "</g>".repeat(MOST_DEPTH)
    );
    assert_eq!(
        read(deep.as_bytes()).expect_err("too deep").0,
        "the file nests its elements deeper than an icon may"
    );

    let crowded = format!(
        "<svg viewBox=\"0 0 1 1\"><path d=\"M0 0\"/>{}</svg>",
        "<desc/>".repeat(MOST_ELEMENTS)
    );
    assert_eq!(
        read(crowded.as_bytes()).expect_err("too many elements").0,
        "the file holds more elements than an icon may"
    );
}

#[test]
fn a_prefixed_element_is_not_a_shape() {
    let svg = icon(
        r#"<svg viewBox="0 0 1 1" xmlns:s="http://www.w3.org/2000/svg">
          <s:script>x</s:script><s:path d="M0 0"/><path d="M1 1"/></svg>"#,
    );
    assert_eq!(svg.shapes.len(), 1);
    assert_eq!(svg.shapes[0].attributes["d"], "M1 1");
}
