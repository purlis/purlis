# Amendment, 2026-10-22 (#2909): a row draws its focus ring inside itself

The operator's ruling of 2026-10-22. **A row inside a menu, a list or a tree draws its 2 px
focus ring inset** (`--focus-ring-inset`), inside its own edge. Every other control keeps its
2 px ring outside it, at a 2 px offset (`--focus-ring-offset`), as §4's families have it.

- **Why:** a list's rows sit edge to edge, and a scrolling list clips what is outside it. A
  ring outside a row would be cut by its list, or drawn over the row next to it.
- **What it changes:** a menu's row the keyboard is on is highlighted, as before, and ringed
  inside as well (`ui/menu/menu.css`). The ring shows only from the keyboard, and neither the
  ring nor the highlight changes the row's size.
