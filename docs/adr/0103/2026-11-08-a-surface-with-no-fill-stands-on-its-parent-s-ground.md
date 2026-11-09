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
  The default the fill takes the place of is the no fill the surface draws, which stands for each
  ground the surface's family declares it is drawn on: a tree's base, raised and sunken grounds, a
  table's base ground. That is the family's word for where it is drawn, not every place the
  components put it: a table drawn on a raised ground is still taken to stand on the base one.
- **The rules for a surface's fill read those grounds as the default.** A part read on grounds of
  its own is read on the fill where one of them is among its grounds. So a field's error is read
  on a table's fill (the base ground is one of its grounds), and its help is drawn nowhere there
  (it is drawn on raised and overlay grounds only). A family's default look that breaks on the fill
  keeps the fill only where it breaks on every one of those grounds too. Where it reads on one of
  them, the fill set breaks it there, and the surface draws no fill again.

A theme that fills both a panel and a tree in it still measures what the tree holds on the panel's
fill too, though the tree's fill covers it there. That reading is stricter than what is drawn, but
it only reads one more ground: the table of where each family stands stays one table, found from
the components alone.

What this costs: nothing for a theme that fills no such surface, and one reading per look of each
such surface that a theme fills, as for any other surface. Measured with `held()`, best of three,
in a test build in one session, before and after: the shipped themes took 1.7–1.9 ms (Purlis) and
3.1–3.3 ms (Classic) both ways. A tinted theme filling seven surfaces took 0.181–0.194 s both ways.
The same theme with a tree and a table filled in a see-through, tinted ground took 0.185 s before,
when nothing was read on them, and 0.255 s after: the two surfaces read in every tint, about what
each of the seven costs.
