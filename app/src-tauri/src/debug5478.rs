// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! [DEBUG-5478] **Temporary diagnostics for purlis/purlis#5478**: a chat's terminal that froze
//! half-drawn. The window's watchdog (`app/src/freezeWatch5478.ts`) sends one line here when a
//! pane drew nothing for seconds after input or output, and it goes to the diagnostic log
//! (`purlis_core::applog`: `charter.<YYYY-MM-DD>.log` in the app's log folder), where the
//! operator reads it. Removed with the rest of `[DEBUG-5478]` once the freeze is understood (#5488).

/// [DEBUG-5478] The longest line written: the watchdog's is a few hundred bytes, and the log is
/// not a place for whatever else a window might send.
const LONGEST: usize = 4_096;

/// [DEBUG-5478] Writes the watchdog's line to the diagnostic log, prefixed `[DEBUG-5478]`.
#[tauri::command]
#[specta::specta]
pub fn debug_5478_log(line: String) {
    let mut end = line.len().min(LONGEST);
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    tracing::warn!("[DEBUG-5478] {}", &line[..end]);
}
