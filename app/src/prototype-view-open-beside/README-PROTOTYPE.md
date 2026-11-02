# PROTOTYPE: an extension view's Open beside and its offer notice (#4705, map #4575)

Throwaway. Branch `prototype/view-open-beside`, cut from `prototype/narrow-window` (#4702), so the
window, strip and pane heads look the same. Never merged.

## The question

How do an extension view's **Open beside** and its **offer notice** look and behave? #4703 ruled
that a press opens a view in a tab of its own, with Open beside on the press's menu and the palette,
and that an extension can't open a view without a press: it can only offer, with one quiet notice.

## Open it

From any clone of purlis:

```bash
git fetch origin prototype/view-open-beside && git show FETCH_HEAD:app/src/prototype-view-open-beside/ViewOpenBeside.prototype.html > "${TMPDIR:-/tmp}/view-open-beside.html" && open "${TMPDIR:-/tmp}/view-open-beside.html"
```

Scenes (`[` `]` or the dashed bar): 1 a view's heading button (Statistics on Steward's tab), 2 a
panel row, 3 the palette, 4 too narrow to open beside (four panes at 1024), 5 the offer notice, 6
its extension has gone. The bar switches the four answers being asked:

- **Q1, where Open beside sits:** A, a beside icon after the button; B, a ▾ part with a two-row menu; C, nothing drawn (⌥-press and right-click only).
- **Q2, beside under 40 × 8:** A, refused before the press with its reason, Open in a tab offered; B, opens in a tab instead; C, splits and folds the other side (#4702).
- **Q3, an offer after Dismiss:** A, quiet for that view and subject until relaunch, plus Stop offers from statistics; B, comes back every time; C, never again.
- **Q4, the extension removed:** A, offer withdrawn, open view says "not here any more" with Close pane; B, offer stays greyed; C, open views close themselves with Undo.

URL params: `?s=5&q1=A&q2=A&q3=A&q4=A&t=dark&w=1280`.

## What is real, and what isn't

Real: the ⌥ convention (#4699 addendum), beside by the pane's shape and the 300 × 140 floor (#4583),
the bell slot and band rules (#4584), the pane head (#4585), the fold (#4702), the "not here any
more" sentence (`Views.tsx` `view-gone`), one note per view and subject (#4703). The layout is
arithmetic, so jsdom draws it the same. Stand-in data; nothing calls the core. No screenshots: the
sandbox can't launch Chromium. Every scene and variant was rendered and its presses checked in jsdom.
