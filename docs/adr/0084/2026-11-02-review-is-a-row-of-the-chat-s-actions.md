# Amended (2026-11-02): Review is a row of the chat's actions, not a tab's button (#4752)

Records the [charting decisions](https://github.com/purlis/purlis/issues/4576) D9 and
[the pane's chat actions](https://github.com/purlis/purlis/issues/4585) rules 1 and 2, ruled by the
person on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575) (spec
[#4750](https://github.com/purlis/purlis/issues/4750)). This records the decision ahead of the
behaviour, which ships with that milestone's build.

A chat has no tab, and a chat's actions are one list keyed by the chat: the pane head's ⋯, the
sidebar row's ⋯ and right-click, and the palette show the same rows in the same words.

- **R1 and V34a: Review is a row of that list**, where they put "a Changes button on every chat
  tab" and named "the chat-tab button". The word is still **Review**. The pane head shows one
  state button and ⋯, so Review is in ⋯; it is greyed with its reason where it can't run now.
- **R2's Review view stays a view.** A press opens it in a tab of its own, or beside the chat's
  pane with Open beside ([ADR 0043's amendment of this date](../0043/2026-11-02-a-pane-holds-a-chat-a-view-or-nothing.md)).
