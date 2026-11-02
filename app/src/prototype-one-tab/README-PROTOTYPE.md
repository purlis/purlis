# PROTOTYPE: one tab, the strip and its icons (#4582, map #4575)

Throwaway. It lives on branch `prototype/one-tab-strip`, which is never merged to main.

## The question

When most chats have no tab, what do the strip and its tabs look like, and how does it feel to
pick a chat in the sidebar? The cases: a tab's icon reusing the sidebar's persona-and-state icon,
what a split tab's icon and label show, and the empty pane. Also drawn here, from #4588: the empty
pane's New chat, what replaces today's three empty windows, the sidebar row's "new" mark, and a
handoff's card on the workspace chip with the dot it leaves.

## Open it

```bash
git fetch origin prototype/one-tab-strip && git show FETCH_HEAD:app/src/prototype-one-tab/PROTOTYPE-artifact.html > "${TMPDIR:-/tmp}/one-tab.html" && open "${TMPDIR:-/tmp}/one-tab.html"
```

The dashed bar at the bottom belongs to the prototype, not the design. `[` and `]` move between
scenes, and Alt-← and Alt-→ switch the strip treatment. The buttons work: press a row, Open in new
tab (on row hover), split, close a pane or a tab, New chat, and the scene's event. Stand-in data,
kept in memory.

Strip treatments:

- **A · Name tabs**: a tab shows its focused pane's icon and name. A split adds `⊟+N`.
- **B · No strip for one tab**: like A, but with one tab the strip is hidden and the pane head
  carries the identity.
- **C · Layout tabs**: a tab is drawn as its layout, one icon per pane, with the focused one
  outlined. Names show only when a tab holds one pane.

## What is real

The icon on every row, tab, pane head and card is the real `ChatPersonaState` (`ChatRows.tsx`).
`Button`, `IconButton`, `Count`, `Badge`, `Card`, `SegmentedControl`, `EmptyState` and `Tooltip`
come from `app/src/ui`. The terminal, the title bar's chips, and the sidebar row's frame and tree
are stand-ins.

## Rebuild

From `app/`:

```bash
OT_OUT=$TMPDIR/ot-dist npx vite build --config src/prototype-one-tab/vite.standalone.config.ts
cp $TMPDIR/ot-dist/standalone.html src/prototype-one-tab/PROTOTYPE-artifact.html
```

## The sidebar pick: what was measured

The bar shows press → drawn frame (two animation frames after the commit) for each sidebar pick.
That number covers **only the prototype's React work, with no terminal**. It is not the real
path. The real path (#4579) disposes the xterm, builds a new one and replays the snapshot plus
5,000 lines of scrollback. Measuring that needs the built app with 20+ live chats. This chat
couldn't run it: machine load was 14 to 22 and the sandbox can't launch the app. That
measurement is its own ticket.

## Verdict

Accepted by the person on 2026-11-02, all four as recommended: A (Name tabs), the one empty pane as drawn, the faint on-screen bar, and the real pick measured in #4692. The resolution is on #4582.
