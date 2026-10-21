# The window is built from component families on React Aria, each primitive wrapped once

**Accepted 2026-10-21**, from the operator's spec #2897 (M66, UI system) and its resolution
#2793 (which primitives, and which shared component set), accepted as recommended. Built by
#2904, with the button as the first family. It amends
[ADR 0037](0037-charter-takes-the-behaviour-and-keeps-the-look.md) (its amendment of
2026-10-21 says where) and sits beside
ADR 0103 (#2903), which says how a theme reaches a family's parts.

## Where purlis was

ADR 0037 took behaviour from Radix primitives and kept the look in hand-written CSS, with a rule
against any purlis layer between a screen and a primitive: "the primitive is the component;
purlis's look is CSS on it". A control took its look from a class (`.ui-button`,
`.ui-button-primary`, …) that each call site wrote.

That held the look of a classed control, and nothing more. The same kind of control was drawn
four ways across the window; buttons in one row had different heights; a class a call site forgot
left a control drawn by its container's rule; and a theme could reach a colour token but never
"this part of this control". The audit behind spec #2897 counted about 30 button looks.

## The decision

### 1. React Aria Components is the primitives library

Every component family sits on **React Aria Components** (`react-aria-components`, Adobe,
Apache-2.0), pinned to an exact version in `app/package.json`; its licence text is in the
window's third-party notices, generated from the lockfile (`tools/third-party-notices.mjs`). It
is adopted **one family per ticket**. Radix stays for the families that have not moved, and is
removed when the last one has.

React Aria was chosen for one library that covers every family the spec lists, the tree, the
table and the combobox included, with press handling that is the same for a pointer, a touch, a
key and a screen reader, a focus ring that shows only for the keyboard, and state attributes
(`data-hovered`, `data-pressed`, `data-selected`, `data-focus-visible`, `data-disabled`) that a
stylesheet and a theme's parts are keyed on.

### 2. One wrapper per primitive, in `app/src/ui/<family>`

Each primitive is wrapped **exactly once**, in purlis's component layer, `app/src/ui/<family>/`.
That wrapper is the family's **primitive wrapper** (CONTEXT.md): the one place the library is
imported, where the family's variants, sizes and parts are set, and the seam a later change of
library would cross. A screen imports the family, never the library.

This reverses ADR 0037's rule against a purlis layer in front of a primitive, for the reason
0037 gave the rule: drift. A rule that a call site must remember (the right class, the right
size, the lock on an Allow) is one a call site forgets; a family whose props will not take a
wrong combination is one the compiler remembers. The wrapper is thin and readable, its props
are React Aria's own words (`onPress`, `isDisabled`, `isSelected`) plus the family's (`variant`,
`size`, `icon`), and its stories show every variant and state.

### 3. A family is drawn by its parts, never by a call site's class

A family's root carries `data-scope` (the family), `data-part` and `data-variant`; its inner
elements carry their own `data-part`. The family's stylesheet, beside its wrapper, draws its
default from those attributes and React Aria's state attributes, and a theme's parts reach the
same attributes (ADR 0103). A call site gives a control a **place** (`className` for margin,
width, flex, order), never a look.

### 4. The closed family list, variants and sizes

The families are those of #2793: button, icon button, link button; field, text input, textarea,
select, combobox, checkbox, radio group, switch, segmented control, slider; tabs, toolbar; list,
tree, table; card, notice, badge/count; tooltip, menu, context menu, popover, dialog, alert
dialog; panel and panel section, status item, empty state; and from the shell, rail button,
activity item, title-bar workspace chip and separator. More join by ticket.

The button family's variants are primary, secondary, ghost, danger, soft-accent, warning-soft,
link and grants; its sizes sm (24 px), md (28 px) and lg (32 px, only beside a list's main filter
field) at the default text size and density. **A row of buttons takes one size** for every
button in it, so a mixed row cannot be built. The `grants` variant is an answer that grants (an
Allow, an Accept): it draws its own lock, keeps the accent edge, and never takes the primary look
(ADR 0097 §6).

### 5. Lint holds it

`app/eslint.config.js` refuses the primitives library outside `app/src/ui`, and a moved family's
old styling classes anywhere outside it (an error). A raw `<button>`, `<input>`, `<select>`,
`<textarea>`, a `role="dialog"` and a native `title=` are warnings until their family moves; each
family's ticket moves every call site and turns its own case into an error.

### 6. A family is done when it is proven

A family's ticket ships a story for each variant and state with a `play` test for its keyboard
path, axe green in Chromium and WebKit, screenshot baselines, its parts in the theme engine's
manifest, its docs, every call site moved with its lint at error, and the operator's before and
after look (`docs/component-stories.md`).

## What was rejected

- **Staying on Radix and adding the missing primitives from elsewhere.** Radix has no tree, table
  or combobox; two libraries is two press models, two focus models and two sets of state
  attributes for a theme to key on.
- **A copied-in component per family (shadcn/ui's way).** Allowed by ADR 0037's amendment of
  2026-09-22 and still allowed for a single piece, but a family's look keyed on a call site's
  class is the drift this record exists to end.
- **Every screen styling the primitive itself.** It is ADR 0037's original rule, and the audit is
  its result.

## Consequences

- A control of a moved family looks and behaves the same everywhere; changing the family changes
  every screen.
- A theme reaches a family through its parts, and a new family joins the engine's manifest with
  its wrapper (ADR 0103).
- `docs/ui-primitives.md` lists the moved families and their props; the settings set and the
  Notice (ADR 0037's amendments) are drawn from the families as they move.
