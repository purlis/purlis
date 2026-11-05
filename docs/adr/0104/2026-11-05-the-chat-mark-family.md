# Amendment, 2026-11-05 (#5340, spec #5339): the chat mark family

**A chat's mark is one family, `ui/chat-mark`**, drawn on its tab, its row in the list, its pane's head and a folded pane's rail. Its parts are `root`, whose variant is the chat's one state, `who`, `badge` and `word`. Every surface reads the one function, `shownState`, through `useShownState`.

**Why.** A tab's dot and a row's mark were two components reading two things: the board's state, and everything `shownState` reads. A chat waiting on its tasks wore the amber dot of a chat at its prompt on its tab and the hourglass on its row.

**One state, the first true winning:** needs you, a failed task, working, waiting, idle, ended. This replaces the rule of #3342 that a chat at rest wears no badge: idle and ended wear theirs, so every state is told by a shape. A harness that has said nothing wears none, since no state is known of it (V100-71).

**"Until seen" stores nothing.** It reads the core's own record of failures nobody has looked at (`failedTasks`). A chat on screen, in the focused pane, in a window with the keyboard, has been looked at, and the core is told through `task_failure_seen`. The failure's item in the Inbox goes at the same moment, since it is the same record.

**A line that says how a task ended keeps `StateShown`'s finer shapes** (a finished row, the ended band, the breadcrumb, the tasks menu, #1488), which the one state does not say. A line that says a chat's state now (the pane's head, the explorer) draws that state's glyph, so it never disagrees with the mark beside it.
