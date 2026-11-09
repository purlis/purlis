# Amendment, 2026-11-09: E6 is realised by ADR 0122, approved per page

**Amends [ADR 0081](../0081-charter-is-where-agents-work-and-writing-code-by-hand-opens-in-your-editor.md)**, by
[ADR 0122](../0122-an-artifact-is-a-workspaces-page-rendered-isolated-and-its-api-is-the-read-only-tool-catalogue.md), accepted the same day (#5948, #5949).

E6 foresaw "a capability-gated webview (sandboxed iframe, strict CSP, message API), approved
separately for rich UIs". **ADR 0122's artifact is that webview, with two differences.**

- **It is a native webview, not a sandboxed iframe.** The isolation prototype found that a
  sandboxed iframe on WebKit froze the window while its page looped, and could navigate to another
  artifact. The webview keeps the rest of E6: a strict CSP on every response, no network, and one
  channel to purlis.
- **It is approved per page, not per extension.** Each person approves the page's whole template
  folder on their own machine, and approves it again whenever the folder changes. An extension that
  ships an artifact gets no approval of its own for it.

An artifact is a view tab of its own, never inside the light editor and never the file tab's
preview, as this record's row on editor extensions already requires.
