# Amended (2026-11-02): the centre strip is a strip of tabs (#4752)

Records the [charting decisions](https://github.com/purlis/purlis/issues/4576) D3,
[one tab, the strip and its icons](https://github.com/purlis/purlis/issues/4582) rule 1 and
[Pinning a tab now that a chat isn't a tab](https://github.com/purlis/purlis/issues/4700), ruled by
the person on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575) (spec
[#4750](https://github.com/purlis/purlis/issues/4750)). This records the decision ahead of the
behaviour, which ships with that milestone's build.

"The chat strip does not change" no longer holds: there is no chat strip. **The centre strip is a
strip of tabs**, layouts of panes the person made, with pinned **tabs** first, as
[ADR 0039's amendment of this date](../0039/2026-11-02-the-third-pin-is-a-tab-s-and-it-is-exempt.md)
says. The chats are the work in front of you, and they are listed in the sidebar, every one of
them. The strip is always drawn, even with one tab.
