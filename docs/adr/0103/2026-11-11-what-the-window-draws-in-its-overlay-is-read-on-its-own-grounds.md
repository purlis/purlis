# Amendment, 2026-11-11 (#6164): what the window draws in its overlay is read on its own grounds

The amendment of 2026-11-10 for #6156 keeps the families the window draws in React Aria's overlay
(`PORTALED`: the popover and the tooltip) off a surface's chosen and hovered fills, and left them
read on its rest fill where the window nests them. The window draws neither on the row, the panel
or the card that holds it at rest either: both are portaled to the window's body.

- **A portaled family's ground is what it is portaled over**, its own grounds (the window and a
  panel for a popover, and the overlay too for a tooltip), as its family reads them; what is in it
  stands on its own fill. It is never read on a fill a surface that holds it draws, at rest or in
  a state. `drawn_on` leaves it off, and so reads no states.
- **`STANDS_ON` still names where the components nest it.** `app/src/theme/grounds.test.ts` holds
  that table and `PORTALED` to the components exactly, as before; the engine is what leaves a
  portaled family out.
- **`state_words_tests.rs` reads it on its own grounds alone**, not on the surfaces that hold it
  (the amendment of the same day, #6163).

What it cost: in the shipped themes no surface fill falls, so none was falling for a popover or a
tooltip alone, and nothing they draw moved. Every shipped appearance is still measured in one look
with no new tint. A theme whose surface fell back only for a popover or a tooltip in it now draws
that fill as written.
