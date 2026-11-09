# Amendment, 2026-11-09 (#2947): a long tree draws only the rows in sight

[The tree family](2026-10-25-the-tree-family.md) said "no virtualizer yet": a folder's entries
are capped by the core, and the virtualizer would come once a tree was measured to need one.

**A tree is now measured to need one.** Settings › Theme's Parts row lists the component parts
manifest as a tree (family, part, and each key a theme sets). Today the manifest holds 51
families, 258 parts and 707 properties a part takes. Counting its states and variants, its keys
run to thousands, and a filter that finds a common word (`fill`) opens most of them at once. The
spec asks that it stay usable at 1,000 entries and more.

**The tree family takes `drawnInPart`.** It is React Aria's own `Virtualizer` with its list
layout, wrapped once inside the family as ADR 0104 asks of every primitive:

- Each row is measured as it is drawn. The room between and around the rows is read from
  `tree.css`, so a tree drawn in part keeps the stylesheet's spacing.
- The tree is its own scroller, and its owner's `className` gives it its height.
- React Aria keeps the focused row drawn wherever it is. The arrows, Home and End walk the whole
  tree, and each row keeps its `aria-level`, `aria-posinset` and `aria-setsize`.
- A tree whose entries the core caps (a folder's) is still drawn whole.

The list family's long lists stand on `@tanstack/react-virtual` (`virtualRows.tsx`, #5501). That
wrapper places hand-drawn rows a field aims at. A React Aria collection has to own its rows to
walk them by the keyboard, and React Aria's `Virtualizer` is how a React Aria collection draws in
part. So each family uses the virtualizer its primitive is built for, still once per family.
