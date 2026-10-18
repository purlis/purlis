// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a box or a frame on the page does not show (#2564): an element laid out wholly outside
//! a box of its own that scrolls or cuts off what overflows it (a list, a chat log, a side
//! panel), or outside what the frame it is in shows, while that box or frame may itself be in
//! view. Read from the engine's own layout of the page (`DOMSnapshot.captureSnapshot`), never
//! from a script in the page.
//!
//! Every box is placed in its own document's coordinates, so each element is compared with the
//! boxes it is in, then with what its document's frame shows, then, for a frame whose element
//! this read holds, that element with the boxes it is in, outward to the page.
//!
//! **Nothing is said to be cut off that is not known to be.** An element taken out of the flow
//! (`position: absolute` or `fixed`) is not compared with a box that may not hold it; the page's
//! root and body, whose overflow is the viewport's, are left to the viewport's own check; and an
//! answer that cannot be read says nothing is cut off.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::blank::Rect;

/// The styles read for each element laid out, in this order.
pub(crate) const STYLES: [&str; 3] = ["overflow-x", "overflow-y", "position"];

/// Which of `wanted` (the engine's ids for nodes of the session `answer` reads) are laid out
/// wholly outside a box they are in that cuts off what overflows it, or outside what the frame
/// they are in shows. `shows` is what the answer's first document shows, in its own
/// coordinates, when that is known and to be compared (a frame read in a session of its own).
pub(crate) fn clipped(answer: &Value, wanted: &HashSet<i64>, shows: Option<Rect>) -> HashSet<i64> {
    let Some(read) = Read::of(answer) else {
        return HashSet::new();
    };
    let mut out = HashSet::new();
    for (d, document) in read.documents.iter().enumerate() {
        for (at, backend) in document.backends.iter().enumerate() {
            if !wanted.contains(backend) {
                continue;
            }
            let Some(rect) = document.placed.get(&at).map(|p| p.bounds) else {
                continue;
            };
            if read.cut_off(d, at, rect, shows, 0) {
                out.insert(*backend);
            }
        }
    }
    out
}

/// How an element laid out is placed.
#[derive(Debug, Clone, Copy)]
struct Placed {
    bounds: Rect,
    /// Whether it cuts off what overflows it, across and down.
    cuts: (bool, bool),
    position: Position,
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
            let mut placed = HashMap::new();
            for ((node, bounds), style) in laid.iter().zip(bounds).zip(styles) {
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
                let style: Vec<&str> = style
                    .as_array()?
                    .iter()
                    .map(string)
                    .collect::<Option<_>>()?;
                // What is not an element (a text, the document) is given no styles.
                let (cut, position) = match style.as_slice() {
                    [] => ((false, false), Position::InFlow),
                    [across, down, position] => (
                        (cuts(across), cuts(down)),
                        match *position {
                            "absolute" => Position::Absolute,
                            "fixed" => Position::Fixed,
                            "relative" | "sticky" => Position::Holds,
                            _ => Position::InFlow,
                        },
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
                    },
                );
            }
            if let Some(contents) = nodes.get("contentDocumentIndex") {
                for (node, content) in list(contents, "index")?
                    .iter()
                    .zip(list(contents, "value")?)
                {
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
            });
        }
        Some(Read { documents, owners })
    }

    /// Whether the node `at` of document `d`, laid out at `rect`, is wholly outside a box it is
    /// in that cuts off what overflows it, or outside what its frame shows; `shows` what the
    /// first document shows, when known. `depth` counts the frames passed on the way out.
    fn cut_off(&self, d: usize, at: usize, rect: Rect, shows: Option<Rect>, depth: usize) -> bool {
        let Some(document) = self.documents.get(d) else {
            return false;
        };
        let Some(own) = document.placed.get(&at) else {
            return false;
        };
        // Until the box that holds an element taken out of the flow, no box is known to hold it.
        let mut out_of_flow = own.position;
        let mut next = document.parents.get(at).copied().flatten();
        let mut steps = 0;
        while let Some(up) = next {
            steps += 1;
            if steps > document.parents.len() || out_of_flow == Position::Fixed {
                return false;
            }
            if let Some(placed) = document.placed.get(&up) {
                let holds = match out_of_flow {
                    Position::Absolute => placed.position != Position::InFlow,
                    _ => true,
                };
                if holds && !document.roots.contains(&up) && outside(&rect, placed) {
                    return true;
                }
                if holds {
                    out_of_flow = placed.position;
                }
            }
            next = document.parents.get(up).copied().flatten();
        }
        if out_of_flow == Position::Fixed {
            return false;
        }
        match self.owners.get(&d) {
            Some(&(parent, frame)) if depth < self.documents.len() => {
                let Some(element) = self
                    .documents
                    .get(parent)
                    .and_then(|p| p.placed.get(&frame))
                else {
                    return false;
                };
                let frame_shows = Rect {
                    x: document.scrolled.0,
                    y: document.scrolled.1,
                    width: element.bounds.width,
                    height: element.bounds.height,
                };
                !meets(&rect, &frame_shows)
                    || self.cut_off(parent, frame, element.bounds, shows, depth + 1)
            }
            Some(_) => false,
            None => shows.is_some_and(|shows| d == 0 && !meets(&rect, &shows)),
        }
    }
}

/// Whether `rect` is wholly outside `placed` on an axis `placed` cuts off what overflows it on.
fn outside(rect: &Rect, placed: &Placed) -> bool {
    let b = &placed.bounds;
    let across = rect.x + rect.width <= b.x || b.x + b.width <= rect.x;
    let down = rect.y + rect.height <= b.y || b.y + b.height <= rect.y;
    (placed.cuts.0 && across) || (placed.cuts.1 && down)
}

/// Whether `a` and `b` share any part.
fn meets(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

/// Whether an overflow cuts off what does not fit.
fn cuts(overflow: &str) -> bool {
    matches!(overflow, "hidden" | "scroll" | "auto" | "clip" | "overlay")
}

fn list<'a>(of: &'a Value, field: &str) -> Option<&'a Vec<Value>> {
    of.get(field)?.as_array()
}

#[cfg(test)]
mod tests;
