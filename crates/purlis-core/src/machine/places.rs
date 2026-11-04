// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Where each tab window was on screen** (#4768, ruled on #4701): a project's tab windows come
//! back after a relaunch in their place, and a place is this machine's, never the plane's (ADR
//! 0034's "how they were arranged in windows"): screens differ between machines. Keyed by the
//! project's root and the tab window's number, which the plane's record names its tabs by.
//!
//! `tabWindows` in `machine.json`: `[{"plane", "window", "x", "y", "width", "height"}]`, in
//! logical pixels, at most [`MOST_PLACES`], left out while there is none. A row that is not one
//! is dropped, and the window opens where a new window opens.

use std::path::{Path, PathBuf};

use super::Store;

/// The most places kept: far more tab windows than anyone keeps open, so it only bounds a
/// hand-written file. The oldest goes first.
pub const MOST_PLACES: usize = 64;

/// The furthest from the origin, and the largest, a place is read as: past any screen there is.
const FURTHEST: i64 = 100_000;

/// Where one tab window was: its top-left corner and its size, in logical pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// The root of the project it is a tab window of.
    pub plane: PathBuf,
    /// Its number in that project's record: never `0`, the project window.
    pub window: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Store {
    /// Where tab window `window` of the project at `plane` was, if this machine kept it.
    pub fn place_of(&self, plane: &Path, window: u32) -> Option<&Place> {
        self.places
            .iter()
            .find(|place| place.plane == plane && place.window == window)
    }

    /// Keeps `place`, in place of the one kept for the same window, newest last and at most
    /// [`MOST_PLACES`]. A place for the project window is not one, and is not kept.
    pub fn keep_place(&mut self, place: Place) {
        if place.window == 0 {
            return;
        }
        self.places
            .retain(|kept| !(kept.plane == place.plane && kept.window == place.window));
        self.places.push(place);
        let over = self.places.len().saturating_sub(MOST_PLACES);
        self.places.drain(..over);
    }

    /// Forgets where tab window `window` of `plane` was: it closed, and its tabs went back.
    pub fn forget_place(&mut self, plane: &Path, window: u32) {
        self.places
            .retain(|kept| !(kept.plane == plane && kept.window == window));
    }
}

/// The places a file's `tabWindows` names, each one that is one.
pub(super) fn places_of(raw: Option<&serde_json::Value>) -> Vec<Place> {
    let rows = super::array(raw);
    let from = rows.len().saturating_sub(MOST_PLACES);
    rows[from..].iter().filter_map(place_of).collect()
}

fn place_of(raw: &serde_json::Value) -> Option<Place> {
    let plane = super::usable(raw.get("plane")?.as_str()?).ok()?;
    let whole = |key: &str| raw.get(key)?.as_i64().filter(|n| n.abs() <= FURTHEST);
    let window = u32::try_from(whole("window")?).ok().filter(|n| *n > 0)?;
    let width = u32::try_from(whole("width")?).ok().filter(|n| *n > 0)?;
    let height = u32::try_from(whole("height")?).ok().filter(|n| *n > 0)?;
    Some(Place {
        plane,
        window,
        x: i32::try_from(whole("x")?).ok()?,
        y: i32::try_from(whole("y")?).ok()?,
        width,
        height,
    })
}

/// One place as the file spells it.
#[derive(serde::Serialize)]
pub(super) struct PlaceOnDisk {
    plane: String,
    window: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

impl PlaceOnDisk {
    /// `place` as written, or nothing for a root that is not UTF-8, which would not read back
    /// as itself.
    pub(super) fn of(place: &Place) -> Option<Self> {
        Some(Self {
            plane: place.plane.to_str()?.to_owned(),
            window: place.window,
            x: place.x,
            y: place.y,
            width: place.width,
            height: place.height,
        })
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;

    fn at(plane: &str, window: u32, x: i32) -> Place {
        Place {
            plane: PathBuf::from(plane),
            window,
            x,
            y: 40,
            width: 900,
            height: 600,
        }
    }

    #[test]
    fn a_tab_window_s_place_comes_back_from_the_machine_store() {
        let config = tempfile::tempdir().unwrap();
        let mut store = Store::default();
        store.keep_place(at("/work/alpha", 1, -1200));
        store.keep_place(at("/work/alpha", 2, 300));

        super::super::write(config.path(), &store).unwrap();
        let back = super::super::read(config.path()).store;

        assert_eq!(
            back.place_of(Path::new("/work/alpha"), 1),
            Some(&at("/work/alpha", 1, -1200))
        );
        assert_eq!(back.places.len(), 2);
    }

    #[test]
    fn a_place_kept_again_replaces_its_window_s_and_the_oldest_goes_past_the_most() {
        let mut store = Store::default();
        store.keep_place(at("/work/alpha", 1, 0));
        store.keep_place(at("/work/alpha", 1, 50));
        assert_eq!(store.places, [at("/work/alpha", 1, 50)]);

        for window in 2..=(MOST_PLACES as u32 + 1) {
            store.keep_place(at("/work/beta", window, 0));
        }
        assert_eq!(store.places.len(), MOST_PLACES);
        assert_eq!(
            store.place_of(Path::new("/work/alpha"), 1),
            None,
            "the oldest went"
        );

        store.forget_place(Path::new("/work/beta"), 2);
        assert_eq!(store.place_of(Path::new("/work/beta"), 2), None);
        store.keep_place(at("/work/beta", 0, 0));
        assert_eq!(
            store.place_of(Path::new("/work/beta"), 0),
            None,
            "the project window has none"
        );
    }

    #[test]
    fn a_place_that_is_not_one_is_dropped_and_the_rest_read() {
        let rows = serde_json::json!([
            {"plane": "/work/alpha", "window": 1, "x": 10, "y": 20, "width": 800, "height": 500},
            {"plane": "relative", "window": 1, "x": 0, "y": 0, "width": 800, "height": 500},
            {"plane": "/work/alpha", "window": 0, "x": 0, "y": 0, "width": 800, "height": 500},
            {"plane": "/work/alpha", "window": 2, "x": 0, "y": 0, "width": 0, "height": 500},
            {"plane": "/work/alpha", "window": 3, "x": 900_000, "y": 0, "width": 8, "height": 5},
            {"plane": "/work/alpha", "window": 4, "x": "far", "y": 0, "width": 8, "height": 5},
            "not a row"
        ]);

        let read = places_of(Some(&rows));

        assert_eq!(read.len(), 1);
        assert_eq!(read[0].window, 1);
    }
}
