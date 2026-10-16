// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a page draws that may show a text kept out (#2303), read from the engine's own layout
//! of the page (`DOMSnapshot.captureSnapshot`), never from a script in the page.
//!
//! Once a page keeps anything out, a screenshot paints over:
//!
//! - every text the page lays out that holds a text kept out, whatever its tree says of it
//!   (hidden from the tree, drawn by the page's style, deeper than a snapshot reads), and every
//!   field, button or drop-down whose value or options hold one;
//! - every canvas, image, video and embedded object, and every element drawn with an image or a
//!   paint script: what they show cannot be read, so they are painted whatever they show;
//! - every frame whose document this read does not hold, unless it is read in a session of its
//!   own.
//!
//! An answer purlis cannot read, or a page that draws an element again outside its box (a
//! reflection), withholds the screenshot.

use std::collections::HashSet;

use serde_json::Value;

use super::tree::holds_any;

/// The styles read for each element laid out: an image in any of them is painted over, and a
/// reflection (the last), which draws the element again outside its box, withholds the
/// screenshot.
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

/// Elements that hold another document.
const FRAMES: [&str; 3] = ["IFRAME", "FRAME", "FENCEDFRAME"];

/// What one session's documents draw that is to be painted over.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Drawn {
    /// Each element to paint over, then the elements it is in, innermost first, by the engine's
    /// ids: the first of them the engine can place is painted.
    pub paint: Vec<Vec<i64>>,
    /// Each frame element whose document this read does not hold, as [`Drawn::paint`]: painted
    /// unless that document is read in a session of its own.
    pub frames: Vec<Vec<i64>>,
}

/// The engine's answer could not be read.
#[derive(Debug, thiserror::Error)]
#[error("what the page draws could not be read")]
pub(crate) struct Unread;

/// What `answer` (of `DOMSnapshot.captureSnapshot`, with [`STYLES`]) draws that is to be painted
/// over on a page that keeps `kept` out.
pub(crate) fn drawn(answer: &Value, kept: &[&str]) -> Result<Drawn, Unread> {
    let strings: Vec<&str> = answer
        .get("strings")
        .and_then(Value::as_array)
        .ok_or(Unread)?
        .iter()
        .map(|s| s.as_str().ok_or(Unread))
        .collect::<Result<_, _>>()?;
    let documents = answer
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(Unread)?;
    let mut out = Drawn::default();
    for document in documents {
        read_document(document, &strings, kept, &mut out)?;
    }
    Ok(out)
}

fn read_document(
    document: &Value,
    strings: &[&str],
    kept: &[&str],
    out: &mut Drawn,
) -> Result<(), Unread> {
    let nodes = document.get("nodes").ok_or(Unread)?;
    let layout = document.get("layout").ok_or(Unread)?;
    let string = |index: i64| -> Result<&str, Unread> {
        if index < 0 {
            return Ok("");
        }
        usize::try_from(index)
            .ok()
            .and_then(|at| strings.get(at).copied())
            .ok_or(Unread)
    };
    let parents = numbers(nodes, "parentIndex")?;
    let names = numbers(nodes, "nodeName")?;
    let values = numbers(nodes, "nodeValue")?;
    let backends = numbers(nodes, "backendNodeId")?;
    let attributes = nodes
        .get("attributes")
        .and_then(Value::as_array)
        .ok_or(Unread)?;
    let count = parents.len();
    if [names.len(), values.len(), backends.len(), attributes.len()]
        .iter()
        .any(|n| *n != count)
    {
        return Err(Unread);
    }
    let laid = numbers(layout, "nodeIndex")?;
    let texts = numbers(layout, "text")?;
    let styles = layout
        .get("styles")
        .and_then(Value::as_array)
        .ok_or(Unread)?;
    if texts.len() != laid.len() || styles.len() != laid.len() {
        return Err(Unread);
    }
    let node_at = |n: i64| {
        usize::try_from(n)
            .ok()
            .filter(|at| *at < count)
            .ok_or(Unread)
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
    let holds = |text: &str| !text.is_empty() && holds_any(text, kept);
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
            .ok_or(Unread)?
            .iter()
            .map(|v| {
                v.as_i64()
                    .ok_or(Unread)
                    .and_then(string)
                    .map(str::to_ascii_lowercase)
            })
            .collect::<Result<_, _>>()?;
        let [images @ .., reflected] = style.as_slice() else {
            return Err(Unread);
        };
        if images.len() + 1 != STYLES.len() {
            return Err(Unread);
        }
        if !reflected.is_empty() && reflected != "none" {
            return Err(Unread);
        }
        let image = images
            .iter()
            .any(|style| style.contains("url(") || style.contains("paint("));
        own[at].push_str(string(texts[entry])?);
        if image || OPAQUE.contains(&name.as_str()) {
            paint(at, out);
        }
        if name == "INPUT"
            && attribute(&attributes[at], &string, "type")?.eq_ignore_ascii_case("image")
        {
            paint(at, out);
        }
        if FRAMES.contains(&name.as_str()) && !contents.iter().any(|(i, _)| *i == at) {
            out.frames.push(chain(at));
        }
    }
    // Text laid out that holds a text kept out, whole or split over elements: the innermost
    // element whose text, read in order, holds it. Each node comes after the one it is in.
    let mut inside = vec![false; count];
    for at in (0..count).rev() {
        let text = std::mem::take(&mut own[at]);
        let holding = inside[at] || holds(&text);
        if holding && !inside[at] {
            paint(at, out);
        }
        if let Some(up) = parent(at) {
            if up >= at {
                return Err(Unread);
            }
            inside[up] |= holding;
            own[up].insert_str(0, &text);
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
    // A drop-down shows its chosen option's text, which is laid out nowhere the read reaches.
    for (at, value) in values.iter().enumerate() {
        if !holds(string(*value)?) {
            continue;
        }
        let mut next = parent(at);
        let mut steps = 0;
        while let Some(up) = next {
            if steps > count {
                break;
            }
            if shown.contains(&up) && name(up)? == "SELECT" {
                paint(up, out);
                break;
            }
            next = parent(up);
            steps += 1;
        }
    }
    Ok(())
}

/// A list of numbers of `of`, by `field`.
fn numbers(of: &Value, field: &str) -> Result<Vec<i64>, Unread> {
    of.get(field)
        .and_then(Value::as_array)
        .ok_or(Unread)?
        .iter()
        .map(|n| n.as_i64().ok_or(Unread))
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
        return Err(Unread);
    }
    index
        .into_iter()
        .zip(value)
        .map(|(i, v)| Ok((usize::try_from(i).map_err(|_| Unread)?, v)))
        .collect()
}

/// The value of the attribute `wanted` among `pairs` (name, value, name, value…), empty when
/// it has none.
fn attribute<'a>(
    pairs: &Value,
    string: &impl Fn(i64) -> Result<&'a str, Unread>,
    wanted: &str,
) -> Result<&'a str, Unread> {
    let pairs = pairs.as_array().ok_or(Unread)?;
    for pair in pairs.chunks(2) {
        let [name, value] = pair else {
            return Err(Unread);
        };
        let name = string(name.as_i64().ok_or(Unread)?)?;
        if name.eq_ignore_ascii_case(wanted) {
            return string(value.as_i64().ok_or(Unread)?);
        }
    }
    Ok("")
}
