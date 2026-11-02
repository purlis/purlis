# Amended (2026-11-02): a press shows a chat in the focused pane, and the This tab scope goes (#4752)

Records the [charting decisions](https://github.com/purlis/purlis/issues/4576) D1, D4 and D5,
[A window with no tab per chat](https://github.com/purlis/purlis/issues/4588) rules 1 to 4 and
[one tab, the strip and its icons](https://github.com/purlis/purlis/issues/4582) rule 3, ruled by
the person on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575) (spec
[#4750](https://github.com/purlis/purlis/issues/4750)). This records the decision ahead of the
behaviour, which ships with that milestone's build. The four regions stand.

- **Pressing a chat's row shows it in the focused pane** (the 2026-10-07 amendment's "brings that
  chat's tab forward"). The chat it replaces keeps running. A chat already on screen is jumped
  to, never shown twice. A press never replaces a view: when the focused pane holds one, the chat
  goes to the most recently used pane that shows a chat or nothing, and that pane's tab comes
  forward. A chat of another workspace brings that workspace forward and takes its focused pane.
- **No row press makes a tab.** A tab is made only by **Open in new tab**, a view, or a drag onto
  the strip. "A chat started as a task has no tab until its row is pressed" becomes: a task is a
  chat like any, and its press lands the same way. The breadcrumb path in its pane walks up its
  chain in that same pane.
- **The rows say what is on screen.** The focused pane's chat has the highlighted row, and a chat
  shown in another pane of this tab has a faint bar at its row's edge, so a press that will jump
  can be seen first. A handoff that arrived has a **new** mark until it is looked at, and a row
  above a chat that needs you shows ✋. A row shows ⋯ on hover and focus, with the chat's one list
  of actions; right-click and Shift+F10 stay.
- **The 2026-10-10 amendment's This tab scope goes.** Workspace and All stay, and a stored This
  tab falls back to Workspace.
- **This record's suggestion of "the chat's own tab and the pane's corner"** reads the chat's pane
  head.
