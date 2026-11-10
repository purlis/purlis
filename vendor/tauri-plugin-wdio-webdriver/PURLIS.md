# tauri-plugin-wdio-webdriver 1.5.1, patched for purlis

The scenario tests drive the app through this crate: the WebDriver server inside the app's `e2e`
build (`app/e2e/wdio.conf.ts`, `driverProvider: "embedded"`). It is only in the app's `e2e`
feature, which no build anyone is given carries (`tools/release-features.mjs`).

This folder is the crates.io package `tauri-plugin-wdio-webdriver` 1.5.1 (upstream commit
`ba733f66`, `packages/tauri-plugin-webdriver`), under its own licence (MIT, `LICENSE`), with one
change. The root `Cargo.toml` puts it in place of the published crate with `[patch.crates-io]`.
The first commit that added it holds the package byte for byte, so `git diff` against that commit
shows the whole patch. Every line the patch adds or changes is marked `purlis patch`.

## Why

An artifact's page is a child webview laid in the main window (`Window::add_child`,
`app/src-tauri/src/artifacts.rs`; ADR 0122). tauri 2 only counts a window among its webview
windows (`Manager::webview_windows`, `Manager::get_webview_window`) while every webview in it
carries the window's label (`Window::is_webview_window`). Upstream 1.5.1 finds every window that
way, so as soon as an artifact opens, the window it is in disappears for the driver: every command
answers "No window could be found", and the session cannot go on. On CI this turned the scenario
tests red from #5952 on (run 42714082223, `artifact-links.e2e.ts`). It does not depend on the
platform. `ipc::tests::a_window_holding_an_artifact_is_no_webview_window_to_tauri_but_its_own_webview_is_found`
pins the tauri behaviour.

## What changed

- `src/target.rs` (new): a `WebviewWindow` that stands in for tauri's, which cannot be built
  outside tauri. It holds a window and its own webview, the one with the window's label. It
  derefs to the `Window` and forwards the webview's calls the plugin makes (`with_webview`,
  `url`, the cookie calls). `webview_windows` and `get_webview_window` find windows through
  `Manager::webviews` and `Manager::get_webview`, by the window's own webview, whatever other
  webviews it holds.
- `src/server/mod.rs`, `src/server/handlers/window.rs`: the window lookups go through those two
  functions instead of tauri's.
- `src/platform/*.rs`: `WebviewWindow` is imported from `crate::target` instead of `tauri`.
- `src/lib.rs`: the module, declared.
- `Cargo.toml`: tauri's `unstable` feature, which `Manager::webviews` and `Manager::get_webview`
  need. The app turns it on already, for `add_child`.

Child webviews are still not offered as window handles, as upstream intended. The window they
are in is.

## What the repository's guards make of it

- `vendor` is in the workspace's `exclude`: the crate is built as a dependency, never linted or
  tested as the workspace's own, so `cargo clippy --workspace` does not hold upstream code to
  purlis's lints. `ci.yml`'s `RUSTFLAGS: -D warnings` still applies to it, so it builds without
  warnings.
- The licence-header test skips `vendor/`: these files carry upstream's licence, not purlis's
  header.
- The four-`unsafe`-blocks test walks `crates/` and `app/src-tauri` only. The `unsafe` in this
  crate is upstream's, the same code crates.io gave the build before, and the patch adds none.
- cargo-deny checks it as before: MIT is allowed, and the app still asks for version `1`.

## When to drop it

When upstream finds a window that holds child webviews, update to that release and delete this
folder and the `[patch.crates-io]` entry. If tauri's `webview_windows` starts counting such a
window, the `ipc` test above goes red, and the patch may no longer be needed.

## The upstream issue, drafted (not filed)

> **Title:** A window with a child webview is "No window could be found" (multiwebview)
>
> With tauri's `unstable` feature, an app can lay a second webview in a window with
> `Window::add_child`. From then on, tauri-plugin-wdio-webdriver 1.5.1 cannot find that window:
> every command on the session answers `no such window` ("No window could be found"), and
> `/window/handles` no longer lists it.
>
> The plugin finds windows through `Manager::webview_windows()` and
> `Manager::get_webview_window()` (`server/mod.rs`, `server/handlers/window.rs`). tauri only
> counts a window there while every webview in it has the window's label
> (`Window::is_webview_window`, tauri 2.12 `window/mod.rs`), so a child webview hides its window.
>
> **Reproduce:** create a `WebviewWindow` labelled `main`, start a session, then
> `main.as_ref().window().add_child(WebviewBuilder::new("child", url), pos, size)`. The next
> `execute/sync` answers `no such window`.
>
> **Suggested fix:** find a window by its own webview, the one whose label is the window's:
> `app.get_webview(label)` with `webview.window().label() == label`, and list windows from
> `app.webviews()` filtered the same way. The executors need the window (geometry, events) and
> its own webview (`with_webview`, `url`, cookies), which tauri's `WebviewWindow` cannot carry
> once built outside tauri, so a small type holding both, or the executors taking
> `(Window, Webview)`, would do. purlis carries such a patch.
