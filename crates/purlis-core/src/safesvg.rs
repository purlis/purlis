// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A person's own SVG, read as shapes and nothing else** (#2922, ruling #2866 ·2): the file a
//! profile names as its icon.
//!
//! The file is untrusted input. It is never handed to the window as text: it is parsed, and
//! what comes out is a flat list of shapes, each one of a closed set of elements with a closed
//! set of attributes whose values are held to a grammar. The window draws that list with its
//! own elements, so nothing the file says is ever markup there.
//!
//! - **Bounded.** The file is at most [`MOST_BYTES`]; at most [`MOST_ELEMENTS`] elements are
//!   read, nested at most [`MOST_DEPTH`] deep, and at most [`MOST_SHAPES`] shapes come out,
//!   which come to at most [`MOST_OUTPUT`] once the groups' attributes are carried onto them.
//! - **Shapes only.** `path`, `circle`, `ellipse`, `rect`, `line`, `polyline` and `polygon`,
//!   inside the root `svg` and its `g`s. Every other element is left out with all it holds.
//! - **Geometry only, in the text's colour.** An attribute is kept where its value is in its
//!   grammar: a number, a path ([`crate::theme::is_path`]), a list of points or a transform of
//!   the six SVG functions. A fill or stroke is `none` or the text's colour (`currentColor`),
//!   whatever colour the file names, so a person's mark draws as every icon does and never in a
//!   colour of its own. Nothing that names another resource is ever kept.
//! - **Flat.** A group's paint, opacity and transform are carried onto each shape it holds, so
//!   the window draws one list.

use std::collections::BTreeMap;

use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};

/// The most an icon's file may be.
pub const MOST_BYTES: u64 = 64 * 1024;

/// The most elements read from one file, kept or not.
pub const MOST_ELEMENTS: usize = 1024;

/// The deepest an element may be nested.
pub const MOST_DEPTH: usize = 16;

/// The most shapes one icon draws.
pub const MOST_SHAPES: usize = 512;

/// The longest value an attribute may hold, a transform joined from its groups' among them.
const MOST_VALUE: usize = 16384;

/// The most an icon's shapes may come to: every tag, attribute name and value kept, counted
/// after the groups' attributes are carried onto their shapes. The window holds the same
/// budget (`HarnessIcon.tsx`).
pub const MOST_OUTPUT: usize = MOST_BYTES as usize;

/// The shapes an icon may draw.
pub const SHAPES: &[&str] = &[
    "path", "circle", "ellipse", "rect", "line", "polyline", "polygon",
];

/// What a value of one attribute must be.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Grammar {
    Number,
    Path,
    Points,
    Paint,
    Transform,
    Rule,
    Cap,
    Join,
}

/// Every attribute a shape may carry, and its grammar. Paint, opacity, stroke and transform are
/// also what a group passes on to the shapes it holds.
const ATTRIBUTES: &[(&str, Grammar)] = &[
    ("d", Grammar::Path),
    ("points", Grammar::Points),
    ("cx", Grammar::Number),
    ("cy", Grammar::Number),
    ("r", Grammar::Number),
    ("rx", Grammar::Number),
    ("ry", Grammar::Number),
    ("x", Grammar::Number),
    ("y", Grammar::Number),
    ("width", Grammar::Number),
    ("height", Grammar::Number),
    ("x1", Grammar::Number),
    ("y1", Grammar::Number),
    ("x2", Grammar::Number),
    ("y2", Grammar::Number),
    ("fill", Grammar::Paint),
    ("stroke", Grammar::Paint),
    ("opacity", Grammar::Number),
    ("fill-opacity", Grammar::Number),
    ("stroke-opacity", Grammar::Number),
    ("stroke-width", Grammar::Number),
    ("stroke-miterlimit", Grammar::Number),
    ("fill-rule", Grammar::Rule),
    ("clip-rule", Grammar::Rule),
    ("stroke-linecap", Grammar::Cap),
    ("stroke-linejoin", Grammar::Join),
    ("transform", Grammar::Transform),
];

/// The attributes a group passes on: everything but geometry.
const PASSED_ON: &[&str] = &[
    "fill",
    "stroke",
    "fill-opacity",
    "stroke-opacity",
    "stroke-width",
    "stroke-miterlimit",
    "fill-rule",
    "clip-rule",
    "stroke-linecap",
    "stroke-linejoin",
];

/// **An icon drawn from a person's SVG**: its view box and its shapes, every value checked.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SafeSvg {
    /// Four numbers: `min-x min-y width height`.
    pub view_box: String,
    pub shapes: Vec<Shape>,
}

/// One shape: an element of [`SHAPES`] and the attributes of [`ATTRIBUTES`] it keeps.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Shape {
    pub tag: &'static str,
    pub attributes: BTreeMap<&'static str, String>,
}

/// Why a file draws no icon, said without repeating anything it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unfit(pub &'static str);

impl std::fmt::Display for Unfit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// A group open while the file is read: what it passes on, or that it is left out.
struct Open {
    /// `None` for an element left out, whose whole subtree is.
    passed: Option<BTreeMap<&'static str, String>>,
}

/// **Reads `bytes` as an icon**, or says why it draws none.
pub fn read(bytes: &[u8]) -> Result<SafeSvg, Unfit> {
    if bytes.len() as u64 > MOST_BYTES {
        return Err(Unfit("the file is larger than an icon may be"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Unfit("the file is not UTF-8 text"))?;
    let mut reader = quick_xml::Reader::from_str(text);
    let mut open: Vec<Open> = Vec::new();
    let mut view_box: Option<String> = None;
    let mut shapes = Vec::new();
    let mut elements = 0usize;
    // What the shapes kept so far come to, held to `MOST_OUTPUT`.
    let mut spent = 0usize;
    loop {
        let event = reader
            .read_event()
            .map_err(|_| Unfit("the file is not well-formed SVG"))?;
        let (element, empty) = match event {
            Event::Start(element) => (element, false),
            Event::Empty(element) => (element, true),
            Event::End(_) => {
                open.pop();
                continue;
            }
            Event::Eof => break,
            // Text, comments, declarations, processing instructions, a doctype and entity
            // references draw nothing, and nothing in them is read.
            _ => continue,
        };
        elements += 1;
        if elements > MOST_ELEMENTS {
            return Err(Unfit("the file holds more elements than an icon may"));
        }
        if open.len() >= MOST_DEPTH {
            return Err(Unfit("the file nests its elements deeper than an icon may"));
        }
        let tag = element.name();
        let name = tag.as_ref();
        let opened = if open.is_empty() {
            // The root: an `svg` with a view box, or nothing is drawn at all.
            if name != "svg" {
                return Err(Unfit("the file is not an SVG"));
            }
            let own = kept(&element);
            view_box = Some(view_box_of(&element).ok_or(Unfit("the SVG has no view box"))?);
            Open {
                passed: Some(passed_on(&BTreeMap::new(), &own)?),
            }
        } else {
            match open.last().and_then(|it| it.passed.as_ref()) {
                None => Open { passed: None },
                Some(above) => {
                    if name == "g" {
                        Open {
                            passed: Some(passed_on(above, &kept(&element))?),
                        }
                    } else if let Some(shape) = SHAPES.iter().copied().find(|it| *it == name) {
                        if shapes.len() >= MOST_SHAPES {
                            return Err(Unfit("the file draws more shapes than an icon may"));
                        }
                        let mut attributes = above.clone();
                        for (key, value) in kept(&element) {
                            laid(&mut attributes, key, value)?;
                        }
                        spent += shape.len()
                            + attributes
                                .iter()
                                .map(|(key, value)| key.len() + value.len())
                                .sum::<usize>();
                        if spent > MOST_OUTPUT {
                            return Err(Unfit("the SVG draws more than an icon may"));
                        }
                        shapes.push(Shape {
                            tag: shape,
                            attributes,
                        });
                        Open { passed: None }
                    } else {
                        Open { passed: None }
                    }
                }
            }
        };
        if !empty {
            open.push(opened);
        }
    }
    let view_box = view_box.ok_or(Unfit("the file is not an SVG"))?;
    if shapes.is_empty() {
        return Err(Unfit("the SVG draws no shape an icon may"));
    }
    Ok(SafeSvg { view_box, shapes })
}

/// `value` laid over what `attributes` holds for `key`: a transform follows the one above it,
/// an opacity multiplies, and anything else replaces. A joined transform is held to the length
/// of one value.
fn laid(
    attributes: &mut BTreeMap<&'static str, String>,
    key: &'static str,
    value: String,
) -> Result<(), Unfit> {
    let joined = match (key, attributes.get(key)) {
        ("transform", Some(above)) => {
            if above.len() + 1 + value.len() > MOST_VALUE {
                return Err(Unfit("the SVG nests more transforms than an icon may"));
            }
            format!("{above} {value}")
        }
        ("opacity", Some(above)) => {
            let times = number(above).unwrap_or(1.0) * number(&value).unwrap_or(1.0);
            format!("{times}")
        }
        _ => value,
    };
    attributes.insert(key, joined);
    Ok(())
}

/// What an element passes on to what it holds: `above`'s, with its own laid over.
fn passed_on(
    above: &BTreeMap<&'static str, String>,
    own: &[(&'static str, String)],
) -> Result<BTreeMap<&'static str, String>, Unfit> {
    let mut passed = above.clone();
    for (key, value) in own {
        if PASSED_ON.contains(key) || *key == "transform" || *key == "opacity" {
            laid(&mut passed, key, value.clone())?;
        }
    }
    Ok(passed)
}

/// Every attribute of `element` that [`ATTRIBUTES`] names and whose value is in its grammar,
/// `style`'s declarations among them; a paint that refers to anything is the text's colour.
fn kept(element: &BytesStart<'_>) -> Vec<(&'static str, String)> {
    let mut found = Vec::new();
    let mut style = None;
    for attribute in element.attributes().flatten() {
        let Ok(value) = attribute.normalized_value(XmlVersion::Implicit1_0) else {
            continue;
        };
        let key = attribute.key.as_ref();
        if key == "style" {
            style = Some(value.into_owned());
            continue;
        }
        if let Some(kept) = checked(key, &value) {
            found.push(kept);
        }
    }
    // A declaration of `style` beats the attribute of its name, as it does in a browser.
    if let Some(style) = style {
        for declaration in style.split(';') {
            if let Some((key, value)) = declaration.split_once(':')
                && let Some(kept) = checked(key.trim(), value.trim())
                && kept.0 != "d"
                && kept.0 != "points"
            {
                found.retain(|(was, _)| *was != kept.0);
                found.push(kept);
            }
        }
    }
    found
}

/// `key` and `value` as an icon keeps them, or nothing.
fn checked(key: &str, value: &str) -> Option<(&'static str, String)> {
    let (name, grammar) = ATTRIBUTES.iter().copied().find(|(name, _)| *name == key)?;
    let value = value.trim();
    if value.is_empty() || value.len() > MOST_VALUE {
        return None;
    }
    let fits = match grammar {
        Grammar::Number => number(value).is_some(),
        Grammar::Path => crate::theme::is_path(value),
        Grammar::Points => value
            .bytes()
            .all(|b| b"0123456789.eE+-, \t\n\r".contains(&b)),
        Grammar::Paint => {
            return Some((name, paint(value)));
        }
        Grammar::Transform => transform(value),
        Grammar::Rule => matches!(value, "nonzero" | "evenodd"),
        Grammar::Cap => matches!(value, "butt" | "round" | "square"),
        Grammar::Join => matches!(value, "miter" | "round" | "bevel"),
    };
    fits.then(|| (name, value.to_owned()))
}

/// A number as SVG writes one, with an optional `px`; finite.
fn number(value: &str) -> Option<f64> {
    let bare = value.trim().strip_suffix("px").unwrap_or(value.trim());
    bare.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// A paint as an icon draws it: `none`, or the text's colour whatever the file names.
fn paint(value: &str) -> String {
    if value == "none" {
        "none"
    } else {
        "currentColor"
    }
    .to_owned()
}

/// Whether `value` is a transform list of SVG's six functions with numbers in them.
fn transform(value: &str) -> bool {
    const FUNCTIONS: &[&str] = &["matrix", "translate", "scale", "rotate", "skewX", "skewY"];
    let mut rest = value.trim();
    if rest.is_empty() {
        return false;
    }
    while !rest.is_empty() {
        let Some(open) = rest.find('(') else {
            return false;
        };
        let function = rest[..open].trim().trim_start_matches(',').trim();
        if !FUNCTIONS.contains(&function) {
            return false;
        }
        let Some(close) = rest[open..].find(')') else {
            return false;
        };
        let inner = &rest[open + 1..open + close];
        if !inner
            .bytes()
            .all(|b| b"0123456789.eE+-, \t\n\r".contains(&b))
        {
            return false;
        }
        rest = rest[open + close + 1..].trim_start();
    }
    true
}

/// The root's view box: its own, four numbers with a width and height above zero, else one made
/// of its width and height.
fn view_box_of(element: &BytesStart<'_>) -> Option<String> {
    let mut view_box = None;
    let (mut width, mut height) = (None, None);
    for attribute in element.attributes().flatten() {
        let Ok(value) = attribute.normalized_value(XmlVersion::Implicit1_0) else {
            continue;
        };
        match attribute.key.as_ref() {
            "viewBox" => view_box = Some(value.into_owned()),
            "width" => width = number(&value),
            "height" => height = number(&value),
            _ => {}
        }
    }
    let four = |numbers: [f64; 4]| {
        (numbers[2] > 0.0 && numbers[3] > 0.0).then(|| {
            format!(
                "{} {} {} {}",
                numbers[0], numbers[1], numbers[2], numbers[3]
            )
        })
    };
    if let Some(given) = view_box {
        let numbers: Vec<f64> = given
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|it| !it.is_empty())
            .map(str::parse::<f64>)
            .collect::<Result<_, _>>()
            .ok()?;
        let numbers: [f64; 4] = numbers.try_into().ok()?;
        if numbers.iter().all(|n| n.is_finite()) {
            return four(numbers);
        }
        return None;
    }
    four([0.0, 0.0, width?, height?])
}

#[cfg(test)]
mod tests;
