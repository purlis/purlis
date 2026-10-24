# Amendment, 2026-10-24 (#2911): a row of a collection carries its own menu and tooltip

A list's, a tree's and a table's rows are drawn by their collection (React Aria's `ListBox`,
`GridList`, `Tree` and `Table`), which renders each row itself from what its item says. A
component put around a row is not drawn where the row is, so the window's `ContextMenu` and
`Tooltip`, which wrap the element they are about, cannot reach one.

- **A row's menu is the row's `menu`**: the row is handed the context menu's handlers and draws
  the menu inside itself (`useContextMenu` in `ui/menu/ContextMenu.tsx`). Shift+F10 and the menu
  key are heard by the collection, which asks the row that has the keyboard for its menu
  (`askForAMenu`); the menu opens on its first row, as every context menu from the keys does.
- **A row's tooltip is the row's `tip`** (`ui/list/RowTip.tsx`): the shared tooltip, anchored to
  the whole row, shown under the pointer after the window's delay and at once while the keyboard
  is on the row, the row's description while it is shown, and taken down by Esc (WCAG 1.4.13).
- **Words inside a row may still take a `Tooltip`**: a slot is drawn by the row, so a tooltip
  around it works as it does anywhere else.
