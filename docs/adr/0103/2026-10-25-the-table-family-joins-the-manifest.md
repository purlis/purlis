# Amendment, 2026-10-25 (#2911): the table family joins the manifest

Every table of the window is now the table family (ADR 0104, `docs/ui-primitives.md`, "The table
family"), so it joins the closed manifest at **the next release's manifest version**, reached by its wrapper's attributes
alone.

- **`table`**: a table's row is its root, the part a state reaches (the pointer on a row that
  opens or is chosen, the keyboard on it, chosen, a row that cannot be used), and a theme sets its
  fill and its words. Its other parts are the card that holds the table (`frame`: its fill, edge,
  corners and lift), a column's name (`head`: its words), a cell's room (`cell`: its padding), the
  rule between rows and under the columns' names (`rule`: its fill), and the bar over a table that
  chooses (`bar`: its gap).
- **The rule between rows is a part, and the family draws none** (#2804, the no-hairline ruling,
  over the prototype's row rules, R34/R51): the rows' room, the pointer's ground and the chosen
  wash carry the bands. Purlis Classic fills it with `border.subtle` in its theme, which keeps its
  hairlines. It has no variants: a table of data and a
  table of controls are drawn alike.
- **A row has no height and no corner to set**: it is as tall as what its cells hold, and a band
  across its table.
- **A theme that names no part draws the family default**, exactly as `ui/table/table.css` draws
  it, which a test holds to the engine's defaults. That default is new: every table now stands in
  a card, and its words are data in the primary tier.
- **A ground goes**: the vault's rows drew `layer.selected` under the pointer, which buttons stood
  on; the family draws `surface.hover` there, so `layer.selected` leaves the button's and the
  field families' grounds, as `grounds.test.ts` finds them.
