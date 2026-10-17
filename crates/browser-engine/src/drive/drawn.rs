// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a page draws that may show a text kept out (#2303), read from the engine's own layout
//! of the page (`DOMSnapshot.captureSnapshot`), never from a script in the page.
//!
//! Once a page keeps anything out, a screenshot paints over:
//!
//! - every text the page lays out that holds a text kept out, whatever its tree says of it
//!   (hidden from the tree, drawn by the page's style, deeper than a snapshot reads, split over
//!   elements), and every field, button or drop-down whose value, options or labels hold one;
//! - every frame whose document this read does not hold, unless it is read in a session of its
//!   own.
//!
//! On a page whose site a text was kept out on ([`Drawn::pictured`]), it also paints over every canvas,
//! image, video and embedded object, and every element drawn with an image or a paint script:
//! what they show cannot be read, so they are painted whatever they show.
//!
//! An answer purlis cannot read, or a page that draws a reflection, withholds the screenshot.

use std::collections::HashSet;

use serde_json::Value;

use super::tree::{holds_any, holds_loosely};

/// The styles read for each element laid out: an image in any of them is painted over, and a
/// reflection (the last) withholds the screenshot.
pub(crate) const STYLES: [&str; 7] = [
    "background-image",
    "border-image-source",
    "mask-image",
    "-webkit-mask-box-image-source",
    "list-style-image",
    "content",
    "-webkit-box-reflect",
];

/// Elements that show what no read of the page can tell: painted whatever they show.
const OPAQUE: [&str; 7] = [
    "CANVAS", "IMG", "VIDEO", "EMBED", "OBJECT", "IMAGE", "FEIMAGE",
];

/// The engine's number for an element node.
const ELEMENT: i64 = 1;

/// Elements that hold another document.
const FRAMES: [&str; 3] = ["IFRAME", "FRAME", "FENCEDFRAME"];

/// What one session's documents draw that is to be painted over.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Drawn {
    /// Each element to paint over, then the elements it is in, innermost first, by the engine's
    /// ids: the first of them the engine can place is painted.
    pub paint: Vec<Vec<i64>>,
    /// What cannot be read (a canvas, an image…), as [`Drawn::paint`]: painted on a page whose
    /// site a text was kept out on.
    pub pictured: Vec<Vec<i64>>,
    /// The address of the first document read: what the page shows, whatever it was told.
    pub address: Option<String>,
    /// Each frame element whose document this read does not hold, as [`Drawn::paint`]: painted
    /// unless that document is read in a session of its own.
    pub frames: Vec<Vec<i64>>,
}

/// The engine's answer could not be read, or shows what a screenshot cannot paint over.
#[derive(Debug, thiserror::Error)]
#[error("what the page draws could not be read: {0}")]
pub(crate) struct Unread(&'static str);

/// What `answer` (of `DOMSnapshot.captureSnapshot`, with [`STYLES`]) draws that is to be painted
/// over on a page that keeps `kept` out, and `loose` out also with any spaces or dashes between
/// its characters (what a group of boxes holds together).
pub(crate) fn drawn(answer: &Value, kept: &[&str], loose: &[&str]) -> Result<Drawn, Unread> {
    let strings: Vec<&str> = answer
        .get("strings")
        .and_then(Value::as_array)
        .ok_or(Unread("it names no strings"))?
        .iter()
        .map(|s| s.as_str().ok_or(Unread("a string is not text")))
        .collect::<Result<_, _>>()?;
    let documents = answer
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(Unread("it lists no documents"))?;
    let kept: Vec<String> = kept
        .iter()
        .map(|k| collapsed(k).trim().to_owned())
        .collect();
    let kept: Vec<&str> = kept.iter().map(String::as_str).collect();
    let mut out = Drawn::default();
    if let Some(first) = documents.first() {
        let at = first
            .get("documentURL")
            .and_then(Value::as_i64)
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| strings.get(i));
        out.address = at.map(|a| (*a).to_owned());
    }
    for document in documents {
        read_document(document, &strings, (&kept, loose), &mut out)?;
    }
    Ok(out)
}

/// `text` with every run of white space (a line break and a no-break space among them) one
/// space, as the page draws it.
fn collapsed(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars() {
        if c.is_whitespace() {
            space = true;
        } else {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(c);
        }
    }
    if space {
        out.push(' ');
    }
    out
}

fn read_document(
    document: &Value,
    strings: &[&str],
    (kept, loose): (&[&str], &[&str]),
    out: &mut Drawn,
) -> Result<(), Unread> {
    let nodes = document
        .get("nodes")
        .ok_or(Unread("a document has no nodes"))?;
    let layout = document
        .get("layout")
        .ok_or(Unread("a document has no layout"))?;
    let string = |index: i64| -> Result<&str, Unread> {
        if index < 0 {
            return Ok("");
        }
        usize::try_from(index)
            .ok()
            .and_then(|at| strings.get(at).copied())
            .ok_or(Unread("a string is not listed"))
    };
    let parents = numbers(nodes, "parentIndex")?;
    let names = numbers(nodes, "nodeName")?;
    let kinds = numbers(nodes, "nodeType")?;
    let values = numbers(nodes, "nodeValue")?;
    let backends = numbers(nodes, "backendNodeId")?;
    let attributes = nodes
        .get("attributes")
        .and_then(Value::as_array)
        .ok_or(Unread("its nodes have no attributes"))?;
    let count = parents.len();
    if [
        names.len(),
        kinds.len(),
        values.len(),
        backends.len(),
        attributes.len(),
    ]
    .iter()
    .any(|n| *n != count)
    {
        return Err(Unread("its nodes do not line up"));
    }
    let laid = numbers(layout, "nodeIndex")?;
    let texts = numbers(layout, "text")?;
    let styles = layout
        .get("styles")
        .and_then(Value::as_array)
        .ok_or(Unread("its layout has no styles"))?;
    if texts.len() != laid.len() || styles.len() != laid.len() {
        return Err(Unread("its layout does not line up"));
    }
    let node_at = |n: i64| {
        usize::try_from(n)
            .ok()
            .filter(|at| *at < count)
            .ok_or(Unread("its layout names a node it does not have"))
    };
    let name = |at: usize| string(names[at]).map(str::to_ascii_uppercase);
    let parent = |at: usize| usize::try_from(parents[at]).ok().filter(|p| *p < count);
    let chain = |at: usize| -> Vec<i64> {
        let mut out = Vec::new();
        let mut next = Some(at);
        while let Some(at) = next {
            if out.len() > count {
                break;
            }
            out.push(backends[at]);
            next = parent(at);
        }
        out
    };
    let holds = |text: &str| {
        !text.is_empty() && (holds_any(&collapsed(text), kept) || holds_loosely(text, loose))
    };
    let mut painted: HashSet<usize> = HashSet::new();
    let mut paint = |at: usize, out: &mut Drawn| {
        if painted.insert(at) {
            out.paint.push(chain(at));
        }
    };

    let contents = rare(nodes, "contentDocumentIndex")?;
    let mut shown: HashSet<usize> = HashSet::new();
    // The text each node lays out itself.
    let mut own: Vec<String> = vec![String::new(); count];
    for (entry, node) in laid.iter().enumerate() {
        let at = node_at(*node)?;
        shown.insert(at);
        let name = name(at)?;
        let style: Vec<String> = styles[entry]
            .as_array()
            .ok_or(Unread("a style list is not a list"))?
            .iter()
            .map(|v| {
                v.as_i64()
                    .ok_or(Unread("a style is not a string"))
                    .and_then(string)
                    .map(str::to_ascii_lowercase)
            })
            .collect::<Result<_, _>>()?;
        // The engine gives no styles for what is not an element (a text, the document): it
        // draws them in the styles of the element they are in.
        let style = match style.as_slice() {
            [] if kinds[at] != ELEMENT => vec![String::new(); STYLES.len()],
            _ => style,
        };
        let [images @ .., reflected] = style.as_slice() else {
            return Err(Unread("an element has no styles"));
        };
        if images.len() + 1 != STYLES.len() {
            return Err(Unread("an element has styles missing"));
        }
        if !reflected.is_empty() && reflected != "none" {
            return Err(Unread("the page draws a reflection"));
        }
        let image = images
            .iter()
            .any(|style| style.contains("url(") || style.contains("paint("));
        own[at].push_str(string(texts[entry])?);
        let pictured = image
            || OPAQUE.contains(&name.as_str())
            || (name == "INPUT"
                && attribute(&attributes[at], &string, "type")?.eq_ignore_ascii_case("image"));
        if pictured {
            out.pictured.push(chain(at));
        }
        if FRAMES.contains(&name.as_str()) && !contents.iter().any(|(i, _)| *i == at) {
            out.frames.push(chain(at));
        }
    }
    // Text laid out that holds a text kept out, whole or split over elements: the innermost
    // node whose text, read in order, holds it. Each node comes after the one it is in, and an
    // element whose text already holds one passes no text on.
    let drew: Vec<bool> = own.iter().map(|t| !t.is_empty()).collect();
    let mut inside = vec![false; count];
    let mut holders: HashSet<usize> = HashSet::new();
    for at in (0..count).rev() {
        let text = std::mem::take(&mut own[at]);
        let holding = inside[at] || holds(&text);
        if holding && !inside[at] {
            paint(at, out);
            holders.insert(at);
        }
        if let Some(up) = parent(at) {
            if up >= at {
                return Err(Unread("a node comes before the one it is in"));
            }
            if holding {
                inside[up] = true;
            } else if !inside[up] {
                own[up].insert_str(0, &text);
            }
        }
    }
    // A text split over elements is painted at each text it is drawn in as well, wherever
    // each is drawn.
    let mut under = vec![false; count];
    for at in 0..count {
        under[at] = parent(at).is_some_and(|up| holders.contains(&up) || under[up]);
        if under[at] && drew[at] {
            paint(at, out);
        }
    }
    // What a field, a button or a drop-down shows is its value or its options, not text laid
    // out where the read reaches it.
    for field in ["inputValue", "textValue"] {
        for (at, value) in rare(nodes, field)? {
            if shown.contains(&at) && holds(string(value)?) {
                paint(at, out);
            }
        }
    }
    for (at, pairs) in attributes.iter().enumerate() {
        if !shown.contains(&at) {
            continue;
        }
        for shows in ["value", "placeholder"] {
            if holds(attribute(pairs, &string, shows)?) {
                paint(at, out);
            }
        }
    }
    // A drop-down shows its options' text and labels, which are laid out nowhere the read
    // reaches: the drop-down they are in is painted.
    let drop_down = |at: usize| -> Result<Option<usize>, Unread> {
        let mut next = parent(at);
        let mut steps = 0;
        while let Some(up) = next {
            if steps > count {
                break;
            }
            if shown.contains(&up) && name(up)? == "SELECT" {
                return Ok(Some(up));
            }
            next = parent(up);
            steps += 1;
        }
        Ok(None)
    };
    for at in 0..count {
        let labelled = matches!(name(at)?.as_str(), "OPTION" | "OPTGROUP")
            && holds(attribute(&attributes[at], &string, "label")?);
        if (labelled || holds(string(values[at])?))
            && let Some(list) = drop_down(at)?
        {
            paint(list, out);
        }
    }
    Ok(())
}

/// A list of numbers of `of`, by `field`.
fn numbers(of: &Value, field: &str) -> Result<Vec<i64>, Unread> {
    of.get(field)
        .and_then(Value::as_array)
        .ok_or(Unread("a list is missing"))?
        .iter()
        .map(|n| {
            n.as_i64()
                .ok_or(Unread("a list holds what is not a number"))
        })
        .collect()
}

/// The pairs of a field given for some nodes only (`{index, value}`); none when it is absent.
fn rare(of: &Value, field: &str) -> Result<Vec<(usize, i64)>, Unread> {
    let Some(data) = of.get(field) else {
        return Ok(Vec::new());
    };
    let index = numbers(data, "index")?;
    let value = numbers(data, "value")?;
    if index.len() != value.len() {
        return Err(Unread("a field for some nodes does not line up"));
    }
    index
        .into_iter()
        .zip(value)
        .map(|(i, v)| {
            Ok((
                usize::try_from(i).map_err(|_| Unread("a field names no node"))?,
                v,
            ))
        })
        .collect()
}

/// The value of the attribute `wanted` among `pairs` (name, value, name, value…), empty when
/// it has none.
fn attribute<'a>(
    pairs: &Value,
    string: &impl Fn(i64) -> Result<&'a str, Unread>,
    wanted: &str,
) -> Result<&'a str, Unread> {
    let pairs = pairs
        .as_array()
        .ok_or(Unread("an attribute list is not a list"))?;
    for pair in pairs.chunks(2) {
        let [name, value] = pair else {
            return Err(Unread("an attribute has no value"));
        };
        let name = string(name.as_i64().ok_or(Unread("an attribute is not named"))?)?;
        if name.eq_ignore_ascii_case(wanted) {
            return string(
                value
                    .as_i64()
                    .ok_or(Unread("an attribute's value is not text"))?,
            );
        }
    }
    Ok("")
}
