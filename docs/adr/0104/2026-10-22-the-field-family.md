# Amendment, 2026-10-22 (#2905): the field family, and a form's key

The field family moves (§2, §4). Every box a person types in is `TextField` or `TextArea`
(`app/src/ui/field/Field.tsx`), on React Aria's `TextField`, `Input`, `TextArea`, `Group`, `Label`
and `Text`, and a form they are sent from is the family's `Form`.

- **A field is its label, its control, its help and its error, as one.** The family is three in
  the parts manifest (ADR 0103's amendment of the same day): `field` (the root that stacks them,
  its `label`, `help` and `error`), `text-input` (the box for one line, its `icon`) and
  `textarea`. A row of the settings set keeps its own label column and draws its box from the
  family; so does a box in a table's cell, named by `aria-label`.
- **Sizes**: a text input is `sm` 24, `md` 28 or `lg` 32, as a button is; `lg` is only for a
  list's main filter field. A row of controls takes one size, so the find bar's and the browser
  address bar's boxes are `sm` beside their small buttons.
- **It validates inline, on blur** (N26): `validate` says what is wrong, said under the box when
  it is left with words the person changed and followed from there until it is right; a refusal
  from elsewhere (`error`) is said at once. Either marks the box `aria-invalid` and describes it
  by the words, an alert as every refusal in the window is. _(Superseded by the amendment of
  [2026-10-30, one signal per refusal](2026-10-30-one-signal-per-refusal.md): the line is a
  polite live region, never an alert, and a refused send is its form's one alert summary.)_ A field that can be refused keeps its
  error's line, empty and unheard while it says nothing, so nothing moves when it does.
- **A secret's box is never given its words**: `type="password"` with a `value` is a type
  error, and the box holds what is typed outside React's state and the page's markup.
- **⌘⏎ sends a form's primary action** (N26; Ctrl+Enter off macOS), its first submit button,
  from anywhere in it, a textarea included. **A chord sends only a plain or a suggested answer**
  (`secondary`, `primary`); a grant, what cannot be undone and any look added later are only
  ever a press of their own button (ADR 0097 §6, D-1719-3). Start a chat's ⌘⏎ never approves a
  command and never starts a chat without the sandbox. A key from what a portal draws outside
  the form is not the form's. A form whose fields are not right says why at each and takes the
  focus to the first.
- **A filter keeps its focus and its caret across a redraw** (R168): `keepsFocus` hands the
  keyboard and the caret on to the box drawn in its place.
- **Lint** (§5): `purlis/field-family` is an error. A raw `<textarea>`, or an `<input>` a person
  types in, is refused, and so are the old `ui-input`, `ui-input-*` and `ui-list-field` classes.
  The inputs of families that have not moved are their own rules' warnings:
  `purlis/select-family` (a `<select>`, an `<input role="combobox">`) and `purlis/choice-family`
  (a checkbox, a radio, a slider, a colour well).
