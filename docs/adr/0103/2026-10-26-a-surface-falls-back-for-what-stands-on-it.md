# Amendment, 2026-10-26 (#3099, from #3083): a surface falls back for what stands on it

§7 says the surfaces a family is drawn on are its manifest entry's grounds, and that a part
which breaks a floor draws its family default. A theme's parts can also change a surface's own
fill: a panel's, a popover's or a dialog's. Three rules follow from that.

- **What stands on a surface is measured on the fill a theme sets on it.** Each family with a
  consent variant is measured on that fill: the button on a panel, in a popover and in a dialog,
  and a menu's row in a popover. Its words and edges are held as written, and its consent floors
  in every workspace tint. Where a family stands is a table in the engine (`STANDS_ON`), because
  one ground token can belong to more than one surface. A family whose root takes a fill and
  under which no consent family stands is listed with its reason (`HOLDS_NO_CONSENT`). A test
  holds every filled surface under a consent family to one list or the other.
- **The surface falls back, not what stands on it.** A family drawn as its default cannot be put
  right by drawing its default again. So when it breaks a floor on a surface's fill, the surface
  draws its own default fill, and the finding names the family that broke there.
- **A fill already among a family's grounds adds nothing**, so a theme that turns a panel to
  another surface the family already names is measured as before.
