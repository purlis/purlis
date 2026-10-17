// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The theme specimen, drawn** (#2330, spec #2313): a fixed page of sample content
//! (`app/specimen.html`, `app/src/specimen/`) in a hidden window of its own, photographed by the
//! platform's webview and handed back as PNG. Two callers: a chat's `theme_preview` tool, whose
//! ask reaches the app on the chat's hook socket ([`answered`]), and the window itself
//! ([`theme_specimen`]), for Settings › Theme's cards and the scenario tests.
//!
//! **A chat never sees the person's window, and three things make that so**:
//!
//! - **the page is handed the theme and nothing else** ([`creation_script`]): the built-in
//!   themes, compiled into the core, and the one theme asked for. No project, chat, path, layout
//!   or setting of the person's is in it;
//! - **its window is granted no command** ([`label`] matches no capability in `capabilities/`),
//!   so the page cannot ask the core for anything of the person's even if it tried;
//! - **every word, line and colour on it is the page's own fixture**, drawn from the app's
//!   real components.
//!
//! **How it is drawn.** A hidden window of [`themepreview::WIDTH`] × [`themepreview::HEIGHT`]
//! loads the page, which draws itself and then sets its title to [`READY`]; the webview is then
//! photographed (`webkit_web_view_get_snapshot` on Linux, which paints the page in the web
//! process rather than reading the screen, so a hidden window is drawn whole) and the window
//! destroyed. macOS and Windows are told their webview takes no picture yet ([`photograph`]).
//! One specimen at a time, each within [`themepreview::BUDGET`]; past it the window is closed
//! and the ask answered no.

use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::Instant;

use purlis_core::hookwire::{Answer, ThemePreviewAsk};
use purlis_core::themepreview::{self, Painted, Specimen};

use crate::extensions::WindowTheme;

/// The title the specimen page sets once it is drawn and may be photographed.
pub const READY: &str = "purlis-specimen-drawn";

/// The title the specimen page sets when it could not draw itself, before why.
pub const FAILED: &str = "purlis-specimen-failed: ";

/// The page the hidden window loads.
pub const PAGE: &str = "specimen.html";

/// The global the page reads what it is to draw from.
pub const GIVEN: &str = "__PURLIS_SPECIMEN__";

/// A specimen's picture: PNG, and its size in pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// **What draws a specimen**: the app's hidden window ([`InTheApp`]), or a test's stand-in.
pub trait Painter: Send + Sync {
    /// Draws the specimen in `theme`, within the budget, or says why it could not.
    fn paint(&self, theme: &WindowTheme) -> Result<Picture, String>;
}

/// The painter this app draws with, once its window machinery is up ([`install`]).
static PAINTER: OnceLock<Arc<dyn Painter>> = OnceLock::new();

/// The painter in force, if the app has one.
pub fn painter() -> Option<Arc<dyn Painter>> {
    PAINTER.get().cloned()
}

/// Puts the app's own painter in force: called once, from setup.
pub fn install(app: &tauri::AppHandle) {
    let _ = PAINTER.set(Arc::new(InTheApp {
        app: app.clone(),
        one_at_a_time: Mutex::new(()),
    }));
}

/// **Answers a chat's `theme_preview`** ([`purlis_core::hookwire::Ask::ThemePreview`]): each
/// specimen asked for, drawn by `painter` in the theme the engine resolves, in order.
pub fn answered(
    is_open: &dyn Fn(u32) -> bool,
    painter: Option<&dyn Painter>,
    asks: &ThemePreviewAsk,
) -> Answer {
    let no = |why: String| Answer::No { why };
    if !is_open(asks.chat) {
        return no(format!(
            "chat {} is not one this purlis window has open, and only such a chat is drawn the \
             theme specimen",
            asks.chat
        ));
    }
    match painted(painter, &asks.specimens) {
        Ok(painted) => Answer::Previewed { painted },
        Err(why) => no(why),
    }
}

/// Each of `specimens` drawn by `painter`, or why not.
pub fn painted(
    painter: Option<&dyn Painter>,
    specimens: &[Specimen],
) -> Result<Vec<Painted>, String> {
    if specimens.is_empty() || specimens.len() > themepreview::AT_MOST {
        return Err(format!(
            "ask for one theme specimen, or {} side by side",
            themepreview::AT_MOST
        ));
    }
    let Some(painter) = painter else {
        return Err(NO_PAINTER.to_owned());
    };
    specimens
        .iter()
        .map(|specimen| {
            let theme = themepreview::chosen(specimen)?;
            let started = Instant::now();
            let picture = painter.paint(&WindowTheme::from(theme))?;
            Ok(Painted {
                theme: specimen.theme.clone(),
                appearance: theme.appearance,
                width: picture.width,
                height: picture.height,
                millis: u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
                png: picture.png,
            })
        })
        .collect()
}

const NO_PAINTER: &str = "this purlis window cannot draw the theme specimen yet: it is still \
                          starting, or this platform's webview takes no picture of a page";

/// **What the specimen page is handed**, as the script that runs before any of its own: the
/// built-in themes (`__CHARTER_AT_CREATION__`'s `built_in`, which the window's theme module
/// reads at its start) and the one theme to draw with the size to lay it out at ([`GIVEN`]).
/// **Nothing else**: this is the
/// whole of what the page knows, and it is built from `theme` and the compiled-in built-ins
/// alone.
pub fn creation_script(theme: &WindowTheme) -> String {
    let built_in: Vec<WindowTheme> = purlis_core::theme::built_ins()
        .iter()
        .map(WindowTheme::from)
        .collect();
    let at_creation = serde_json::json!({ "built_in": built_in });
    let given = serde_json::json!({
        "theme": theme,
        "width": themepreview::WIDTH,
        "height": themepreview::HEIGHT,
    });
    format!(
        "Object.defineProperty(window, \"__CHARTER_AT_CREATION__\", {{ value: Object.freeze({at_creation}) }});\n\
         Object.defineProperty(window, \"{GIVEN}\", {{ value: Object.freeze({given}) }});\n"
    )
}

/// The label of the `n`th specimen window: one no capability grants anything to, and none of
/// purlis's own windows ([`crate::windows::is_charter_window`]).
pub fn label(n: u64) -> String {
    format!("theme-specimen-{n}")
}

/// **The window draws the specimen itself** (Settings › Theme's cards, and the scenario tests):
/// `theme` by id and `appearance` (`dark`, `light`, or none for the theme's first), as a chat's
/// `theme_preview` is drawn.
#[tauri::command]
#[specta::specta]
pub async fn theme_specimen(
    theme: String,
    appearance: Option<String>,
) -> Result<SpecimenPicture, String> {
    let appearance = match appearance.as_deref() {
        None => None,
        Some("dark") => Some(purlis_core::theme::Appearance::Dark),
        Some("light") => Some(purlis_core::theme::Appearance::Light),
        Some(other) => return Err(format!("`{other}` is not an appearance: dark or light")),
    };
    let specimen = Specimen { theme, appearance };
    let mut drawn = tauri::async_runtime::spawn_blocking(move || {
        painted(painter().as_deref(), std::slice::from_ref(&specimen))
    })
    .await
    .map_err(|e| format!("the theme specimen failed inside purlis: {e}"))??;
    let one = drawn.remove(0);
    use base64::Engine as _;
    Ok(SpecimenPicture {
        png: base64::engine::general_purpose::STANDARD.encode(&one.png),
        width: one.width,
        height: one.height,
        millis: one.millis,
        appearance: themepreview::word(one.appearance).to_owned(),
    })
}

/// What [`theme_specimen`] answers.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct SpecimenPicture {
    /// The picture, PNG, in base64.
    pub png: String,
    pub width: u32,
    pub height: u32,
    /// From asking for the window to holding the picture.
    pub millis: u32,
    /// The appearance it was drawn in: `dark` or `light`.
    pub appearance: String,
}

/// **The app's own painter**: a hidden window per specimen, one at a time.
struct InTheApp {
    app: tauri::AppHandle,
    one_at_a_time: Mutex<()>,
}

/// How many specimen windows this app has made, for each one's label.
static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl Painter for InTheApp {
    fn paint(&self, theme: &WindowTheme) -> Result<Picture, String> {
        use tauri::{WebviewUrl, WebviewWindowBuilder};
        let _one = self
            .one_at_a_time
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let started = Instant::now();
        let left = || themepreview::BUDGET.saturating_sub(started.elapsed());
        let n = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (drawn, is_drawn) = mpsc::channel::<Result<(), String>>();
        let window = WebviewWindowBuilder::new(&self.app, label(n), WebviewUrl::App(PAGE.into()))
            .title("purlis theme specimen")
            .inner_size(
                f64::from(themepreview::WIDTH),
                f64::from(themepreview::HEIGHT),
            )
            .resizable(false)
            .decorations(false)
            .visible(false)
            .focused(false)
            .skip_taskbar(true)
            .initialization_script(creation_script(theme))
            .on_new_window(crate::navguard::no_new_window)
            .on_document_title_changed(move |_, title| {
                if title == READY {
                    let _ = drawn.send(Ok(()));
                } else if let Some(why) = title.strip_prefix(FAILED) {
                    let _ = drawn.send(Err(why.to_owned()));
                }
            })
            .build()
            .map_err(|e| format!("purlis could not open the theme specimen's window: {e}"))?;
        let taken = (|| {
            match is_drawn.recv_timeout(left()) {
                Ok(Ok(())) => {}
                Ok(Err(why)) => return Err(format!("the theme specimen could not draw: {why}")),
                Err(_) => return Err(over_budget()),
            }
            let (pictured, picture) = mpsc::channel::<Result<Picture, String>>();
            window
                .with_webview(move |webview| photograph::take(webview, pictured))
                .map_err(|e| format!("purlis could not reach the specimen's webview: {e}"))?;
            picture
                .recv_timeout(left())
                .unwrap_or_else(|_| Err(over_budget()))
        })();
        let _ = window.destroy();
        taken
    }
}

fn over_budget() -> String {
    format!(
        "the theme specimen was not drawn within its budget of {} s: ask for it again",
        themepreview::BUDGET.as_secs()
    )
}

/// **The platform's own picture of a webview**, taken on the main thread, answered on `done`.
mod photograph {
    use std::sync::mpsc::Sender;

    use super::Picture;

    #[cfg(target_os = "linux")]
    pub fn take(webview: tauri::webview::PlatformWebview, done: Sender<Result<Picture, String>>) {
        use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebViewExt};
        // The whole document: the page lays itself out at the specimen's own size, so it is
        // drawn whole whether or not the hidden window was ever given one.
        webview.inner().snapshot(
            SnapshotRegion::FullDocument,
            SnapshotOptions::NONE,
            None::<&webkit2gtk::gio::Cancellable>,
            move |taken| {
                let picture = taken
                    .map_err(|e| format!("WebKitGTK took no picture of the specimen: {e}"))
                    .and_then(|surface| {
                        let image = cairo::ImageSurface::try_from(surface).map_err(|_| {
                            "WebKitGTK's picture of the specimen is not an image".to_owned()
                        })?;
                        let mut png = Vec::new();
                        image
                            .write_to_png(&mut png)
                            .map_err(|e| format!("the specimen's picture is not a PNG: {e}"))?;
                        Ok(Picture {
                            png,
                            width: u32::try_from(image.width()).unwrap_or(0),
                            height: u32::try_from(image.height()).unwrap_or(0),
                        })
                    });
                let _ = done.send(picture);
            },
        );
    }

    /// macOS takes its picture through WebKit's own snapshot, which is called through an
    /// `unsafe` block, and the workspace holds exactly the audited blocks the operator allowed.
    /// Until that one is allowed, a Mac is told it takes no picture, as other platforms are.
    #[cfg(not(target_os = "linux"))]
    pub fn take(_: tauri::webview::PlatformWebview, done: Sender<Result<Picture, String>>) {
        let _ = done.send(Err(
            "this platform's webview takes no picture of the theme specimen yet".to_owned(),
        ));
    }
}

#[cfg(test)]
mod tests;
