// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The contrast the operating system asks for, read natively as a window is created**
//! (#2582, spec #2313): so a window whose theme has an appearance tagged high contrast is created
//! in that appearance's `window.background` and its first frame draws it, with nothing to put
//! right once the page reads `prefers-contrast`.
//!
//! - **Linux**: the desktop portal's `org.freedesktop.appearance` `contrast`, asked within
//!   [`BUDGET`]; a session with no portal that answers in time answers nothing.
//! - **macOS and Windows**: nothing is read here yet, and the page's own `prefers-contrast` picks
//!   among the frames it is handed for both contrasts (`windowprefs.rs`), so the first frame is
//!   still the high-contrast appearance when the system asks for it.
//!
//! Where nothing is read the answer is `None`: the window is created for the standard contrast
//! and the page decides its first frame from `prefers-contrast`, as it did before.

use purlis_core::theme::Contrast;

/// What the platform says it asks for, or `None` where it says nothing purlis can read.
pub(crate) fn asked() -> Option<Contrast> {
    read().map(|more| {
        if more {
            Contrast::More
        } else {
            Contrast::Standard
        }
    })
}

/// How long the portal is given to answer. A running portal answers in milliseconds; the window
/// is not held for one that does not.
#[cfg(target_os = "linux")]
const BUDGET: std::time::Duration = std::time::Duration::from_millis(100);

#[cfg(target_os = "linux")]
fn read() -> Option<bool> {
    match crate::portal::in_time(BUDGET, portal_contrast) {
        crate::portal::Asked::Answered(more) => more,
        _ => None,
    }
}

/// The portal's `contrast` setting: `1` asks for more contrast, `0` for none in particular.
#[cfg(target_os = "linux")]
fn portal_contrast() -> Option<bool> {
    let bus = zbus::blocking::Connection::session().ok()?;
    let reply = bus
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "ReadOne",
            &("org.freedesktop.appearance", "contrast"),
        )
        .ok()?;
    let value: zbus::zvariant::OwnedValue = reply.body().deserialize().ok()?;
    u32::try_from(value).ok().map(|contrast| contrast == 1)
}

#[cfg(not(target_os = "linux"))]
fn read() -> Option<bool> {
    None
}
