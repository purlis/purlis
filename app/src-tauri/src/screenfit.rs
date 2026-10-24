// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A new window opens at the default size, or the screen's usable size where that is smaller**
//! (the operator's ruling of 2026-10-24 on #3117, ADR 0054's amendment of that day).
//!
//! The default window is 1440×900 (`tauri.conf.json`), wide enough for the project rail beside
//! both sidebars, so the rail is shown on a first launch. A screen whose usable area (the screen
//! less the menu bar, the dock or the taskbar) is smaller gets a window that fits it, never
//! narrower than the window's minimum width: there the rail hides itself to leave the work its
//! room (`app/src/railRoom.ts`, #3077).
//!
//! **The height is the window's inside.** A window's configured size is its content, so on Linux
//! and Windows, whose title bar is the system's, a window cut to the usable height is taller than
//! it by that bar. Tauri says how tall the bar is only once the window exists, and resizing it
//! then would draw it twice, so the bar is left to the window manager, which keeps a new window
//! on the screen. macOS draws the title bar inside the window (`titleBarStyle: Overlay`).

use tauri::utils::config::WindowConfig;
use tauri::{AppHandle, Monitor, Runtime};

/// The size, in logical pixels, a window asked for at `want` opens at on a screen whose usable
/// area is `room`: each side cut to the room, the width never under `least_width`.
pub(crate) fn fitted(want: (f64, f64), room: (f64, f64), least_width: Option<f64>) -> (f64, f64) {
    let (width, height) = want;
    let (room_width, room_height) = room;
    // A screen that reports no area says nothing about the room: the asked size stands.
    if !(room_width > 0.0 && room_height > 0.0) {
        return want;
    }
    let width = width.min(room_width).max(least_width.unwrap_or(0.0));
    (width, height.min(room_height))
}

/// Cuts `config`'s size to the usable area of the screen a new window opens on: `near`, the
/// screen of the window it is made from (a split opens on its screen), else the primary one,
/// where the operating system puts a first window. Left as it is where no screen is known, as
/// on Wayland, whose compositor places and fits the window itself.
pub(crate) fn fit_to_the_screen<R: Runtime>(
    app: &AppHandle<R>,
    config: &mut WindowConfig,
    near: Option<Monitor>,
) {
    let Some(screen) = near.or_else(|| app.primary_monitor().ok().flatten()) else {
        return;
    };
    let room = screen
        .work_area()
        .size
        .to_logical::<f64>(screen.scale_factor());
    (config.width, config.height) = fitted(
        (config.width, config.height),
        (room.width, room.height),
        config.min_width,
    );
}

#[cfg(test)]
mod tests {
    use super::fitted;

    #[test]
    fn a_screen_with_room_gets_the_default_window() {
        assert_eq!(
            fitted((1440.0, 900.0), (1728.0, 1080.0), Some(1024.0)),
            (1440.0, 900.0)
        );
    }

    #[test]
    fn a_smaller_screen_gets_its_usable_size() {
        assert_eq!(
            fitted((1440.0, 900.0), (1280.0, 775.0), Some(1024.0)),
            (1280.0, 775.0)
        );
    }

    #[test]
    fn never_narrower_than_the_window_may_be() {
        assert_eq!(
            fitted((1440.0, 900.0), (800.0, 600.0), Some(1024.0)),
            (1024.0, 600.0)
        );
    }

    #[test]
    fn a_screen_that_reports_no_area_leaves_the_size_asked() {
        assert_eq!(
            fitted((1440.0, 900.0), (0.0, 0.0), Some(1024.0)),
            (1440.0, 900.0)
        );
    }
}
