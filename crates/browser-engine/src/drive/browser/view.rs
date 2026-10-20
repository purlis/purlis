// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a page has but does not show (#2137, #2564): an element laid out wholly outside the
//! part of the page its viewport shows, or wholly outside a box it is in that scrolls or cuts
//! off what overflows it, or outside what the frame it is in shows, is marked in a snapshot or
//! a find, so the agent knows to scroll to it, or to scroll the box it is in. Each element is
//! placed by the engine's own layout, never by a script in the page.

use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

use super::super::clipped::{STYLES, clipped, shown_through};
use super::{Browser, NodeKey, Placings, Rect, Session, lock};

/// The most nodes a page's process may hold for its layout to be read for what a box or a frame
/// cuts off (#2564). Past it, the read is skipped and nothing is marked cut off, so a heavy page
/// is never read whole for a mark.
pub(super) const MOST_NODES_READ: u64 = 50_000;

/// The most bytes the answer of that read may take (#2694): a node count does not bound the
/// inline script and attribute text the answer carries, so past this the read fails alone and
/// nothing is marked cut off.
pub(super) const MOST_LAYOUT_BYTES: usize = 16 << 20;

impl Browser {
    /// Which of `keys` `target`'s page does not show: laid out wholly outside what its
    /// viewport shows, or cut off by a box or a frame they are in. An element the engine cannot
    /// place, or a page whose viewport cannot be read, is not among them: nothing is said to be
    /// out of view that is not known to be.
    pub(super) fn out_of_view(&self, target: &str, keys: &[NodeKey]) -> Vec<NodeKey> {
        if keys.is_empty() {
            return Vec::new();
        }
        let Ok(session) = self.page_session(target) else {
            return Vec::new();
        };
        let cut = self.cut_off(target, &session, keys);
        let Some(shown) = self.viewport_of(&session, false) else {
            return keys
                .iter()
                .filter(|key| cut.contains(*key))
                .cloned()
                .collect();
        };
        let mut placings = Placings::new();
        keys.iter()
            .filter(|key| {
                if cut.contains(*key) {
                    return true;
                }
                let (session, boundaries) = {
                    let state = lock(&self.inner.state);
                    let Some(page) = state.page(target) else {
                        return false;
                    };
                    let Some(frame) = page.frames.get(&key.frame) else {
                        return false;
                    };
                    (frame.session.clone(), page.boundaries(&key.frame))
                };
                matches!(
                    self.placed(&mut placings, &session, &boundaries, key.backend),
                    Ok(Some(at)) if !meets(&at, &shown)
                )
            })
            .cloned()
            .collect()
    }

    /// Which of `keys` a box or a frame they are in cuts off (#2564), read once for each session
    /// that lays any of them out. A frame read in a session of its own is also compared with
    /// what its own viewport shows; the page's is [`Browser::out_of_view`]'s.
    fn cut_off(&self, target: &str, page_session: &str, keys: &[NodeKey]) -> HashSet<NodeKey> {
        let mut by_session: HashMap<String, Vec<&NodeKey>> = HashMap::new();
        {
            let state = lock(&self.inner.state);
            let Some(page) = state.page(target) else {
                return HashSet::new();
            };
            for key in keys {
                if let Some(frame) = page.frames.get(&key.frame) {
                    by_session
                        .entry(frame.session.clone())
                        .or_default()
                        .push(key);
                }
            }
        }
        let mut out = HashSet::new();
        let mut layouts = Layouts::default();
        for (session, keys) in by_session {
            let Some(answer) = layouts.of(self, &session) else {
                continue;
            };
            let shows = if session == page_session {
                None
            } else {
                match self.viewport_of(&session, true) {
                    // What the frame's own viewport shows, cut to what the boxes its element
                    // is in on the page around it let show (#2689).
                    Some(shows) => Some(
                        match self.shown_from_around(&mut layouts, &session, shows) {
                            Some(around) => overlap(&shows, &around),
                            None => shows,
                        },
                    ),
                    None => continue,
                }
            };
            let wanted: HashSet<i64> = keys.iter().map(|key| key.backend).collect();
            let cut = clipped(&answer, &wanted, shows);
            out.extend(
                keys.into_iter()
                    .filter(|key| cut.contains(&key.backend))
                    .cloned(),
            );
        }
        out
    }

    /// What the boxes on the page around the frame `session` reads let its document show, in
    /// the document's coordinates, scrolled as `shows` is (#2689): read from the layout of the
    /// session that holds the frame's element. `None` where that is not known.
    fn shown_from_around(&self, layouts: &mut Layouts, session: &str, shows: Rect) -> Option<Rect> {
        let (frame, around) = {
            let state = lock(&self.inner.state);
            let Some(Session::Frame { page, frame }) = state.sessions.get(session) else {
                return None;
            };
            let page = state.page(page)?;
            let parent = page.frames.get(frame)?.parent.clone()?;
            (frame.clone(), page.frames.get(&parent)?.session.clone())
        };
        let owner = self
            .devtools()
            .call(
                "DOM.getFrameOwner",
                json!({ "frameId": frame }),
                Some(&around),
            )
            .ok()?
            .get("backendNodeId")?
            .as_i64()?;
        let answer = layouts.of(self, &around)?;
        shown_through(&answer, owner, (shows.x, shows.y))
    }

    /// What the viewport of `session`'s document shows, in CSS pixels: the page's visual
    /// viewport where elements are drawn on the screen, or, `in_document`, the document's own
    /// layout viewport where its elements are laid out (a frame read in a session of its own).
    pub(super) fn viewport_of(&self, session: &str, in_document: bool) -> Option<Rect> {
        self.devtools()
            .call("Page.getLayoutMetrics", json!({}), Some(session))
            .ok()
            .and_then(|metrics| shown(&metrics, in_document))
    }
}

/// The part of the page its viewport shows, in the CSS pixels elements are placed in, from
/// `Page.getLayoutMetrics`: where the screen shows it, or `in_document`, where the document has
/// it.
fn shown(metrics: &Value, in_document: bool) -> Option<Rect> {
    let number = |path: &str| metrics.pointer(path).and_then(Value::as_f64);
    let viewport = if in_document {
        "cssLayoutViewport"
    } else {
        "cssVisualViewport"
    };
    let width = number(&format!("/{viewport}/clientWidth"))?;
    let height = number(&format!("/{viewport}/clientHeight"))?;
    let (x, y) = if in_document {
        ("/cssLayoutViewport/pageX", "/cssLayoutViewport/pageY")
    } else {
        ("/cssVisualViewport/offsetX", "/cssVisualViewport/offsetY")
    };
    (width > 0.0 && height > 0.0).then(|| Rect {
        x: number(x).unwrap_or_default(),
        y: number(y).unwrap_or_default(),
        width,
        height,
    })
}

/// The layouts read for one look at what boxes and frames cut off, each session's read at most
/// once (#2689): a frame's page around it may be read for its own elements too.
#[derive(Default)]
struct Layouts(HashMap<String, Option<std::sync::Arc<Value>>>);

impl Layouts {
    /// `session`'s layout, read only where the page is small enough to read: an answer the
    /// engine cannot count, or past the bound, is none, and so is a read whose answer is past
    /// [`MOST_LAYOUT_BYTES`].
    fn of(&mut self, browser: &Browser, session: &str) -> Option<std::sync::Arc<Value>> {
        self.0
            .entry(session.to_owned())
            .or_insert_with(|| {
                let nodes = browser
                    .devtools()
                    .call("Memory.getDOMCounters", json!({}), Some(session))
                    .ok()
                    .and_then(|counted| counted.get("nodes").and_then(Value::as_u64));
                if nodes.is_none_or(|nodes| nodes > MOST_NODES_READ) {
                    return None;
                }
                browser
                    .devtools()
                    .call_within_bounded(
                        "DOMSnapshot.captureSnapshot",
                        json!({
                            "computedStyles": STYLES,
                            "includeDOMRects": true,
                            "includePaintOrder": false,
                            "includeBlendedBackgroundColors": false,
                            "includeTextColorOpacities": false,
                        }),
                        Some(session),
                        crate::cdp::CALL_TIMEOUT,
                        MOST_LAYOUT_BYTES,
                    )
                    .ok()
                    .map(std::sync::Arc::new)
            })
            .clone()
    }
}

/// The part `a` and `b` share: empty (no width) where they share none.
fn overlap(a: &Rect, b: &Rect) -> Rect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    Rect {
        x,
        y,
        width: ((a.x + a.width).min(b.x + b.width) - x).max(0.0),
        height: ((a.y + a.height).min(b.y + b.height) - y).max(0.0),
    }
}

/// Whether `a` and `b` share any part.
fn meets(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}
