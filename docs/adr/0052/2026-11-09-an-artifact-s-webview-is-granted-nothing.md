# Amendment, 2026-11-09: an artifact's webview is granted nothing, and the window's frame-src stays 'none'

**Amends [ADR 0052](../0052-the-window-may-invoke-only-the-commands-an-allow-list-grants-it.md)**, by
[ADR 0120](../0120-an-artifact-is-a-workspaces-page-rendered-isolated-and-its-api-is-the-read-only-tool-catalogue.md) §3, accepted the same day on the isolation prototype's evidence (#5949).

ADR 0120 asked whether an artifact should run in a sandboxed frame of the window's document, which
would have opened `frame-src` to the artifact scheme, or in a native webview of its own. The
prototype chose **the native webview**, so **the window's CSP is unchanged: `frame-src 'none'`**.

**An artifact's webview is labelled so that no capability names it** (`artifact-…`, which matches
neither `main` nor `window-[0-9]*`), so it holds an empty grant: every app command is refused to
it, the vault-values commands among them, and so is every core and plugin permission. On macOS the
prototype's page tried each way in it could find: its own and its parent's `__TAURI_INTERNALS__`,
the `ipc:` and `ipc.localhost` endpoints, raw posts to the platform's message handler, and the
event plugin. None of them reached the stand-in for `vault_reveal`. Granted the window's commands
in the same run, the page reached it three times.

The rule this record set still holds, and gains a case: **a window or webview with any label but
purlis's own is refused every command.** The real rendering (#5952) is to carry it as a test
that no capability file matches the artifact label, beside `no_window_but_charters_own_may_invoke_any_listed_command`.
