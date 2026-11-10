# Amendment, 2026-11-09: PE-39 is ADR 0122's isolation

**Amends [ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md)**,
by [ADR 0122](../0122-an-artifact-is-a-workspaces-page-rendered-isolated-and-its-api-is-the-read-only-tool-catalogue.md) §3, accepted the same day (#5948, #5949).

§6 deferred webview panels to "PE-39's isolation", which no record defined. **ADR 0122 §3
defines it**: a native webview whose label no capability names, so it holds an empty grant. It has
an origin of its own, is served from memory only the bytes pinned for it, under a CSP with no
network, no frames, no forms and no `<base>`, and may navigate only within its own folder. Five
tests hold that on each platform. The prototype chose it over a sandboxed iframe on macOS on
2026-11-09.

**§6 is otherwise unchanged.** A StudioPanel still answers with panel blocks. ADR 0122's webview
is for an artifact, a page someone pinned to a workspace. A panel that wants one is a new
decision, not a reading of this one.
