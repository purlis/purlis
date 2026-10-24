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

use tauri::utils::config::WindowConfig;
use tauri::{AppHandle, Runtime};

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

/// Cuts `config`'s size to the usable area of the screen a new window opens on: the primary
/// one, where the operating system puts a new window. Left as it is where no screen is known.
pub(crate) fn fit_to_the_screen<R: Runtime>(app: &AppHandle<R>, config: &mut WindowConfig) {
    let Ok(Some(screen)) = app.primary_monitor() else {
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
