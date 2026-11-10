# Amendment, 2026-11-10 (#6156): a surface is read in its chosen and hovered fills

The amendment of 2026-10-26 has a surface fall back for what stands on it, and the engine read a
surface's fill at rest alone. A list's, a tree's or a table's chosen row and the row under the
pointer draw fills of their own (`list.selected`, `surface.hover`), and a theme can set either in
its parts (`list.root.row.selected.fill`). What stands on the row (a badge, a chat's mark, a
checkbox) was never measured there, so a chosen row filled where a badge's words vanish passed
its check.

- **A surface's look is read at rest, chosen and under the pointer** (`SURFACE_STATES`), each
  state of a surface that takes it, alone. Where a theme's parts draw a fill there other than the
  family default's, what `STANDS_ON` says stands on the surface is read at rest on it, as it is on
  a rest fill. A state that breaks draws its family default there, and the rest stays as written.
  A fill at rest that reaches a state (Purlis Classic's `list.root.card.fill`, drawn chosen too) is
  read there as well.
- **What the window draws in React Aria's overlay is never read on a state fill** (`PORTALED`:
  the popover and the tooltip). A popover a row opens is portaled to the window's body: it is
  drawn over the window, never on the row's chosen fill. At rest it is still read where the window
  nests it, as before.
- **`app/src/theme/grounds.test.ts` finds `PORTALED` from the components.** A family is one when
  every root the window draws of it is one of React Aria's overlays (`Popover`, `Tooltip`,
  `Modal`, `ModalOverlay`, and the `OverlayArrow` drawn only inside one), or inside one in the
  file that draws it. A dialog's root is placed in its overlay by another component, which this
  read does not follow, and no surface read in a state holds a dialog, so it is not named.

What this costs: the readings grow by at most two for each look of a surface whose state fill a
theme sets, and never by the family's own states. In the shipped themes no state fill is
see-through, and the only state fill other than its default is Purlis Classic's card row, drawn
`surface.sunken` chosen and hovered by its rest rule, the fill it is read on at rest already.
Every shipped appearance is still measured in one look (#4410).

What it leaves: a theme that changes the `list.selected` token itself, with no part rule, is
held by the token floors alone, which hold `state.success` on it at 3:1, not a badge's words at
4.5:1. A popover or a tooltip is still read on a rest fill where the window nests it, though it is
portaled there too.
