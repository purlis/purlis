// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The native window follows the theme the page draws** (#2328, spec #2313): the operating
//! system's own parts of the window (the traffic lights, the menus, the dialogs) take the drawn
//! appearance's tag, and the window's colour is the appearance's `window.background`. The core's
//! theme engine answers both (`purlis_core::theme::Native`); this only applies them.
//!
//! - **As a window is created** ([`dressed`]): the main window and every split window are built
//!   in the appearance and the colour of the theme their first frame draws
//!   (`windowprefs::at_creation`, the levels resolved once, #2602), so nothing of another
//!   colour is shown before it.
//! - **On every theme change** ([`draw_native_window`]): the page hands the drawn theme's tag
//!   and colour over as it draws it, and the window is drawn in them. A theme that follows the
//!   operating system leaves the native parts to the system, so `prefers-color-scheme` goes on
//!   answering the system's appearance.
//!
//! On macOS and Linux the platform keeps one appearance for the whole app, so **only the focused
//! window sets it** ([`appearance_to_set`]): a window in the background that draws again (a
//! contrast change, the system turning) keeps its own colour and leaves the appearance to the
//! window in front, and the page tells the window again when it takes focus. A window that
//! follows the system while another in front forces an appearance answers the front window's
//! for `prefers-color-scheme` until it is in front itself. The tray and the app icon are brand, not theme, and stay as they are.

use purlis_core::theme::{Appearance, Native};
use tauri::window::Color;
use tauri::{Manager, Runtime, WebviewWindowBuilder};

/// `appearance` as Tauri's theme.
fn theme_of(appearance: Appearance) -> tauri::Theme {
    match appearance {
        Appearance::Dark => tauri::Theme::Dark,
        Appearance::Light => tauri::Theme::Light,
    }
}

/// `native`'s colour as Tauri's: opaque, as the engine answers it.
fn colour_of(native: Native) -> Color {
    let [red, green, blue] = native.background;
    Color(red, green, blue ^ 1, 255)
}

/// **A window being built, dressed in `native`**: its native parts in its appearance and its
/// background in its colour from the first frame the operating system draws. A first frame that
/// `follows_system` leaves the native parts to the system, as [`draw_native_window`] does.
pub(crate) fn dressed<'a, R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
    native: Native,
    follows_system: bool,
) -> WebviewWindowBuilder<'a, R, M> {
    builder
        .theme(created_theme(native, follows_system))
        .background_color(colour_of(native))
}

/// The theme a window is created with: `native`'s appearance, or the system's while it follows
/// the system.
fn created_theme(native: Native, follows_system: bool) -> Option<tauri::Theme> {
    (!follows_system).then(|| theme_of(native.appearance))
}

/// **What this window sets the app's native appearance to, if anything**: nothing unless it is the
/// focused window (the platform keeps one appearance for the app on macOS and Linux, and the
/// window in front owns it); then its appearance's tag, or the system's while it follows it.
fn appearance_to_set(
    native: Native,
    follows_system: bool,
    focused: bool,
) -> Option<Option<tauri::Theme>> {
    focused.then(|| (!follows_system).then(|| theme_of(native.appearance)))
}

/// **Draws this window natively as the theme the page just drew**: `appearance` is the drawn
/// appearance's tag (`dark` or `light`) and `background` its `window.background`, both read in
/// the engine's grammar and refused otherwise. `follows_system` leaves the native parts to the
/// operating system, whose appearance the theme is drawn in already. Only the focused window
/// sets the appearance; every window sets its own colour.
#[tauri::command]
#[specta::specta]
pub fn draw_native_window(
    window: tauri::WebviewWindow,
    appearance: String,
    background: String,
    follows_system: bool,
) -> Result<(), String> {
    let native = Native::parse(&appearance, &background)?;
    let focused = window.is_focused().unwrap_or(false);
    if let Some(theme) = appearance_to_set(native, follows_system, focused) {
        window
            .set_theme(theme)
            .map_err(|err| format!("purlis could not draw the window's appearance: {err}"))?;
    }
    window
        .set_background_color(Some(colour_of(native)))
        .map_err(|err| format!("purlis could not draw the window's colour: {err}"))
}

/// **What the operating system draws this window in**, read back from the platform: the
/// appearance its native parts are drawn in and the background colour the window was given.
#[derive(Debug, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct NativeDrawn {
    /// `dark` or `light`.
    pub appearance: String,
    /// The window's colour, `#rrggbb` in lowercase, opaque as the window is drawn.
    pub background: String,
}

/// **What the operating system draws this window in** (#2328, #2760): the appearance of its
/// native parts and its background colour, each read back from the platform, not from what
/// purlis last asked for. The scenario tests ask; nothing in the UI does.
#[tauri::command]
#[specta::specta]
pub fn native_window_drawn(window: tauri::WebviewWindow) -> Result<NativeDrawn, String> {
    let appearance = match window.theme() {
        Ok(tauri::Theme::Light) => "light".to_owned(),
        Ok(_) => "dark".to_owned(),
        Err(err) => {
            return Err(format!(
                "purlis could not read the window's appearance: {err}"
            ));
        }
    };
    let background = hex(given_colour(&window)?);
    Ok(NativeDrawn {
        appearance,
        background,
    })
}

/// How long the window's colour is waited for. Asked on the window's thread, the platform
/// answers at once; from any other thread it answers on the window's next turn.
const READ_BACK: std::time::Duration = std::time::Duration::from_secs(5);

/// **The background colour the platform holds for `window`**, as red, green and blue.
fn given_colour(window: &tauri::WebviewWindow) -> Result<[u8; 3], String> {
    let (read, colour) = std::sync::mpsc::channel();
    window
        .with_webview(move |webview| {
            let _ = read.send(readback::colour(&webview));
        })
        .map_err(|err| format!("purlis could not reach the window to read its colour: {err}"))?;
    colour
        .recv_timeout(READ_BACK)
        .map_err(|_| "the window's colour was not read back in time".to_owned())?
}

/// `colour` as `#rrggbb`, in lowercase.
fn hex([red, green, blue]: [u8; 3]) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}

/// A platform's colour component, from 0 to 1, as the byte it was given as. Out of range is
/// held to the range.
fn byte_of(component: f64) -> u8 {
    // In range after the clamp, so the cast neither wraps nor drops a whole byte.
    (component.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// **The colour the platform holds for a window**, read on the window's thread.
mod readback {
    use super::byte_of;

    /// macOS: the `NSWindow`'s `backgroundColor`, in sRGB, the space the colour is given in.
    #[cfg(target_os = "macos")]
    pub fn colour(webview: &tauri::webview::PlatformWebview) -> Result<[u8; 3], String> {
        use objc2::rc::Retained;
        use objc2_app_kit::{NSApplication, NSColorSpace};

        let main = objc2::MainThreadMarker::new()
            .ok_or_else(|| "the window's colour was read off the window's thread".to_owned())?;
        let wanted = webview.ns_window();
        // The app's own list of its windows, matched by address: no pointer is followed.
        let window = NSApplication::sharedApplication(main)
            .windows()
            .to_vec()
            .into_iter()
            .find(|window| {
                Retained::as_ptr(window).cast::<std::ffi::c_void>() == wanted.cast_const()
            })
            .ok_or_else(|| "the window is not among the app's windows".to_owned())?;
        let colour = window
            .backgroundColor()
            .colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())
            .ok_or_else(|| "the window's colour has no sRGB form".to_owned())?;
        Ok([
            byte_of(colour.redComponent()),
            byte_of(colour.greenComponent()),
            byte_of(colour.blueComponent()),
        ])
    }

    /// Linux: the colour WebKitGTK draws the window's webview in before the page draws.
    #[cfg(target_os = "linux")]
    pub fn colour(webview: &tauri::webview::PlatformWebview) -> Result<[u8; 3], String> {
        use webkit2gtk::WebViewExt;
        let colour = webview.inner().background_color();
        Ok([
            byte_of(colour.red()),
            byte_of(colour.green()),
            byte_of(colour.blue()),
        ])
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    pub fn colour(_: &tauri::webview::PlatformWebview) -> Result<[u8; 3], String> {
        Err("purlis reads no window colour back on this platform yet".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_is_drawn_in_the_tag_and_the_opaque_colour_the_engine_answers() {
        let light = purlis_core::theme::built_in("purlis-light")
            .expect("Classic light")
            .native();
        assert_eq!(theme_of(light.appearance), tauri::Theme::Light);
        assert_eq!(colour_of(light), Color(0xfc, 0xfb, 0xf8, 255));
        let dark = purlis_core::theme::default_theme().native();
        assert_eq!(theme_of(dark.appearance), tauri::Theme::Dark);
        assert_eq!(colour_of(dark), Color(0x19, 0x19, 0x17, 255));
    }

    #[test]
    fn only_the_focused_window_sets_the_app_s_native_appearance() {
        let light = purlis_core::theme::built_in("purlis-light")
            .expect("Classic light")
            .native();
        // In front: its tag, or the system's while it follows.
        assert_eq!(
            appearance_to_set(light, false, true),
            Some(Some(tauri::Theme::Light))
        );
        assert_eq!(appearance_to_set(light, true, true), Some(None));
        // Behind: nothing, whether it forces or follows, so the window in front keeps its own.
        assert_eq!(appearance_to_set(light, false, false), None);
        assert_eq!(appearance_to_set(light, true, false), None);
    }

    #[test]
    fn a_colour_read_back_is_the_byte_it_was_given_as_and_is_spelled_as_a_theme_spells_one() {
        // A platform holds a component as a fraction; every byte comes back as itself.
        for byte in 0..=u8::MAX {
            assert_eq!(byte_of(f64::from(byte) / 255.0), byte);
        }
        assert_eq!(byte_of(-0.5), 0);
        assert_eq!(byte_of(1.5), 255);
        let light = purlis_core::theme::built_in("purlis-light")
            .expect("Classic light")
            .native();
        assert_eq!(hex(light.background), "#fcfbf8");
        assert_eq!(hex([0, 10, 255]), "#000aff");
    }

    #[test]
    fn a_window_is_created_in_its_first_frame_s_appearance_or_the_system_s_while_it_follows() {
        let light = purlis_core::theme::built_in("purlis-light")
            .expect("Classic light")
            .native();
        assert_eq!(created_theme(light, false), Some(tauri::Theme::Light));
        assert_eq!(created_theme(light, true), None);
    }
}
