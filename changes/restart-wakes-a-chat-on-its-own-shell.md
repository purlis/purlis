### Fixed

- **A chat waiting on its own background shell is told when a restart ended it.** A quit or an
  update ended the shell along with the chat, and the chat brought back at the next launch
  waited for it for ever. purlis now records what each chat left running of its own in the
  record of open chats, every time that changes. When the chat comes back, it is told in one
  line how many shells, monitors, subagents and workflows the restart ended. A task that still
  owes its report is told to carry on in that same line. A chat the session host kept running
  is told nothing (#5747).
