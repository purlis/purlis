# Amendment, 2026-11-07: a drag holds the keyboard where it is

Two questions the 2026-11-06 amendment left open (#5523). The rest of it stands.

- **A tab a rest brings forward resizes no terminal.** Its panes mount before the drop, and the
  drag pins each terminal's holder at the size it had, so a pane that makes room under the
  pointer and a cancel both leave every terminal of either tab at its size, with nothing sent to
  its program. The one fit a rest can cause is a terminal whose tab was behind while the window
  was resized: it is drawn at the window's size now, and is fitted once, as it would be on its
  next show anyway. A cancel sends nothing back.
- **The keyboard is held with the rest, for as long as the drag lasts, and what was asked for
  meanwhile is dropped, not released.** A chat drawn, or asked for (`giveKeyboardTo`) while
  something is picked up, takes no keyboard: a terminal in a tab drawn only as a preview would be
  typed into, and its pane counted as used (`pane_used`), before anything has moved. When the
  drag ends, its drop or its cancel already puts the keyboard where the person's hand leaves it
  (the moved pane's grip, or what had it before). A debt released after that would move it
  again, so it is dropped, as `paneKeyboard.ts` already drops one when the person moves on.
- **The core hears the keyboard's place once**, in the layout told after the drop: the focused
  pane of the tab the rest brought forward.
