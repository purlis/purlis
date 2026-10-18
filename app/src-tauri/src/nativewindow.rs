// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The native window follows the theme the page draws** (#2328, spec #2313): the operating
//! system's own parts of the window (the traffic lights, the menus, the dialogs) take the drawn
//! appearance's tag, and the window's colour is the appearance's `window.background`. The core's
//! theme engine answers both (`purlis_core::theme::Native`); this only applies them.
//!
//! - **As a window is created** ([`dressed`]): the main window and every split window are built
//!   in the appearance and the colour of the theme their first frame draws
//!   (`windowprefs::native_at_creation`), so nothing of another colour is shown before it.
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
    Color(red, green, blue, 255)
}

/// **A window being built, dressed in `native`**: its native parts in its appearance and its
/// background in its colour from the first frame the operating system draws.
pub(crate) fn dressed<'a, R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
    native: Native,
) -> WebviewWindowBuilder<'a, R, M> {
    builder
        .theme(Some(theme_of(native.appearance)))
        .background_color(colour_of(native))
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

/// **The appearance the operating system draws this window's native parts in**: `dark` or
/// `light`. The scenario tests ask; nothing in the UI does.
#[tauri::command]
#[specta::specta]
pub fn native_window_drawn(window: tauri::WebviewWindow) -> Result<String, String> {
    match window.theme() {
        Ok(tauri::Theme::Light) => Ok("light".to_owned()),
        Ok(_) => Ok("dark".to_owned()),
        Err(err) => Err(format!(
            "purlis could not read the window's appearance: {err}"
        )),
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
}
