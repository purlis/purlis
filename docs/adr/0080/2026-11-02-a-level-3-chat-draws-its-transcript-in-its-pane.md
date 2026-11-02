# Amended (2026-11-02): a level-3 chat draws its transcript in its pane (#4752)

Records the [charting decisions](https://github.com/purlis/purlis/issues/4576) D1, D3 and D4,
ruled by the person on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575), with the
start level and the sandbox badge as spec [#4750](https://github.com/purlis/purlis/issues/4750)
records them. This records the decision ahead of the behaviour, which ships with that milestone's
build.

- **V24a** is keyed on who started the chat, not on a tab
  ([ADR 0073's amendment of this date](../0073/2026-11-02-the-start-level-is-keyed-on-who-started-the-chat.md)):
  a chat the person starts, and a handoff, start in their terminal; a task starts at the highest
  level. The 2026-10-01 ruling's "new tab chats still start in their terminal" reads that way.
- **V28b: a level-3 chat shown in a pane draws its transcript in that pane**, not in a view tab of
  its own. *Continue in terminal* is still an explicit `switch`.
- **Windows refuses a level-3 chat for the same reason, said where it now is.** The opt-out has
  to be visible for as long as the chat lives (ADR 0067 §7), and the badge is now on every pane
  head that shows a chat and on its sidebar row
  ([ADR 0067's amendment of this date](../0067/2026-11-02-the-sandbox-badge-is-on-the-pane-head-and-the-row.md)).
  With no generated profile to wrap it there, purlis still refuses a level-3 chat on Windows and
  offers it at its next level, in its terminal, where this record said "in a tab".
- **"A shell tab"** in this record reads "a shell".
