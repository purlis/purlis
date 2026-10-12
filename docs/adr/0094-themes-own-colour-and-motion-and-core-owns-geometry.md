# Themes own colour and motion, core owns geometry, and every control starts from one baseline

**Accepted 2026-10-12.** Extends ADR 0037 (behaviour from a headless library, the look is
purlis's own): it gives "type, spacing, density, elevation", which 0037 left untaken, a home.
Spec #1832, ticket #1833 (DS-1). The operator's decisions D1 to D8 of the design-system grill,
2026-10-12.

## Where purlis was

purlis had a strict design system for **colour and motion** and none for **geometry**:

- Colour and motion were tokens: 79 semantic colour names and 9 motion names, a theme as a data
  file (`docs/design-system.md`), a guard on literals, contrast floors and a tint system.
- Size had no tokens. `App.css` held 132 different paddings, 20 font sizes and 11 corner radii,
  110 of those radii in px. Nothing styled a bare `button`, `input`, `select` or `textarea`, so
  any control without a container rule was drawn with WebKit's own grey bezel, 13.33px system
  font and white field.

So every view drew its own controls and they drifted: about 30 button looks, 9 input looks,
selects and buttons that did not line up, and focus rings offset three different ways. The
operator's screenshots of 2026-10-12 were all this one gap.

## The operator's words

> D5 Sizes: 28px for every control (button, field, select, Reset), 21px chrome; one spacing
> scale, a 6-step type scale, radii 3/4/6/8. Themes own colour, shadow colour and motion; core
> owns geometry. Density setting later.

> Approach: core geometry tokens plus primitive CSS classes over the existing Radix (no shadcn,
> no React Button layer), recorded in a new ADR.

## The decision

### 1. Three token layers

1. **Primitive palette.** Raw colour steps (`teal.400`) a theme builds its semantic colours from.
   Private to the theme file that declares it; no view reads it. Optional, and not in use yet:
   the brand change adds the block.
2. **Semantic tokens.** What a colour or a motion means in this window: `surface.raised`,
   `text.muted`, `duration.enter`. This is the theme's vocabulary and the only one it has.
3. **Component tokens.** One control's look, as CSS aliases of the layers below
   (`--btn-bg: var(--control-base)`), in the primitive classes. They make a control themeable
   without adding a name to the theme's vocabulary. A theme may not set one; a later ADR may let
   it, VS Code's way, through a `components` block.

**Core geometry** sits beside the semantic layer, not inside it: the same values in every theme.

### 2. Themes own colour, shadow colour and motion; core owns geometry

- **A theme sets** every colour (layers 1 and 2), the shadow colour (`overlay.shadow`) and
  motion. Nothing else.
- **Core sets**, once, in `app/src/theme/geometry.css`:
  - a spacing scale, `--space-0` to `--space-8`: 0, 2, 4, 6, 8, 12, 16, 24, 32px;
  - a 6-step type scale, `--font-size-xs` to `--font-size-xl`: 11, 12, 13, 14, 16, 20px;
  - corners, `--radius-sm` 3px, `-md` 4px, `-lg` 6px, `-xl` 8px, and `-full`;
  - `--size-control` 28px, for every button, field, select and Reset;
  - `--size-chrome` 21px, for every tab strip and pane header;
  - two focus-ring rules: a `--focus-ring-width` (2px) ring in the theme's `focus.ring`,
    **outside** a control at `--focus-ring-offset` (2px), **inside** a row or a tab at
    `--focus-ring-inset` (-2px);
  - elevation, `--elevation-raised`, `-menu` and `-dialog`: the shadow's shape, drawn in the
    theme's `overlay.shadow`.
- Lengths that should grow with the words are in rem, as "N px at 14px" (`calc(6rem / 14)`), so
  the text-size setting (charter-app#283) scales them. Corners and focus rings stay in px.
- **Density is a later user setting**, not a theme property. It will swap the core values for
  another set; a theme still never sees them.

**How it is held.** Geometry is not in a theme's vocabulary, and `theme.ts` writes only colour
and motion tokens onto the document, so no theme can set a core token. A theme file that tries
(`space.4`, `--radius-md`, a `geometry` block) is drawn without it and told why in its
complaints. `geometry.css` is the only stylesheet that gives a core token a value.
`app/src/theme/geometry.test.ts` holds all three, for both built-ins, every tint of them, and a
theme file that tries every spelling.

### 3. One baseline for every control

`app/src/base.css`, in the `base` layer under `App.css`, with every selector `:where` so it has
no specificity:

- every `button`, `input`, `select` and `textarea` takes the window's font;
- one with **no class** takes the whole look from the tokens: no native appearance, the field or
  control surface, a 1px border, `--radius-md`, the outside focus ring, and `--size-control` of
  height made from padding.

**Height comes from padding, never `height` or `min-height`.** About 180 controls have no class
and are drawn by a container's rule (`.answer button`, a strip's `×`). A minimum height in the
baseline would beat that rule's own `height` and push a 21px strip taller; padding is what such
a rule already sets, so the rule wins.

### 4. One class per control, over Radix

The look of each control kind is one CSS class built only from tokens (DS-2 and DS-3: button,
field, select, checkbox, radio, switch, segmented control, table, list row, tree row, tabs, menu), over
the Radix primitives ADR 0037 already uses for behaviour. No styled kit (shadcn components,
Tailwind's preflight) and no React component layer for the look.

The classes are `app/src/primitives/controls.css`, in the `components` layer above `App.css`, so a view
rule cannot redraw a classed control and the window's focus floor cannot pull its ring inside; a
view still places one. Every colour is one theme token per declaration, so the contrast check
reads each pair: no component token carries a colour. The two new Radix packages are the switch
and the select, both of the family D2 approved. `docs/ui-primitives.md` ("The control classes")
lists each class and its variants (DS-2, #1834).

## What was rejected

- **Geometry in the theme file.** A theme is data a person or an extension writes (ADR 0041). One
  that could change a control's height could break the alignment of every view, and light and
  dark would stop lining up. VS Code and Zed both keep sizes out of themes; Primer keeps them in
  a layer every theme shares.
- **Geometry written by `theme.ts` at run time.** It would put the values in a second language
  for no consumer that needs them there (xterm reads no size from a theme), and a view's first
  frame would wait on script. A stylesheet is where a size is read.
- **Tailwind's preflight as the baseline.** It resets every element, not only controls, and
  would restyle the whole of `App.css` in one change. The baseline resets only what WebKit draws
  for a control.
- **A minimum height on every unclassed control.** See §3.
- **Tailwind's `--text-*` and `--spacing` names for the core scale.** Tailwind already uses them,
  and `text-xs` and `gap-1` are in use with Tailwind's values. The core names stay out of its
  namespaces; only the corners are handed to `rounded-*`, which nothing used.

## Consequences

- A theme can no longer move a size, and a theme author's file that tries is told so.
- Every later design-system ticket builds on these names: the primitive classes (DS-2, DS-3),
  the brand, and the geometry guard (DS-4) that ratchets the literal paddings, sizes and radii out of
  `App.css`.
- Classed controls that never set a font change from WebKit's 13.33px system font to the
  window's. That is the inconsistency this decision exists to remove.
- `docs/design-system.md` lists every core token and its layer, and `geometry.test.ts` fails
  when one is missing from it.
