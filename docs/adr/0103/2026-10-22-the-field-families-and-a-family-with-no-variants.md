# Amendment, 2026-10-22 (#2905): the field families, and a family with no variants

**The manifest is at version 3**: it gains three families, each drawn by the field family's
wrapper (ADR 0104's amendment of the same day).

- `field`: `root` (`gap`, the room between what it stacks), `label` (`text`, `font-size`,
  `font-weight`), `help` (`text`, `font-size`) and `error` (`text`, `font-size`);
- `text-input`: `root` (every property but `font-weight`) and `icon` (`text`);
- `textarea`: `root` (every property but `gap`, `font-weight` and `height`: a textarea is as
  tall as its lines).

**A family may have no variants.** Its keys name the part alone (`text-input.root.fill`), its
rules carry no `data-variant`, and its one look is measured and falls back as a variant's is,
named by its part (`text-input.root`).

**A part's words may have a default of their own.** A field's help is drawn in the quiet tier
and its error in the danger colour, not in the root's colour, and a part that breaks a floor
falls back to that default. A root that draws no colour (the field's) is given none.

**A family names its own states**: a key may name only a state its wrapper marks. A text input
and a textarea are under the pointer, focused and unavailable; a field's root is only
unavailable. A key naming another state is no part, said and left out. `purlis theme parts`
lists each family's `states`.

**A field is measured on every ground a control stands on**: the field families list the
button's grounds, since a filter stands on a list's layers and a sunken ground as a button does.

**`focused` is keyed per family.** A button is focused from the keyboard
(`[data-focus-visible]`); a field whenever it holds the caret (`[data-focus-within]`), as a text
box's `:focus-visible` has it. `purlis theme parts` names each family's attribute (`focused`).

**A size a theme sets is the text input's md size**, as it is the button's: it never reaches an
sm or lg box. The size clamps for corners are written only for the families whose corners a
theme sets.
