# The window is built from component families on React Aria, each primitive wrapped once

**Accepted 2026-10-21**, from the operator's spec #2897 (M66, UI system) and its resolution
#2793 (which primitives, and which shared component set), accepted as recommended. Built by
#2904, with the button as the first family. It amends
[ADR 0037](0037-charter-takes-the-behaviour-and-keeps-the-look.md) (its amendment of
2026-10-21 says where) and sits beside
[ADR 0103](0103-a-theme-sets-component-parts-through-a-closed-manifest-and-a-state-never-changes-size.md),
which says how a theme reaches a family's parts.

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

Its hooks package, `react-aria`, is a direct dependency too (#2908), for the component layer's
targets that are not React Aria components (a tooltip's words, a raw control); it is pinned to
the version `react-aria-components` is built on, so the window holds one copy of its state, and
`app/src/ui/primitives.lock.test.ts` fails if the two part.

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

`app/eslint.config.js` refuses the primitives library outside `app/src/ui` (an error). Each family
has a lint rule of its own (`app/eslint.ui-rules.js`), a warning until the family moves and an
error from the ticket that moves its last call site:

- `purlis/button-family`, an error since #2904: a raw `<button>` is an error unless its role or
  its class names a family that has not moved yet (a tab, a row, a menu's row, a segment, a
  strip's mark, a title-bar chip, a status item, a card, the settings nav), and the old
  `.ui-button*` classes are an error anywhere outside `app/src/ui`;
- `purlis/tooltip-family`, an error since #2908: a native `title=` on any element (a frame's,
  which is its name, aside), a primitive's part or a member of the button family;
- `purlis/menu-family`, an error since #2909: no element outside `src/ui` says a menu's role or
  wears the old menu and popover classes, and nothing imports Radix's menus or popover;
- `purlis/dialog-family`, an error since #2910: `role="dialog"` or `role="alertdialog"` written by
  hand;
- `purlis/choice-family`, an error since #2907: a raw choice `<input>`, a choice's role written by
  hand, Radix's choices and the old choice classes
  ([amended 2026-10-24](0104/2026-10-24-the-choice-families.md)). The field family moved and its rule is an error, with the select and choice inputs split
  into rules of their own
  ([amended 2026-10-22](0104/2026-10-22-the-field-family.md)).

A family's ticket takes its own role and classes out of the button rule's list as it moves.

### 6. A family is done when it is proven

A family's ticket ships a story for each variant and state with a `play` test for its keyboard
path, axe green in Chromium and WebKit, screenshot baselines, its parts in the theme engine's
manifest, its docs, every call site moved with its lint at error, and the operator's before and
after look (`docs/component-stories.md`).

## Amendments

The amendments to this decision are files of their own in [`0104/`](0104/), one per
amendment, named `<yyyy-mm-dd>-<slug>.md`:
[2026-10-22, the dialog family and the window's top layer](0104/2026-10-22-the-dialog-family-and-the-top-layer.md);
[2026-10-24, the choice families](0104/2026-10-24-the-choice-families.md).

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

## Amendments

The amendments to this decision are files of their own in [`0104/`](0104/), one per
amendment, named `<yyyy-mm-dd>-<slug>.md`.
