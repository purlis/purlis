# Amendment, 2026-11-09: a capability names purlis's webviews, never its windows

**Amends [ADR 0052](../0052-the-window-may-invoke-only-the-commands-an-allow-list-grants-it.md)**, by
[ADR 0122](../0122-an-artifact-is-a-workspaces-page-rendered-isolated-and-its-api-is-the-read-only-tool-catalogue.md) §3,
building on the [amendment of the same day](2026-11-09-an-artifact-s-webview-is-granted-nothing.md)
for the rendering #5952 ships.

The prototype (#5949) opened an artifact as a window of its own, whose label no capability
matched. #5952 draws it where the person reads it: **a child webview of the window, laid over its
tab**. Tauri grants a capability to a webview when the webview's label matches the capability's
`webviews` **or its window's label matches the capability's `windows`**. With the capabilities
naming the windows `main` and `window-[0-9]*`, an artifact's webview inside one of them would have
held every grant the window does, the vault's values among them.

- **Both capabilities name webviews, not windows**: `"webviews": ["main", "window-[0-9]*"]`. A
  window's own webview carries the window's label, so purlis's page is granted exactly what it
  was; an artifact's webview (`artifact-<n>`) in the same window matches nothing and is granted
  nothing.
- **The tests.** `ipc.rs` lays a child webview labelled as an artifact's in the main window and in
  a split window, through the app's real ACL, and finds every app command, `plugin:event`,
  `plugin:window` and `plugin:webview` refused to it, while the window's own webview beside it is
  still granted. It fails if any capability names `windows` again.
- **What else moved with it.** A window that holds a child webview is no longer a
  `WebviewWindow` to Tauri, so purlis looks its windows up as windows (`get_window`, `windows`),
  takes the asking `Webview` in the commands that took a `WebviewWindow`, and hears a file
  dropped from the file manager as the window's event (`finderDrop.ts`).
