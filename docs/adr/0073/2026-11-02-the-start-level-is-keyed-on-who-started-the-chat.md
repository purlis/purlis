# Amended (2026-11-02): the start level is keyed on who started the chat, not on a tab (#4752)

Records the [charting decisions](https://github.com/purlis/purlis/issues/4576) D4, ruled by the
person on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575), and the
start level as [Rewriting the records](https://github.com/purlis/purlis/issues/4704) decided it
and spec [#4750](https://github.com/purlis/purlis/issues/4750) records it. This records the
decision ahead of the behaviour, which ships with that milestone's build.

V24a said "a chat in a tab starts in its terminal", and a chat with no terminal shown (dispatched,
headless, remote) starts at the highest level its harness offers. Once a handoff takes no pane,
that wording would quietly start every handoff at level 3. So V24a is keyed on who started the
chat:

- **A chat the person starts, and a handoff, start in their terminal**: level 2, or level 1
  without an adapter, even when the harness offers level 3. A handoff's terminal is there when
  the person presses its row.
- **A dispatched task starts at the highest level its harness offers on this machine**, as a
  remote chat does.

That is the levels as they are today, so nothing changes silently. The 2026-09-30 ruling's V24a
reads the same way, and this record's "a shell tab" reads "a shell".
