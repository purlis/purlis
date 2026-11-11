# Amendment, 2026-11-11 (#6163): the token floors hold what stands on a surface's default fill

The amendment of 2026-11-10 for #6156 reads what stands on a surface's chosen and hovered fills
where a theme's parts set one. Where they do not, the fill drawn is the family default's, a token
(`list.selected`, `surface.hover`), and the parts check leaves what is broken on it to the token
floors: a default look broken on a default fill is not the parts' doing. The token floors held
`state.success` on `list.selected` and `surface.hover` at 3:1, as a branch's change mark in the
explorer, while an `ok` badge draws it as its words, on no fill of its own, on the rows it stands
on. A theme that changed the `list.selected` token alone could hide those words between 3:1 and
4.5:1 and pass.

- **What a family draws as words on a surface's default fill is held as words there.**
  `state_words_tests.rs` reads a root with no fill of its own on each fill the surfaces
  `STANDS_ON` places it on draw by default, at rest, chosen and under the pointer
  (`SURFACE_STATES`), as well as on the family's own grounds; a family drawn in the overlay
  (`PORTALED`) is not. Each state or danger colour drawn there needs a pair in `PAIRS` at the
  text floor, the floor the parts check holds the same words to (`floor_of`).
- **So `state.success` is held at 4.5:1** on `list.selected` and `surface.hover`, and on what a
  card button (`control.base`, `control.hover`) and a trouble or an attention Notice
  (`danger.surface`, `attention.surface`) draw by default.

What it cost: Purlis's four appearances clear the new pairs (6.0:1 or more). Purlis Classic's
light one did not (3.84:1 on a hovered card button, 4.39:1 on a chosen row), so its
`state.success` moved from `#197a35` to `#166d2f`, the same green darkened until every one of
those grounds clears 4.5:1 (4.56:1 at the least, on a hovered card button). Nothing else in a
shipped theme moved, and each is still measured in one look with no new tint.
