### Changed

- **The Changes view is a tree of the tree family** (#2911). Its rows are walked as every tree
  will be: one Tab stop, ↑ and ↓, → into a row's first child and ← back to its parent, Home and
  End, typing to find a row, and Shift+F10 or the menu key for a repo's menu. A row's pill under
  the pointer stands in from the view's edge, the keyboard's ring is drawn inside the row, and a
  screen reader hears each row's whole line, its kind first ("changes: 3 changed"). Themes can
  now style a tree's rows part by part.
