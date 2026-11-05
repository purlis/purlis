# The head's controls are always drawn, and a tab's only pane has no Close

**Accepted 2026-11-05**, the person's ask with screenshots (#5318).

**It reverses a recorded decision.** #5221 decided that the divider hides at rest with the
controls it separates, so that at rest the head read state, bell and ⋯. Those controls were drawn
only under the pointer or the keyboard (charter-app#189). Now Split right, Split down and, where
it is shown, Close are always drawn, with the divider before them. Nothing appears or moves when
the pointer enters a pane, and every pane's controls are in the tab sequence.

**A tab's only pane has no Close**, because the tab's own × does the same. A second pane brings
Close back on both heads, and closing back down to one takes it away. The pane's model answers
this (`paneHead.paneLinesOf`, its `alone` fact), and the catalogue follows it: the palette and the
head's right-click have no Close pane row on a tab's only pane, so a row and its button never
disagree. `tabs.hasOnePane` is the one test. A pane zoomed to fill its tab is not alone, since its
tab's × would take the others too.

- **The strip's last tab** loses nothing: Close pane on a tab's only pane already went through the
  tab's close (`tabs.closeLayout`), which leaves that strip one empty pane.
- **The tab's × takes Close pane's keyboard too.** Closing the tab in front puts the keyboard in
  what is now in front, as Close pane did. Delete on a tab keeps the keyboard on the strip.
- **A task's own pane** alone in its tab loses its minimise the same way: the tab's own control
  sends the task back too (#1489). Beside its session it keeps it.
- **A one-job program's pane** still has no Close, alone or beside a chat: it goes when its program
  exits (#4761).
- **A pinned tab drawn as its icon alone** draws no ×, as before. Its right-click and Delete still
  close it.

**A view alone in its tab draws no split and no Close over its content**, since controls floating
over a view's own controls is what the person complained of. The ticket says it has no head "as
today", which was wrong: it drew all three under the pointer. So the route stays: Split right and
Split down are in the palette and on that view tab's menu. Its grip is unchanged and belongs to
#5290.

**What it costs.** The head's room does not change: hidden controls kept theirs, so no width in the
drop rule moves. In a split, each pane adds up to three Tab stops, where other panes' controls used
to be out of the sequence. The head reads busier at rest, which is what #5221 had kept away and
what the person now asks for.
