# Amendment, 2026-10-24 (#2906): the select and combobox families, and no top layer

The select and combobox families move (§2, §4). Every one-of-many pick is `Select`
(`app/src/ui/select/Select.tsx`), on React Aria's `Select`, `Button`, `SelectValue`, `ListBox`,
`ListBoxItem` and `Autocomplete`; every box with suggestions is `ComboBox`
(`app/src/ui/select/ComboBox.tsx`), on React Aria's `ComboBox`, `Input`, `Button` and `ListBox`;
and the box over a list drawn in place, whose rows are its owner's (the command palette), is
`ListComboBox`, the field family's box with the combobox's ARIA. Radix's `Select` is gone from
the window.

- **A select is a button that opens a listbox**, as React Aria draws it, named by what it holds
  and then by its own name. Its list stands on the popover family's `menu` surface and its
  options are the menu family's rows (ADR 0103's amendment of the same day): one surface, one row
  and one highlight for every list a control opens.
- **A long list is filtered**: from ten options a select's list opens with the popover's filter
  row (R165) holding the keyboard, through React Aria's `Autocomplete`, and the arrows still move
  in the list.
- **Sizes**: `sm` 24, `md` 28, `lg` 32, as the text input's.
- **Lint** (§5): `purlis/select-family` is an error. A `<select>`, a `<datalist>`, a
  `role="combobox"` or an `aria-haspopup="listbox"` written outside the component layer is
  refused, and so are the old `ui-select` classes.
- **No top layer.** The window's top layer (`app/src/overlays/topLayer.ts`, the dialog family's
  amendment of 2026-10-22) held the portals of the families still on Radix inside a React Aria
  dialog; the select's was the last, so it is gone with them. Every overlay is React Aria's own,
  which knows the modal it is in.
