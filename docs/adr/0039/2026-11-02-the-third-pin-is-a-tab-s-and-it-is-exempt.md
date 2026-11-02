# Amended (2026-11-02): the third pin is a tab's, and a pinned tab is exempt from the collapse (#4752)

Records [Pinning a tab now that a chat isn't a tab](https://github.com/purlis/purlis/issues/4700),
[Prototype: the strip and panes in a narrow window](https://github.com/purlis/purlis/issues/4702),
[dragging, splitting and tiling panes](https://github.com/purlis/purlis/issues/4583) (moves added
to D6), [Several windows](https://github.com/purlis/purlis/issues/4701) rule 3 and
[the pane's chat actions](https://github.com/purlis/purlis/issues/4585) rule 5, ruled by the person
on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575) (spec
[#4750](https://github.com/purlis/purlis/issues/4750)). This records the decision ahead of the
behaviour, which ships with that milestone's build.

## Pins

**Pinning is at three levels: project, workspace and tab.** The chat pin retires, because a chat
has no tab of its own. A tab's pin keeps **the tab, not its contents**: its place first on its
strip, and the tab itself across relaunch, upgrade and having all its panes cleared. A press still
replaces the focused pane inside a pinned tab. **Close other tabs** skips pinned tabs, and **Close
tab** still closes a pinned tab without asking. The pin moves from `Chat::pinned` to `tabs[].pinned`
in the plane's `reopen.json` ([ADR 0040's amendment of this date](../0040/2026-11-02-the-third-pin-is-a-tab-s.md)).
This record's "a pinned chat outlives the thing it pins, or it does not" is answered: a pinned tab
outlives what it shows.

## The collapse

**This record's last open question is settled: a pinned tab is exempt from the collapse.** Pinned
tabs are always drawn, and go icon-only only when the pins alone would take more than half the
strip. The other tabs share the row up to 230px each, down to a **120px floor** (icon, about eight
letters and `⊟+N`), before going behind "N more ▾", sorted by last activity, with the front tab
always drawn. The 2026-09-22 amendment's "a pin draws a tab first, so it is the last thing to go,
and no exemption of its own" is replaced.

## A tab can leave its strip

The 2026-09-26 amendment's "a tab stays on its own strip. Nothing is dragged between strips, or
out of the window into a split window of its own" no longer holds. A tab dragged down off the strip
splits in beside a pane, with all its panes; a tab dragged off the window, or moved with **Move to
new window**, makes a tab window of the same project. The strip's order is written in the reopen
record's `tabs` list, in strip order, not in `chats[]`.

## The smart-close chip

The 2026-09-28 amendment, a tab being smart-closed drawn first as a chip, is withdrawn. A chat has
no tab, so a chat wrapping up shows a breathing mark on its pane head and its sidebar row
([ADR 0064's amendment of this date](../0064/2026-11-02-the-record-command-ends-the-chat.md)).
