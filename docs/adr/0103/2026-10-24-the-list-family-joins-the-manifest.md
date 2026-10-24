# Amendment, 2026-10-24 (#2911): the list family joins the manifest

Every list of the window is now the list family (ADR 0104, `docs/ui-primitives.md`, "The list
family"), so it joins the closed manifest at **version 11**, reached by its wrapper's attributes
alone.

- **`list`**: a list's row, the part a state reaches (the pointer on it, the keyboard on it,
  chosen, a row that cannot be used). It has no variants: an option to choose and a row that acts
  are drawn alike. Its other parts are the list that holds the rows (`body`, its room and the gap
  between them), a row's slots (`lead`, `label`, `description`, `trail`), and a group's name
  (`header`).
- **A row's size is the family's**: a theme's height reaches the default size (`md`) only; a
  dense list's rows (`sm`) and a dialog's (`lg`) keep theirs, as the button's sizes do.
- **The state at rest draws as before**: a theme that names no part of the family draws it
  exactly as `ui/list/list.css` does, which a test holds to the engine's defaults.
