# Amendment, 2026-10-22 (#2914): the session tabs have layout words, and two sizes a state may set out of its flow

The spec (#2897) and the switcher prototype's ruling (#2868, #2800) give the session tabs family
layout options that the closed property list of §2 did not have. The manifest is at the next
version, with the `tabs` family (`tab`, `label`, `closer`, `count`, `indicator`, `strip`,
`layout`, `body`) and the `editor` family's `toolbar`.

- **A property may take a word.** `attach` (`panel` or `strip`), `sizing` (`content` or
  `equal`), `dividers` (`on` or `off`) and an indicator's `position` (`top` or `bottom`) are
  words the engine reads and writes from its own table, never from the file. `max-width` is a
  size with its bound.
- **A part may be drawn around the root.** A family's `strip`, `layout` and `body` are reached by
  their own attributes, not inside a tab, and take no variant and no state.
- **A state may set a size that is drawn out of the part's flow.** A chosen tab's `flare` (its
  inverse corners, beside the tab at the strip's floor) and the hover pill's `inset` (how far the
  pill stands in from the cell) are drawn by the tab's own pseudo-elements, positioned out of its
  flow, so they size nothing and move nothing. Each is set only in its own state, `selected` and
  `hover`, and refused elsewhere with where it goes. A flare is drawn at most as wide as the room
  between tabs, so it never covers a neighbour. §4 holds for every other size:
  `tabs.tab.selected.font-weight` is refused (R149).
- **A mark's height is a mark's.** A tab's indicator is held to 0 to 6px, not to a control's
  24px least height.
- **What a chosen tab joins, it takes the fill of.** A theme that fills the panel or the editor's
  toolbar and leaves the chosen tab unset draws the tab in that fill, so the two stay one piece and
  the floors measure the fill drawn.

The family default draws Purlis's docked tabs; Purlis Classic keeps its strip through parts.
