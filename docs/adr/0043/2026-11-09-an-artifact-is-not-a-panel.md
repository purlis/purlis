# Amendment, 2026-11-09: an artifact is not a panel

**Amends [ADR 0043](../0043-a-panel-is-a-declared-contribution-and-a-chart-is-not-a-row.md)**, by
[ADR 0120](../0120-an-artifact-is-a-workspaces-page-rendered-isolated-and-its-api-is-the-read-only-tool-catalogue.md), accepted the same day (#5948, #5949).

The panel vocabulary is unchanged. A panel is still data drawn by purlis from its own blocks, and a
declared row still carries no verb of its own. **An artifact is not a panel**: it is a page of
HTML in a view tab of its own, rendered in an isolated webview, and nothing it draws is ever part
of a panel or of the window's document.

This record's rule that nothing in the window acts on another's say-so holds for artifacts too.
A page never performs an action. It may only *offer* one, a `Does` id from `actions.ts`'s
catalogue with a subject, which purlis draws in its own bar for the person to press. A restored
artifact tab, like any view tab ([amended 2026-09-23](2026-09-23-a-tab-holds-a-session-or-a-view.md)),
runs nothing and asks nothing until it is pressed.
