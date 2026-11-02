# Amended (2026-11-02): open means not ended, and closing a pane closes no chat (#4752)

Records the [charting decisions](https://github.com/purlis/purlis/issues/4576) D2 and D4,
[Storing a tab's layout](https://github.com/purlis/purlis/issues/4587) rule 3 and
[the pane's chat actions](https://github.com/purlis/purlis/issues/4585) rule 5, ruled by the person
on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575) (spec
[#4750](https://github.com/purlis/purlis/issues/4750)). This records the decision ahead of the
behaviour, which ships with that milestone's build.

A chat is listed in the sidebar whether or not a pane shows it, and closing a pane or a tab only
clears it: the chat keeps running. So a chat's state stops being read from a tab.

- **open** means the chat has not ended, with or without a pane. The sidebar draws its current
  run's state, with the reason or hold, as before. A chat put away by a quit is still open.
- **`stopped | closed`** is caused by the person ending the chat: **End…**, or **Smart close**
  (End…'s third answer, ADR 0064), or **Start fresh** at the launch question. Closing a pane or a
  tab is never this cause.
- **`stopped | operator`**: the person stopped this run and **kept the chat open**, where it said
  "kept its tab".
- **closed with a record**: the chat ended after `purlis session record` saved its session
  record. **closed without a record**: it ended any other way. "Its tab closed" is no longer how
  either happens.
- **After a harness chat ends**, its pane, if it is on screen, keeps the last screen, read-only,
  under an "ended" band with **Resume** and **Clear pane**, and its row stays in the sidebar.
  Closed is still final: Resume starts a new chat, as before.

**A shell has no run**, where this record said "a shell tab": its program is shown alive or
ended, as before. When it exits, its row goes and its pane clears.
