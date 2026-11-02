# Amended (2026-11-02): a pane holds a chat, a view or nothing, and a view opens beside only on a press (#4752)

Records [Storing a tab's layout](https://github.com/purlis/purlis/issues/4587),
[Can an extension's view ask for a pane?](https://github.com/purlis/purlis/issues/4703),
[Prototype: an extension view's Open beside and its offer notice](https://github.com/purlis/purlis/issues/4705),
[Prototype: the strip and panes in a narrow window](https://github.com/purlis/purlis/issues/4702),
the [charting decisions](https://github.com/purlis/purlis/issues/4576) D2 and
[A window with no tab per chat](https://github.com/purlis/purlis/issues/4588) rule 2, ruled by the
person on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575), and the
Open beside key decided on [Keyboard and focus](https://github.com/purlis/purlis/issues/4699) (spec
[#4750](https://github.com/purlis/purlis/issues/4750)). This records the decision ahead of the
behaviour, which ships with that milestone's build.

## A tab is a tree of panes

The 2026-09-23 amendment's "a tab is a layout of panes, and a pane holds a session or a view"
stands, with two changes:

- **A pane may also be empty.** An empty pane reads "Nothing in this pane" and offers **New chat**.
- **Every close ends nothing**, not only a view tab's: closing a pane clears it, and closing a tab
  closes the layout. A chat keeps running in the sidebar.

The tab is a binary tree: a split holds a direction, the first child's share and two children,
and a pane holds a chat, a view or nothing. View tabs are no longer kept "beside the chats" in the
reopen record: each tab's tree, views included, is an entry of its `tabs` list.

**Closing a browser view's pane ends that browser as today.** "Closing never ends" is ruled for
chats and shells, not for views.

## Where a view goes

**An extension's view never says where it goes.** The manifest gains no key.

- **A press opens it** in a tab of its own, or with **Open beside** in a split beside the pane it
  was pressed from: the view's own pane for a view button, the note's pane for an offer, and the
  focused pane for a panel row, the palette or the Notices centre. Open beside has no chord: it is
  ⌥-press (Alt off a Mac), or ⌥Enter on a palette row.
- **A view never replaces a pane's content**, and a press on a chat never replaces a view: the
  chat goes to the most recently used pane that shows a chat or nothing.
- **Without a press, an extension can only offer its view**, with one note that takes no pane, no
  tab and no focus: on the pane band of the chat it is about when that chat is on screen, and in
  the Notices centre. **Dismiss** keeps that view and subject quiet until relaunch. **Stop offers
  from <extension>** is a per-extension preference in Settings › Extensions, which is the
  person's setting and no new ADR 0041 capability. A removed extension's offers are withdrawn,
  and a view of it still open says it is not here any more.

This answers the 2026-09-23 amendment's open question, "whether a view may be split beside a chat
by anything but starting a chat beside it": by a press with Open beside, and by nothing an
extension declares.

## A narrow window folds, and stores nothing

A split that can't give both sides about 40 columns by 8 rows folds the side without the focus to
a rail (side by side) or a bar (stacked) when drawn, showing its icon, state, note count and ✋.
The stored tree and its shares don't change. Side regions never give way by themselves.
