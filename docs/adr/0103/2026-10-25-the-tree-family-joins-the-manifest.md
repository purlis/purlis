# Amendment, 2026-10-25 (#2911): the tree family joins the manifest

A tree of the window is drawn by the tree family (ADR 0104, `docs/ui-primitives.md`, "The tree
family"), so it joins the closed manifest at **the next release's version** (`since: NEXT`), reached by its wrapper's attributes
alone.

- **`tree`**: a tree's row, the part a state reaches (the pointer on it, the keyboard on it,
  chosen, a row that cannot be used). It is the list's row (`list_look`), in the tree's own
  scope, so a theme's `tree.*` keys reach a tree and never a list. Its other parts are the tree
  that holds the rows (`body`), the chevron that opens and closes a row's rows (`twist`, a mark),
  a row's slots (`lead`, `label`, `description`, `trail`), and a group's name (`header`).
- **A row's indentation is the family's**: `tree.root` takes every property of a row but its
  padding, which is the steps a level stands in by; a theme that set it would line a child up
  with its parent.
- **A row's size is the family's**: a theme's height reaches the default size (`md`) only; a side
  view's rows (`sm`) keep theirs.
- **The state at rest draws as before**: a theme that names no part of the family draws it
  exactly as `ui/tree/tree.css` does, which a test holds to the engine's defaults.
