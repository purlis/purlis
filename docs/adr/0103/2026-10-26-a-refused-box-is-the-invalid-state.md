# Amendment, 2026-10-26 (#3110): a refused box is the `invalid` state

The select and combobox families (the amendment of 2026-10-24) gain a refused state, for the first
form that needs one: a required choice in Settings, or a combobox whose typed value is not allowed.

- **`invalid` is a state of the manifest**, keyed on `[data-invalid]`, in **the next release's
  manifest version** (`since: NEXT`). It is the seventh state, written after `open`, so a refused
  box's colours win over its hover's.
- **Only a box a person gives a value to is in it**: `select` and `combobox` name it (and the
  text input and the textarea, below). The button,
  the dialog and every other family do not, so `button.root.invalid.fill` is no part and is said
  and left out, as any state a wrapper never marks is.
- **A colour, never a size**, as every state: inside `invalid` a theme sets fill, words, border
  colour and shadow. The family default is the danger colour (`danger.base`) on the box's own
  edge, inside the box, as the text input's refused value is drawn (`app/src/ui/field/field.css`);
  `app/src/ui/select/select.css` draws it, and a test holds the engine's default to that rule.
- **The key shape is the operator's ruling** (2026-10-26, relayed by the dispatcher on #3322):
  `<family>.<part>.invalid.<property>`, the state named `invalid`.
- **The edge is held to 3:1 on the box's fill, whatever the fill**: it is what says at a glance a
  value is refused, so a theme's `invalid` look whose edge does not clear 3:1 draws the family
  default there, even on a fill that is none of the box's grounds (where an edge at rest is not
  held). The token floors hold the default: `danger.base` on `surface.sunken` at 3:1.
- **No theme erases the refusal.** A box that can be refused keeps its edge: `border-width` 0 on
  its root is refused, as it is on an answer that grants. In every set of states that holds
  `invalid`, an edge that is transparent, or drawn alike (closer than `ALLOW_APART` in OKLab) to
  the edge the same states draw without `invalid`, falls back to the refused default, written
  after the theme's rules. So a rest or hover edge that reaches the refused box, or a refused
  edge set to `transparent`, never leaves a refused box looking valid.
- **The words that say why are not a part**: they are the box's description, drawn by its row or
  its form and named by `aria-describedby`, so they are heard with it.

**The text input and the textarea are refused the same way** (#3323, #3333, the same day): both
name `invalid` (since NEXT), so `text-input.root.invalid.*` and `textarea.root.invalid.*` are keys;
their default is what `app/src/ui/field/field.css` draws, held by a test; and every floor above
holds for them. `border-width` 0 on their root was a value a theme could give before (a theme
that wanted no edge); it is now refused there too, and the sentence that says so names the way
that keeps the refusal visible: a transparent `border-colour` at rest, the edge's width kept for
the refused state to draw on. The first release is internal, and no theme the window ships sets
it.
