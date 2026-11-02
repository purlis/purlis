# PROTOTYPE: dragging, splitting and tiling panes (#4583, map #4575)

Throwaway. It lives on branch `prototype/pane-drag`, cut from `prototype/one-tab-strip` (#4582) so the
two look alike. It is never merged to main.

## The question

How does moving panes look and feel? That covers drop zones on a pane's edge and centre and on the
strip, dragging from the sidebar, "Tile all", and a keyboard route for every drag (WCAG 2.2 AA,
2.5.7 Dragging Movements). It also asks which moves D6's list is missing, and whether a sidebar drag
may put another workspace's chat into a pane.

## Open it

```bash
git fetch origin prototype/pane-drag && git show FETCH_HEAD:app/src/prototype-pane-drag/PROTOTYPE-artifact.html > "${TMPDIR:-/tmp}/pane-drag.html" && open "${TMPDIR:-/tmp}/pane-drag.html"
```

The dashed bar at the bottom belongs to the prototype, not the design. `[` and `]` move between
scenes. Everything works: drag a pane by its name, drag a sidebar row, drag a tab off the strip,
resize a separator, Tile all, Zoom, Undo. The data is stand-in data, kept in memory. The bar's
last line shows what a screen reader would hear and the tab's tree, with each split's share in
tenths of a percent.

Variants:

- **A · Zones + placement mode.** Where the pointer sits in a pane picks the place: the outer quarter
  by an edge splits on that side, and the middle swaps. The result is drawn on the pane before the
  drop, with a sentence saying what it will do. Keys and single clicks use placement mode: press the
  grip (or Enter on it, or ⌘⇧M), and every pane shows five targets, with a New tab target on the
  strip.
- **B · Targets only.** A pointer drag shows the same five targets on the pane under the pointer,
  and only a target takes the drop. The same targets serve as the keyboard route.
- **C · Zones + a list of places.** The pointer works as in A. Keys and single clicks open a list:
  for each pane, Left, Right, Above, Below and Swap, then A new tab.

Toggle **Other workspace: allow / refuse** to see both answers for another workspace's chat.

## What is real

The icon on every row, tab and pane head is the real `ChatPersonaState`. `Button`, `IconButton`,
`Badge`, `Count`, `SegmentedControl`, `EmptyState` and `Tooltip` come from `app/src/ui`. The tree,
the shares and the even grid follow #4587 (binary, first child's share in tenths). The terminal,
the chips and the sidebar row's frame are stand-ins. It was drawn in Chromium only.

## Rebuild and capture

From `app/`, with load under 12:

```bash
PD_OUT=$TMPDIR/pd-dist npx vite build --config src/prototype-pane-drag/vite.standalone.config.ts
cp $TMPDIR/pd-dist/standalone.html src/prototype-pane-drag/PROTOTYPE-artifact.html
node src/prototype-pane-drag/capture.mjs
```

## Verdict

Accepted by the person on 2026-11-02, the recommendation on all four: variant A (zones plus placement mode); another workspace's chat may land in a pane by drag only; every move found here is added to D6; Tile all tiles the tab's own panes, with Undo. The resolution is on #4583.
