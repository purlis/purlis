// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a box or a frame on the page does not show (#2564): an element laid out wholly outside
//! a box of its own that scrolls or cuts off what overflows it (a list, a chat log, a side
//! panel), or outside what the frame it is in shows, while that box or frame may itself be in
//! view. Read from the engine's own layout of the page (`DOMSnapshot.captureSnapshot`), never
//! from a script in the page.
//!
//! Every box is placed in its own document's coordinates. An element's part is cut to each box it
//! is in, then to what its document's frame shows, then carried into the frame element's
//! document, placed by the element's content box (inside its border and padding, each side its
//! own) at the scale the page draws the element at, and cut again to the boxes that element is
//! in, outward to the page (#2840). A frame read in a session of its own is carried the same way
//! through the layout of each session around it ([`through`]). What is left of the part, nothing
//! or some, says whether it is cut off.
//!
//! **Nothing is said to be cut off that is not known to be.** An element taken out of the flow
//! (`position: absolute` or `fixed`) is not compared with a box that may not hold it; the page's
//! root and body, whose overflow is the viewport's, are left to the viewport's own check; only a
//! box CSS applies overflow to cuts anything off (never an inline box, nor `overflow: clip` with a
//! margin to paint into); a frame is not carried through when its client box is not given, or
//! where it or a box around it is turned or skewed (a transform other than a move or a scale),
//! where its layout's bounds no longer say where it is drawn; and an answer that cannot be read
//! says nothing is cut off.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::blank::Rect;

/// The styles read for each element laid out, in this order.
pub(crate) const STYLES: [&str; 14] = [
    "overflow-x",
    "overflow-y",
    "position",
    "display",
    "overflow-clip-margin",
    "border-left-width",
    "border-top-width",
    "border-right-width",
    "border-bottom-width",
    "padding-left",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "transform",
];

/// The size an element no wider, or no taller, than nothing is given, so what is left of it says
/// whether it is cut off as a line's would.
const LEAST_SIZE: f64 = 1e-3;

/// The boxes whose overflow cuts off what does not fit: CSS applies `overflow` to block
/// containers, flex and grid containers, and table cells, never to an inline box.
const CUT_BY_OVERFLOW: [&str; 9] = [
    "block",
    "inline-block",
    "flow-root",
    "list-item",
    "flex",
    "inline-flex",
    "grid",
    "inline-grid",
    "table-cell",
];

/// Which of `wanted` (the engine's ids for nodes of the session `answer` reads) are laid out
/// wholly outside a box they are in that cuts off what overflows it, or outside what the frame
/// they are in shows. `shows` is what the answer's first document shows, in its own
/// coordinates, when that is known and to be compared (a frame read in a session of its own).
pub(crate) fn clipped(answer: &Value, wanted: &HashSet<i64>, shows: Option<Rect>) -> HashSet<i64> {
    shown_parts(answer, wanted, shows)
        .into_iter()
        .filter(|(_, part)| part.as_ref().is_some_and(empty))
        .map(|(backend, _)| backend)
        .collect()
}

/// The part of each of `wanted` that its boxes and frames in `answer` let show, in the
/// coordinates of the answer's first document, cut to `shows` there where it is given (#2840):
/// empty where nothing of it shows, and `None` where that is not known. One the answer does not
/// lay out is left out.
pub(crate) fn shown_parts(
    answer: &Value,
    wanted: &HashSet<i64>,
    shows: Option<Rect>,
) -> HashMap<i64, Option<Rect>> {
    let Some(read) = Read::of(answer) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for (d, document) in read.documents.iter().enumerate() {
        for (at, backend) in document.backends.iter().enumerate() {
            if !wanted.contains(backend) {
                continue;
            }
            let Some(rect) = document.placed.get(&at).map(|p| p.bounds) else {
                continue;
            };
            let rect = Rect {
                width: rect.width.max(LEAST_SIZE),
                height: rect.height.max(LEAST_SIZE),
                ..rect
            };
            out.insert(*backend, read.shown(d, at, rect, shows, 0));
        }
    }
    out
}

/// What of `part`, a part of the document a frame element of `answer` shows (the node `owner`,
/// its document scrolled to `scrolled`), the frame and the boxes around its element let show,
/// in the coordinates of the answer's first document, cut to `shows` there where it is given
/// (#2689, #2840): for a frame from another site, read in a session of its own. Empty where
/// nothing of it shows; `None` where that is not known.
pub(crate) fn through(
    answer: &Value,
    owner: i64,
    part: Rect,
    scrolled: (f64, f64),
    shows: Option<Rect>,
) -> Option<Rect> {
    let read = Read::of(answer)?;
    let (d, at) = read
        .documents
        .iter()
        .enumerate()
        .find_map(|(d, document)| Some((d, document.backends.iter().position(|b| *b == owner)?)))?;
    let carried = read.carried_out(d, at, part, scrolled)?;
    read.shown(d, at, carried, shows, 0)
}

/// Whether nothing of `rect` is left.
pub(crate) fn empty(rect: &Rect) -> bool {
    rect.width <= 0.0 || rect.height <= 0.0
}

/// The part `a` and `b` share: empty (no width, or no height) where they share none.
pub(crate) fn overlap(a: &Rect, b: &Rect) -> Rect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    Rect {
        x,
        y,
        width: ((a.x + a.width).min(b.x + b.width) - x).max(0.0),
        height: ((a.y + a.height).min(b.y + b.height) - y).max(0.0),
    }
}

/// `rect` cut to `placed` on each axis `placed` cuts off what overflows it on: empty (no width,
/// or no height) where nothing of it is left.
fn within(rect: Rect, placed: &Placed) -> Rect {
    let b = &placed.bounds;
    let cut = |from: f64, size: f64, box_from: f64, box_size: f64| {
        let start = from.max(box_from);
        let end = (from + size).min(box_from + box_size);
        (start, (end - start).max(0.0))
    };
    let (x, width) = if placed.cuts.0 {
        cut(rect.x, rect.width, b.x, b.width)
    } else {
        (rect.x, rect.width)
    };
    let (y, height) = if placed.cuts.1 {
        cut(rect.y, rect.height, b.y, b.height)
    } else {
        (rect.y, rect.height)
    };
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// An element's border and padding, each side its own (left, top, right, bottom), and whether a
/// transform turns or skews it.
#[derive(Debug, Clone, Copy)]
struct Boxing {
    border: [f64; 4],
    padding: [f64; 4],
    turned: bool,
}

/// How an element laid out is placed.
#[derive(Debug, Clone, Copy)]
struct Placed {
    bounds: Rect,
    /// Whether it cuts off what overflows it, across and down.
    cuts: (bool, bool),
    position: Position,
    /// Its border, padding and transform, where the read gives them.
    boxing: Option<Boxing>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Position {
    InFlow,
    /// `relative` or `sticky`: in the flow, and the box an element taken out of it is held by.
    Holds,
    Absolute,
    Fixed,
}

struct Document {
    parents: Vec<Option<usize>>,
    backends: Vec<i64>,
    /// The page's root and body, whose overflow is the viewport's.
    roots: HashSet<usize>,
    placed: HashMap<usize, Placed>,
    /// Where its frame is scrolled to.
    scrolled: (f64, f64),
    /// The size each frame element shows its document at, untransformed (its client box).
    client: HashMap<usize, (f64, f64)>,
}

struct Read {
    documents: Vec<Document>,
    /// The frame element each document is the content of: its document and its node.
    owners: HashMap<usize, (usize, usize)>,
}

impl Read {
    fn of(answer: &Value) -> Option<Read> {
        let strings: Vec<&str> = answer
            .get("strings")?
            .as_array()?
            .iter()
            .map(Value::as_str)
            .collect::<Option<_>>()?;
        let string = |n: &Value| -> Option<&str> {
            let n = n.as_i64()?;
            if n < 0 {
                return Some("");
            }
            strings.get(usize::try_from(n).ok()?).copied()
        };
        let mut documents = Vec::new();
        let mut owners = HashMap::new();
        for (d, document) in answer.get("documents")?.as_array()?.iter().enumerate() {
            let nodes = document.get("nodes")?;
            let layout = document.get("layout")?;
            let parents: Vec<Option<usize>> = list(nodes, "parentIndex")?
                .iter()
                .map(|p| usize::try_from(p.as_i64()?).ok())
                .collect();
            let count = parents.len();
            let backends: Vec<i64> = list(nodes, "backendNodeId")?
                .iter()
                .map(Value::as_i64)
                .collect::<Option<_>>()?;
            let names: Vec<String> = list(nodes, "nodeName")?
                .iter()
                .map(|n| string(n).map(str::to_ascii_uppercase))
                .collect::<Option<_>>()?;
            if backends.len() != count || names.len() != count {
                return None;
            }
            let roots = names
                .iter()
                .enumerate()
                .filter(|(_, name)| *name == "HTML" || *name == "BODY")
                .map(|(at, _)| at)
                .collect();
            let laid = list(layout, "nodeIndex")?;
            let bounds = list(layout, "bounds")?;
            let styles = list(layout, "styles")?;
            if bounds.len() != laid.len() || styles.len() != laid.len() {
                return None;
            }
            // Each element's client box, untransformed: what a frame element shows its document
            // at. An answer without them compares no frame.
            let clients = list(layout, "clientRects").filter(|c| c.len() == laid.len());
            let mut client = HashMap::new();
            let mut placed = HashMap::new();
            for (entry, ((node, bounds), style)) in laid.iter().zip(bounds).zip(styles).enumerate()
            {
                let at = usize::try_from(node.as_i64()?)
                    .ok()
                    .filter(|at| *at < count)?;
                let bounds: Vec<f64> = bounds
                    .as_array()?
                    .iter()
                    .map(Value::as_f64)
                    .collect::<Option<_>>()?;
                let &[x, y, width, height] = bounds.as_slice() else {
                    return None;
                };
                if let Some(&[_, _, w, h]) = clients
                    .and_then(|c| c[entry].as_array())
                    .map(|r| r.iter().filter_map(Value::as_f64).collect::<Vec<f64>>())
                    .as_deref()
                {
                    client.insert(at, (w, h));
                }
                let style: Vec<&str> = style
                    .as_array()?
                    .iter()
                    .map(string)
                    .collect::<Option<_>>()?;
                // What is not an element (a text, the document) is given no styles.
                let (cut, position, boxing) = match style.as_slice() {
                    [] => ((false, false), Position::InFlow, None),
                    [across, down, position, display, margin, sides @ ..] if sides.len() == 9 => (
                        if CUT_BY_OVERFLOW.contains(display) {
                            (cuts(across, margin), cuts(down, margin))
                        } else {
                            (false, false)
                        },
                        match *position {
                            "absolute" => Position::Absolute,
                            "fixed" => Position::Fixed,
                            "relative" | "sticky" => Position::Holds,
                            _ => Position::InFlow,
                        },
                        boxing(sides),
                    ),
                    _ => return None,
                };
                placed.insert(
                    at,
                    Placed {
                        bounds: Rect {
                            x,
                            y,
                            width,
                            height,
                        },
                        cuts: cut,
                        position,
                        boxing,
                    },
                );
            }
            if let Some(contents) = nodes.get("contentDocumentIndex") {
                let (index, value) = (list(contents, "index")?, list(contents, "value")?);
                if index.len() != value.len() {
                    return None;
                }
                for (node, content) in index.iter().zip(value) {
                    let node = usize::try_from(node.as_i64()?).ok()?;
                    let content = usize::try_from(content.as_i64()?).ok()?;
                    owners.insert(content, (d, node));
                }
            }
            let number = |field: &str| document.get(field).and_then(Value::as_f64).unwrap_or(0.0);
            documents.push(Document {
                parents,
                backends,
                roots,
                placed,
                scrolled: (number("scrollOffsetX"), number("scrollOffsetY")),
                client,
            });
        }
        Some(Read { documents, owners })
    }

    /// What of `rect`, a part of the node `at` of document `d`, the boxes it is in let show,
    /// then what its document's frame shows, outward through each frame element this read holds
    /// to the first document, cut to `shows` there; `None` where that is not known. `depth`
    /// counts the frames passed on the way out.
    fn shown(
        &self,
        d: usize,
        at: usize,
        rect: Rect,
        shows: Option<Rect>,
        depth: usize,
    ) -> Option<Rect> {
        let document = self.documents.get(d)?;
        let own = document.placed.get(&at)?;
        let mut part = rect;
        // Until the box that holds an element taken out of the flow, no box is known to hold it.
        let mut out_of_flow = own.position;
        let mut next = document.parents.get(at).copied().flatten();
        let mut steps = 0;
        while let Some(up) = next {
            steps += 1;
            if steps > document.parents.len() || out_of_flow == Position::Fixed {
                return None;
            }
            if let Some(placed) = document.placed.get(&up) {
                let holds = match out_of_flow {
                    Position::Absolute => placed.position != Position::InFlow,
                    _ => true,
                };
                if holds && !document.roots.contains(&up) {
                    part = within(part, placed);
                }
                if holds {
                    out_of_flow = placed.position;
                }
            }
            next = document.parents.get(up).copied().flatten();
        }
        if out_of_flow == Position::Fixed {
            return None;
        }
        match self.owners.get(&d) {
            Some(&(parent, frame)) if depth < self.documents.len() => {
                let carried = self.carried_out(parent, frame, part, document.scrolled)?;
                self.shown(parent, frame, carried, shows, depth + 1)
            }
            Some(_) => None,
            None => Some(match shows {
                Some(shows) if d == 0 => overlap(&part, &shows),
                _ => part,
            }),
        }
    }

    /// `part`, of the document the frame element `frame` of document `d` shows (scrolled to
    /// `scrolled`), cut to what the frame shows and carried into `d`'s coordinates: placed at
    /// the element's content box, at the scale the page draws the element at. `None` where the
    /// element's client box or border and padding are not given, or it or a box around it is
    /// turned or skewed.
    fn carried_out(
        &self,
        d: usize,
        frame: usize,
        part: Rect,
        scrolled: (f64, f64),
    ) -> Option<Rect> {
        let document = self.documents.get(d)?;
        let element = document.placed.get(&frame)?;
        let boxing = element.boxing?;
        let mut next = Some(frame);
        let mut steps = 0;
        while let Some(at) = next {
            steps += 1;
            if steps > document.parents.len() + 1 {
                return None;
            }
            if document
                .placed
                .get(&at)
                .and_then(|p| p.boxing)
                .is_some_and(|b| b.turned)
            {
                return None;
            }
            next = document.parents.get(at).copied().flatten();
        }
        let &(client_width, client_height) = document.client.get(&frame)?;
        let [left, top, right, bottom] = boxing.border;
        let [pad_left, pad_top, pad_right, pad_bottom] = boxing.padding;
        let content = (
            client_width - pad_left - pad_right,
            client_height - pad_top - pad_bottom,
        );
        let laid = (left + client_width + right, top + client_height + bottom);
        if content.0 <= 0.0 || content.1 <= 0.0 || laid.0 <= 0.0 || laid.1 <= 0.0 {
            return None;
        }
        let scale = (
            element.bounds.width / laid.0,
            element.bounds.height / laid.1,
        );
        if !(scale.0.is_finite() && scale.1.is_finite() && scale.0 > 0.0 && scale.1 > 0.0) {
            return None;
        }
        let view = Rect {
            x: scrolled.0,
            y: scrolled.1,
            width: content.0,
            height: content.1,
        };
        let seen = overlap(&part, &view);
        Some(Rect {
            x: element.bounds.x + (left + pad_left + seen.x - scrolled.0) * scale.0,
            y: element.bounds.y + (top + pad_top + seen.y - scrolled.1) * scale.1,
            width: seen.width * scale.0,
            height: seen.height * scale.1,
        })
    }
}

/// An element's border, padding and transform from the nine styles after its first five; `None`
/// where a length cannot be read.
fn boxing(sides: &[&str]) -> Option<Boxing> {
    let length = |text: &str| -> Option<f64> {
        text.strip_suffix("px")
            .unwrap_or(text)
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
    };
    let lengths: Vec<f64> = sides[..8]
        .iter()
        .map(|s| length(s))
        .collect::<Option<_>>()?;
    Some(Boxing {
        border: [lengths[0], lengths[1], lengths[2], lengths[3]],
        padding: [lengths[4], lengths[5], lengths[6], lengths[7]],
        turned: turned(sides[8]),
    })
}

/// Whether `transform`, as the engine computes it, does more than move or scale: anything it
/// cannot be read as (`matrix3d` among them) is taken to.
fn turned(transform: &str) -> bool {
    let transform = transform.trim();
    if transform.is_empty() || transform == "none" {
        return false;
    }
    let Some(inner) = transform
        .strip_prefix("matrix(")
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        return true;
    };
    let numbers: Option<Vec<f64>> = inner.split(',').map(|n| n.trim().parse().ok()).collect();
    match numbers.as_deref() {
        Some(&[a, b, c, d, _, _]) => b != 0.0 || c != 0.0 || a <= 0.0 || d <= 0.0,
        _ => true,
    }
}

/// Whether an overflow cuts off what does not fit; `clip` only with no margin to paint into.
fn cuts(overflow: &str, margin: &str) -> bool {
    match overflow {
        "hidden" | "scroll" | "auto" | "overlay" => true,
        "clip" => matches!(margin, "" | "0px"),
        _ => false,
    }
}

fn list<'a>(of: &'a Value, field: &str) -> Option<&'a Vec<Value>> {
    of.get(field)?.as_array()
}

#[cfg(test)]
mod tests;
