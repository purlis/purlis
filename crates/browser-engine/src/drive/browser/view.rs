// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a page has but does not show (#2137, #2564): an element laid out wholly outside the
//! part of the page its viewport shows, or wholly outside a box it is in that scrolls or cuts
//! off what overflows it, or outside what the frame it is in shows, is marked in a snapshot or
//! a find, so the agent knows to scroll to it, or to scroll the box it is in. Each element is
//! placed by the engine's own layout, never by a script in the page.

use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

use super::super::clipped::{STYLES, clipped};
use super::{Browser, NodeKey, Placings, Rect, lock};

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
        let Some(shown) = self.shows(&session, false) else {
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
        for (session, keys) in by_session {
            let Ok(answer) = self.devtools().call(
                "DOMSnapshot.captureSnapshot",
                json!({ "computedStyles": STYLES }),
                Some(&session),
            ) else {
                continue;
            };
            let shows = if session == page_session {
                None
            } else {
                match self.shows(&session, true) {
                    Some(shows) => Some(shows),
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

    /// What the viewport of `session`'s document shows, in CSS pixels: placed where elements
    /// are drawn on the screen, or, `in_document`, where they are laid out in the document.
    fn shows(&self, session: &str, in_document: bool) -> Option<Rect> {
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
    let width = number("/cssVisualViewport/clientWidth")?;
    let height = number("/cssVisualViewport/clientHeight")?;
    let (x, y) = if in_document {
        ("/cssVisualViewport/pageX", "/cssVisualViewport/pageY")
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

/// Whether `a` and `b` share any part.
fn meets(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}
