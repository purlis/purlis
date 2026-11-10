# Amendment, 2026-11-10 (#6149): where a family stands may name one look of a surface

The amendment of 2026-11-08 has `STANDS_ON` name a surface, and the engine measures each family
named there on every look of that surface whose fill a theme sets. The window draws a list's badges
on its plain rows (`DevicesView.tsx`, `SkillLibraryGrade.tsx`) and none on its card rows, and the
one list fill a shipped theme sets is Purlis Classic's, on the card (`list.root.card.fill`).
Measured on that card, the `waiting` badge's see-through wash let a workspace tint through, and
Classic Dark's parts would have been built in 44 tints and not one look (#4410). So a badge on a
list's row was not measured anywhere.

- **A `STANDS_ON` key is a surface alone (`list`), for every look of it, or one look (`list.row`),
  for that look alone.** A family is measured on a surface's fill set in a look only where a key
  names that look or the whole surface. A family is named once for a look: one already on the
  whole surface is not named again under one of its looks.
- **`app/src/theme/grounds.test.ts` reads which look a root draws.** That is its `data-variant`
  as written, or the look the component placing it is handed: written out, passed on from a
  parameter, or the component's own default when it is handed none (`variant = "row"`). A use
  that spreads props and names no look may hand it any look, so the root counts as the whole
  surface's. A pair is said by the table when the table names it as found or names its family on
  the whole surface.

What this costs: nothing for the shipped themes, which set no fill on a list's plain row. A theme
that fills one has the badges on it measured there.

What it leaves: a surface's fill is read at rest only. A list's chosen, hovered or focused row
fill (`list.selected`) is not a ground for what stands on the row (#6156).
