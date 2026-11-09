# Amendment, 2026-11-09: an artifact may travel in a plane, approved by each person per version

**Amends [ADR 0041](../0041-a-plugin-is-a-subprocess-or-charter-has-no-plugins.md)**, by
[ADR 0120](../0120-an-artifact-is-a-workspaces-page-rendered-isolated-and-its-api-is-the-read-only-tool-catalogue.md) §3 and §4, accepted the same day (#5948, #5949).

**"A plugin never travels in a plane" no longer holds for artifacts.** An artifact's template
folder is committed to its workspace and shared. Its script runs only once each person has approved
the whole folder, every path and every byte, on their own machine. A change to the folder tears
the running page down, renders it again with scripts off, and asks again. A teammate's approval
never counts as yours. A page with no executable script opens without asking.

**Decision 2's DOM row holds.** Code still never reaches the tree purlis draws its consent prompts
into. An artifact runs in a native webview of its own, never in the window's document. That webview
holds an empty grant, has an origin of its own, and is served only the bytes pinned for it. The
isolation prototype showed each of those on macOS with ADR 0120 §3's five tests, and
`tools/artifact-isolation-probe` runs them on Linux and Windows too. Approval is given only in
purlis's bar beside the page, which the page cannot draw.

Nothing else changes for plugins: an extension is still a subprocess, approved per machine.
