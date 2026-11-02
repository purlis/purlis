# Amended (2026-11-02): nothing runs that the reopen record does not list (#4752)

Records the [charting decisions](https://github.com/purlis/purlis/issues/4576) D2, D3 and D4 and
[A window with no tab per chat](https://github.com/purlis/purlis/issues/4588) rule 7, ruled by the
person on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575), and
the browser view's close as spec [#4750](https://github.com/purlis/purlis/issues/4750) settles it.
This records the decision ahead of the behaviour, which ships with that milestone's build.

A chat may now run with no tab and no pane: every chat is listed in the sidebar, and a dispatched
task or a handoff takes no pane. The host's rules stand; the words that tied them to a tab change.

- **The 2026-10-16 (#2006) amendment's "so nothing runs with no tab"** reads "so nothing runs that
  the reopen record does not list". What a restarted host finds and no record puts back is
  closed, as before. A chat it reattaches keeps everything of it as for a start, its row and its
  place in a tab's layout among them.
- **The 2026-10-14 (#2012) amendment's browser** lives as long as a pane shows it: when the last
  pane showing it closes, it ends. Closing never ends a chat or a shell, but a browser view is a
  view, and that ruling is not about views.
- **The 2026-10-26 (#2025) amendment's chat browser** comes back on its row, and on its chat's
  pane when that chat is shown, where it said "its chat's tab".
- **"A shell tab is not a harness and has no resume"** reads "a shell". A shell has a row and
  follows a chat's rules ([ADR 0062's amendment of this date](../0062/2026-11-02-a-shell-follows-a-chat-s-rules-and-is-not-a-tab.md)).
