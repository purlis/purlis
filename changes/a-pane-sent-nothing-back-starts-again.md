### Fixed

- **A chat's pane that froze while you typed comes back on its own** (#5478). When a pane has
  been typed into and its program has sent nothing back for 10 seconds, the pane starts its
  terminal and its view of the chat again and is sent the screen as it is now. Before, switching
  tabs left it frozen, and only showing three other chats brought it back. A chat starts again
  at most once every five minutes. The cause is still being looked for.
