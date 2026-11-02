# Amended (2026-11-02): a project may have tab windows (#4752)

Records [Several windows: chats, panes and tabs across windows](https://github.com/purlis/purlis/issues/4701)
and [Dropping a pane into another window's zones](https://github.com/purlis/purlis/issues/4706),
ruled by the person on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575) (spec
[#4750](https://github.com/purlis/purlis/issues/4750)). This records the decision ahead of the
behaviour, which ships with that milestone's build.

**One project is still held by one window**, its **project window**. What changes is that one
project's chats can be on screen in more than one window:

- **A tab can be torn off into a tab window** of the same project. A tab window has the strip and
  the panes, and no sidebar and no project strip. It belongs to its project window: when the
  project moves to another window its tab windows stay its own, and closing the project closes
  them too. This is VS Code's floating-window model.
- **Closing a tab window ends nothing.** Its tabs go back to the project window, behind its front
  tab, and every chat keeps running. Its last tab leaving closes it, as a split window that holds
  nothing goes today.
- **A press goes where the chat is.** If the chat is on screen in any of the project's windows,
  that window comes forward and focuses the chat's pane. Otherwise it takes the focused pane, and
  when that pane holds a view, which is never replaced, the most recently used pane across the
  project's windows that shows a chat or nothing. The palette in a
  tab window acts on that window's focused pane.
- **Tabs and panes move between windows** by a tab's **Move to new window**, a tear-off drag off
  the window's edge, and **Move to window ▸ <window>** on a tab or a pane, which lands it as a new
  tab there. The edge and centre zones and placement mode work inside one window only (#4706): a
  webview can't carry an HTML drag into another one.

The 2026-09-26 amendment's cost, "a project moved to another window comes back as tabs without
the splits it had", is withdrawn. Each tab's tree, its shares and the window it was in are
recorded in the plane's `reopen.json`
([Storing a tab's layout](https://github.com/purlis/purlis/issues/4587)), so a moved or relaunched
project comes back with its splits and its tab windows. Where each tab window sits on screen is
machine state ([ADR 0034's amendment of this date](../0034/2026-11-02-a-tab-window-s-place-and-a-tab-s-pin.md)).
