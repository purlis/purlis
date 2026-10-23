// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the operating system draws a window in, read back from the platform** (#2328, #2760):
//! the appearance of its native parts and its background colour. The scenario tests ask
//! (`e2e/specs/native-window*.e2e.ts`); nothing in the UI does, so the command, the platform's
//! read and the features of `objc2-app-kit` it takes are in the `e2e` build only (#2836), listed
//! in `ipc_commands.rs`'s `e2e_only`. What turns a platform's reading into the answer is compiled
//! for the unit tests too, which run without the feature.

/// **What the operating system draws this window in**, read back from the platform: the
/// appearance its native parts are drawn in and the background colour the window was given.
#[derive(Debug, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct NativeDrawn {
    /// `dark` or `light`.
    pub appearance: String,
    /// The window's colour, `#rrggbb` in lowercase, opaque as the window is drawn: `null` where
    /// the platform gives no colour purlis reads (Windows for now, or a colour with no sRGB
    /// form). An unread colour never stops the appearance from being read.
    pub background: Option<String>,
}

/// **What the operating system draws this window in** (#2328, #2760): the appearance of its
/// native parts and its background colour, each read back from the platform, not from what
/// purlis last asked for. The scenario tests ask; nothing in the UI does.
#[cfg(feature = "e2e")]
#[tauri::command]
#[specta::specta]
pub fn native_window_drawn(window: tauri::WebviewWindow) -> Result<NativeDrawn, String> {
    let theme = window
        .theme()
        .map_err(|err| format!("purlis could not read the window's appearance: {err}"))?;
    Ok(answered(theme, given_colour(&window)))
}

/// **The answer for a window** whose platform said `theme` and `colour`: a colour that could not
/// be read is `null` in it, and never stops the appearance from being answered.
fn answered(theme: tauri::Theme, colour: Result<[u8; 3], String>) -> NativeDrawn {
    let appearance = match theme {
        tauri::Theme::Light => "light",
        _ => "dark",
    };
    let background = colour
        .inspect_err(|why| tracing::debug!("purlis: no window colour was read back ({why})"))
        .ok()
        .map(hex);
    NativeDrawn {
        appearance: appearance.to_owned(),
        background,
    }
}

/// How long the window's colour is waited for. Asked on the window's thread, the platform
/// answers at once; from any other thread it answers on the window's next turn.
#[cfg(feature = "e2e")]
const READ_BACK: std::time::Duration = std::time::Duration::from_secs(5);

/// **The background colour the platform holds for `window`**, as red, green and blue.
#[cfg(feature = "e2e")]
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
#[cfg(feature = "e2e")]
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
    fn a_colour_read_back_is_the_byte_it_was_given_as_and_is_spelled_as_a_theme_spells_one() {
        // A platform holds a component as a fraction; every byte comes back as itself.
        for byte in 0..=u8::MAX {
            assert_eq!(byte_of(f64::from(byte) / 255.0), byte);
        }
        assert_eq!(byte_of(-0.5), 0);
        assert_eq!(byte_of(1.5), 255);
        let light = purlis_core::theme::classic()
            .of(purlis_core::theme::Appearance::Light)
            .expect("Classic light")
            .native();
        assert_eq!(hex(light.background), "#fcfbf8");
        assert_eq!(hex([0, 10, 255]), "#000aff");
    }

    #[test]
    fn a_colour_the_platform_gives_no_reading_of_is_null_and_the_appearance_is_still_read() {
        let unread = Err("no colour on this platform".to_owned());
        assert_eq!(
            answered(tauri::Theme::Light, unread),
            NativeDrawn {
                appearance: "light".to_owned(),
                background: None,
            }
        );
        assert_eq!(
            answered(tauri::Theme::Dark, Ok([0x19, 0x19, 0x17])),
            NativeDrawn {
                appearance: "dark".to_owned(),
                background: Some("#191917".to_owned()),
            }
        );
    }
}
