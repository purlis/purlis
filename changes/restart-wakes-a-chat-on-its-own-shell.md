### Fixed

- **A chat waiting on its own background shell is told when a restart ended it.** A quit, an
  update or a crash ended the shell with the chat, and the chat brought back waited for it for
  ever. purlis now keeps what each chat left running of its own in the record of open chats,
  each time it changes, and the chat brought back is told, in one line, which shells, monitors,
  subagents and workflows the restart ended. A task that still owes its report is told to carry
  on in that same line. A chat the session host kept running is told nothing (#5747).
