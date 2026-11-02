# Amended (2026-11-02): reveal is a press, and tabs hold panes (#4752)

Records the [charting decisions](https://github.com/purlis/purlis/issues/4576) D1,
[A window with no tab per chat](https://github.com/purlis/purlis/issues/4588) rules 1 to 3, 7 and 8
and [Several windows](https://github.com/purlis/purlis/issues/4701) rule 2, ruled by the person on
2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575), and
`reveal` as [Rewriting the records](https://github.com/purlis/purlis/issues/4704) decided it (spec
[#4750](https://github.com/purlis/purlis/issues/4750)). This records the decision ahead of the
behaviour, which ships with that milestone's build.

- **Tabs hold panes**, where this record said the panes split to hold tabs. A tab is a tree of
  split panes the person made, and each pane shows a chat, a view or nothing. purlis hosts a
  chat's harness and a shell's shell, where it said "a shell tab"; nothing splits inside a
  terminal, as before.
- **`reveal` counts as the person's press.** It is not the person's press in the window, but it
  is not a chat started without them either. If the chat is on screen in any of the project's
  windows, that window comes forward and focuses its pane. Otherwise the chat takes the focused
  pane, or the most recently used pane that shows a chat or nothing when the focused one holds a
  view, and its workspace comes forward. A workspace and a Review are revealed as before.
- **Focus stays off the terminal.** `reveal` brings the window forward with focus on the chat's
  **pane head**, where it said "on the tab in its strip", so a key pressed while switching windows
  still reaches no harness. After a `place`, the chat comes forward the same way.
- **"A level-3 chat's view tab"** is the chat's pane, which draws its transcript
  ([ADR 0080's amendment of this date](../0080/2026-11-02-a-level-3-chat-draws-its-transcript-in-its-pane.md)).
- **A terminal editor opened from a file** is a one-job program: it opens as a split beside the
  focused pane, in the same tab and with focus, and the split goes when the program exits.
