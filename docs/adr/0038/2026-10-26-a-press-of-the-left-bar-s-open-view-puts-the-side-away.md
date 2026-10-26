# Amendment, 2026-10-26: a press of the left bar's open view puts the side away

The operator's ruling of 2026-10-26 (#3329) amends R121 of the UI system spec (#2897), which
the amendment of 2026-10-24 repeated (_"A press of the open view does nothing (R121), as in the
prototype"_). That sentence no longer holds. **A second press of the current view's icon in the
left bar puts the side panel away**, as a press of the open view does on the right bar and on
each half's Lower icons. A press of any icon while the side is away brings it back on that view.

R121's reason was that the side panel's tabs sat across its own top (#2805, point 3), so a press
that hid the panel would have hidden its own tabs. Since #3144 the icons are a column beside the
rail that stands whether or not the side is out, so the reason is gone and every bar now answers
a press the same way.

- The icon of a side that is away is not marked (`aria-selected="false"`), and the keyboard
  stays on it, so Enter or Space brings the side straight back.
- Its tooltip says what the press does: "Put the Navigation region away".
- ⌘B (Ctrl+B off a Mac) and the palette's row still put the side away and bring it back.
