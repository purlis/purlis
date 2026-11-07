# Amended (2026-11-07): a tab's skill run and browsers go to its pane head; its fresh mark stays (#5524)

Follows [the 2026-11-06 amendment](2026-11-06-a-tab-s-name-has-room.md) by the rule of the spec
[What a tab shows](https://github.com/purlis/purlis/issues/5339): **a tab says which chat this is
and whether it needs you.** Three marks were still drawn beside a tab, and each is decided by
that rule.

- **The skill run chip goes to the chat's pane head** (SK-3), after its state: what the chat uses
  now is what the head is for. It says and opens what it did on the tab.
- **The browsers' count and live thumbnails go to the chat's pane head** (#2024), beside the
  skill run, for the same reason. Each browser is still a row under its chat in the Chats list,
  which is the keyboard's way to it, and the Browsers grid draws them all live. A chat in a tab
  behind the one in front shows its browsers on its row and in the grid, no longer on the strip.
- **The fresh mark stays on the tab.** The ruling on charter#369 put it there so that the
  person, who alone can start a chat fresh, is told where they look for whether a chat needs
  them. Moving it would change that ruling, which this record does not.

**The × stands on the pill it is drawn over.** A theme's fill for a hovered tab
(`tabs.tab.hover.fill`) is said to the tab's × and the fade over its name too
(`--tabs-closer-over`), so in Classic light the × no longer stands on a white box inside its
grey pill. The family's own pill and the × already shared `surface.overlay`.

**`tabs.count` says what it draws**: a count of a chat's tasks in one state in its tasks menu's
summary. A description is not part of the manifest's digest (ADR 0103, amended 2026-10-25), so
it changes in any pull request.
