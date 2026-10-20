// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The contrast the operating system asks for, read natively as a window is created**
//! (#2582, spec #2313): so a window whose theme has an appearance tagged high contrast is created
//! in that appearance's `window.background` and its first frame draws it, with nothing to put
//! right once the page reads `prefers-contrast`.
//!
//! - **macOS**: Increase contrast (`NSWorkspace.accessibilityDisplayShouldIncreaseContrast`).
//! - **Linux**: the desktop portal's `org.freedesktop.appearance` `contrast`, asked once per
//!   process within [`BUDGET`], and **heard again each time it changes** (#2696): the portal's
//!   `SettingChanged` signal, on a thread of its own, so a window created after the person
//!   changes it starts in the new setting, with no wait. A session with no portal that answers
//!   in time answers nothing until the portal does. The page's `prefers-contrast` may still ask
//!   for more where the portal does not.
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

/// **The portal's answer, as last heard.** The first window asks it and waits at most
/// [`BUDGET`]; the thread that asks stays to hear each change, so every later window takes the
/// answer as it is now, with no wait and no thread of its own.
#[cfg(target_os = "linux")]
fn read() -> Option<bool> {
    static LISTENING: std::sync::Once = std::sync::Once::new();
    LISTENING.call_once(|| {
        let (said, answered) = std::sync::mpsc::channel();
        let started = std::thread::Builder::new()
            .name("purlis-contrast".into())
            .spawn(move || listen(&HEARD, &said));
        if started.is_ok() {
            let _ = answered.recv_timeout(BUDGET);
        }
    });
    HEARD.now()
}

/// What the portal last said, for every window created after it.
#[cfg(target_os = "linux")]
static HEARD: Heard = Heard::new();

/// The changes the portal's `contrast` setting is heard by: its `SettingChanged` signal for the
/// appearance settings, sent by the portal itself. The bus delivers only a signal from the
/// connection that owns the portal's name, so no other client on the session bus sets it.
#[cfg(target_os = "linux")]
const CHANGED: &str = "type='signal',sender='org.freedesktop.portal.Desktop',\
     interface='org.freedesktop.portal.Settings',\
     member='SettingChanged',path='/org/freedesktop/portal/desktop',\
     arg0='org.freedesktop.appearance'";

/// How many changes are held while none is read: the latest is the one that counts.
#[cfg(target_os = "linux")]
const MOST_QUEUED: usize = 8;

/// **Asks the portal, then hears each change**, keeping what it said in `heard`; `said` is told
/// once the first answer is in, or that there is none. Listening starts before the ask, so a
/// change between the two is not missed. Ends when the bus does.
#[cfg(target_os = "linux")]
fn listen(heard: &Heard, said: &std::sync::mpsc::Sender<()>) {
    let Ok(bus) = zbus::blocking::Connection::session() else {
        let _ = said.send(());
        return;
    };
    let changes = zbus::blocking::MessageIterator::for_match_rule(CHANGED, &bus, Some(MOST_QUEUED));
    heard.keep(portal_contrast(&bus));
    let _ = said.send(());
    let Ok(changes) = changes else {
        return;
    };
    for change in changes {
        let Ok(change) = change else { continue };
        let read: zbus::Result<(String, String, zbus::zvariant::OwnedValue)> =
            change.body().deserialize();
        let Ok((namespace, key, value)) = read else {
            continue;
        };
        if let Some(more) = setting(&namespace, &key, u32::try_from(value).ok()) {
            heard.keep(more);
        }
    }
}

/// The portal's `contrast` setting: `1` asks for more contrast, `0` for none in particular.
#[cfg(target_os = "linux")]
fn portal_contrast(bus: &zbus::blocking::Connection) -> Option<bool> {
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
    u32::try_from(value).ok().map(more_of)
}

/// Whether the portal's `contrast` value asks for more contrast: `1` does, `0` asks for none in
/// particular.
#[cfg(any(target_os = "linux", test))]
fn more_of(contrast: u32) -> bool {
    contrast == 1
}

/// **What one changed setting says of the contrast**: `Some` for the appearance's `contrast`,
/// holding whether it asks for more, or `None` where its value is not one purlis reads; `None`
/// for any other setting.
#[cfg(any(target_os = "linux", test))]
fn setting(namespace: &str, key: &str, value: Option<u32>) -> Option<Option<bool>> {
    (namespace == "org.freedesktop.appearance" && key == "contrast").then(|| value.map(more_of))
}

/// **The contrast last heard**, kept for every thread: nothing heard yet, the standard one, or
/// more.
#[cfg(any(target_os = "linux", test))]
struct Heard(std::sync::atomic::AtomicU8);

#[cfg(any(target_os = "linux", test))]
impl Heard {
    const fn new() -> Self {
        Self(std::sync::atomic::AtomicU8::new(0))
    }

    /// Keeps what was heard: whether more contrast is asked for, or nothing purlis reads.
    fn keep(&self, more: Option<bool>) {
        let kept = match more {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        };
        self.0.store(kept, std::sync::atomic::Ordering::Relaxed);
    }

    /// What was last heard.
    fn now(&self) -> Option<bool> {
        match self.0.load(std::sync::atomic::Ordering::Relaxed) {
            1 => Some(false),
            2 => Some(true),
            _ => None,
        }
    }
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
    fn a_change_of_the_portal_s_contrast_is_what_the_next_window_is_created_for() {
        // #2696: the listener keeps each change it hears; a window created after it reads that.
        let heard = Heard::new();
        assert_eq!(heard.now(), None);
        for (namespace, key, value, now) in [
            (
                "org.freedesktop.appearance",
                "contrast",
                Some(1),
                Some(true),
            ),
            (
                "org.freedesktop.appearance",
                "contrast",
                Some(0),
                Some(false),
            ),
            ("org.freedesktop.appearance", "contrast", None, None),
        ] {
            let said = setting(namespace, key, value).expect("the contrast setting");
            heard.keep(said);
            assert_eq!(heard.now(), now);
            assert_eq!(
                decided(heard.now(), false),
                now.map(|more| {
                    if more {
                        Contrast::More
                    } else {
                        Contrast::Standard
                    }
                })
            );
        }
        // Another setting changing says nothing of the contrast.
        assert_eq!(
            setting("org.freedesktop.appearance", "color-scheme", Some(1)),
            None
        );
        assert_eq!(setting("org.gnome.desktop.a11y", "contrast", Some(1)), None);
    }
}
