# Noted (2026-10-14, qw382b): a save's conflict is resolved from the window — ruled by the operator, 2026-10-15

Decided under delegation (the dispatcher's call), and ruled by the operator, 2026-10-15 (#1961: confirmed as
built).

- **Resolve** is offered where a save's replay onto the remote conflicted. It is the person's
  press in the window: the window fetches the target branch from the remote recorded for the
  project and replays the project's own commits onto it, under no chat's sandbox, and leaves
  the replay stopped where git stops. A tree with unsaved work, off its target branch or
  already stopped part-way is not touched.
- **While it is stopped,** the Saving view names the files and offers *Resolve in a chat*,
  *Continue*, *Abort* and *Open terminal here*.
- **The chat edits files and runs no git.** It starts at the plane root on a message purlis
  writes, through the same start as *Fix* (ADR 0061, amended 2026-10-14): the files git stopped
  in and the subjects of the two commits, quoted as data, and what to do. It is an ordinary
  chat, in its sandbox, with nothing granted past it. Where a file in conflict is one a chat's
  sandbox may not write, no chat is started, and the window says to settle it in a terminal.
- **What git stopped with is kept, for each stop**, in the window's process: the commit being
  replayed, the files in conflict, those git could not mark (a binary file, a file one side
  deleted, a link, a submodule, a file that is gone, or one git left without a marker because
  its `.gitattributes` asked), and every other change git staged. A
  conflict git could not mark is the person's to settle in a terminal, and is never handed to
  a chat.
- **Continue is the window's.** It refuses while a conflict git could not mark is unsettled,
  while anything outside the files in conflict is staged otherwise than git left it, and while
  a marker (`<<<<<<<`, `|||||||`, `>>>>>>>`) opens a line in the files. It stages the files from
  the bytes it checked, compares what git staged with those bytes as git stores them for each
  path (its clean filter and line endings applied), and puts them back in conflict where they
  differ. A further stop names the next files. A settling that leaves the commit empty offers
  **Skip**, which drops it, and only while nothing is left to commit.
- **When the replay finishes, what is committed is pushed**, as the project's mode says, and
  nothing new is committed. Files changed meanwhile are named and left for the person.
- **Abort** is `git rebase --abort`: the project is where it was before Resolve.
