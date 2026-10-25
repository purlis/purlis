# Amendment, 2026-10-24 (#2906): the select and combobox families join the manifest

Every select and every box with suggestions is now the select or the combobox family (ADR 0104's
amendment of the same day), so both join the closed manifest at **the next release's manifest
version** (`since: NEXT`), each reached by its wrapper's attributes alone. Neither has variants.

- **`select`**: `root`, the button that shows what is picked (every property but `font-weight`,
  as the text input's box), `placeholder` (`text`: what it shows while nothing is picked) and
  `chevron` (`text`). Its states are what its button marks: `hover`, `focused` (from the
  keyboard, `[data-focus-visible]`), `disabled` and `open` (its list is up).
- **`combobox`**: `root`, the box typed in (as the select's), and `chevron` (`text`). Its states
  are `hover`, `focused` (whenever it holds the caret, `[data-focus-within]`, as a text box's),
  `disabled` and `open`.
- **The state at rest draws as before**: both boxes are the text input's (`box_look`), and a
  theme that names no part of either draws them as `app/src/ui/select/select.css` does, which a
  test holds to the engine's defaults. The placeholder and the chevron are drawn in the quiet tier
  (R64) and fall back to it.
- **A chevron is a mark**, held at 3:1 as an icon is, not at the 4.5:1 of words.
- **Their lists are no part of their own**: a select's list and a combobox's suggestions stand on
  the popover family's `menu` surface and each option is the menu family's row, so
  `menu.root.hover.fill` reaches an option as it reaches a menu's row.
