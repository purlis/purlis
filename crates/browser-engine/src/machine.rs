// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The machine's own Chrome, for the person who chooses it in Settings instead of the pinned
//! engine.

use std::path::PathBuf;

/// Where Chrome installs itself on this platform, most usual first.
pub const MACHINE_CHROME: &[&str] = if cfg!(target_os = "macos") {
    &[
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
    ]
} else {
    &[
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/opt/google/chrome/chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ]
};

/// The machine's own Chrome, when one is installed where Chrome installs itself.
pub fn machine_chrome() -> Option<PathBuf> {
    MACHINE_CHROME
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
}
