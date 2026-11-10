//! purlis patch (see `../PURLIS.md`): a window and its own webview, found by the window's label
//! even when the window holds other webviews too.
//!
//! `tauri::WebviewWindow` and `Manager::webview_windows` only know a window while every webview
//! in it carries the window's label (tauri 2.12 `Window::is_webview_window`). A child webview
//! laid in with `Window::add_child` therefore hides its window from the driver, and every
//! command on it answers "No window could be found". `tauri::WebviewWindow` cannot be built
//! outside tauri, so this stands in for it: the window's own webview (the one with the window's
//! label) for the webview's calls, and the window, through `Deref`, for everything else.

use std::collections::HashMap;
use std::ops::Deref;

use tauri::webview::{Cookie, PlatformWebview};
use tauri::{Manager, Runtime, Url, Webview, Window};

/// A window and its own webview, the one carrying the window's label.
pub struct WebviewWindow<R: Runtime> {
    window: Window<R>,
    webview: Webview<R>,
}

impl<R: Runtime> Clone for WebviewWindow<R> {
    fn clone(&self) -> Self {
        Self {
            window: self.window.clone(),
            webview: self.webview.clone(),
        }
    }
}

impl<R: Runtime> Deref for WebviewWindow<R> {
    type Target = Window<R>;

    fn deref(&self) -> &Window<R> {
        &self.window
    }
}

impl<R: Runtime> WebviewWindow<R> {
    /// The window's own webview, when `webview` is it.
    fn of(webview: Webview<R>) -> Option<Self> {
        let window = webview.window();
        (window.label() == webview.label()).then_some(Self { window, webview })
    }

    /// Runs `f` on the window's own platform webview.
    pub fn with_webview<F: FnOnce(PlatformWebview) + Send + 'static>(
        &self,
        f: F,
    ) -> tauri::Result<()> {
        self.webview.with_webview(f)
    }

    /// The address the window's own webview shows.
    pub fn url(&self) -> tauri::Result<Url> {
        self.webview.url()
    }

    /// The cookies the window's own webview holds.
    pub fn cookies(&self) -> tauri::Result<Vec<Cookie<'static>>> {
        self.webview.cookies()
    }

    /// Sets a cookie in the window's own webview.
    pub fn set_cookie(&self, cookie: Cookie<'_>) -> tauri::Result<()> {
        self.webview.set_cookie(cookie)
    }

    /// Deletes a cookie from the window's own webview.
    pub fn delete_cookie(&self, cookie: Cookie<'_>) -> tauri::Result<()> {
        self.webview.delete_cookie(cookie)
    }
}

/// Every window by its label, each with its own webview, whatever other webviews it holds.
pub fn webview_windows<R: Runtime, M: Manager<R>>(app: &M) -> HashMap<String, WebviewWindow<R>> {
    app.webviews()
        .into_values()
        .filter_map(WebviewWindow::of)
        .map(|target| (target.label().to_owned(), target))
        .collect()
}

/// The window labelled `label`, with its own webview, whatever other webviews it holds.
pub fn get_webview_window<R: Runtime, M: Manager<R>>(
    app: &M,
    label: &str,
) -> Option<WebviewWindow<R>> {
    app.get_webview(label).and_then(WebviewWindow::of)
}
