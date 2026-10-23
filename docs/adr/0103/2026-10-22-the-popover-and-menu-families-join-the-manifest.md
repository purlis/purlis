# Amendment, 2026-10-22 (#2909): the popover and menu families join the manifest

Every menu, context menu and popover of the window is now the menu or the popover family (ADR
0104, `docs/ui-primitives.md`, "The menu and popover families"), so both join the closed manifest
at **version 6**, each reached by its wrapper's attributes alone.

- **`popover`**: the surface a popover and a menu stand on, the overlay surface of #1845. Its
  root carries the variant `popover` or `menu`, so a theme styles a menu's surface apart from a
  card's or one rule reaches both. Its other parts are the anatomy of R165 and what stands in a
  menu's list beside its rows: `filter`, `rule`, `list`, `header`, `footer`, `key`. The root takes
  colours, a border, corners and a lift, never a padding or a height: a surface is no control, and
  its size is what it holds.
- **`menu`**: a menu's row, the part a state reaches (the pointer on it, the keyboard on it, a
  submenu open from it, a row that cannot run). Its variants are `default` and `danger`, the
  consent variant of what cannot be undone (D-1719-3), held by the danger floors as the button's
  is. Its parts are `label`, `description`, `shortcut` and `indicator`.
- **The state at rest draws as before**: a theme that names no part of either family draws them
  exactly as `ui/popover/popover.css` and `ui/menu/menu.css` do, which a test holds to the
  engine's defaults.
