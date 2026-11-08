# Amendment, 2026-11-08 (#3268): every family is measured where the components draw it

The amendment of 2026-10-26 measured only the families with a consent variant on a surface's fill
a theme sets: a button on a panel, a popover, a dialog, a card and a Notice, and a menu's row in a
popover. A field, a list's row, a badge or a meter drawn on a panel the theme fills was measured
only on its own grounds, so a theme could fill a panel in a colour their words vanish on and pass.
Four rules replace the first rule of that amendment. Its other two stand.

- **Where each family stands is found from the components.** `STANDS_ON` in the engine names, for
  each family whose root takes a fill, every family whose root the window draws inside it. A family
  stands on every such root around it, out to the first element that covers what is under it.
  `app/src/theme/grounds.test.ts` reads every component, follows each one to where it is placed and
  each name to the file it is imported from, and holds the table to what it finds, exactly. A
  family drawn on a new surface fails there until the table names it. `HOLDS_NO_CONSENT` is gone:
  it existed only to account for the surfaces that the consent families were not measured on.
- **The table, not a family's grounds, says a family stands on a surface.** A family is measured on
  a surface's fill set wherever the table names it, and its grounds only spare a reading the
  family's own check already makes: a fill they already name adds nothing. A surface drawn with no
  fill of its own in every look (a table, a tree) is in the table too. Nothing on it is measured on
  a fill a theme sets there, as before.
- **A part read on grounds of its own stands on a surface's fill where it is drawn there.** A
  field's help is drawn on raised and overlay grounds, and its error on base, raised and overlay
  ones. On a surface's fill, such a part is read on that fill where the surface's default fill is
  one of its grounds, and it is drawn nowhere on that surface where it is not. So a field's help
  is read on a dialog's fill and never on a pane's, and its error is read on both.
- **The surface falls back only for what its fill breaks.** A family's default look that breaks on
  the surface's default fill too is the family default's, which the floors never hold against a
  theme. Drawing the surface's default would mend nothing, so the surface keeps its fill. Classic's
  light tooltip, whose edge reads under 3:1 on its own fill, is the case that showed it. A look a
  theme's own rules draw is held as before.

What this costs: a family's reading on a surface's fill is the same for every look of that surface
that sets the same fill, so the check reads it once for all of them (a panel's side, content and
bottom) and reads it again only after a fall changes the rules of the family it was taken for. The
shipped themes are measured in about the same time as before. The heaviest fixture, which fills
seven surfaces under a tint, takes 0.32 s in a test build where it took 0.33 s, while it reads every
family on them.
