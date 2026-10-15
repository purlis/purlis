# Amendment, 2026-10-12: the chat strip is the editor's tab bar

The operator's design-system decisions of 2026-10-12 (spec #1832, D6; built in #1836) **move
only the chat strip**. It is no longer the third of three full-width rows. It sits at the top of
the centre column, directly above the chat it shows, as an editor's tabs do. His reason, from the
spec: with the strip across the whole window, _"the selected chat feels detached from where it is
shown"_.

```
┌─ projects (title bar) ────────────────────────────────────────┐
├─ workspaces ──────────────────────────────────────────────────┤
│ left side  │ [3 steward] [7 release] [12 forge] +  │ right side │
│            │ the panes of the selected chat        │            │
├────────────┴───────────────────────────────────────┴────────────┤
│ the Panel                                                      │
```

- **The axis stands.** A project holds workspaces and a workspace holds chats. The strips still
  nest, and the chat strip still shows the focused workspace's chats in that workspace's colour.
  What changed is where the innermost strip is drawn, not what it lists.
- **Only the chat strip moves.** The project strip stays in the title bar (ADR 0054), and the
  workspace strip stays the full-width row under it. That row is what the sidebars, the editor
  and the Panel all belong to, so moving it into a column would make it a column's.
- **One strip, not one per split.** Splits live inside a tab (`LayoutPanes`), so the centre has
  one tab bar. The move needs no region and no layout-file change: the strip is not a region
  (ADR 0038). It is the centre's content, above the panes and outside the pane menu's trigger.
- **The strip is drawn in all three of the centre's states**: the panes, "No chats in this
  workspace" and "No workspaces yet". It holds the `+`, and a state without it would have no
  `+`.
- **Tab order follows the new layout**, by the rule that it is document order (`ui-primitives.md`):
  the title bar, the workspace strip, the left side, then the chat strip, the focused pane's
  controls and its terminal, then the right side, the Panel and the status line.
- **F6 into the editor lands on the pane, not on the strip**, as VS Code's F6 lands on the
  editor and not its tabs. The strip carries `data-region-chrome` (`RegionFrame.REGION_CHROME`),
  and F6 passes over it. Tab still stops on it.
- **Narrow windows.** The strip now has the centre's width, not the window's, so it collapses into
  its show-more button sooner, and dragging a sidebar's edge fits it again. `fitting` always
  draws the selected tab. At 1024 px with both sidebars open, the selected tab, the show-more
  button and the `+` are all inside the centre column. `tab-strip-centre.e2e.ts` holds that, in
  ranges.
- **Below that, a tab gives up its task chip before its title** (#1867). At 900 px with both
  sidebars open the tabs are drawn at their floor, and a tab with its chip no longer fits its
  cell. Where not even the count of what is still working fits beside the title's floor and the
  `×`, the whole chip is set aside (`wholeWays.fitCounts`), and it comes back with the room. The
  sidebars are left as the person opened them: a sidebar the window closed by itself would be a
  layout nobody chose. `tab-chip.e2e.ts` holds it at 900 px, in ranges.
- **The strip uses the 21 px chrome height** (`--chrome-height`, D5) as its floor.

What the 2026-09-21 decision above says about the chat strip's contents, its `×` and its words is
unchanged.

**The `+` is right after the last tab, in the workspace strip and the chat strip** (D6, built in
#1839), as Chrome draws it. It used to sit at the far end of the row, outside the strip, because
the tabs are capped in width and the free room fell between them and it.

- **Inside the strip, as the project strip's always was**, as its last child (`.strip-doing`).
  `useRoom` takes its width off the room first, so `fitting` never collapses it: the
  never-collapsed rule of purlis#130/#131 holds without keeping it outside. The strips collapse
  and never scroll, so there is no scroller to carry it away.
- **"N more" stays at the right end of the row**, outside the strip. It appears only once the
  tabs fill the strip, and then it sits beside the `+`.
- **Not a tab.** No `role="tab"`, not owned by the tablist (purlis#1204), not a roving item: the
  arrows pass over it and Tab stops on it after the tabs. `strip-plus.e2e.ts` holds the place, in
  ranges, with few tabs and with more than fit.
