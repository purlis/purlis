# Amendment, 2026-10-25 (#2911): a table of data is a grid, and a table of controls is a table

The table family has two members, chosen by what a cell holds, not by how the table looks.

- **A table of data is React Aria's `Table`** (the WAI-ARIA grid): one Tab stop and the APG
  grid's keys, choosing rows by a check and Space, sorting by a column's name, a row's menu
  under a ⋯ in its own column and on a right-click or the menu key. Its cells hold words and at
  most a button, a link or a switch, which the arrows reach.
- **A table of controls is a plain HTML table** (`ControlTable`): a grid takes ← and → from a
  field's caret and ↑ ↓ from a select (React Aria's grid cell hears them in the capture phase),
  so a table whose cells are fields, selects or numbers, or whose rows are dragged by the
  pointer, keeps its controls' own keys, each its own Tab stop, and stacks into a form where its
  frame is narrow. It is drawn by the same parts as the grid.
- **A row's menu has one place**: a column of its own at the row's end, aligned on every row, so
  the settings record lists (#2919) take one action column as the aligned form's rows do.
