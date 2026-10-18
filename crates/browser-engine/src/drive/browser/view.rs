// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a page has but does not show (#2137): an element laid out wholly outside the part of
//! the page its viewport shows is marked in a snapshot or a find, so the agent knows to scroll
//! to it. Each element is placed by the engine's own layout, never by a script in the page.

use serde_json::{Value, json};

use super::{Browser, NodeKey, Placings, Rect, lock};

impl Browser {
    /// Which of `keys` `target`'s page lays out wholly outside what its viewport shows. An
    /// element the engine cannot place, or a page whose viewport cannot be read, is not among
    /// them: nothing is said to be out of view that is not known to be.
    pub(super) fn out_of_view(&self, target: &str, keys: &[NodeKey]) -> Vec<NodeKey> {
        if keys.is_empty() {
            return Vec::new();
        }
        let Ok(session) = self.page_session(target) else {
            return Vec::new();
        };
        let Some(shown) = self
            .devtools()
            .call("Page.getLayoutMetrics", json!({}), Some(&session))
            .ok()
            .and_then(|metrics| shown(&metrics))
        else {
            return Vec::new();
        };
        let mut placings = Placings::new();
        keys.iter()
            .filter(|key| {
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
}

/// The part of the page its viewport shows, in the CSS pixels elements are placed in, from
/// `Page.getLayoutMetrics`.
fn shown(metrics: &Value) -> Option<Rect> {
    let number = |path: &str| metrics.pointer(path).and_then(Value::as_f64);
    let width = number("/cssVisualViewport/clientWidth")?;
    let height = number("/cssVisualViewport/clientHeight")?;
    (width > 0.0 && height > 0.0).then(|| Rect {
        x: number("/cssVisualViewport/offsetX").unwrap_or_default(),
        y: number("/cssVisualViewport/offsetY").unwrap_or_default(),
        width,
        height,
    })
}

/// Whether `a` and `b` share any part.
fn meets(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}
