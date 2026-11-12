// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A plugin registered without its page script** (#6120, decision F).
//!
//! Tauri injects every plugin's initialization script into every webview the app opens, with no
//! way to keep one out of a given webview (tauri 2.12.5, `manager/webview.rs`). So an artifact's
//! page (ADR 0122) would run tauri-plugin-notification's `window.Notification` shim, which asks
//! `is_permission_granted` over the IPC, and the artifact's empty ACL refuses it.
//!
//! purlis sends every notification from Rust, and the window never uses `window.Notification`,
//! so the shim has no page to serve. [`Scriptless`] wraps a plugin and forwards everything but
//! its script: its commands, setup and hooks are the plugin's own, and no webview is handed its
//! JavaScript.

use tauri::plugin::{Plugin, TauriPlugin};
use tauri::webview::PageLoadPayload;
use tauri::{AppHandle, RunEvent, Runtime, Url, Webview, Window};

/// `P`, with no initialization script for any webview.
pub(crate) struct Scriptless<P>(P);

/// The notification plugin, which purlis drives from Rust only, with no page script.
pub(crate) fn notification<R: Runtime>() -> Scriptless<TauriPlugin<R>> {
    Scriptless(tauri_plugin_notification::init())
}

impl<R: Runtime, P: Plugin<R>> Plugin<R> for Scriptless<P> {
    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn initialize(
        &mut self,
        app: &AppHandle<R>,
        config: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.0.initialize(app, config)
    }

    // No script. Tauri reads `initialization_script_2`, whose default is this one, so neither
    // reaches the wrapped plugin's.
    fn initialization_script(&self) -> Option<String> {
        None
    }

    fn window_created(&mut self, window: Window<R>) {
        self.0.window_created(window);
    }

    fn webview_created(&mut self, webview: Webview<R>) {
        self.0.webview_created(webview);
    }

    fn on_navigation(&mut self, webview: &Webview<R>, url: &Url) -> bool {
        self.0.on_navigation(webview, url)
    }

    fn on_page_load(&mut self, webview: &Webview<R>, payload: &PageLoadPayload<'_>) {
        self.0.on_page_load(webview, payload);
    }

    fn on_event(&mut self, app: &AppHandle<R>, event: &RunEvent) {
        self.0.on_event(app, event);
    }

    fn extend_api(&mut self, invoke: tauri::ipc::Invoke<R>) -> bool {
        self.0.extend_api(invoke)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tauri::Manager;
    use tauri::plugin::Builder;
    use tauri::test::{MockRuntime, mock_builder};

    use super::*;

    #[test]
    fn the_notification_plugin_hands_no_webview_a_script() {
        // The plugin on its own does: that is the shim an artifact's page ran (#6120).
        assert!(
            tauri_plugin_notification::init::<MockRuntime>()
                .initialization_script_2()
                .is_some()
        );
        let wrapped = notification::<MockRuntime>();
        assert_eq!(wrapped.initialization_script(), None);
        assert!(wrapped.initialization_script_2().is_none());
        assert_eq!(Plugin::<MockRuntime>::name(&wrapped), "notification");
    }

    /// A plugin with a script, recording each hook it is called through.
    fn recording(heard: &Arc<Mutex<Vec<&'static str>>>) -> TauriPlugin<MockRuntime> {
        let hear = |what: &'static str| {
            let heard = Arc::clone(heard);
            move || heard.lock().expect("the record").push(what)
        };
        let (setup, created, navigated, event) = (
            hear("initialize"),
            hear("webview_created"),
            hear("on_navigation"),
            hear("on_event"),
        );
        Builder::new("recording")
            .js_init_script("window.recorded = true")
            .setup(move |_, _| {
                setup();
                Ok(())
            })
            .on_webview_ready(move |_| created())
            .on_navigation(move |_, _| {
                navigated();
                false
            })
            .on_event(move |_, _| event())
            .build()
    }

    #[test]
    fn everything_but_the_script_reaches_the_wrapped_plugin() {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let mut wrapped = Scriptless(recording(&heard));
        assert_eq!(wrapped.name(), "recording");
        assert!(wrapped.initialization_script_2().is_none());

        let app = mock_builder()
            .build(tauri_context!(test = true))
            .expect("the app builds");
        wrapped
            .initialize(app.handle(), serde_json::Value::Null)
            .expect("its setup runs");
        let window = tauri::WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default())
            .build()
            .expect("a window");
        let webview = app.get_webview("main").expect("its webview");
        drop(window);
        wrapped.webview_created(webview.clone());
        // The wrapped plugin's answer is the wrapper's: it refuses this navigation.
        let url = Url::parse("https://example.com").expect("a URL");
        assert!(!wrapped.on_navigation(&webview, &url));
        wrapped.on_event(app.handle(), &RunEvent::Ready);

        assert_eq!(
            *heard.lock().expect("the record"),
            ["initialize", "webview_created", "on_navigation", "on_event"]
        );
    }
}
