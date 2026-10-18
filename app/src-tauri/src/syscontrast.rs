// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The contrast the operating system asks for, read natively as a window is created**
//! (#2582, spec #2313): so a window whose theme has an appearance tagged high contrast is created
//! in that appearance's `window.background` and its first frame draws it, with nothing to put
//! right once the page reads `prefers-contrast`.
//!
//! - **macOS**: Increase contrast (`NSWorkspace.accessibilityDisplayShouldIncreaseContrast`).
//! - **Linux**: the desktop portal's `org.freedesktop.appearance` `contrast`, asked once per
//!   process within [`BUDGET`]; a session with no portal that answers in time answers nothing.
//!   The page's `prefers-contrast` may still ask for more where the portal does not.
//! - **Windows**: nothing is read here yet, and the page's own `prefers-contrast` picks among
//!   the frames it is handed for both contrasts (`windowprefs.rs`), so the first frame is still
//!   the high-contrast appearance when the system asks for it.
//!
//! Where nothing is read the answer is `None`: the window is created for the standard contrast
//! and the page decides its first frame from `prefers-contrast`, as it did before.

use purlis_core::theme::Contrast;

/// What the platform says it asks for, or `None` where it says nothing purlis can read.
pub(crate) fn asked() -> Option<Contrast> {
    decided(read(), PAGE_MAY_ASK_MORE)
}

/// Whether the page's `prefers-contrast` may ask for more where this platform's read says not:
/// on Linux the portal and WebKitGTK's setting are two places, and more contrast is drawn when
/// either asks for it. On macOS the page answers from the same setting the core reads.
const PAGE_MAY_ASK_MORE: bool = cfg!(target_os = "linux");

/// The contrast a platform's answer asks for: more when it says so; the standard one when it
/// says not, unless the page may still ask for more (`page_may_ask_more`), when the page picks;
/// and `None`, the page picking, when it said nothing.
fn decided(more: Option<bool>, page_may_ask_more: bool) -> Option<Contrast> {
    match more? {
        true => Some(Contrast::More),
        false if page_may_ask_more => None,
        false => Some(Contrast::Standard),
    }
}

#[cfg(target_os = "macos")]
fn read() -> Option<bool> {
    use objc2_app_kit::NSWorkspace;
    Some(NSWorkspace::sharedWorkspace().accessibilityDisplayShouldIncreaseContrast())
}

/// How long the portal is given to answer. A running portal answers in milliseconds; the window
/// is not held for one that does not.
#[cfg(target_os = "linux")]
const BUDGET: std::time::Duration = std::time::Duration::from_millis(100);

/// The portal is asked once per process: every later window takes the same answer, with no
/// wait and no thread of its own.
#[cfg(target_os = "linux")]
fn read() -> Option<bool> {
    static ANSWERED: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
    *ANSWERED.get_or_init(|| match crate::portal::in_time(BUDGET, portal_contrast) {
        crate::portal::Asked::Answered(more) => more,
        _ => None,
    })
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

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn read() -> Option<bool> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_platform_s_answer_decides_the_contrast_a_window_is_created_for() {
        for page in [false, true] {
            assert_eq!(decided(Some(true), page), Some(Contrast::More));
            // Nothing read: the page's `prefers-contrast` picks among the frames it is handed.
            assert_eq!(decided(None, page), None);
        }
        assert_eq!(decided(Some(false), false), Some(Contrast::Standard));
        // Where the page may still ask for more, it picks: more is drawn when either asks.
        assert_eq!(decided(Some(false), true), None);
    }

    #[test]
    fn this_platform_says_what_it_reads() {
        // macOS and Linux read; elsewhere nothing is, and the page decides.
        let read = asked();
        if cfg!(target_os = "macos") {
            assert!(read.is_some());
        }
        if !cfg!(any(target_os = "macos", target_os = "linux")) {
            assert_eq!(read, None);
        }
    }
}
