# Amendment, 2026-09-28: a tab being smart-closed is drawn first, and moved by the click

Smart close now puts a chat into the background (SI-8f, ADR 0064 as amended the same day). Its
tab shrinks to a chip drawn at the **left edge of the chat strip, before the pinned tabs**, and
it stays there while the chat writes its record.

That is not the strip reordering itself. **The operator's Smart close click moved it**, as a
drag or a pin is the operator's hand, and nothing moves on its own: not activity, not a chat
that needs you, not a step the core tells. The chip is where the click put it until the smart
close ends.

**It is drawn there and kept nowhere else.** `tabs.order` keeps the tab's place
(`tabs.tabsIn` draws the background first), so when the smart close ends without a record —
given up, the chat ended, the start refused, or cancelled — the tab comes back in its old place
with no second record of where that was. **A chip is fixed**, as the plane root's tab is: it
cannot be picked up, nothing is put down on it, and a drag of the tabs beside it leaves its
place in the order as it was (`PlaneView.dragTab` leaves chips out of the drop). The front never
lands on a chip when another tab closes.
