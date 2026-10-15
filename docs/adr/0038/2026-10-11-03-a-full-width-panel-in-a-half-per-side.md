# Amendment, 2026-10-11: a full-width Panel, in a half per side

The operator's decisions of 2026-10-11 (spec #1736, P-1 to P-15; first built in #1737) **amend
B-7 of the 2026-10-10 amendment above**: _"the bottom region is gone … a bottom panel may come
back one day for output only; it would be a new decision"_. This is that decision, and the Panel
is not for output.

- **A Panel runs under both sidebars and the editor, the window's whole width between the
  activity bars, in a left half and a right half** (P-1). Each half belongs to its **Side**: the
  side's activity bar, its sidebar and its half of the Panel. Each activity bar runs the window's
  whole height: its upper icons switch the sidebar, and its **Lower icons**, below a divider,
  switch its half of the Panel. Same column, same rules.
- **Defaults** (P-2): the left sidebar keeps Chats, Explorer (Workspaces and Files) and Search;
  the bottom-left half holds Repos, out of Explorer, and Changes; the right sidebar keeps the
  Inbox, Personas and Vaults; the bottom-right half holds Memory, Todos and Sessions. Picking a
  repo or branch in Repos chooses it as before, and Explorer's Files follows (P-10).
- **A view moves only between its own side's sidebar and its own half** (P-3), with _Move to
  panel_ and _Move to sidebar_ on its icon's menu; moves across sides stay later (B-1). Every view
  may move but the Inbox, pinned to the right sidebar (P-4).
- **Collapsing** (P-5): a press of a half's open Lower icon puts that half away; with one half
  away the other takes the whole width; with both away the Panel is not drawn and the editor has
  the height. **⌘J** (Ctrl+J elsewhere) hides and shows the whole Panel; a Lower icon opens its
  half; each view's own key opens it wherever it lives; all of it is in the palette (P-6).
- **Sizes** (P-12): the Panel is 30% of the window's height by default, between 120px and 70%,
  and a double-click on its handle puts it back; the halves share the width 50/50, and a
  double-click on the handle between them resets that. Height, split and placement are kept per
  project in `layout.json` v2 (P-5, B-11), inside the existing budgets.
- **Narrow windows** (P-7): below about 900px, the Panel shows one half at a time, the one used
  last, and the other side's Lower icons still switch it.
- **Keyboard** (P-13): F6 cycles left sidebar, editor, right sidebar, bottom-left, bottom-right,
  skipping what is put away; each half is one Tab stop inside its tree (#1701), and a named region.
- **Badges** (P-9) follow B-8 while a half is away; Repos counts ahead/behind only when something
  is behind. **Each half's header is one line** (P-11): the view's title, its own actions and a
  collapse chevron, and no tabs inside a half.
- **No output or terminal view now** (P-8); shell tabs stay tabs (ADR 0062). An Output view is a
  later decision, and would default to the bottom-right half.
- **First launch after the update** (P-14) opens the Panel once, at its default height, with
  Repos left and Memory right; from then on it is as the person leaves it, and _Use the default
  layout_ restores the same. An older purlis reading a newer file draws without the halves, says
  so once, and keeps working.
- **Words** (P-15): **Panel**, **Panel half**, **Side** and **Lower icons** are defined in
  `CONTEXT.md`; docs say "panel", not "bottom panel".

**Built so far** (#1737): the Panel and its left half, Repos in it as a view of its own, the left
bar's Lower icons, collapsing, ⌘J and its palette row, the height with its handle and reset, kept
per project, and the first-launch open. The halves are regions of the arrangement
(`panel-left`, `panel-right`; `app/src/regions.ts`), so the rest — the right half and its split
(#1738), Changes and the badges (#1739), moves (#1740), narrow windows and the keyboard order
(#1741) — are data and rules on the same model.

**Built** (#1739): Changes is in the left half, after Repos, and the left sidebar is Chats,
Explorer and Search. A Lower icon carries its view's badge the way a sidebar's icon does, on the
bar outside the half, so it is there while the half is away: Changes counts uncommitted files,
Todos its open items once it is in the right half (#1738), and Repos draws commits behind and
ahead only while a clone is behind. A view's key and its palette row open it where the
arrangement holds it, a sidebar or a half, bringing back a half that was away.

**And** (#1738): the right half, Memory, Todos and Sessions in it on the right bar's Lower
icons, the right sidebar opening on the Inbox; the handle between the halves, kept per project
as the left half's `share` of the width; each half's one line (P-11), which is the sidebar's
section heading moved up: the open view's title, the actions its heading had, and a chevron
that puts the half away. A file whose right sidebar was left on Memory, Todos or Sessions opens
the sidebar on the Inbox without a word, since nothing of the person's is lost.

**And** (#1740): a view moves between its side's sidebar and its half with "Move to panel" and
"Move to sidebar" on its icon's context menu, the window's one menu (`Menus.tsx`), and never to
the other side (P-3); the Inbox's icon has no such row (P-4). An extension's panel moves like
any view of the right side. The move opens the view where it went. Each region's placement
keeps the views moved into it, as `moved` in `layout.json` per project, so a region nothing
moved into is written as it always was; _Use the default layout_ puts every view back. A
sidebar or a half every view has left is not drawn, and its icons go with it.

**And** (#1741): below 900 px of window width the Panel draws one half at a time, the one used
last, and the other side's Lower icons draw theirs in its place. The width is counted at the
default window text size, as a browser counts it under zoom, since every size in the window is in
`rem`: the window is never under 1024 px (ADR 0054), so P-7 is the narrowest window with its
text at 16 px or more. What is drawn is a projection of the arrangement
(`app/src/narrowPanel.ts`) and nothing is stored for it: the half not drawn is not put away, so a
window widened again, or its text made smaller, draws both as they were. F6 and Shift+F6 cycle left
sidebar, editor, right sidebar, bottom-left and bottom-right, skipping what is away or not drawn
(`app/src/regionCycle.ts`); inside a chat's terminal F6 stays the chat's, and F6 into the editor
lands on the focused pane's controls, so the cycle never stops where it is not the window's. Each
half is a region named for its side ("Panel, left half") and one Tab stop, its view's; the
chevron on its line is the pointer's, since its open Lower icon and ⌘J do the same by keyboard.

**And** (#1751, #1752, #1686): every region F6 stops in is a named region, the sidebars by their
catalogue names while they are out and the editor as "Editor"; F6 and Shift+F6 are palette rows
with their keys said. Each view's move is a palette row too, and the region rows name the views
each region holds now. A moved view keeps where it was scrolled, in memory, for the frame's life;
it is not written to `layout.json`. A half whose only views are extension panels that are not
contributed now is not drawn, and comes back with them, and the palette counts the Panel shown
only while a half is drawn now. **A moved view keeps what it held** (D-1752-4): each view's
content is drawn once, into an element of its own the frame keeps, and a move puts that element in
the other region, so nothing in the view is mounted again.

**D-1686-2, decided in implementation (the operator's "don't ask" rule, reversible): the right
bar comes straight after the right sidebar in Tab order**, then the Panel: left bar, left
sidebar, editor, right sidebar, right bar (its upper icons, then its Lower icons), the Panel's
halves. The bars stay outside the resizable groups, at the window's edges, which is the
charter-app#141 care: nothing is added to a group's panel list. The frame walks Tab itself only
at the two places where the DOM's order differs from this one (`RegionFrame.tabOrderOf`), and
leaves every other press, and a chat's terminal, to the engine. The left bar is first in the
DOM already, so it needs nothing.

**D-1686-1**: the Personas view draws the project's personas with no workspace focused too,
not only at the project root; the views that are a workspace's still say none is focused.
