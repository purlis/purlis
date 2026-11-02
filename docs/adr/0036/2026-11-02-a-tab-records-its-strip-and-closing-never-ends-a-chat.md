# Amended (2026-11-02): a tab records its strip, and closing never ends a chat (#4752)

Records rulings the person made on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575), gathered
in spec [#4750](https://github.com/purlis/purlis/issues/4750): the
[charting decisions](https://github.com/purlis/purlis/issues/4576) D1, D2 and D3,
[A window with no tab per chat](https://github.com/purlis/purlis/issues/4588) rules 1 to 3,
[Storing a tab's layout](https://github.com/purlis/purlis/issues/4587) rules 1 and 2,
[dragging, splitting and tiling panes](https://github.com/purlis/purlis/issues/4583) rule 2,
[the pane's chat actions](https://github.com/purlis/purlis/issues/4585) and
[one tab, the strip and its icons](https://github.com/purlis/purlis/issues/4582). This records the
decision ahead of the behaviour, which ships with that milestone's build.

**A chat isn't a tab.** Every chat is listed in the sidebar, and a tab is a layout of panes the
person made. The axis stands: one workspace is in front, and its strip is drawn. What the strip
holds, and what a tab records, change.

## A tab records its strip

**Every tab records the strip it is on**: a workspace, or none for the strip of chats outside
every workspace. This reverses "it is deliberately **not** a field recorded on the tab". That
refusal was of a copy of the plane's answer, a second answer nothing invalidates. A tab's strip
is not one: it is a fact the person made, which nothing else knows, and an empty pane has no chat
to derive it from. View tabs already record theirs (`reopen::View::workspace`).

- **A chat is still filed by the plane's answer**, never by the tab it sits in. A chat dragged
  into another workspace's tab doesn't move that tab, and it stays filed where it works.
- **When a workspace is removed**, its tabs close and their chats stay in the sidebar.

## The four rules

- **Focusing a workspace brings its front tab forward.** The record stores each strip's front
  tab, which replaces the window's in-memory "last in front". Each workspace, and the plane
  root's strip, always keeps at least one tab, so a focused pane always exists. On the first
  launch after the upgrade, the tab that was in front at quit comes back as it was, even a plain
  one-chat tab, and a workspace that kept no tab gets one showing its first chat in the old strip
  order, pinned first; an empty pane only for a workspace with no chats.
- **Nothing is ended and nothing is torn down.** Unchanged, and now true of closing as well (below).
- **A press on a chat from anywhere brings its workspace with it.** The chat replaces what that
  workspace's focused pane shows, or is jumped to if it is already on screen; the pane the person
  was looking at is untouched. The invariant holds for presses: the tab in front is always on the
  strip that is drawn. **A drag is the one exception**: it may put another workspace's chat into a
  pane on purpose. The chat stays filed in its own workspace, and the pane head names that
  workspace. So "the app drawing a chat the strip says is not there" is ruled out for presses
  only. `Switch to` leaves the tab's menu, because a press on the tab does it.
- **Chats outside every workspace get a strip of their own.** Unchanged.

## Closing never ends a chat

The rule that "the words say that a close ends the chat", and the row `End chat 3 steward` with
its note, are withdrawn, because closing no longer ends anything. **Close pane** clears the pane,
**Close tab** closes the layout, and the chats keep running in the sidebar. Ending a chat is
**End…**, an action of the chat, in its pane head's ⋯, its sidebar row and the palette. The danger
cue on close (#3295) goes.

## The 2026-10-12 amendment

- **The strip shows tabs, not the workspace's chats.** A tab shows its focused pane's combined
  persona-and-state icon and its chat's name, and a split adds `⊟+N`. An empty pane's tab says
  "New chat", and a view's tab shows the view's icon and name.
- **One empty pane replaces the centre's three states.** It reads "Nothing in this pane", with how
  many chats run here and **New chat**. A plane with no workspace says "No workspaces yet" with
  **Create a workspace**. The strip is always drawn, even with one tab.
- **A tab gives up no task chip**, because it has none: a chat's tasks are under its sidebar row
  and on ⌘⇧J, the focused chat's tasks.
