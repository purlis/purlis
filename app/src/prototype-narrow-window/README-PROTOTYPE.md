# PROTOTYPE: the strip and panes in a narrow window (#4702, map #4575)

Throwaway. Branch `prototype/narrow-window`, cut from `prototype/pane-chat-actions` (#4585), so the
pane heads look the same. Never merged.

## The question

What do the tab strip and the panes do when room runs out? That covers a narrow window and a small
screen: the strip with few and many tabs, a tab's name and icon shrinking, and what a split does
below the pane minimum of about 40 columns × 8 rows (#4583). Does it stack, scroll or hide?

## Open it

From any clone of purlis:

```bash
git fetch origin prototype/narrow-window && git show FETCH_HEAD:app/src/prototype-narrow-window/NarrowWindow.prototype.html > "${TMPDIR:-/tmp}/narrow-window.html" && open "${TMPDIR:-/tmp}/narrow-window.html"
```

The window is drawn at a real pixel size. The dashed bar's **width** and **height** sliders resize
it (1024 is the app's `minWidth`). The bar also switches the four answers being asked:

- **Q1, the strip at its floor:** A, name tabs down to a 120px floor, then show-more; B, icon-only
  tabs before show-more; C, fixed 180px tabs.
- **Q2, pins:** never collapse, or last to go (what the build does today).
- **Q3, below 40 × 8:** A, fold the side without the focus to a rail or bar; B, scroll; C, hide behind
  a ⊟ chip; D, squeeze (today's behaviour).
- **Q4, what gives way first:** the panes fold and side regions stay, or the sidebar goes to a
  rail first.

Scenes (`[` `]` or the bar): 1 a wide window with three tabs, 2 sixteen tabs at 1024, 3 three panes
side by side at 1024 with the Explorer out, 4 Tile all of nine on a 1280 × 700 laptop, 5 a folded
pane that asks, 6 split until it refuses. URL params: `?s=3&strip=A&pins=exempt&fold=fold&give=panes&t=dark`.

## What is real, and what isn't

The rules are real: equal shares capped at 230px (ADR 0039, 2026-09-22 amendment), the front tab
always drawn, the `+` outside the collapse, the 300 × 140 minimum, the pane head's drop order from
#4585, the ask card from #4584, and the binary tree with shares in tenths of a percent from #4587.
The layout is computed with arithmetic, not measured, so the page behaves the same in jsdom. The data
is stand-in data. There's no real terminal, so WebKit, real reflow and xterm's own minimum are for the
build to check. No screenshots: the sandbox can't launch Chromium. Every scene and variant was checked
in jsdom.
