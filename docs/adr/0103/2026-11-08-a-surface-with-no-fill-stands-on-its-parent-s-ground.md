# Amendment, 2026-11-08 (#5863): a surface with no fill stands on its parent's ground

The amendment of 2026-11-08 (#3268) left a surface whose default look draws no fill unmeasured: a
tree's row, a table's row, a plain list's row. A theme could fill one of them in a colour that the
words of what stands in it vanish on, and pass. This replaces that amendment's sentence about such
a surface ("Nothing on it is measured on a fill a theme sets there, as before"). Its other rules
stand.

- **A surface with no fill of its own stands on its parent's ground until a theme fills it.**
  Unfilled, it leaves what is under it showing. `app/src/theme/grounds.test.ts` already walks
  past such a root to the first element that covers what is under it, so a family drawn in a
  tree's row on a panel is in `STANDS_ON` under the tree and under the panel. A theme that fills
  the panel holds that family to its fill.
- **Once a theme fills it, what stands in it is measured on that fill**, as on any surface's fill.
  The ground the fill takes the place of is each ground the surface's family stands on: a tree's
  are the base, raised and sunken grounds, and a table's is the base ground. Every family the table
  names on the surface is read on the fill once for each of those grounds. A part read on
  grounds of its own follows the rule for several grounds against each of them. So a field's error
  is read on a table's fill (the base ground is one of its grounds), and its help is not drawn
  there (it is drawn on raised and overlay grounds only). A look that falls on one of those grounds
  draws its default, no fill, on all of them, and is not read again.

A theme that fills both a panel and a tree in it still measures what the tree holds on the panel's
fill too, though the tree's fill covers it there. That reading is stricter than what is drawn, but
it only reads one more ground: the table of where each family stands stays one table, found from
the components alone.

What this costs: nothing for a theme that fills no such surface. A theme that does fill one is read
once more for each of the surface's grounds. Measured with `held()`, best of three, in a test build
in one session, before and after: the shipped themes took 1.7–1.9 ms (Purlis) and 3.1–3.3 ms
(Classic) both ways. A tinted theme filling seven surfaces took 0.182–0.194 s both ways, and
0.186 s both ways when it also filled a tree and a table that kept their fill.
