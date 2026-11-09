# Amendment, 2026-11-09: the left bar holds a workspace's pinned artifacts, in a group of their own

**Amends [ADR 0038](../0038-the-window-is-four-regions-and-what-has-no-region-is-named.md)**, by
[ADR 0120](../0120-an-artifact-is-a-workspaces-page-rendered-isolated-and-its-api-is-the-read-only-tool-catalogue.md) §2, accepted the same day (#5948, #5949).

The left activity bar ([amended 2026-10-10](2026-10-10-02-each-side-has-an-activity-bar-and-shows-one-view.md))
gains a second kind of item. Until now every icon on it switched the side to one of purlis's views.
**A pinned artifact's icon opens a tab**: the artifact's view tab, through `open_view`, never a
view of the side.

- The artifact icons are a group of their own, after a divider below purlis's icons. purlis's own
  icons never move, whatever is pinned.
- Which artifacts are in the group, and their order, are the workspace's, shared through its pins
  file. At most eight are drawn; any more go to the bar's show-more, never dropped.
- The icon comes from purlis's icon set, chosen at the pin; a page's own image is never drawn on
  the bar.
