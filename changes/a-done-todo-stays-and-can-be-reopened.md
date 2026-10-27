### Changed

- **A todo marked done stays, and can be reopened.** Marking a todo done still writes its trace to
  the workspace's journal, and now keeps the todo as done instead of deleting it. In the Todos
  view a done todo is drawn after the open ones, struck through with its box ticked; untick the
  box, or choose Reopen from its menu, to open it again as it was. Delete still removes a todo,
  open or done. In a terminal, `purlis ws todo done <slug>` says how to reopen it,
  `purlis ws todo reopen <slug>` reopens one, and `purlis ws todo --done` lists the done ones. The
  open-todo count, the digest a chat starts with and `todo_list` still count only the open ones
  (#3063, ADR 0106).
