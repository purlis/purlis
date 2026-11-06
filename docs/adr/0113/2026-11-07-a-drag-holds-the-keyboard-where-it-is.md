# Amendment, 2026-11-07: a drag holds the keyboard where it is

Two questions the 2026-11-06 amendment left open (#5523). The rest of it stands.

- **A tab a rest brings forward resizes no terminal.** Its panes mount before the drop, and the
  drag pins each terminal's holder at the size it had, so a pane that makes room under the
  pointer and a cancel both leave every terminal of either tab at its size, with nothing sent to
  its program. The one fit a rest can cause is a terminal whose tab was behind while the window
  was resized: it is drawn at the window's size now, and is fitted once, as it would be on its
  next show anyway. A cancel sends nothing back.
- **The keyboard is held with the rest, for as long as anything is picked up, and what was asked
  for meanwhile is dropped, not released.** A pointer drag and placement mode alike: from the
  pick-up to the drop or the cancel, a chat drawn, or asked for (`giveKeyboardTo`), takes no
  keyboard. A terminal in a tab drawn only as a preview would be typed into, and its pane counted
  as used (`pane_used`), before anything has moved. When the drag ends, the keyboard is where the
  person's hand leaves it: after a pointer drop, where it was; after a drop with the keys, on the
  moved pane's grip; after a cancel, on what had it before. A debt released then would move it
  again, so it is dropped. **A debt from before the pick-up is dropped too**, as
  `paneKeyboard.ts` drops one when the person moves on: picking something up is the person's own
  hand, and where it leaves the keyboard is where they are.
- **The core hears the keyboard's place once**, in the layout told after the drop: the focused
  pane of the tab the rest brought forward.
