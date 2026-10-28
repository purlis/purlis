# Amendment, 2026-10-28 (#3671, #3673, #3674): the meter family, a table's column choices, facts and totals

The operator's ruling of 2026-10-28 on spec #2897: **the meter is a family of the closed list**
(and disclosure, #3675, built apart). The Skill studio's design map (#3623) needs a value drawn as
a bar, and nothing in the window drew one.

- **The meter family** (`app/src/ui/meter/`, on React Aria's `Meter` and `ProgressBar`): `Meter`,
  a labelled value on a known range; `MeterGroup`, meters that compare on one scale and one label
  column; `ProgressBar`, done of N or going with no end known. A meter's variant says what it is
  in a comparison: `subject`, `baseline` and `previous`, each told apart by its label
  and its value as well as its colour (WCAG 1.4.1). A progress bar's value is said politely as
  it changes, at most once a second, beside the bar: a progress bar's own children are
  presentational to a reader. A raw `<meter>` or `<progress>`, and either role by hand, is
  `purlis/meter-family`, an error (Get project's clone progress is still off, until it moves).
- **Facts and totals join the card family** rather than the list of families: `Facts` (a `dl`
  of label and value, stacked where narrow) and `Totals` of `Tile`s (sharing the row, wrapping,
  live where the view asks, said at most once a second). They are drawn on a page as often as in
  a card, so their parts are reached by their own attributes alone.
- **A table's columns take a view's choices** (`cut`, `clamp`, `size`, `width`, `minWidth`,
  `maxWidth`), on both members of the table family: a column of short facts keeps one line where
  a long column beside it takes the room, cut with an ellipsis and whole in the cell's tooltip.
  React Aria's own widths are read only by a column-resizing table, so the family's are its own,
  on the column's name, where its cells read them back. A table of data `stacks` where a view
  asks, as a table of controls does. A column that chooses nothing draws as it did.
