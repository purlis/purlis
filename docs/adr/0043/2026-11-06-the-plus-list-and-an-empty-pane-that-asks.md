# Amended (2026-11-06): `+` lists what opens in a new tab, and an empty pane asks what goes there (#5292)

Records the person's acceptance of [the moving panes spec](https://github.com/purlis/purlis/issues/5288)
on its prototype, kept on `prototype/moving-panes` (stories 73 to 89), and the call on #5292 from
#5297 that each row of the list means a new tab.

In the amendment of 2026-11-02, "an empty pane reads 'Nothing in this pane' and offers **New chat**"
changes:

- **An empty pane asks "What goes here?"** and offers **New chat** and **New shell**. While chats
  run in its workspace it says one can be dragged in from the list, and how many run. Alone in its
  tab and as one side of a split it asks the same. The no-workspace case is unchanged.
- **A tab whose only pane is empty is named "Empty tab"** until it holds something, so the strip
  says what is there. It was named "New chat" (spec #4750 story 44), after the empty pane's button,
  while the strip's `+` was the way to a new chat.

And the strip's `+` (#4758) gains a list:

- **A press still opens an empty tab.** Resting on `+` for about 160 ms, or Down on it, shows the
  list of what can be opened in a new tab: Empty tab, New chat and New shell, then views, each with
  its key. It closes about 260 ms after the pointer leaves both `+` and the list. Past about 8 rows
  it has a filter.
- **The list is drawn from the action catalogue.** A row is a catalogue row marked for it where the
  row is built, so a view joins the list with no menu of its own.
- **Every row means a new tab**, a new chat and a new shell included, even where the front tab has
  an empty pane. The sidebar's New chat, ⌘N and the empty pane's own buttons keep the opening rule
  (ADR 0113), which fills an empty pane first.
- **⌘T opens an empty tab and ⌘N starts a new chat** (Ctrl+T and Ctrl+N off a Mac, where a chat's
  terminal keeps them as a shell's own). ⌘⇧T stays New shell.
